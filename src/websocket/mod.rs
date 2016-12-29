//! Websocket support stuff
//!
//! Websockets are initiated by server implementation, this module only
//! contains websocket message types and similar stuff.

mod error;
mod zero_copy;
mod alloc;
mod codec;

pub use self::error::Error;
pub use self::zero_copy::Frame;
pub use self::alloc::Packet;
pub use self::codec::Codec;
