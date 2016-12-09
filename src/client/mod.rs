//! The HTTP/1.x client protocol implementation
//!
mod simple;
mod errors;
mod client;
mod encoder;
mod proto;
mod parser;
mod config;
pub mod buffered;

pub use self::simple::fetch_once_buffered;
pub use self::errors::Error;
pub use self::client::{Client, Codec, Head};
pub use self::encoder::{Encoder, EncoderDone};
pub use self::proto::{Proto};

/// Fine-grained configuration of the HTTP connection
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
}

