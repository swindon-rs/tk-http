use std::io;
use tk_bufstream::{Buf, Encode, Decode};

use super::{Packet};
use super::zero_copy::{parse_frame, write_packet, write_close};


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
        use super::Packet::*;
        match data {
            Ping(data) => write_packet(buf, 0x9, &data, false),
            Pong(data) => write_packet(buf, 0xA, &data, false),
            Text(data) => write_packet(buf, 0x1, data.as_bytes(), false),
            Binary(data) => write_packet(buf, 0x2, &data, false),
            // TODO(tailhook) should we also change state somehow?
            Close(c, t) => write_close(buf, c, &t, false),
        }
    }
}

impl Decode for ServerCodec {
    type Item = Packet;
    fn decode(&mut self, buf: &mut Buf) -> Result<Option<Packet>, io::Error> {
        let parse_result = parse_frame(buf, MAX_PACKET_SIZE, true)
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

impl Encode for ClientCodec {
    type Item = Packet;
    fn encode(&mut self, data: Packet, buf: &mut Buf) {
        use super::Packet::*;
        match data {
            Ping(data) => write_packet(buf, 0x9, &data, true),
            Pong(data) => write_packet(buf, 0xA, &data, true),
            Text(data) => write_packet(buf, 0x1, data.as_bytes(), true),
            Binary(data) => write_packet(buf, 0x2, &data, true),
            // TODO(tailhook) should we also change state somehow?
            Close(c, t) => write_close(buf, c, &t, true),
        }
    }
}

impl Decode for ClientCodec {
    type Item = Packet;
    fn decode(&mut self, buf: &mut Buf) -> Result<Option<Packet>, io::Error> {
        let parse_result = parse_frame(buf, MAX_PACKET_SIZE, false)
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
