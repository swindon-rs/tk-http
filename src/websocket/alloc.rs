use websocket::zero_copy::Frame;

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

impl<'a> From<&'a Packet> for Frame<'a> {
    fn from(pkt: &'a Packet) -> Frame<'a> {
        use websocket::zero_copy::Frame as F;
        use self::Packet as P;
        match *pkt {
            P::Ping(ref x) => F::Ping(x),
            P::Pong(ref x) => F::Pong(x),
            P::Text(ref x) => F::Text(x),
            P::Binary(ref x) => F::Binary(x),
            P::Close(c, ref t) => F::Close(c, t),
        }
    }
}
