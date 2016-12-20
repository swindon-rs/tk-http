//! HTTP server protocol implementation
//!
mod config;
mod error;
mod codec;
mod proto;
mod parser;
mod encoder;
mod buffered;

pub use self::error::Error;
pub use self::encoder::{Encoder, EncoderDone};
pub use self::codec::{Codec, RecvMode, Head};


/// Fine-grained configuration of the HTTP server
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
}
