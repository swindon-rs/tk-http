use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::str::from_utf8;

use futures::{Future, Async, Poll};
use tokio_core::io::Io;
use httparse;
use httparse::parse_chunk_size;
use tk_bufstream::{ReadBuf, Buf};

use enums::Version;
use client::client::{BodyKind, RecvMode, Head};
use headers;
use client::encoder::RequestState;
use client::{Codec, Error};


/// Number of headers to allocate on a stack
const MIN_HEADERS: usize = 16;
/// A hard limit on the number of headers
const MAX_HEADERS: usize = 1024;


// TODO(tailhook) review usizes here, probaby we may accept u64
#[derive(Debug, Clone)]
enum BodyProgress {
    Fixed(usize), // bytes left
    Eof,
    Chunked { buffered: usize, pending_chunk: usize, done: bool },
}

#[derive(Debug, Clone)]
enum State {
    Headers {
        request_state: Arc<AtomicUsize>,
        close_signal: Arc<AtomicBool>,
    },
    Body {
        mode: RecvMode,
        progress: BodyProgress,
    },
}

pub struct Parser<S: Io, C: Codec<S>> {
    io: Option<ReadBuf<S>>,
    codec: C,
    close: bool,
    state: State,
}

impl BodyProgress {
    fn new(mode: BodyKind) -> BodyProgress {
        use client::client::BodyKind as B;
        use self::{BodyProgress as P};
        match mode {
            // TODO(tailhook) check size < usize
            B::Fixed(x) => P::Fixed(x as usize),
            B::Chunked => P::Chunked {
                buffered: 0,
                pending_chunk: 0,
                done: false
            },
            B::Eof => P::Eof,
        }
    }
    /// Returns useful number of bytes in buffer and "end" ("done") flag
    fn check_buf<S: Io>(&self, io: &ReadBuf<S>) -> (usize, bool) {
        use self::BodyProgress::*;
        match *self {
            Fixed(x) if x <= io.in_buf.len() => (x, true),
            Fixed(_) => (io.in_buf.len(), false),
            Chunked { buffered: x, done, .. } => (x, done),
            Eof => (io.in_buf.len(), io.done()),
        }
    }
    fn parse<S: Io>(&mut self, io: &mut ReadBuf<S>) -> Result<(), Error> {
        use self::BodyProgress::*;
        match *self {
            Fixed(_) => {},
            Chunked { ref mut buffered, ref mut pending_chunk, ref mut done }
            => {
                while *buffered < io.in_buf.len() {
                    if *pending_chunk == 0 {
                        use httparse::Status::*;
                        match parse_chunk_size(&io.in_buf[*buffered..])? {
                            Complete((bytes, 0)) => {
                                io.in_buf.remove_range(
                                    *buffered..*buffered+bytes);
                                *done = true;
                            }
                            Complete((bytes, chunk_size)) => {
                                // TODO(tailhook) optimized multiple removes
                                io.in_buf.remove_range(
                                    *buffered..*buffered+bytes);
                                // TODO(tailhook) check that chunk_size < u32
                                *pending_chunk = chunk_size as usize;
                            }
                            Partial => {
                                return Ok(());
                            }
                        }
                    } else {
                        if *buffered + *pending_chunk <= io.in_buf.len() {
                            *buffered += *pending_chunk;
                            *pending_chunk = 0;
                        } else {
                            *pending_chunk -= io.in_buf.len() - *buffered;
                            *buffered = io.in_buf.len();
                        }
                    }
                }
            }
            Eof => {}
        }
        Ok(())
    }
    fn consume<S: Io>(&mut self, io: &mut ReadBuf<S>, n: usize) {
        use self::BodyProgress::*;
        io.in_buf.consume(n);
        match *self {
            Fixed(ref mut x) => {
                assert!(*x >= n);
                *x -= n;
            }
            Chunked { buffered: ref mut x, .. } => {
                assert!(*x >= n);
                *x -= n;
            }
            Eof => {}
        }
    }
}

fn scan_headers(is_head: bool, code: u16, headers: &[httparse::Header])
    -> Result<(BodyKind, bool), Error>
{
    /// Implements the body length algorithm for requests:
    /// http://httpwg.github.io/specs/rfc7230.html#message.body.length
    ///
    /// Algorithm:
    ///
    /// 1. For HEAD, 1xx, 204, 304 -- no body
    /// 2. If last transfer encoding is chunked -> Chunked
    /// 3. If Content-Length -> Fixed
    /// 4. Else Eof
    use client::client::BodyKind::*;
    let mut has_content_length = false;
    let mut close = false;
    if is_head || (code > 100 && code < 200) || code == 204 || code == 304 {
        for header in headers.iter() {
            // TODO(tailhook) check for transfer encoding and content-length
            if headers::is_connection(header.name) {
                if header.value.split(|&x| x == b',').any(headers::is_close) {
                    close = true;
                }
            }
        }
        return Ok((Fixed(0), close))
    }
    let mut result = BodyKind::Eof;
    for header in headers.iter() {
        if headers::is_transfer_encoding(header.name) {
            if let Some(enc) = header.value.split(|&x| x == b',').last() {
                if headers::is_chunked(enc) {
                    if has_content_length {
                        // override but don't allow keep-alive
                        close = true;
                    }
                    result = Chunked;
                }
            }
        } else if headers::is_content_length(header.name) {
            if has_content_length {
                // duplicate content_length
                return Err(Error::DuplicateContentLength);
            }
            has_content_length = true;
            if result != Chunked {
                let s = from_utf8(header.value)
                    .map_err(|_| Error::BadContentLength)?;
                let len = s.parse()
                    .map_err(|_| Error::BadContentLength)?;
                result = Fixed(len);
            } else {
                // tralsfer-encoding has preference and don't allow keep-alive
                close = true;
            }
        } else if headers::is_connection(header.name) {
            if header.value.split(|&x| x == b',').any(headers::is_close) {
                close = true;
            }
        }
    }
    Ok((result, close))
}

