use std::sync::Arc;
use std::borrow::Cow;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::str::from_utf8;
#[allow(unused_imports)]
use std::ascii::AsciiExt;

use futures::{Future, Async, Poll};
use httparse;
use tk_bufstream::{ReadBuf, Buf};
use tokio_io::AsyncRead;

use enums::Version;
use client::client::{BodyKind};
use client::errors::ErrorEnum;
use client::recv_mode::Mode;
use headers;
use chunked;
use body_parser::BodyProgress;
use client::encoder::RequestState;
use client::{Codec, Error, Head};


/// Number of headers to allocate on a stack
const MIN_HEADERS: usize = 16;
/// A hard limit on the number of headers
const MAX_HEADERS: usize = 1024;


#[derive(Debug, Clone)]
enum State {
    Headers {
        request_state: Arc<AtomicUsize>,
        close_signal: Arc<AtomicBool>,
    },
    Body {
        mode: Mode,
        progress: BodyProgress,
    },
}

pub struct Parser<S, C: Codec<S>> {
    io: Option<ReadBuf<S>>,
    codec: C,
    close: bool,
    state: State,
}


fn scan_headers<'x>(is_head: bool, code: u16, headers: &'x [httparse::Header])
    -> Result<(BodyKind, Option<Cow<'x, str>>, bool), ErrorEnum>
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
    use client::errors::ErrorEnum::ConnectionInvalid;
    let mut has_content_length = false;
    let mut connection = None::<Cow<_>>;
    let mut close = false;
    if is_head || (code > 100 && code < 200) || code == 204 || code == 304 {
        for header in headers.iter() {
            // TODO(tailhook) check for transfer encoding and content-length
            if header.name.eq_ignore_ascii_case("Connection") {
                let strconn = from_utf8(header.value)
                    .map_err(|_| ConnectionInvalid)?.trim();
                connection = match connection {
                    Some(x) => Some(x + ", " + strconn),
                    None => Some(strconn.into()),
                };
                if header.value.split(|&x| x == b',').any(headers::is_close) {
                    close = true;
                }
            }
        }
        return Ok((Fixed(0), connection, close))
    }
    let mut result = BodyKind::Eof;
    for header in headers.iter() {
        if header.name.eq_ignore_ascii_case("Transfer-Encoding") {
            if let Some(enc) = header.value.split(|&x| x == b',').last() {
                if headers::is_chunked(enc) {
                    if has_content_length {
                        // override but don't allow keep-alive
                        close = true;
                    }
                    result = Chunked;
                }
            }
        } else if header.name.eq_ignore_ascii_case("Content-Length") {
            if has_content_length {
                // duplicate content_length
                return Err(ErrorEnum::DuplicateContentLength);
            }
            has_content_length = true;
            if result != Chunked {
                let s = from_utf8(header.value)
                    .map_err(|_| ErrorEnum::BadContentLength)?;
                let len = s.parse()
                    .map_err(|_| ErrorEnum::BadContentLength)?;
                result = Fixed(len);
            } else {
                // tralsfer-encoding has preference and don't allow keep-alive
                close = true;
            }
        } else if header.name.eq_ignore_ascii_case("Connection") {
            let strconn = from_utf8(header.value)
                .map_err(|_| ConnectionInvalid)?.trim();
            connection = match connection {
                Some(x) => Some(x + ", " + strconn),
                None => Some(strconn.into()),
            };
            if header.value.split(|&x| x == b',').any(headers::is_close) {
                close = true;
            }
        }
    }
    Ok((result, connection, close))
}

fn new_body(mode: BodyKind, recv_mode: Mode)
    -> Result<BodyProgress, ErrorEnum>
{
    use super::client::BodyKind as B;
    use super::recv_mode::Mode as M;
    use client::errors::ErrorEnum::*;
    use body_parser::BodyProgress as P;
    match (mode, recv_mode) {
        // TODO(tailhook) check size < usize
        (B::Fixed(x), M::Buffered(b)) if x > b as u64 => {
            Err(ResponseBodyTooLong)
        }
        (B::Fixed(x), _)  => Ok(P::Fixed(x as usize)),
        (B::Chunked, _) => Ok(P::Chunked(chunked::State::new())),
        (B::Eof, _) => Ok(P::Eof),
    }
}

fn parse_headers<S, C: Codec<S>>(
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
            match result.map_err(ErrorEnum::Header)? {
                httparse::Status::Complete(bytes) => {
                    let ver = raw.version.unwrap();
                    let code = raw.code.unwrap();
                    (ver, code, raw.reason.unwrap(), raw.headers, bytes)
                }
                _ => return Ok(None),
            }
        };
        let (body, conn, close) = try!(scan_headers(is_head, code, &headers));
        let head = Head {
            version: if ver == 1
                { Version::Http11 } else { Version::Http10 },
            code: code,
            reason: reason,
            headers: headers,
            body_kind: body,
            connection_header: conn,
            // For HTTP/1.0 we could implement Connection: Keep-Alive
            // but hopefully it's rare enough to ignore nowadays
            connection_close: close || ver == 0,
        };
        let mode = codec.headers_received(&head)?;
        (mode, body, close, bytes)
    };
    buffer.consume(bytes);
    Ok(Some((
        State::Body {
            mode: mode.mode,
            progress: new_body(body, mode.mode)?,
        },
        close,
    )))
}

impl<S, C: Codec<S>> Parser<S, C> {
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
    fn read_and_parse(&mut self) -> Poll<(), Error>
        where S: AsyncRead
    {
        use self::State::*;
        use client::recv_mode::Mode::*;
        let mut io = self.io.as_mut().expect("buffer is still here");
        self.state = if let Headers {
                ref request_state,
                ref close_signal,
            } = self.state
        {
            let state;
            loop {
                if io.read().map_err(ErrorEnum::Io)? == 0 {
                    if io.done() {
                        return Err(ErrorEnum::ResetOnResponseHeaders.into());
                    } else {
                        return Ok(Async::NotReady);
                    }
                }
                let reqs = request_state.load(Ordering::SeqCst);
                if reqs == RequestState::Empty as usize {
                    return Err(ErrorEnum::PrematureResponseHeaders.into());
                }
                let is_head = reqs == RequestState::StartedHead as usize;
                match parse_headers(&mut io.in_buf, &mut self.codec, is_head)? {
                    None => continue,
                    Some((body, close)) => {
                        if close {
                            close_signal.store(true, Ordering::SeqCst);
                            self.close = true;
                        }
                        state = body;
                        break
                    },
                }
            };
            state
        } else {
            // TODO(tailhook) optimize this
            self.state.clone()
        };
        loop {
            match self.state {
                Headers {..} => unreachable!(),
                Body { ref mode, ref mut progress } => {
                    progress.parse(&mut io).map_err(ErrorEnum::ChunkSize)?;
                    let (bytes, done) = progress.check_buf(&io);
                    let operation = if done {
                        Some(self.codec.data_received(
                            &io.in_buf[..bytes], true)?)
                    } else if io.done() {
                        // If it's ReadUntilEof it will be detected in
                        // check_buf so we can safefully put error here
                        return Err(ErrorEnum::ResetOnResponseBody.into());
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
            if io.read().map_err(ErrorEnum::Io)? == 0 {
                if io.done() {
                    continue;
                } else {
                    return Ok(Async::NotReady);
                }
            }
        }
    }
}

impl<S: AsyncRead, C: Codec<S>> Future for Parser<S, C> {
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
