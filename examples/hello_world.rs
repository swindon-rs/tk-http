extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate minihttp;
extern crate futures;
#[macro_use] extern crate log;
extern crate env_logger;

use std::io;
use std::env;

use tokio_core::reactor::Core;
use tokio_service::Service;
use futures::{Finished, Async};


#[derive(Clone)]
struct HelloWorld;

impl Service for HelloWorld {
    type Request = minihttp::Request;
    type Response = minihttp::Response;
    type Error = io::Error;
    type Future = Finished<minihttp::Response, io::Error>;

    fn call(&self, req: minihttp::Request) -> Self::Future {
        info!("{:?} {:?}", req.method, req.path);
        let resp = req.new_response();
        //resp.header("Content-Type", "text/html");
        //resp.body("<h1>Hello world</h4>\n");
        let resp = resp;

        futures::finished(resp)
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

    minihttp::serve(&lp.handle(), addr, HelloWorld).unwrap();

    lp.run(futures::empty::<(), ()>()).unwrap();
}