fn parse_headers<S: Io, C: Codec<S>>(
    buffer: &mut Buf, codec: &mut C, is_head: bool)
    -> Result<Option<(State, bool)>, Error>
{
    let (mode, body, close, bytes) = {
        let mut vec;
        let mut headers = [httparse::EMPTY_HEADER; MIN_HEADERS];
        let (ver, code, reason, headers, bytes) = {
            let mut raw = httparse::Response::new(&mut headers);
            let mut result = raw.parse(&buffer[..]);
            if matches!(result, Err(httparse::Error::TooManyHeaders)) {
                vec = vec![httparse::EMPTY_HEADER; MAX_HEADERS];
                raw = httparse::Response::new(&mut vec);
                result = raw.parse(&buffer[..]);
            }
            match result? {
                httparse::Status::Complete(bytes) => {
                    let ver = raw.version.unwrap();
                    let code = raw.code.unwrap();
                    (ver, code, raw.reason.unwrap(), raw.headers, bytes)
                }
                _ => return Ok(None),
            }
        };
        let (body, close) = try!(scan_headers(is_head, code, &headers));
        let head = Head {
            version: if ver == 1
                { Version::Http11 } else { Version::Http10 },
            code: code,
            reason: reason,
            headers: headers,
            body_kind: body,
            // For HTTP/1.0 we could implement Connection: Keep-Alive
            // but hopefully it's rare enough to ignore nowadays
            close: close || ver == 0,
        };
        let mode = codec.headers_received(&head)?;
        (mode, body, close, bytes)
    };
    buffer.consume(bytes);
    Ok(Some((
        State::Body {
            mode: mode,
            progress: BodyProgress::new(body),
        },
        close,
    )))
}

impl<S: Io, C: Codec<S>> Parser<S, C> {
    pub fn new(io: ReadBuf<S>, codec: C,
        request_state: Arc<AtomicUsize>, close_signal: Arc<AtomicBool>)
        -> Parser<S, C>
    {
        Parser {
            io: Some(io),
            codec: codec,
            close: false,
            state: State::Headers {
                request_state: request_state,
                close_signal: close_signal,
            },
        }
    }
    fn read_and_parse(&mut self) -> Poll<(), Error> {
        use self::State::*;
        use client::client::RecvMode::*;
        let mut io = self.io.as_mut().expect("buffer is still here");
        self.state = if let Headers {
                ref request_state,
                ref close_signal,
            } = self.state
        {
            if io.read()? == 0 {
                if io.done() {
                    return Err(Error::ResetOnResponseHeaders);
                } else {
                    return Ok(Async::NotReady);
                }
            }
            let reqs = request_state.load(Ordering::SeqCst);
            if reqs == RequestState::Empty as usize {
                return Err(Error::PrematureResponseHeaders);
            }
            let is_head = reqs == RequestState::StartedHead as usize;
            match parse_headers(&mut io.in_buf, &mut self.codec, is_head)? {
                None => {
                    return Ok(Async::NotReady);
                }
                Some((body, close)) => {
                    if close {
                        close_signal.store(true, Ordering::SeqCst);
                        self.close = true;
                    }
                    body
                },
            }
        } else {
            // TODO(tailhook) optimize this
            self.state.clone()
        };
        loop {
            match self.state {
                Headers {..} => unreachable!(),
                Body { ref mut mode, ref mut progress } => {
                    progress.parse(&mut io)?;
                    let (bytes, done) = progress.check_buf(&io);
                    let operation = if done {
                        Some(self.codec.data_received(
                            &io.in_buf[..bytes], true)?)
                    } else if io.done() {
                        /// If it's ReadUntilEof it will be detected in
                        /// check_buf so we can safefully put error here
                        return Err(Error::ResetOnResponseBody);
                    } else if matches!(*mode, Progressive(x) if x <= bytes) {
                        Some(self.codec.data_received(
                            &io.in_buf[..bytes], false)?)
                    } else {
                        None
                    };
                    match operation {
                        Some(Async::Ready(consumed)) => {
                            progress.consume(&mut io, consumed);
                            if done && consumed == bytes {
                                return Ok(Async::Ready(()));
                            }
                        }
                        Some(Async::NotReady) => {
                            if matches!(*mode, Progressive(x) if x > bytes) {
                                return Ok(Async::NotReady);
                            }
                        }
                        None => {} // Read more
                    }
                }
            }
            if io.read()? == 0 {
                if io.done() {
                    continue;
                } else {
                    return Ok(Async::NotReady);
                }
            }
        }
    }
}

impl<S: Io, C: Codec<S>> Future for Parser<S, C> {
    type Item = Option<ReadBuf<S>>;
    type Error = Error;
    /// Returns None if response contains `Connection: close`
    fn poll(&mut self) -> Poll<Option<ReadBuf<S>>, Error> {
        match self.read_and_parse()? {
            Async::Ready(()) => {
                let io = self.io.take().expect("buffer still here");
                if self.close {
                    Ok(Async::Ready(None))
                } else {
                    Ok(Async::Ready(Some(io)))
                }
            }
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}
