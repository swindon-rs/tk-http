extern crate time;
extern crate tokio_core;
extern crate futures;
extern crate tk_bufstream;
extern crate netbuf;
extern crate minihttp;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;
use std::time::Duration;

use tokio_core::reactor::{Core, Timeout};
use tokio_core::net::{TcpListener};
use tokio_core::io::Io;
use futures::{Stream, Future, Sink};
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::{unbounded, UnboundedSender};

use minihttp::{Status};
use minihttp::server::buffered::{Request, BufferedDispatcher};
use minihttp::server::{Encoder, EncoderDone, Config, Proto, Error};
use minihttp::websocket::{Loop, Config as WebsockConfig, Dispatcher, Frame};
use minihttp::websocket::Packet::{self, Text};


const INDEX: &'static str = include_str!("ws.html");
const JS: &'static str = include_str!("ws.js");

fn service<S:Io>(req: Request, mut e: Encoder<S>)
    -> FutureResult<EncoderDone<S>, Error>
{
    if let Some(ws) = req.websocket_handshake() {
        e.status(Status::SwitchingProtocol);
        e.format_header("Date", time::now_utc().rfc822()).unwrap();
        e.add_header("Server",
            concat!("minihttp/", env!("CARGO_PKG_VERSION"))
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
            concat!("minihttp/", env!("CARGO_PKG_VERSION"))
        ).unwrap();
        if e.done_headers().unwrap() {
            e.write_body(data.as_bytes());
        }
        ok(e.done())
    }
}

struct Echo(UnboundedSender<Packet>);

impl Dispatcher for Echo {
    type Future = FutureResult<(), ()>;
    fn frame(&mut self, frame: &Frame) -> FutureResult<(), ()> {
        self.0.start_send(frame.into()).unwrap();
        ok(())
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
    let wcfg = WebsockConfig::new().done();

    let done = listener.incoming()
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(move |(socket, addr)| {
            let wcfg = wcfg.clone();
            let h2 = h1.clone();
            Proto::new(socket, &cfg,
                BufferedDispatcher::new_with_websockets(addr, &h1,
                    service,
                    move |out, inp| {
                        let (tx, rx) = unbounded();
                        let tx2 = tx.clone();
                        h2.spawn(
                            Timeout::new(Duration::new(10, 0), &h2).unwrap()
                            .map_err(|_| unreachable!())
                            .and_then(move |_| {
                                tx2.send(Text("hello".to_string()))
                                .map_err(|_| ())
                            })
                            .then(|_| Ok(())));
                        Loop::new(out, inp, rx, Echo(tx), &wcfg)
                    }))
            .map_err(|e| { println!("Connection error: {}", e); })
            .then(|_| Ok(())) // don't fail, please
        })
        .buffer_unordered(200000)
          .for_each(|()| Ok(()));

    lp.run(done).unwrap();
}
