extern crate futures;
extern crate minihttp;
extern crate argparse;
extern crate env_logger;
extern crate tokio_core;
#[macro_use] extern crate log;

use std::env;
use std::time::Duration;
use std::net::ToSocketAddrs;

use futures::{Future, Stream};
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::unbounded;
use tokio_core::net::TcpStream;
use tokio_core::reactor::{Timeout};
use minihttp::websocket::{Loop, Frame, Error, Dispatcher, Config};
use minihttp::websocket::client::{HandshakeProto, SimpleAuthorizer};
use minihttp::websocket::Packet::{Text};

struct Echo;


impl Dispatcher for Echo {
    type Future = FutureResult<(), Error>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), Error> {
        println!("Frame arrived: {:?}", frame);
        ok(())
    }
}


pub fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut lp = tokio_core::reactor::Core::new().expect("loop created");
    let handle = lp.handle();
    let h2 = lp.handle();
    let addr = ("echo.websocket.org", 80).to_socket_addrs()
        .expect("resolve address").next().expect("at least one IP");
    let wcfg = Config::new().done();

    lp.run(futures::lazy(move || {
        TcpStream::connect(&addr, &handle)
        .map_err(|e| error!("Error {}", e))
        .and_then(|sock| {
            HandshakeProto::new(sock, SimpleAuthorizer::new(
                "echo.websocket.org", "/"))
            .map_err(|e| error!("Error {}", e))
        })
        .and_then(move |(out, inp, ())| {
            println!("Connected");
            let (tx, rx) = unbounded();

            println!("Preparing to send packet in 5 seconds");
            let mut tx2 = tx.clone();
            h2.spawn(
                Timeout::new(Duration::new(5, 0), &h2).unwrap()
                .map_err(|_| unreachable!())
                .and_then(move |_| {
                    println!("Sending 'hello'");
                    tx2.send(Text("hello".to_string()))
                    .map_err(|_| ())
                })
                .then(|_| Ok(())));

            let rx = rx.map_err(|_| format!("stream closed"));
            Loop::client(out, inp, rx, Echo, &wcfg)
            .map_err(|e| println!("websocket closed: {}", e))
        })
        .then(|_| -> Result<(), &'static str> { Ok(()) })
    })).expect("request failed");
}
