//! Simple HTTP service based on `tokio` tools
//!
//! # Examples
//!
//! Simple Hello world example.
//!
//! ```rust,no_run
//! extern crate futures;
//! extern crate minihttp;
//! extern crate netbuf;
//! extern crate tokio_core;
//! extern crate tk_bufstream;
//! extern crate tokio_service;
//! use std::io;
//! use tk_bufstream::IoBuf;
//! use tokio_service::{Service, NewService};
//! use tokio_core::reactor::Core;
//! use tokio_core::net::TcpStream;
//! use futures::{Async, Finished, finished};
//! use minihttp::server::{Request, Error, ResponseFn, Status};
//! use minihttp::enums::{Status};
//!
//! #[derive(Clone)]
//! struct HelloWorld;
//!
//! impl Service for HelloWorld {
//!    type Request = Request;
//!    type Response = ResponseFn<Finished<IoBuf<TcpStream>, Error>,
//!                               TcpStream>;
//!    type Error = Error;
//!    type Future = Finished<Self::Response, Error>;
//!
//!     fn call(&self, _req: minihttp::Request) -> Self::Future {
//!        // Note: rather than allocating a response object, we return
//!        // a lambda that pushes headers into `ResponseWriter` which
//!        // writes them directly into response buffer without allocating
//!        // intermediate structures
//!        finished(ResponseFn::new(move |mut res| {
//!            res.status(Status::Ok);
//!            res.add_chunked().unwrap();
//!            if res.done_headers().unwrap() {
//!                res.write_body(b"Hello world!");
//!            }
//!            res.done()
//!        }))
//!     }
//! }
//!
//! fn main() {
//!     let mut lp = Core::new().unwrap();
//!
//!     let addr = "0.0.0.0:8080".parse().unwrap();
//!
//!     minihttp::serve(&lp.handle(), addr, || Ok(HelloWorld));
//!     lp.run(futures::empty::<(), ()>()).unwrap();
//! }
//! ```
#![recursion_limit="100"]

extern crate futures;
extern crate url;
extern crate httparse;
extern crate tokio_core;
extern crate tokio_service;
extern crate netbuf;
extern crate tk_bufstream;
#[macro_use(quick_error)] extern crate quick_error;
#[macro_use] extern crate matches;
#[macro_use] extern crate log;
// These ones for "simple" interface
extern crate abstract_ns;
extern crate futures_cpupool;
extern crate ns_std_threaded;


pub mod server;
pub mod enums;
pub mod client;
mod headers;
mod base_serializer;
mod opt_future;

use std::net::SocketAddr;

use futures::Future;
use futures::stream::{Stream};
use tokio_core::reactor::Handle;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_service::NewService;

pub use enums::{Version, Status};
pub use opt_future::OptFuture;


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
/// serve(&lp.handle(), addr, service);
///
/// lp.run(futures::empty<(), ()>() ).unwrap();
/// ```
pub fn serve<T>(handle: &Handle, addr: SocketAddr, service: T)
    where
        T: NewService<Request=server::Request, Error=server::Error> + 'static,
        T::Response: server::GenericResponse<TcpStream>,
{
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let handle2 = handle.clone();

    handle.spawn(listener.incoming().for_each(move |(stream, addr)| {
        trace!("Got incomming connection: {:?}, {:?}", stream, addr);
        let handler = service.new_service().unwrap();
        handle2.spawn(
            server::HttpServer::new(stream, handler, addr)
            .map(|()| { trace!("Connection closed"); })
            .map_err(|err| { debug!("Connection error: {:?}", err); }));
        Ok(())
    }).map_err(|e| {
        println!("Server error: {:?}", e)
    }));
}
