use std::mem;
use std::sync::Arc;
use std::collections::VecDeque;
use std::time::Instant;

use futures::{Future, Poll, Async};
use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::io::Io;
use tokio_core::reactor::{Handle, Timeout};

use super::encoder::{self, get_inner, ResponseConfig};
use super::{Dispatcher, Codec, Config};
use super::headers::parse_headers;
use super::codec::BodyKind;
use server::error::{ErrorEnum, Error};
use server::recv_mode::{Mode, get_mode};
use chunked;
use body_parser::BodyProgress;


enum OutState<S: Io, F, C> {
    Idle(WriteBuf<S>),
    Write(F),
    Switch(F, C),
    Void,
}

struct BodyState<C> {
    mode: Mode,
    progress: BodyProgress,
    response_config: ResponseConfig,
    codec: C,
}

enum InState<C> {
    Connected,
    KeepAlive,
    Headers,
    Body(BodyState<C>),
    Hijack,
    Closed,
}

pub struct PureProto<S: Io, D: Dispatcher<S>> {
    dispatcher: D,
    inbuf: Option<ReadBuf<S>>, // it's optional only for hijacking
    reading: InState<D::Codec>,
    waiting: VecDeque<(ResponseConfig, D::Codec)>,
    writing: OutState<S, <D::Codec as Codec<S>>::ResponseFuture, D::Codec>,
    config: Arc<Config>,

    last_byte_read: Instant,
    last_byte_written: Instant,
    /// Long-term deadline for reading (headers- or input body_whole- timeout)
    read_deadline: Instant,
    response_deadline: Instant,
}

/// A low-level HTTP/1.x server protocol handler
pub struct Proto<S: Io, D: Dispatcher<S>> {
    proto: PureProto<S, D>,
    handle: Handle,
    timeout: Timeout,
}

fn new_body(mode: BodyKind, recv_mode: Mode)
    -> Result<BodyProgress, ErrorEnum>
{
    use super::codec::BodyKind as B;
    use super::recv_mode::Mode as M;
    use body_parser::BodyProgress as P;
    match (mode, recv_mode) {
        // TODO(tailhook) check size < usize
        (B::Unsupported, _) => Err(ErrorEnum::UnsupportedBody),
        (B::Fixed(x), M::BufferedUpfront(b)) if x > b as u64 => {
            Err(ErrorEnum::RequestTooLong)
        }
        (B::Fixed(x), _)  => Ok(P::Fixed(x as usize)),
        (B::Chunked, _) => Ok(P::Chunked(chunked::State::new())),
    }
}

impl<S: Io, D: Dispatcher<S>> Proto<S, D> {
    /// Create a new protocol implementation from a TCP connection and a config
    ///
    /// You should use this protocol as a `Sink`
    pub fn new(conn: S, cfg: &Arc<Config>, dispatcher: D,
        handle: &Handle)
        -> Proto<S, D>
    {
        return Proto {
            proto: PureProto::new(conn, cfg, dispatcher),
            handle: handle.clone(),
            timeout: Timeout::new(cfg.first_byte_timeout, handle)
                .expect("can always add a timeout"),
        }
    }
}

