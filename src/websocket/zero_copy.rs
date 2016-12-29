use std::str::from_utf8;

use tk_bufstream::Buf;
use futures::Poll;
use futures::Async::{NotReady, Ready};
use byteorder::{BigEndian, ByteOrder};

use super::{Error, Packet};


/// A borrowed frame of websocket data
pub enum Frame<'a> {
    Ping(&'a [u8]),
    Pong(&'a [u8]),
    Text(&'a str),
    Binary(&'a [u8]),
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
        }
    }
}


fn parse_frame<'x>(buf: &'x mut Buf, limit: usize)
    -> Poll<(Frame<'x>, usize), Error>
{
    use self::Frame::*;

    if buf.len() < 2 {
        return Ok(NotReady);
    }
    let (size, fsize) = {
        match buf[1] & 0x7F {
            126 => {
                if buf.len() < 4 {
                    return Ok(NotReady);
                }
                (BigEndian::read_u16(&buf[2..4]) as u64, 4)
            }
            127 => {
                if buf.len() < 10 {
                    return Ok(NotReady);
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
        return Ok(NotReady);
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
    return Ok(Ready((frame, start + size)));
}
