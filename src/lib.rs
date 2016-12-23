//! Simple HTTP service based on `tokio` tools
#![recursion_limit="100"]

extern crate futures;
extern crate url;
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


pub mod server;
pub mod client;
mod enums;
mod headers;
mod base_serializer;
mod opt_future;
mod chunked;

pub use enums::{Version, Status};
pub use opt_future::OptFuture;
