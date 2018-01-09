extern crate tokio_core;
extern crate futures;
extern crate futures_cpupool;
extern crate netbuf;
extern crate argparse;
extern crate tk_http;
extern crate tk_sendfile;
extern crate tk_bufstream;
extern crate tk_listen;
extern crate log;
extern crate env_logger;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use argparse::{ArgumentParser, Parse};
use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use futures::{Stream, Future};
use futures_cpupool::CpuPool;
use tk_sendfile::DiskPool;
use futures::future::{ok};

use tk_http::Status;
use tk_http::server::buffered::{BufferedDispatcher};
use tk_http::server::{Encoder, Config, Proto, Error};
use tk_listen::ListenExt;


fn main() {
    let mut filename = PathBuf::from("examples/sendfile.rs");
    let mut addr = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Serve a file via HTTP on any open connection");
        ap.refer(&mut addr)
           .add_option(&["-l", "--listen"], Parse,
            "Listening address");
        ap.refer(&mut filename)
           .add_option(&["-f", "--filename"], Parse,
            "File to serve");
        ap.parse_args_or_exit();
    }

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init().expect("init logging");

    let mut lp = Core::new().unwrap();
    let listener = TcpListener::bind(&addr, &lp.handle()).unwrap();
    let disk_pool = DiskPool::new(CpuPool::new(40));
    let cfg = Config::new().done();
    let h1 = lp.handle();

    let done = listener.incoming()
        .sleep_on_error(Duration::from_millis(100), &lp.handle())
        .map(move |(socket, addr)| {
            let filename = filename.clone();
            let disk_pool = disk_pool.clone();
            Proto::new(socket, &cfg,
                BufferedDispatcher::new(addr, &h1, move || {
                    let filename = filename.clone();
                    let disk_pool = disk_pool.clone();
                    move |_, mut e: Encoder<_>| {
                        disk_pool.open(filename.clone())
                        .and_then(move |file| {
                            e.status(Status::Ok);
                            e.add_length(file.size()).unwrap();
                            if e.done_headers().unwrap() {
                                Box::new(e.raw_body()
                                .and_then(|raw_body| file.write_into(raw_body))
                                .map(|raw_body| raw_body.done()))
                                as Box<Future<Item=_, Error=_>>
                            } else {
                                Box::new(ok(e.done()))
                                as Box<Future<Item=_, Error=_>>
                            }
                        })
                        .map_err(|_| -> Error { unimplemented!(); })
                    }
                }),
                &h1)
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .listen(1000);

    lp.run(done).unwrap();
}
