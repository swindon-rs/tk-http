extern crate tokio_core;
extern crate tokio_service;
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
use tokio_core::net::TcpStream;
use tokio_service::Service;
use futures::{BoxFuture, Future};
use futures_cpupool::CpuPool;
use tk_bufstream::IoBuf;
use tk_sendfile::DiskPool;

use minihttp::enums::Status;
use minihttp::server::{ResponseWriter, GenericResponse, Error, Request};

#[derive(Clone)]
struct HelloWorld {
    pool: DiskPool,
    path: PathBuf,
}

struct Response(DiskPool, PathBuf);

impl GenericResponse<TcpStream> for Response {
    type Future = BoxFuture<IoBuf<TcpStream>, Error>;
    fn into_serializer(self, mut response: ResponseWriter<TcpStream>)
        -> Self::Future
    {
        self.0.open(self.1)
        .and_then(move |file| {
            response.status(Status::Ok);
            response.add_length(file.size()).unwrap();
            if response.done_headers().unwrap() {
                response.steal_socket()
                .and_then(|stream| file.write_into(stream))
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

    let disk_pool = DiskPool::new(CpuPool::new(40));

    let mut lp = Core::new().unwrap();
    let svc = HelloWorld {
        pool: disk_pool,
        path: filename,
    };

    minihttp::serve(&lp.handle(), addr, move || Ok(svc.clone()));

    lp.run(futures::empty::<(), ()>()).unwrap();
}
