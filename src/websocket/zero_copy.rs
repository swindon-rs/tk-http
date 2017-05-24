use std::str::from_utf8;

use rand::{thread_rng, Rng};
use tk_bufstream::Buf;
use byteorder::{BigEndian, ByteOrder};

use super::{Packet};
use websocket::error::ErrorEnum;


/// A borrowed frame of websocket data
#[derive(Debug, Clone, PartialEq)]
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


pub fn parse_frame<'x>(buf: &'x mut Buf, limit: usize, masked: bool)
    -> Result<Option<(Frame<'x>, usize)>, ErrorEnum>
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
        return Err(ErrorEnum::TooLong);
    }
    let size = size as usize;
    let start = fsize + if masked { 4 } else { 0 } /* mask size */;
    if buf.len() < start + size {
        return Ok(None);
    }

    let fin = buf[0] & 0x80 != 0;
    let opcode = buf[0] & 0x0F;
    // TODO(tailhook) should we assert that reserved bits are zero?
    let mask = buf[1] & 0x80 != 0;
    if !fin {
        return Err(ErrorEnum::Fragmented);
    }
    if mask != masked {
        return Err(ErrorEnum::Unmasked);
    }
    if mask {
        let mask = [buf[start-4], buf[start-3], buf[start-2], buf[start-1]];
        for idx in 0..size { // hopefully llvm is smart enough to optimize it
            buf[start + idx] ^= mask[idx % 4];
        }
    }
    let data = &buf[start..(start + size)];
    let frame = match opcode {
        0x9 => Ping(data),
        0xA => Pong(data),
        0x1 => Text(from_utf8(data)?),
        0x2 => Binary(data),
        // TODO(tailhook) implement shutdown packets
        0x8 => {
            if data.len() < 2 {
                Close(1006, "")
            } else {
                Close(BigEndian::read_u16(&data[..2]), from_utf8(&data[2..])?)
            }
        }
        x => return Err(ErrorEnum::InvalidOpcode(x)),
    };
    return Ok(Some((frame, start + size)));
}

