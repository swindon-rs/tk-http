/// A websocket packet
///
/// Note: unlike `Frame` this has data allocated on the heap so has static
/// lifetime
#[derive(Debug, Clone)]
pub enum Packet {
    /// Ping packet (with data)
    Ping(Vec<u8>),
    /// Pong packet (with data)
    Pong(Vec<u8>),
    /// Text (utf-8) messsage
    Text(String),
    /// Binary message
    Binary(Vec<u8>),
    /// Close message
    Close(u16, String),
}
