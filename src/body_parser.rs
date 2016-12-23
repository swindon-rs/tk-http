use httparse::InvalidChunkSize;
use tokio_core::io::Io;
use tk_bufstream::{ReadBuf, Buf};


use chunked;

// TODO(tailhook) review usizes here, probaby we may accept u64
#[derive(Debug, Clone)]
pub enum BodyProgress {
    Fixed(usize), // bytes left
    Eof, // only for client implemementation
    Chunked(chunked::State),
}

impl BodyProgress {
    /// Returns useful number of bytes in buffer and "end" ("done") flag
    pub fn check_buf<S: Io>(&self, io: &ReadBuf<S>) -> (usize, bool) {
        use self::BodyProgress::*;
        match *self {
            Fixed(x) if x <= io.in_buf.len() => (x, true),
            Fixed(_) => (io.in_buf.len(), false),
            Chunked(ref s) => (s.buffered(), s.is_done()),
            Eof => (io.in_buf.len(), io.done()),
        }
    }
    pub fn parse<S: Io>(&mut self, io: &mut ReadBuf<S>)
        -> Result<(), InvalidChunkSize>
    {
        use self::BodyProgress::*;
        match *self {
            Fixed(_) => {},
            Chunked(ref mut s) => s.parse(&mut io.in_buf)?,
            Eof => {}
        }
        Ok(())
    }
    pub fn consume<S: Io>(&mut self, io: &mut ReadBuf<S>, n: usize) {
        use self::BodyProgress::*;
        io.in_buf.consume(n);
        match *self {
            Fixed(ref mut x) => {
                assert!(*x >= n);
                *x -= n;
            }
            Chunked(ref mut s) => s.consume(n),
            Eof => {}
        }
    }
}
