extern crate time;
extern crate tokio_core;
extern crate futures;
extern crate tk_bufstream;
extern crate netbuf;
extern crate minihttp;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;

use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use tokio_core::io::Io;
use futures::{Stream, Future};
use futures::future::{FutureResult, ok};

use minihttp::{Status};
use minihttp::server::buffered::{Request, BufferedDispatcher};
use minihttp::server::{Encoder, EncoderDone, Config, Proto, Error};


const BODY: &'static str = "Hello World!";

fn service<S:Io>(_: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    e.status(Status::Ok);
    e.add_length(BODY.as_bytes().len() as u64).unwrap();
    e.format_header("Date", time::now_utc().rfc822()).unwrap();
    e.add_header("Server",
        concat!("minihttp/", env!("CARGO_PKG_VERSION"))
    ).unwrap();
    if e.done_headers().unwrap() {
        e.write_body(BODY.as_bytes());
    }
    ok(e.done())
}


fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init().expect("init logging");

    let mut lp = Core::new().unwrap();

    let addr = "0.0.0.0:8080".parse().unwrap();
    let listener = TcpListener::bind(&addr, &lp.handle()).unwrap();
    let cfg = Config::new().done();

    let done = listener.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(|(socket, addr)| {
            Proto::new(socket, &cfg, BufferedDispatcher::new(addr, || service))
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .buffer_unordered(200000)
          .for_each(|()| Ok(()));

    lp.run(done).unwrap();
}
