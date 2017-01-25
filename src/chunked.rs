use httparse::{InvalidChunkSize, parse_chunk_size};
use tk_bufstream::Buf;


// TODO(tailhook) review usizes here, probaby we may accept u64
#[derive(Debug, Clone, PartialEq)]
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
        if *done {
            return Ok(());
        }
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
                if *buffered + *pending + 2 <= buf.len() {
                    *buffered += *pending;
                    *pending = 0;
                    // TODO(tailhook) optimize this
                    buf.remove_range(*buffered..*buffered+2);
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

#[cfg(test)]
mod test {
    use super::State;
    use tk_bufstream::Buf;

    #[test]
    fn simple() {
        let mut state = State::new();
        let mut buf = Buf::new();
        buf.extend(b"4\r\nhell\r\n");
        assert_eq!(state.parse(&mut buf), Ok(()));
        assert_eq!(state, State { buffered: 4, pending: 0, done: false });
        state.consume(4);
        buf.consume(4);
        assert_eq!(state.buffered, 0);
        buf.extend(b"0\r\n");
        assert_eq!(state.parse(&mut buf), Ok(()));
        assert_eq!(state, State { buffered: 0, pending: 0, done: true });
    }
}
