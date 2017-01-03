extern crate tokio_core;
extern crate futures;
extern crate futures_cpupool;
extern crate netbuf;
extern crate argparse;
extern crate minihttp;
extern crate tk_sendfile;
extern crate tk_bufstream;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use argparse::{ArgumentParser, Parse};
use tokio_core::reactor::Core;
use tokio_core::net::{TcpListener};
use futures::{Stream, Future};
use futures_cpupool::CpuPool;
use tk_sendfile::DiskPool;
use futures::future::{ok};

use minihttp::Status;
use minihttp::server::buffered::{BufferedDispatcher};
use minihttp::server::{Encoder, Config, Proto, Error};

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
        .map_err(|e| { println!("Accept error: {}", e); })
        .map(|(socket, addr)| {
            Proto::new(socket, &cfg,
                BufferedDispatcher::new(addr, &h1, || |_, mut e: Encoder<_>| {

                    disk_pool.open(filename.clone())
                    .and_then(move |file| {
                        e.status(Status::Ok);
                        e.add_length(file.size()).unwrap();
                        if e.done_headers().unwrap() {
                            e.raw_body()
                            .and_then(|raw_body| file.write_into(raw_body))
                            .map(|raw_body| raw_body.done())
                            .boxed()
                        } else {
                            ok(e.done()).boxed()
                        }
                    })
                    .map_err(|_| -> Error { unimplemented!(); })
                }))
            .map_err(|e| { println!("Connection error: {}", e); })
        })
        .buffer_unordered(200000)
          .for_each(|()| Ok(()));

    lp.run(done).unwrap();
}
