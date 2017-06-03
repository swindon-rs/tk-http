extern crate argparse;
extern crate env_logger;
extern crate futures;
extern crate tk_http;
extern crate tokio_core;
extern crate url;
extern crate native_tls;
extern crate tokio_tls;

#[macro_use] extern crate log;

use std::io::{self, Write, BufReader};
use std::env;
use std::fs::File;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use futures::{Future, Sink};
use native_tls::TlsConnector;
use tokio_core::net::TcpStream;
use tokio_tls::TlsConnectorExt;
use tk_http::client::buffered::{Buffered};
use tk_http::client::{Proto, Config, Error};


pub fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let host = "www.rust-lang.org";
    let uri = ["https://", host, "/documentation.html"].join("");

    let mut lp = tokio_core::reactor::Core::new().expect("loop created");
    let handle = lp.handle();
    let h2 = lp.handle();
    let addr = (host, 443).to_socket_addrs()
        .expect("resolve address").next().expect("at least one IP");

    let cx = TlsConnector::builder().unwrap().build().unwrap();
    let response = lp.run(futures::lazy(move || {
        TcpStream::connect(&addr, &handle)
        .and_then(move |sock| {
            cx.connect_async(host, sock).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e)
            })
        })
        .map_err(|e| error!("{}", e))
        .and_then(move |sock| {
            let (codec, receiver) = Buffered::get(
                uri.parse().unwrap());
            let proto = Proto::new(sock, &h2, &Arc::new(Config::new()));
            proto.send(codec)
            .join(receiver.map_err(|_| -> Error { unimplemented!() }))
            .map_err(|e| e)
            .and_then(|(_proto, result)| {
                result
            })
            .map_err(|e| error!("{}", e))
        })
    })).expect("request failed");
    io::stdout().write_all(response.body()).unwrap();
}
