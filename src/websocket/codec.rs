use std::io;
use tk_bufstream::{Buf, Encode, Decode};

use websocket::{Packet, Frame};
use websocket::error::Error;


const MAX_PACKET_SIZE: usize = 10 << 20;

/// Websocket codec for use with tk-bufstream in `Codec::hijack()`
///
/// This codec is used out of the box in
/// `BufferedDispatcher::new_with_websockets`
pub struct ServerCodec;

/// Websocket codec for use with tk-bufstream
///
/// This codec is used out of the box in `HandshakeProto`
pub struct ClientCodec;


impl Encode for ServerCodec {
    type Item = Packet;
    fn encode(&mut self, data: Packet, buf: &mut Buf) {
        // TODO(tailhook) should we also change state on close somehow?
        Frame::from(&data).write(buf, false)
    }
}

impl Decode for ServerCodec {
    type Item = Packet;
    fn decode(&mut self, buf: &mut Buf) -> Result<Option<Packet>, io::Error> {
        let parse_result = Frame::parse(buf, MAX_PACKET_SIZE, true)
            // TODO(tailhook) fix me when error type in bufstream
            // is associated type
            .map_err(|e| io::Error::new(io::ErrorKind::Other, Error::from(e)))?
            .map(|(p, b)| (p.into(), b));
        if let Some((p, b)) = parse_result {
            buf.consume(b);
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }
}

impl Encode for ClientCodec {
    type Item = Packet;
    fn encode(&mut self, data: Packet, buf: &mut Buf) {
        // TODO(tailhook) should we also change state on close somehow?
        Frame::from(&data).write(buf, true)
    }
}

impl Decode for ClientCodec {
    type Item = Packet;
    fn decode(&mut self, buf: &mut Buf) -> Result<Option<Packet>, io::Error> {
        let parse_result = Frame::parse(buf, MAX_PACKET_SIZE, false)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .map(|(p, b)| (p.into(), b));
        if let Some((p, b)) = parse_result {
            buf.consume(b);
            Ok(Some(p))
        } else {
            Ok(None)
        }
    }
}
