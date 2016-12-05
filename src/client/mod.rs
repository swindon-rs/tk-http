mod simple;
mod errors;
mod client;
mod encoder;
mod proto;
mod parser;
pub mod buffered;

// utils, move to another crate(s)
mod connect;

pub use self::simple::fetch;
pub use self::errors::Error;
pub use self::client::{Client, Codec, Head};
pub use self::encoder::{Encoder, EncoderDone};
