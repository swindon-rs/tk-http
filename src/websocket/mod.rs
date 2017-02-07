//! Websocket support stuff
//!
//! Websockets are initiated by server implementation, this module only
//! contains websocket message types and similar stuff.
use std::time::Duration;

mod error;
mod zero_copy;
mod alloc;
mod codec;
mod dispatcher;
mod config;
mod client;

pub use self::error::Error;
pub use self::zero_copy::Frame;
pub use self::alloc::Packet;
pub use self::codec::Codec;
pub use self::dispatcher::{Loop, Dispatcher};


/// Configuration of a `websocket::Loop` object (a server-side websocket
/// connection).
#[derive(Debug, Clone)]
pub struct Config {
    ping_interval: Duration,
    inactivity_timeout: Duration,
    max_packet_size: usize,
}
