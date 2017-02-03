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


pub fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut lp = tokio_core::reactor::Core::new().expect("loop created");
    let handle = lp.handle();
    let addr = ("echo.websocket.org", 80).to_socket_addrs()
        .expect("resolve address").next().expect("at least one IP");

    let response = lp.run(futures::lazy(move || {
        TcpStream::connect(&addr, &handle)
        .and_then(|sock| {
            println!("Socket {:?}", sock);
            Ok(())
        })
    })).expect("request failed");
}