impl<S: Io, D: Dispatcher<S>> PureProto<S, D> {
    pub fn new(conn: S, cfg: &Arc<Config>, dispatcher: D)
        -> PureProto<S, D>
    {
        let (cout, cin) = IoBuf::new(conn).split();
        PureProto {
            dispatcher: dispatcher,
            inbuf: Some(cin),
            reading: InState::Connected,
            waiting: VecDeque::with_capacity(
                cfg.inflight_request_prealloc),
            writing: OutState::Idle(cout),
            config: cfg.clone(),

            last_byte_read: Instant::now(),
            last_byte_written: Instant::now(),
            read_deadline: Instant::now() + cfg.first_byte_timeout,
            response_deadline: Instant::now(),  // irrelevant at start
        }
    }
    /// Resturns Ok(true) if new data has been read
    fn do_reads(&mut self) -> Result<bool, Error> {
        use self::InState::*;
        let mut changed = false;
        let mut inbuf = self.inbuf.as_mut();
        let mut inbuf = if let Some(ref mut inbuf) = inbuf {
            inbuf
        } else {
            // Buffer has been stolen
            return Ok(false);
        };
        loop {
            let limit = match self.reading {
                Headers| Connected | KeepAlive
                => self.config.inflight_request_limit,
                Body(..) => self.config.inflight_request_limit-1,
                Closed | Hijack => return Ok(changed),
            };
            if self.waiting.len() >= limit {
                break;
            }
            // TODO(tailhook) Do reads after parse_headers() [optimization]
            if inbuf.read().map_err(ErrorEnum::Io)? > 0 {
                self.last_byte_read = Instant::now();
            }
            let (next, cont) = match mem::replace(&mut self.reading, Closed) {
                KeepAlive | Connected if inbuf.in_buf.len() > 0 => {
                    self.read_deadline = Instant::now()
                        + self.config.headers_timeout;
                    (Headers, true)
                }
                Connected => (Connected, false),
                KeepAlive => (KeepAlive, false),
                Headers => {
                    match parse_headers(&mut inbuf.in_buf,
                                        &mut self.dispatcher)?
                    {
                        Some((body, mut codec, cfg)) => {
                            changed = true;
                            let mode = codec.recv_mode();
                            if get_mode(&mode) == Mode::Hijack {
                                self.waiting.push_back((cfg, codec));
                                (Hijack, true)
                            } else {
                                let timeo = mode.timeout.unwrap_or(
                                    self.config.input_body_whole_timeout);
                                self.read_deadline = Instant::now() + timeo;
                                (Body(BodyState {
                                    mode: get_mode(&mode),
                                    response_config: cfg,
                                    progress: new_body(body, get_mode(&mode))?,
                                    codec: codec }),
                                 true)
                            }
                        }
                        None => (Headers, false),
                    }
                }
                Body(mut body) => {
                    body.progress.parse(inbuf)
                        .map_err(ErrorEnum::ChunkParseError)?;
                    let (bytes, done) = body.progress.check_buf(inbuf);
                    let operation = if done {
                        Some(body.codec.data_received(
                            &inbuf.in_buf[..bytes], true)?)
                    } else if inbuf.done() {
                        return Err(ErrorEnum::ConnectionReset.into());
                    } else if matches!(body.mode, Mode::Progressive(x) if x <= bytes) {
                        Some(body.codec.data_received(
                            &inbuf.in_buf[..bytes], false)?)
                    } else {
                        None
                    };
                    match operation {
                        Some(Async::Ready(consumed)) => {
                            body.progress.consume(inbuf, consumed);
                            if done && consumed == bytes {
                                changed = true;
                                self.waiting.push_back(
                                    (body.response_config, body.codec));
                                self.read_deadline = Instant::now()
                                    + self.config.keep_alive_timeout;
                                (KeepAlive, true)
                            } else {
                                (Body(body), true) // TODO(tailhook) check
                            }
                        }
                        Some(Async::NotReady) => {
                            if matches!(body.mode, Mode::Progressive(x) if x > bytes) {
                                (Body(body), false)
                            } else {
                                (Body(body), true) // TODO(tailhook) check
                            }
                        }
                        None => (Body(body), false),
                    }
                }
                Hijack => (Hijack, false),
                Closed => unreachable!(),
            };
            self.reading = next;
            if !cont {
                break;
            }
        }
        Ok(changed)
    }
    fn do_writes(&mut self) -> Result<(), Error> {
        use self::OutState::*;
        use self::InState::*;
        use server::recv_mode::Mode::{BufferedUpfront, Progressive};
        loop {
            let (next, cont) = match mem::replace(&mut self.writing, Void) {
                Idle(mut io) => {
                    let old_len = io.out_buf.len();
                    if old_len > 0 {
                        io.flush().map_err(ErrorEnum::Io)?;
                        if io.out_buf.len() < old_len {
                            self.last_byte_written = Instant::now();
                        }
                    }

                    if let Some((rc, mut codec)) = self.waiting.pop_front() {
                        self.response_deadline = Instant::now()
                            + self.config.output_body_whole_timeout;
                        let e = encoder::new(io, rc);
                        if matches!(self.reading, Hijack) {
                            (Switch(codec.start_response(e), codec), true)
                        } else {
                            (Write(codec.start_response(e)), true)
                        }
                    } else {
                        match self.reading {
                            Body(BodyState { mode: BufferedUpfront(..), ..})
                            | Closed | Headers | Connected | KeepAlive
                            => {
                                (Idle(io), false)
                            }
                            Body(BodyState { mode: Mode::Hijack, ..}) => {
                                unreachable!();
                            }
                            Body(BodyState {
                                mode: Progressive(_),
                                codec: ref mut _codec, ..})
                            => {
                                self.response_deadline = Instant::now()
                                    + self.config.output_body_whole_timeout;
                                // TODO(tailhook) start writing now
                                unimplemented!();
                            }
                            Hijack => unreachable!(),
                        }
                    }
                }
                Write(mut f) => {
                    match f.poll()? {
                        Async::Ready(x) => {
                            self.read_deadline = Instant::now()
                                + self.config.keep_alive_timeout;
                            (Idle(get_inner(x)), true)
                        }
                        Async::NotReady => {
                            (Write(f), false)
                        }
                    }
                }
                Switch(mut f, mut codec) => {
                    match f.poll()? {
                        Async::Ready(x) => {
                            let wr = get_inner(x);
                            let rd = self.inbuf.take()
                                .expect("can hijack only once");
                            codec.hijack(wr, rd);
                            return Ok(());
                        }
                        Async::NotReady => {
                            (Switch(f, codec), false)
                        }
                    }
                }
                Void => unreachable!(),
            };
            self.writing = next;
            if !cont {
                return Ok(());
            }
        }
    }
}

