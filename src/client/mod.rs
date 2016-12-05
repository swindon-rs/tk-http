//! The HTTP/1.x client protocol implementation
//!
mod simple;
mod errors;
mod client;
mod encoder;
mod proto;
mod parser;
pub mod buffered;

// utils, move to another crate(s)
mod connect;

pub use self::simple::fetch_once_buffered;
pub use self::errors::Error;
pub use self::client::{Client, Codec, Head};
pub use self::encoder::{Encoder, EncoderDone};
