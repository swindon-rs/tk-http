use rand::{Rng, thread_rng};
use std::fmt;
use std::str::{from_utf8_unchecked};

use sha1::Sha1;


/// WebSocket GUID constant (provided by spec)
pub const GUID: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The `Sec-WebSocket-Accept` header value
///
/// You can add it using `enc.format_header("Sec-WebSocket-Accept", accept)`.
/// Or use any other thing that supports `Display`.
pub struct Accept([u8; 20]);

/// The `Sec-WebSocket-Key` header value
///
/// You can add it using `enc.format_header("Sec-WebSocket-Key", key)`.
/// Or use any other thing that supports `Display`.
pub struct Key([u8; 16]);

impl Key {
    /// Create a new (random) key, eligible to use for client connection
    pub fn new() -> Key {
        let mut key = [0u8; 16];
        thread_rng().fill_bytes(&mut key);
        return Key(key);
    }
}

impl Accept {
    /// Create an Accept header value from a key received in header
    ///
    /// Note: key here is a key as passed in header value (base64-encoded)
    /// despite that it's accepted as bytes (not as 16 bytes stored in Key)
    ///
    /// Note 2: this does not validate a key (which is not required by spec)
    pub fn from_key_bytes(key: &[u8]) -> Accept {
        let mut sha1 = Sha1::new();
        sha1.update(key);
        sha1.update(GUID.as_bytes());
        Accept(sha1.digest().bytes())
    }
}

impl fmt::Display for Accept {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const CHARS: &'static[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                      abcdefghijklmnopqrstuvwxyz\
                                      0123456789+/";
        let mut buf = [0u8; 28];
        for i in 0..6 {
            let n = ((self.0[i*3+0] as usize) << 16) |
                    ((self.0[i*3+1] as usize) <<  8) |
                     (self.0[i*3+2] as usize) ;
            buf[i*4+0] = CHARS[(n >> 18) & 63];
            buf[i*4+1] = CHARS[(n >> 12) & 63];
            buf[i*4+2] = CHARS[(n >>  6) & 63];
            buf[i*4+3] = CHARS[(n >>  0) & 63];
        }
        let n = ((self.0[18] as usize) << 16) |
                ((self.0[19] as usize) <<  8);
        buf[24] = CHARS[(n >> 18) & 63];
        buf[25] = CHARS[(n >> 12) & 63];
        buf[26] = CHARS[(n >> 6) & 63];
        buf[27] = b'=';
        fmt::Write::write_str(f, unsafe {
            from_utf8_unchecked(&buf)
        })
    }
}

impl fmt::Debug for Accept {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "websocket::Accept({})", self)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const CHARS: &'static[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                      abcdefghijklmnopqrstuvwxyz\
                                      0123456789+/";
        let mut buf = [0u8; 24];
        for i in 0..5 {
            let n = ((self.0[i*3+0] as usize) << 16) |
                    ((self.0[i*3+1] as usize) <<  8) |
                     (self.0[i*3+2] as usize) ;
            buf[i*4+0] = CHARS[(n >> 18) & 63];
            buf[i*4+1] = CHARS[(n >> 12) & 63];
            buf[i*4+2] = CHARS[(n >>  6) & 63];
            buf[i*4+3] = CHARS[(n >>  0) & 63];
        }
        let n = (self.0[15] as usize) << 16;
        buf[20] = CHARS[(n >> 18) & 63];
        buf[21] = CHARS[(n >> 12) & 63];
        buf[22] = b'=';
        buf[23] = b'=';
        fmt::Write::write_str(f, unsafe {
            from_utf8_unchecked(&buf)
        })
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "websocket::Key({})", self)
    }
}
