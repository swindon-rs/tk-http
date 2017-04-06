extern crate time;
extern crate tokio_core;
extern crate futures;
extern crate tk_bufstream;
extern crate netbuf;
extern crate tk_http;
extern crate tk_listen;
extern crate env_logger;

use std::env;
use std::time::Duration;

use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use futures::{Stream, Future};
use futures::future::{FutureResult, ok};

use tk_http::{Status};
use tk_http::server::buffered::{Request, BufferedDispatcher};
use tk_http::server::{Encoder, EncoderDone, Config, Proto, Error};
use tk_listen::ListenExt;


const BODY: &'static str = "Hello World!";

fn service<S>(_: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    e.status(Status::Ok);
    e.add_length(BODY.as_bytes().len() as u64).unwrap();
    e.format_header("Date", time::now_utc().rfc822()).unwrap();
    e.add_header("Server",
        concat!("tk_http/", env!("CARGO_PKG_VERSION"))
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
    let h1 = lp.handle();

    let done = listener.incoming()
        .sleep_on_error(Duration::from_millis(100), &lp.handle())
        .map(move |(socket, addr)| {
            Proto::new(socket, &cfg,
                BufferedDispatcher::new(addr, &h1, || service),
                &h1)
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .listen(1000);

    lp.run(done).unwrap();
}
