use std::io;
use tk_bufstream::{Buf, Encode, Decode};

use super::{Packet, Error};
use super::zero_copy::{parse_frame, write_packet};


const MAX_PACKET_SIZE: usize = 10 << 20;

pub struct Codec;


impl Encode for Codec {
    type Item = Packet;
    fn encode(&mut self, data: Packet, buf: &mut Buf) {
        use super::Packet::*;
        match data {
            Ping(data) => write_packet(buf, 0x9, &data),
            Pong(data) => write_packet(buf, 0xA, &data),
            Text(data) => write_packet(buf, 0x1, data.as_bytes()),
            Binary(data) => write_packet(buf, 0x2, &data),
        }
    }
}

impl Decode for Codec {
    type Item = Packet;
    fn decode(&mut self, buf: &mut Buf) -> Result<Option<Packet>, io::Error> {
        let parse_result = parse_frame(buf, MAX_PACKET_SIZE)
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
