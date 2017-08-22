extern crate env_logger;
extern crate futures;
extern crate netbuf;
extern crate tk_bufstream;
extern crate tk_http;
extern crate tk_listen;
extern crate tokio_core;
extern crate tokio_io;

use std::env;
use std::time::Duration;

use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use tokio_io::AsyncWrite;
use futures::{Stream, Future, Async};
use futures::future::{FutureResult, ok, Either};

use tk_http::Status;
use tk_http::server::buffered::{Request, BufferedDispatcher};
use tk_http::server::{Encoder, EncoderDone, Config, Proto, Error};
use tk_listen::ListenExt;

struct Fibonacci<S> {
    encoder: Encoder<S>,
    current: u64,
}

impl<S: AsyncWrite> Future for Fibonacci<S> {
    type Item = EncoderDone<S>;
    type Error = Error;
    fn poll(&mut self) -> Result<Async<EncoderDone<S>>, Error> {
        use std::io::Write;
        while self.encoder.bytes_buffered() < 4096 {
            for _ in 0..1000 {
                self.current += 1;
                writeln!(self.encoder, "{}", self.current).unwrap();
            }
            if self.current % 1000000 == 0 {
                println!("Reached {}M", self.current / 1000000);
            }
            self.encoder.flush()?;
        }
        Ok(Async::NotReady)
    }
}

fn service<S>(req: Request, mut e: Encoder<S>)
    -> Either<Fibonacci<S>, FutureResult<EncoderDone<S>, Error>>
{
    println!("{:?} {}", req.method(), req.path());
    e.status(Status::Ok);
    e.add_chunked().unwrap();
    if e.done_headers().unwrap() {
        Either::A(Fibonacci {
            encoder: e,
            current: 1,
        })
    } else {
        Either::B(ok(e.done()))
    }
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
        .map(|(socket, addr)| {
            Proto::new(socket, &cfg,
                BufferedDispatcher::new(addr, &h1, || service),
                &h1)
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .listen(1000);

    lp.run(done).unwrap();
}
