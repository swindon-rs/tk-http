//! The HTTP/1.x client protocol implementation
//!
mod client;
mod config;
mod encoder;
mod errors;
mod head;
mod parser;
mod proto;
mod recv_mode;
pub mod buffered;

pub use self::errors::Error;
pub use self::client::{Client, Codec};
pub use self::encoder::{Encoder, EncoderDone, WaitFlush};
pub use self::proto::{Proto};

use std::borrow::Cow;
use std::time::Duration;

use httparse::Header;

use self::client::BodyKind;
use {Version};

/// Fine-grained configuration of the HTTP connection
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
    keep_alive_timeout: Duration,
    safe_pipeline_timeout: Duration,
    max_request_timeout: Duration,
}

/// A borrowed structure that represents response headers
///
/// It's passed to `Codec::headers_received` and you are free to store or
/// discard any needed fields and headers from it.
///
#[derive(Debug)]
pub struct Head<'a> {
    version: Version,
    code: u16,
    reason: &'a str,
    headers: &'a [Header<'a>],
    body_kind: BodyKind,
    connection_header: Option<Cow<'a, str>>,
    connection_close: bool,
}

/// This type is returned from `headers_received` handler of either
/// client client or server protocol handler
///
/// The marker is used to denote whether you want to have the whole request
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
}
