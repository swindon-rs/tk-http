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
//!         let resp = req.new_response();
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
//!     minihttp::serve(&lp.handle(), addr, HelloWorld);
//!     lp.run(futures::empty::<(), ()>()).unwrap();
//! }
//! ```

extern crate bytes;
extern crate futures;
extern crate httparse;
extern crate tokio_core;
extern crate tokio_service;
// extern crate url;
extern crate netbuf;


pub mod request;
pub mod response;
pub mod server;
pub mod enums;
mod error;

use std::net::SocketAddr;

use futures::Future;
use futures::stream::{Stream};
use tokio_core::reactor::Handle;
use tokio_core::net::TcpListener;
use tokio_service::NewService;

pub use request::Request;
pub use response::Response;
pub use error::Error;


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
pub fn serve<S>(handle: &Handle, addr: SocketAddr, service: S)
    where S: NewService<Request=Request, Response=Response, Error=server::HttpError> + 'static,
{
    let listener = TcpListener::bind(&addr, handle).unwrap();
    let handle2 = handle.clone();

    handle.spawn(listener.incoming().for_each(move |(stream, addr)| {
        println!("Got incomming connection: {:?}, {:?}", stream, addr);
        let handler = service.new_service().unwrap();
        handle2.spawn(
            server::HttpServer::new(stream, handler)
            .map(|_| {println!("done"); })
            .map_err(|err| { println!("Got Error: {:?}", err); }));
        Ok(())
    }).map_err(|e| {
        println!("Server error: {:?}", e)
    }));
}
