/// A websocket packet
///
/// Note: unlike `Frame` this has data allocated on the heap so has static
/// lifetime
pub enum Packet {
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Text(String),
    Binary(Vec<u8>),
}
