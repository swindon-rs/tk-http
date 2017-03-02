extern crate time;
extern crate tokio_core;
extern crate futures;
extern crate tk_bufstream;
extern crate netbuf;
extern crate tk_http;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;

use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use tokio_core::io::Io;
use futures::{Stream, Future};
use futures::future::{FutureResult, ok};

use tk_http::{Status};
use tk_http::server::buffered::{Request, BufferedDispatcher};
use tk_http::server::{Encoder, EncoderDone, Config, Proto, Error};


const INDEX: &'static str = include_str!("ws.html");
const JS: &'static str = include_str!("ws.js");

fn service<S:Io>(req: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    if let Some(ws) = req.websocket_handshake() {
        e.status(Status::SwitchingProtocol);
        e.format_header("Date", time::now_utc().rfc822()).unwrap();
        e.add_header("Server",
            concat!("tk_http/", env!("CARGO_PKG_VERSION"))
        ).unwrap();
        e.add_header("Connection", "upgrade").unwrap();
        e.add_header("Upgrade", "websocket").unwrap();
        e.format_header("Sec-Websocket-Accept", &ws.accept).unwrap();
        e.done_headers().unwrap();
        ok(e.done())
    } else {
        let (data, ctype) = match req.path() {
            "/ws.js" => (JS, "text/javascript; charset=utf-8"),
            _ => (INDEX, "text/html; charset=utf-8"),
        };
        e.status(Status::Ok);
        e.add_length(data.as_bytes().len() as u64).unwrap();
        e.format_header("Date", time::now_utc().rfc822()).unwrap();
        e.add_header("Content-Type", ctype).unwrap();
        e.add_header("Server",
            concat!("tk_http/", env!("CARGO_PKG_VERSION"))
        ).unwrap();
        if e.done_headers().unwrap() {
            e.write_body(data.as_bytes());
        }
        ok(e.done())
    }
}


fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init().expect("init logging");

    let mut lp = Core::new().unwrap();
    let h1 = lp.handle();

    let addr = "0.0.0.0:8080".parse().unwrap();
    let listener = TcpListener::bind(&addr, &lp.handle()).unwrap();
    let cfg = Config::new().done();

    let done = listener.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(move |(socket, addr)| {
            Proto::new(socket, &cfg,
                BufferedDispatcher::new_with_websockets(addr, &h1,
                    service,
                    |out, inp| {
                        inp.forward(out)
                        .map(|_| ())
                        .map_err(|e| error!("Websock err: {}", e))
                    }),
                &h1)
            .map_err(|e| { println!("Connection error: {}", e); })
            .then(|_| Ok(())) // don't fail, please
        })
        .buffer_unordered(200000)
          .for_each(|()| Ok(()));

    lp.run(done).unwrap();
}
