use std::str::from_utf8;

use tk_bufstream::Buf;
use byteorder::{BigEndian, ByteOrder};

use super::{Error, Packet};


/// A borrowed frame of websocket data
pub enum Frame<'a> {
    /// Ping mesage
    Ping(&'a [u8]),
    /// Pong mesage
    Pong(&'a [u8]),
    /// Text (utf-8) message
    Text(&'a str),
    /// Binary message
    Binary(&'a [u8]),
    /// Close message
    Close(u16, &'a str),
}

impl<'a> Into<Packet> for Frame<'a> {
    fn into(self) -> Packet {
        use self::Frame as F;
        use super::Packet as P;
        match self {
            F::Ping(x) => P::Ping(x.to_owned()),
            F::Pong(x) => P::Pong(x.to_owned()),
            F::Text(x) => P::Text(x.to_owned()),
            F::Binary(x) => P::Binary(x.to_owned()),
            F::Close(c, t) => P::Close(c, t.to_owned()),
        }
    }
}

impl<'a> Into<Packet> for &'a Frame<'a> {
    fn into(self) -> Packet {
        use self::Frame as F;
        use super::Packet as P;
        match *self {
            F::Ping(x) => P::Ping(x.to_owned()),
            F::Pong(x) => P::Pong(x.to_owned()),
            F::Text(x) => P::Text(x.to_owned()),
            F::Binary(x) => P::Binary(x.to_owned()),
            F::Close(c, t) => P::Close(c, t.to_owned()),
        }
    }
}


pub fn parse_frame<'x>(buf: &'x mut Buf, limit: usize)
    -> Result<Option<(Frame<'x>, usize)>, Error>
{
    use self::Frame::*;

    if buf.len() < 2 {
        return Ok(None);
    }
    let (size, fsize) = {
        match buf[1] & 0x7F {
            126 => {
                if buf.len() < 4 {
                    return Ok(None);
                }
                (BigEndian::read_u16(&buf[2..4]) as u64, 4)
            }
            127 => {
                if buf.len() < 10 {
                    return Ok(None);
                }
                (BigEndian::read_u64(&buf[2..10]), 10)
            }
            size => (size as u64, 2),
        }
    };
    if size > limit as u64 {
        return Err(Error::TooLong);
    }
    let size = size as usize;
    let start = fsize + 4 /* mask size */;
    if buf.len() < start + size {
        return Ok(None);
    }

    let fin = buf[0] & 0x80 != 0;
    let opcode = buf[0] & 0x0F;
    // TODO(tailhook) should we assert that reserved bits are zero?
    let mask = buf[1] & 0x80 != 0;
    if !fin {
        return Err(Error::Fragmented);
    }
    if !mask {
        return Err(Error::Unmasked);
    }
    let mask = [buf[start-4], buf[start-3], buf[start-2], buf[start-1]];
    for idx in 0..size { // hopefully llvm is smart enough to optimize it
        buf[start + idx] ^= mask[idx % 4];
    }
    let data = &buf[start..(start + size)];
    let frame = match opcode {
        0x9 => Ping(data),
        0xA => Pong(data),
        0x1 => Text(from_utf8(data)?),
        0x2 => Binary(data),
        // TODO(tailhook) implement shutdown packets
        x => return Err(Error::InvalidOpcode(x)),
    };
    return Ok(Some((frame, start + size)));
}

pub fn write_packet(buf: &mut Buf, opcode: u8, data: &[u8]) {
    debug_assert!(opcode & 0xF0 == 0);
    let first_byte = opcode | 0x80;  // always fin
    match data.len() {
        len @ 0...125 => {
            buf.extend(&[first_byte, len as u8]);
        }
        len @ 126...65535 => {
            buf.extend(&[first_byte, 126,
                (len >> 8) as u8, (len & 0xFF) as u8]);
        }
        len => {
            buf.extend(&[first_byte, 127,
                ((len >> 56) & 0xFF) as u8,
                ((len >> 48) & 0xFF) as u8,
                ((len >> 40) & 0xFF) as u8,
                ((len >> 32) & 0xFF) as u8,
                ((len >> 24) & 0xFF) as u8,
                ((len >> 16) & 0xFF) as u8,
                ((len >> 8) & 0xFF) as u8,
                (len & 0xFF) as u8]);
        }
    }
    buf.extend(data);
}

/// Write close message to websocket
pub fn write_close(buf: &mut Buf, code: u16, reason: &str) {
    let data = reason.as_bytes();
    assert!(data.len() <= 123);
    buf.extend(&[0x88, (data.len() + 2) as u8,
                  (code >> 8) as u8, (code & 0xFF) as u8]);
    buf.extend(data);
}
