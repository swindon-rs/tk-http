//! Simple HTTP service based on `tokio` tools
#![recursion_limit="100"]
#![warn(missing_docs)]

extern crate futures;
extern crate url;
extern crate sha1;
extern crate httparse;
extern crate tokio_core;
extern crate netbuf;
extern crate tk_bufstream;
#[macro_use(quick_error)] extern crate quick_error;
#[macro_use] extern crate matches;
#[macro_use] extern crate log;
// These ones for "simple" interface
extern crate abstract_ns;
extern crate futures_cpupool;
extern crate ns_std_threaded;
#[cfg(feature="sendfile")] extern crate tk_sendfile;


pub mod server;
pub mod client;
mod enums;
mod headers;
mod base_serializer;
mod opt_future;
mod chunked;
mod body_parser;

pub use enums::{Version, Status};
pub use opt_future::OptFuture;