pub fn write_packet(buf: &mut Buf, opcode: u8, data: &[u8], mask: bool) {
    debug_assert!(opcode & 0xF0 == 0);
    let first_byte = opcode | 0x80;  // always fin
    let mask_bit = if mask { 0x80 } else { 0 };
    match data.len() {
        len @ 0...125 => {
            buf.extend(&[first_byte, (len as u8) | mask_bit]);
        }
        len @ 126...65535 => {
            buf.extend(&[first_byte, 126 | mask_bit,
                (len >> 8) as u8, (len & 0xFF) as u8]);
        }
        len => {
            buf.extend(&[first_byte, 127 | mask_bit,
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
    let mask_data = if mask {
        let mut bytes = [0u8; 4];
        thread_rng().fill_bytes(&mut bytes[..]);
        buf.extend(&bytes[..]);
        Some((buf.len(), bytes))
    } else {
        None
    };
    buf.extend(data);
    if let Some((start, bytes)) = mask_data {
        for idx in 0..(buf.len() - start) { // hopefully llvm will optimize it
            buf[start + idx] ^= bytes[idx % 4];
        }
    };
}

/// Write close message to websocket
pub fn write_close(buf: &mut Buf, code: u16, reason: &str, mask: bool) {
    let data = reason.as_bytes();
    let mask_bit = if mask { 0x80 } else { 0 };
    assert!(data.len() <= 123);
    buf.extend(&[0x88, ((data.len() + 2) as u8) | mask_bit]);
    let mask_data = if mask {
        let mut bytes = [0u8; 4];
        thread_rng().fill_bytes(&mut bytes[..]);
        buf.extend(&bytes[..]);
        Some((buf.len(), bytes))
    } else {
        None
    };
    buf.extend(&[(code >> 8) as u8, (code & 0xFF) as u8]);
    buf.extend(data);
    if let Some((start, bytes)) = mask_data {
        for idx in 0..(buf.len() - start) { // hopefully llvm will optimize it
            buf[start + idx] ^= bytes[idx % 4];
        }
    };
}

#[cfg(test)]
mod test {
    use netbuf::Buf;
    use std::iter::repeat;
    use super::parse_frame;
    use super::Frame::*;

    #[test]
    fn empty_frame() {
        let mut buf = Buf::new();
        assert_eq!(parse_frame(&mut buf, 1000, false).unwrap(), None);
        assert_eq!(parse_frame(&mut buf, 1000, true).unwrap(), None);
    }

    #[test]
    fn invalid_close_frame() {
        let mut buf = Buf::new();
        let data = b"\x88\x80\x00\x00\x00\x00";
        buf.extend(data);
        assert_eq!(parse_frame(&mut buf, 1000, true).unwrap(),
                   Some((Close(1006, ""), 6)));
    }

    #[test]
    fn parse_small_masked() {
        let data = b"\x81\x85\x00\x00\x00\x00hello";
        for i in 0..data.len()-1 {
            let mut buf = Buf::new();
            buf.extend(&data[..i]);
            assert_eq!(parse_frame(&mut buf, 1000, true).unwrap(), None);
        }
        let mut buf = Buf::new();
        buf.extend(data);
        assert_eq!(parse_frame(&mut buf, 1000, true).unwrap(),
            Some((Text("hello"), 11)));
    }

    #[test]
    fn parse_125m() {
        let data = b"\x81\xFD\x00\x00\x00\x00";
        for i in 0..124 {
            let mut buf = Buf::new();
            buf.extend(data);
            for _ in 0..i {
                buf.extend(&[b'x']);
            }
            assert_eq!(parse_frame(&mut buf, 1000, true).unwrap(), None);
        }
        let mut buf = Buf::new();
        buf.extend(data);
        for _ in 0..125 {
            buf.extend(&[b'x']);
        }
        assert_eq!(parse_frame(&mut buf, 1000, true).unwrap(),
            Some((Text(&repeat('x').take(125).collect::<String>()), 131)));
    }
    #[test]
    fn parse_4k_masked() {
        let data = b"\x81\xFE\x10\x00\x00\x00\x00\x00";
        for i in 0..4095 {
            let mut buf = Buf::new();
            buf.extend(data);
            for _ in 0..i {
                buf.extend(&[b'x']);
            }
            assert_eq!(parse_frame(&mut buf, 4096, true).unwrap(), None);
        }
        let mut buf = Buf::new();
        buf.extend(data);
        for _ in 0..4096 {
            buf.extend(&[b'x']);
        }
        assert_eq!(parse_frame(&mut buf, 4096, true).unwrap(),
            Some((Text(&repeat('x').take(4096).collect::<String>()), 4104)));
    }

    #[test]
    fn parse_small() {
        let data = b"\x81\x05hello";
        for i in 0..data.len()-1 {
            let mut buf = Buf::new();
            buf.extend(&data[..i]);
            assert_eq!(parse_frame(&mut buf, 1000, false).unwrap(), None);
        }
        let mut buf = Buf::new();
        buf.extend(data);
        assert_eq!(parse_frame(&mut buf, 1000, false).unwrap(),
            Some((Text("hello"), 7)));
    }

    #[test]
    fn parse_125() {
        let data = b"\x81\x7D";
        for i in 0..124 {
            let mut buf = Buf::new();
            buf.extend(data);
            for _ in 0..i {
                buf.extend(&[b'x']);
            }
            assert_eq!(parse_frame(&mut buf, 1000, false).unwrap(), None);
        }
        let mut buf = Buf::new();
        buf.extend(data);
        for _ in 0..125 {
            buf.extend(&[b'x']);
        }
        assert_eq!(parse_frame(&mut buf, 1000, false).unwrap(),
            Some((Text(&repeat('x').take(125).collect::<String>()), 127)));
    }
    #[test]
    fn parse_4k() {
        let data = b"\x81\x7E\x10\x00";
        for i in 0..4095 {
            let mut buf = Buf::new();
            buf.extend(data);
            for _ in 0..i {
                buf.extend(&[b'x']);
            }
            assert_eq!(parse_frame(&mut buf, 4096, false).unwrap(), None);
        }
        let mut buf = Buf::new();
        buf.extend(data);
        for _ in 0..4096 {
            buf.extend(&[b'x']);
        }
        assert_eq!(parse_frame(&mut buf, 4096, false).unwrap(),
            Some((Text(&repeat('x').take(4096).collect::<String>()), 4100)));
    }
}
