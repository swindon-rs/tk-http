//! Simple HTTP service based on `tokio` tools
#![recursion_limit="200"]
#![warn(missing_docs)]

extern crate futures;
extern crate url;
extern crate sha1;
extern crate rand;
extern crate httparse;
extern crate tokio_core;
extern crate netbuf;
extern crate tk_bufstream;
extern crate byteorder;
#[macro_use(quick_error)] extern crate quick_error;
#[macro_use] extern crate matches;
#[macro_use] extern crate log;


pub mod server;
pub mod client;
pub mod websocket;
mod enums;
mod headers;
mod base_serializer;
mod chunked;
mod body_parser;

pub use enums::{Version, Status};
