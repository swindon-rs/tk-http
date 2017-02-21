//! Websocket support stuff
//!
//! Websockets are initiated by server implementation, this module only
//! contains websocket message types and similar stuff.
use std::time::Duration;

mod alloc;
mod codec;
mod config;
mod dispatcher;
mod error;
mod keys;
mod zero_copy;
pub mod client;

pub use self::alloc::Packet;
pub use self::codec::{ServerCodec, ClientCodec};
pub use self::dispatcher::{Loop, Dispatcher};
pub use self::error::Error;
pub use self::keys::{GUID, Accept, Key};
pub use self::zero_copy::Frame;


/// Configuration of a `websocket::Loop` object (a server-side websocket
/// connection).
#[derive(Debug, Clone)]
pub struct Config {
    ping_interval: Duration,
    inactivity_timeout: Duration,
    max_packet_size: usize,
}
