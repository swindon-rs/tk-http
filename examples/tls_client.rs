extern crate argparse;
extern crate env_logger;
extern crate futures;
extern crate minihttp;
extern crate tokio_core;
extern crate url;
extern crate rustls;
extern crate tokio_rustls;

#[macro_use] extern crate log;

use std::io::{self, Write, BufReader};
use std::env;
use std::fs::File;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use url::Url;
use futures::future::{FutureResult, ok};
use futures::{Future, Stream, Sink};
use futures::sync::mpsc::unbounded;
use rustls::ClientConfig;
use tokio_core::net::TcpStream;
use tokio_core::reactor::{Timeout};
use tokio_rustls::ClientConfigExt;
use minihttp::client::buffered::{Buffered, Response};
use minihttp::client::{Proto, Config, Error};


pub fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut lp = tokio_core::reactor::Core::new().expect("loop created");
    let handle = lp.handle();
    let addr = ("google.com", 443).to_socket_addrs()
        .expect("resolve address").next().expect("at least one IP");
    let config = Arc::new({
        let mut cfg = ClientConfig::new();
        let mut pem = BufReader::new(
            File::open("/etc/ssl/certs/ca-certificates.crt")
            .expect("certificates exist"));
        cfg.root_store.add_pem_file(&mut pem).unwrap();
        cfg
    });
    let response = lp.run(futures::lazy(move || {
        TcpStream::connect(&addr, &handle)
        .and_then(move |sock| config.connect_async("google.com", sock))
        .map_err(|e| error!("{}", e))
        .and_then(move |sock| {
            let (codec, receiver) = Buffered::get(
                "https://rust-lang.org".parse().unwrap());
            let proto = Proto::new(sock, &Arc::new(Config::new()));
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
