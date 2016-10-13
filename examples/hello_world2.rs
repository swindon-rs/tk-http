extern crate tokio_core;
extern crate futures;
extern crate minihttp;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;
use std::io;

use tokio_core::reactor::Core;

use minihttp::server;
use minihttp::request::Request;
use minihttp::response::Response;


struct HelloWorld;

impl server::HttpService for HelloWorld {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = futures::Finished<server::Message<Self::Response>, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let mut resp = req.new_response();
        resp.set_status(204)
            .set_reason("No Content".to_string());
        futures::finished(server::Message::WithoutBody(resp))
    }
}
impl server::NewHandler for HelloWorld {
    type Handler = HelloWorld;

    fn new_handler(&self) -> HelloWorld {
        HelloWorld {}
    }
}


fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init().expect("init logging");

    let mut lp = Core::new().unwrap();

    let addr = "0.0.0.0:8080".parse().unwrap();

    let h = HelloWorld {};
    minihttp::serve(&lp.handle(), addr, h);

    lp.run(futures::empty::<(), ()>()).unwrap();
}
