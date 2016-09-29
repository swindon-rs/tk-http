//! Simple HTTP service based on `tokio` tools
//!
//! # Examples
//!
//! Simple Hello world example.
//!
//! ```rust,no_run
//! extern crate futures;
//! extern crate minihttp;
//! extern crate tokio_core;
//! extern crate tokio_service;
//! use std::io;
//! use tokio_service::{Service, NewService};
//! use tokio_core::reactor::Core;
//! use futures::{Finished, Async};
//!
//! #[derive(Clone)]
//! struct HelloWorld;
//!
//! impl Service for HelloWorld {
//!     type Request = minihttp::Request;
//!     type Response = minihttp::Response;
//!     type Error = io::Error;
//!     type Future = Finished<minihttp::Response, io::Error>;
//!
//!     fn call(&self, req: minihttp::Request) -> Self::Future {
//!         let resp = minihttp::Response::new();
//!         // resp.header("Content-Type", "text/plain");
//!         // resp.body("Hello, World");
//!         futures::finished(resp)
//!
//!     }
//!     fn poll_ready(&self) -> Async<()> { Async::Ready(()) }
//! }
//!
//! fn main() {
//!     let mut lp = Core::new().unwrap();
//!
//!     let addr = "0.0.0.0:8080".parse().unwrap();
//!
//!     minihttp::serve(&lp.handle(), addr, HelloWorld).unwrap();
//!     lp.run(futures::empty::<(), ()>()).unwrap();
//! }
//! ```

extern crate bytes;
extern crate futures;
extern crate httparse;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
extern crate netbuf;

pub mod request;
pub mod response;
pub mod server;

use std::io;
use std::net::SocketAddr;

use bytes::buf::BlockBuf;
use futures::{Future, Map, Async};
use futures::stream::Receiver;
use tokio_core::reactor::Handle;
use tokio_proto::{server,pipeline};
use tokio_proto::{Framed, Message};
use tokio_proto::server::ServerHandle;
use tokio_service::{Service, NewService};

pub use request::Request;
pub use response::Response;


/// HTTP Service.
///
/// A wrapper around `tokio_service::Service`
pub struct HttpService<T> {
    handler: T,
}

impl<T> HttpService<T> {
    pub fn new(handler: T) -> HttpService<T> {
        HttpService {
            handler: handler,
        }
    }
}

impl<T> Service for HttpService<T>
    where T: Service<Request=Request, Response=Response, Error=io::Error>
{
    type Request = Request;
    type Response = Message<Response, Receiver<(), io::Error>>;
    type Error = io::Error;
    type Future = Map<T::Future, fn(Response) -> Self::Response>;

    fn call(&self, req: Request) -> Self::Future {
        self.handler.call(req).map(Message::WithoutBody)

        // Inside HttpService we receive parsed requests
        //  and must wrap Responses into Message variant
        //
        // Also we must control response headers:
        //  * HTTP version in status line
        //  * Connection header -- close or keep-alive
        //  * Server name;
        //  * Date;
        // First two headers depends on Request received
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}



/// Bind to address and start serving the service
///
/// # Examples
///
/// ```rust,ignore
/// let service = SomeHTTPService::new();
///
/// let mut lp = Core::new().unwrap();
///
/// let addr = "0.0.0.0:8080".parse().unwrap();
///
/// serve(&lp.handle(), addr, service).unwrap();
///
/// lp.run(futures::empty<(), ()>() ).unwrap();
/// ```
pub fn serve<T>(handle: &Handle, addr: SocketAddr, service: T) -> io::Result<ServerHandle>
    where T: NewService<Request=Request, Response=Response, Error=io::Error> + Send + 'static
    {
    tokio_proto::server::listen(handle, addr, move |socket| {
        let service = try!(service.new_service());
        let server = HttpService::new(service);
        // Create the transport
        let transport =
            Framed::new(socket,
                        request::Parser,
                        response::Serializer,
                        BlockBuf::default(),
                        BlockBuf::default());
        pipeline::Server::new(server, transport)
    })
}


pub fn core_serve(handle: &Handle, addr: SocketAddr) {
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let handle2 = handle.clone();
    handle.spawn(listener.incoming().for_each(move |(stream, addr)| {
        println!("Got incomming connection: {:?}, {:?}", stream, addr);
        handle2.spawn(
            server::HttpServer::new(stream)
            .map(|i| {println!("done"); })
            .map_err(|err| { println!("Got Error: {:?}", err); }));
        // * Spawn handler for connection;
        // * Count handled connections;
        //let (reader, writer) = stream.split();
        // Start handler task with two ends
        // handle2.spawn();
        Ok(())
    }).map_err(|e| {
        println!("Server error: {:?}", e)
    }));
}
