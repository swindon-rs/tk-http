//! Simple HTTP service based on `tokio` tools
//!
//! # Examples
//!
//! Simple Hello world example.
//!
//! ```rust
//! struct HelloWorld;
//!
//! impl Service for HelloWorld {
//!     type Request = minihttp::Request;
//!     type Response = minihttp::Response;
//!     type Error = io::Error;
//!     type Futute = futures::Future;
//!
//!     fn call(&self, req: minihttp::Request) -> Self::Future {
//!         let resp = minihttp::Response::new();
//!         resp.header("Content-Type", "text/plain");
//!         resp.body("Hello, World");
//!         futures::finished(resp)
//!
//!     }
//!     fn poll(&self) -> Async<()> { Async::Ready(()) }
//! }
//!
//! fn main() {
//!     let lp = Core::new().unwrap();
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

pub mod request;
pub mod response;

use std::io;
use std::net::SocketAddr;

use bytes::BlockBuf;
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
    pub inner: T,
}

impl<T> Service for HttpService<T>
    where T: Service<Request=Request, Response=Response, Error=io::Error>
{
    type Request = Request;
    type Response = Message<Response, Receiver<(), io::Error>>;
    type Error = io::Error;
    type Future = Map<T::Future, fn(Response) -> Self::Response>;

    fn call(&self, req: Request) -> Self::Future {
        self.inner.call(req).map(Message::WithoutBody)
    }

    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}


/// Bind to address and start serving the service
///
/// # Examples
///
/// ```rust
/// let service = SomeHTTPService::new();
///
/// let lp = Core::new().unwrap();
///
/// let addr = "0.0.0.0:8080".parse().unwrap();
///
/// serve(&lp.handle(), addr, service).unwrap();
///
/// lp.run(futures::empty<(), ()>()).unwrap();
/// ```
pub fn serve<T>(handle: &Handle, addr: SocketAddr, service: T) -> io::Result<ServerHandle>
    where T: NewService<Request=Request, Response=Response, Error=io::Error> + Send + 'static
    {
    server::listen(handle, addr, move |socket| {
        let service = try!(service.new_service());
        let service = HttpService { inner: service };
        // Create the transport
        let transport =
            Framed::new(socket,
                        request::Parser,
                        response::Serializer,
                        BlockBuf::default(),
                        BlockBuf::default());
        pipeline::Server::new(service, transport)
    })
}
