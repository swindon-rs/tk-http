//! This crate contains implementation of HTTP/1.0 and HTTP/1.1 with
//! websockets support. (HTTP/2 support is planned)
//!
//! See [examples](https://github.com/swindon-rs/tk-http/tree/master/examples)
//! for usage examples.
//!
//! For client implementation it's recommended to use the library
//! together with [tk-pool](https://crates.io/crates/tk-pool).
//!
#![recursion_limit="200"]
#![warn(missing_docs)]

extern crate futures;
extern crate url;
extern crate sha1;
extern crate rand;
extern crate httparse;
extern crate tokio_core;
extern crate tokio_io;
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
