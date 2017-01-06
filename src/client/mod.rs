//! The HTTP/1.x client protocol implementation
//!
mod simple;
mod errors;
mod client;
mod encoder;
mod proto;
mod parser;
mod config;
mod head;
pub mod buffered;

pub use self::simple::fetch_once_buffered;
pub use self::errors::Error;
pub use self::client::{Client, Codec, RecvMode};
pub use self::encoder::{Encoder, EncoderDone};
pub use self::proto::{Proto};

use std::borrow::Cow;

use httparse::Header;

use self::client::BodyKind;
use {Version};

/// Fine-grained configuration of the HTTP connection
#[derive(Debug, Clone)]
pub struct Config {
    inflight_request_limit: usize,
    inflight_request_prealloc: usize,
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
