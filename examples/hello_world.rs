extern crate tokio_core;
extern crate tokio_service;
extern crate futures;
extern crate netbuf;
extern crate minihttp;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;

use netbuf::Buf;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tokio_service::Service;
use futures::{Async, Finished, finished};

use minihttp::{ResponseFn, Error};
use minihttp::request::Request;

#[derive(Clone)]
struct HelloWorld;

impl Service for HelloWorld {
    type Request = Request;
    type Response = ResponseFn<Finished<(TcpStream, Buf), Error>>;
    type Error = Error;
    type Future = Finished<Self::Response, Error>;

    fn call(&self, _req: Self::Request) -> Self::Future {
        finished(ResponseFn::new(move |mut res| {
            res.status(200, "OK");
            res.add_chunked().unwrap();
            if res.done_headers().unwrap() {
                res.write_body(b"Hello world!");
            }
            res.done()
        }))
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}


fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init().expect("init logging");

    let mut lp = Core::new().unwrap();

    let addr = "0.0.0.0:8080".parse().unwrap();

    minihttp::serve(&lp.handle(), addr, HelloWorld);

    lp.run(futures::empty::<(), ()>()).unwrap();
}
