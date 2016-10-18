extern crate tokio_core;
extern crate tokio_service;
extern crate futures;
extern crate netbuf;
extern crate argparse;
extern crate minihttp;
extern crate tk_sendfile;
#[macro_use] extern crate log;
extern crate env_logger;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use netbuf::Buf;
use argparse::{ArgumentParser, Parse};
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tokio_service::Service;
use futures::{Async, BoxFuture, Future};
use tk_sendfile::DiskPool;

use minihttp::request::Request;
use minihttp::{ResponseWriter, GenericResponse, Error};

#[derive(Clone)]
struct HelloWorld {
    pool: DiskPool,
    path: PathBuf,
}

struct Response(DiskPool, PathBuf);

impl GenericResponse for Response {
    type Future = BoxFuture<(TcpStream, Buf), Error>;
    fn make_serializer(self, mut response: ResponseWriter)
        -> Self::Future
    {
        self.0.open(self.1)
        .and_then(move |file| {
            response.status(200, "OK");
            response.add_length(file.size()).unwrap();
            if response.done_headers().unwrap() {
                response.steal_socket()
                .and_then(|(socket, buf)| {
                    file.write_into(socket).map(|sock| (sock, buf))
                })
            } else {
                // Don't send any body
                unimplemented!();
            }
        }).map_err(|e| e.into()).boxed()
    }
}

impl Service for HelloWorld {
    type Request = Request;
    type Response = Response;
    type Error = Error;
    type Future = futures::Finished<Self::Response, Self::Error>;

    fn call(&self, _req: Self::Request) -> Self::Future {
        futures::finished(Response(self.pool.clone(), self.path.clone()))
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}


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

    let disk_pool = DiskPool::new();

    let mut lp = Core::new().unwrap();

    minihttp::serve(&lp.handle(), addr, HelloWorld {
        pool: disk_pool,
        path: filename,
    });

    lp.run(futures::empty::<(), ()>()).unwrap();
}
