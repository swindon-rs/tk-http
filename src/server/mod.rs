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
mod recv_mode;
pub mod buffered;

pub use self::error::Error;
pub use self::encoder::{Encoder, EncoderDone, FutureRawBody, RawBody};
pub use self::codec::{Codec, Dispatcher};
pub use self::proto::Proto;
pub use self::headers::{Head, HeaderIter};
pub use self::request_target::RequestTarget;
pub use self::websocket::{WebsocketAccept, WebsocketHandshake};

use std::time::Duration;


/// Fine-grained configuration of the HTTP server
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
    first_byte_timeout: Duration,
    keep_alive_timeout: Duration,
    headers_timeout: Duration,
    input_body_byte_timeout: Duration,
    input_body_whole_timeout: Duration,
    output_body_byte_timeout: Duration,
    output_body_whole_timeout: Duration,
}

/// This type is returned from `headers_received` handler of either
/// client client or server protocol handler
///
/// The marker is used to denote whether you want to have the whole response
/// buffered for you or read chunk by chunk.
///
/// The `Progressive` (chunk by chunk) mode is mostly useful for proxy servers.
/// Or it may be useful if your handler is able to parse data without holding
/// everything in the memory.
///
/// Otherwise, it's best to use `Buffered` mode (for example, comparing with
/// using your own buffering). We do our best to optimize it for you.
#[derive(Debug, Clone)]
pub struct RecvMode {
    mode: recv_mode::Mode,
    timeout: Option<Duration>,
}
