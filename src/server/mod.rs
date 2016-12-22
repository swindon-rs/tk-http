//! HTTP server protocol implementation
//!
mod config;
mod error;
mod codec;
mod proto;
mod parser;
mod encoder;
mod request_target;
pub mod buffered;

pub use self::error::Error;
pub use self::encoder::{Encoder, EncoderDone};
pub use self::codec::{Codec, Dispatcher, RecvMode, Head};
pub use self::proto::Proto;


/// Fine-grained configuration of the HTTP server
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
}
