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
//! use minihttp::{Request, Error, ResponseFn, Status};
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
//!     fn poll_ready(&self) -> Async<()> { Async::Ready(()) }
//! }
//!
//! fn main() {
//!     let mut lp = Core::new().unwrap();
//!
//!     let addr = "0.0.0.0:8080".parse().unwrap();
//!
//!     minihttp::serve(&lp.handle(), addr, HelloWorld);
//!     lp.run(futures::empty::<(), ()>()).unwrap();
//! }
//! ```

extern crate futures;
extern crate httparse;
extern crate tokio_core;
extern crate tokio_service;
extern crate netbuf;
extern crate tk_bufstream;
#[macro_use(quick_error)] extern crate quick_error;
#[macro_use] extern crate matches;
#[macro_use] extern crate log;


pub mod request;
pub mod server;
pub mod enums;
mod error;
mod lambda;
mod simple_error_page;
mod serve;
mod base_serializer;

use std::net::SocketAddr;

use futures::Future;
use futures::stream::{Stream};
use tokio_core::reactor::Handle;
use tokio_core::net::{TcpListener, TcpStream};
use tokio_service::NewService;

pub use enums::{Version, Status};
pub use request::Request;
pub use error::Error;
pub use serve::{GenericResponse, ResponseWriter};
pub use lambda::ResponseFn;
pub use simple_error_page::SimpleErrorPage;


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
    where T: NewService<Request=Request, Error=Error> + 'static,
          T::Response: GenericResponse<TcpStream>,
{
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let handle2 = handle.clone();

    handle.spawn(listener.incoming().for_each(move |(stream, addr)| {
        trace!("Got incomming connection: {:?}, {:?}", stream, addr);
        let handler = service.new_service().unwrap();
        handle2.spawn(
            server::HttpServer::new(stream, handler)
            .map(|()| { trace!("Connection closed"); })
            .map_err(|err| { debug!("Connection error: {:?}", err); }));
        Ok(())
    }).map_err(|e| {
        println!("Server error: {:?}", e)
    }));
}
