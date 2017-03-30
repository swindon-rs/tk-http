extern crate time;
extern crate tokio_core;
extern crate futures;
extern crate tk_bufstream;
extern crate netbuf;
extern crate tk_http;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicUsize};

use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use futures::{Stream, Future};
use futures::future::{FutureResult, ok};

use tk_http::{Status};
use tk_http::server::buffered::{Request, BufferedDispatcher};
use tk_http::server::{Encoder, EncoderDone, Config, Proto, Error};


fn service<S>(counter: usize, _: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    let formatted = format!("Visit #{}", counter);
    e.status(Status::Ok);
    e.add_length(formatted.as_bytes().len() as u64).unwrap();
    e.format_header("Date", time::now_utc().rfc822()).unwrap();
    e.add_header("Server",
        concat!("tk_http/", env!("CARGO_PKG_VERSION"))
    ).unwrap();
    if e.done_headers().unwrap() {
        e.write_body(formatted.as_bytes());
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
    let counter = Arc::new(AtomicUsize::new(0));

    let done = listener.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(move |(socket, addr)| {
            let counter = counter.clone();
            Proto::new(socket, &cfg,
                BufferedDispatcher::new(addr, &h1, move || {
                    let counter = counter.clone();
                    move |r, e| {
                        let val = counter.fetch_add(1, Ordering::SeqCst);
                        service(val, r, e)
                    }
                }),
                &h1)
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .buffer_unordered(200000)
          .for_each(|()| Ok(()));

    lp.run(done).unwrap();
}
