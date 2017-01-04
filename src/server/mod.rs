//! HTTP server protocol implementation
//!
mod config;
mod error;
mod codec;
mod proto;
mod encoder;
mod request_target;
mod headers;
mod websocket;
pub mod buffered;

pub use self::error::Error;
pub use self::encoder::{Encoder, EncoderDone, FutureRawBody, RawBody};
pub use self::codec::{Codec, Dispatcher, RecvMode};
pub use self::proto::Proto;
pub use self::headers::{Head, HeaderIter};
pub use self::request_target::RequestTarget;
pub use self::websocket::{WebsocketAccept, WebsocketHandshake};


/// Fine-grained configuration of the HTTP server
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
}