impl<S: Io, D: Dispatcher<S>> PureProto<S, D> {
    /// Does all needed processing and returns Ok(true) if connection is fine
    /// and Ok(false) if it needs to be closed
    fn process(&mut self) -> Result<bool, Error> {
        self.do_writes()?;
        while self.do_reads()? {
            self.do_writes()?;
        }
        if self.inbuf.as_ref().map(|x| x.done()).unwrap_or(true) {
            Ok(false)
        } else {
            Ok(true)
        }
    }
    fn timeout(&mut self) -> Option<Instant> {
        use self::OutState::*;

        match self.writing {
            Idle(..) => {}
            Write(..) => return Some(self.response_deadline),
            Switch(..) => return None,  // TODO(tailhook) is it right?
            Void => return None,  // TODO(tailhook) is it reachable?
        }
        if self.waiting.len() > 0 { // if there are requests processing now
                                    // we don't have a read timeout
            return None;
        }
        return Some(self.read_deadline);
    }
}

impl<S: Io, D: Dispatcher<S>> Future for Proto<S, D> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<(), Error> {
        match self.proto.process() {
            Ok(false) => Ok(Async::Ready(())),
            Ok(true) => {
                // TODO(tailhook) schedule notification with timeout
                match self.proto.timeout() {
                    Some(val) => {
                        let now = Instant::now();
                        if now > val {
                            Err(ErrorEnum::Timeout.into())
                        } else {
                            self.timeout = Timeout::new(val - Instant::now(),
                                &self.handle)
                                .expect("can always add a timeout");
                            let timeo = self.timeout.poll()
                                .expect("timeout can't fail on poll");
                            match timeo {
                                Async::Ready(()) => {
                                    Err(ErrorEnum::Timeout.into())
                                }
                                Async::NotReady => Ok(Async::NotReady),
                            }
                        }
                    }
                    None => {
                        // No timeout. This means we are waiting for request
                        // handler to do it's work. Request handler should have
                        // some timeout handler itself.
                        Ok(Async::NotReady)
                    }
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use futures::{Empty, Async, empty};
    use tk_bufstream::{MockData, ReadBuf, WriteBuf};

    use super::PureProto;
    use server::{Config, Dispatcher, Codec};
    use server::{Head, RecvMode, Error, Encoder, EncoderDone};

    struct MockDisp {
    }

    struct MockCodec {
    }

    impl Dispatcher<MockData> for MockDisp {
        type Codec = MockCodec;

        fn headers_received(&mut self, _headers: &Head)
            -> Result<Self::Codec, Error>
        {
            Ok(MockCodec {})
        }
    }

    impl Codec<MockData> for MockCodec {
        type ResponseFuture = Empty<EncoderDone<MockData>, Error>;
        fn recv_mode(&mut self) -> RecvMode {
            RecvMode::buffered_upfront(1024)
        }
        fn data_received(&mut self, data: &[u8], end: bool)
            -> Result<Async<usize>, Error>
        {
            assert!(end);
            assert_eq!(data.len(), 0);
            Ok(Async::Ready(0))
        }
        fn start_response(&mut self, _e: Encoder<MockData>)
            -> Self::ResponseFuture
        {
            empty()
        }
        fn hijack(&mut self, _write_buf: WriteBuf<MockData>,
                             _read_buf: ReadBuf<MockData>){
            unimplemented!();
        }
    }

    #[test]
    fn simple_get_request() {
        let mock = MockData::new();
        let mut proto = PureProto::new(mock.clone(),
            &Arc::new(Config::new()), MockDisp {});
        proto.process().unwrap();
        mock.add_input("GET / HTTP/1.0\r\n\r\n");
        proto.process().unwrap();
    }

    #[test]
    #[should_panic(expected="Version")]
    fn failing_get_request() {
        let mock = MockData::new();
        let mut proto = PureProto::new(mock.clone(),
            &Arc::new(Config::new()), MockDisp {});
        proto.process().unwrap();
        mock.add_input("GET / TTMP/2.0\r\n\r\n");
        proto.process().unwrap();
    }
}
