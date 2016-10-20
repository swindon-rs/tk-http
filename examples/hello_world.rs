extern crate time;
extern crate tokio_core;
extern crate tokio_service;
extern crate futures;
extern crate tk_bufstream;
extern crate netbuf;
extern crate minihttp;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;

use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tokio_service::Service;
use tk_bufstream::IoBuf;
use futures::{Async, Finished, finished};

use minihttp::{ResponseFn, Error};
use minihttp::request::Request;

#[derive(Clone)]
struct HelloWorld;

const BODY: &'static str = "Hello World!";

impl Service for HelloWorld {
    type Request = Request;
    type Response = ResponseFn<Finished<IoBuf<TcpStream>, Error>, TcpStream>;
    type Error = Error;
    type Future = Finished<Self::Response, Error>;

    fn call(&self, _req: Self::Request) -> Self::Future {
        finished(ResponseFn::new(move |mut res| {
            res.status(200, "OK");
            res.add_length(BODY.as_bytes().len() as u64).unwrap();
            res.format_header("Date", time::now_utc().rfc822()).unwrap();
            res.add_header("Server", concat!("minihttp/",
                                     env!("CARGO_PKG_VERSION"))).unwrap();
            if res.done_headers().unwrap() {
                res.write_body(BODY.as_bytes());
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
