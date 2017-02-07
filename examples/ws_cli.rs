extern crate futures;
extern crate minihttp;
extern crate argparse;
extern crate env_logger;
extern crate tokio_core;
#[macro_use] extern crate log;

use std::io::{self, Write};
use std::env;
use std::fs::File;
use std::path::{PathBuf, Path};
use std::str::FromStr;
use std::net::ToSocketAddrs;

use futures::Future;
use tokio_core::net::TcpStream;
use minihttp::websocket::Loop;
use minihttp::websocket::client::{HandshakeProto, SimpleAuthorizer};


/*
impl Dispatcher for Echo {
    type Future = FutureResult<(), WsErr>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), WsErr> {
        self.0.start_send(frame.into()).unwrap();
        ok(())
    }
}
*/


pub fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut lp = tokio_core::reactor::Core::new().expect("loop created");
    let handle = lp.handle();
    let addr = ("echo.websocket.org", 80).to_socket_addrs()
        .expect("resolve address").next().expect("at least one IP");

    lp.run(futures::lazy(move || {
        TcpStream::connect(&addr, &handle)
        .map_err(|e| e.into())
        .and_then(|sock| {
            HandshakeProto::new(sock, SimpleAuthorizer::new("/"))
        })
        .and_then(|(out, inp, ())| {
            println!("Connected");
            /*
            Loop::new(out, inp, rx, Echo, &wcfg)
            .map_err(|e| println!("websocket closed: {}", e))
            */
            Ok(())
        })
    })).expect("request failed");
}
