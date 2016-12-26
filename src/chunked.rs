use httparse::{InvalidChunkSize, parse_chunk_size};
use tk_bufstream::Buf;


// TODO(tailhook) review usizes here, probaby we may accept u64
#[derive(Debug, Clone)]
pub struct State {
    buffered: usize,
    pending: usize,
    done: bool,
}

impl State {
    pub fn new() -> State {
        State {
            buffered: 0,
            pending: 0,
            done: false,
        }
    }
    pub fn parse(&mut self, buf: &mut Buf) -> Result<(), InvalidChunkSize> {
        let State { ref mut buffered, ref mut pending, ref mut done } = *self;
        while *buffered < buf.len() {
            if *pending == 0 {
                use httparse::Status::*;
                match parse_chunk_size(&buf[*buffered..])? {
                    Complete((bytes, 0)) => {
                        buf.remove_range(
                            *buffered..*buffered+bytes);
                        *done = true;
                    }
                    Complete((bytes, chunk_size)) => {
                        // TODO(tailhook) optimized multiple removes
                        buf.remove_range(
                            *buffered..*buffered+bytes);
                        // TODO(tailhook) check that chunk_size < u32
                        *pending = chunk_size as usize;
                    }
                    Partial => {
                        return Ok(());
                    }
                }
            } else {
                if *buffered + *pending <= buf.len() {
                    *buffered += *pending;
                    *pending = 0;
                } else {
                    *pending -= buf.len() - *buffered;
                    *buffered = buf.len();
                }
            }
        }
        Ok(())
    }
    pub fn buffered(&self) -> usize {
        self.buffered
    }
    pub fn is_done(&self) -> bool {
        self.done
    }
    pub fn consume(&mut self, n: usize) {
        assert!(self.buffered >= n);
        self.buffered -= n;
    }
}
