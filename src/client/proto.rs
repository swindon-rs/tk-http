use std::collections::VecDeque;
use std::cmp::max;
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::time::Instant;

use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::net::TcpStream;
use tokio_core::reactor::{Handle, Timeout};
use tokio_io::{AsyncRead, AsyncWrite};
use futures::{Future, AsyncSink, Async, Sink, StartSend, Poll};

use client::parser::Parser;
use client::encoder::{self, get_inner};
use client::errors::ErrorEnum;
use client::{Codec, Error, Config};


enum OutState<S, F> {
    Idle(WriteBuf<S>, Instant),
    Write(F, Instant),
    Void,
}

enum InState<S, C: Codec<S>> {
    Idle(ReadBuf<S>, Instant),
    Read(Parser<S, C>, Instant),
    Void,
}

struct Waiting<C> {
    codec: C,
    state: Arc<AtomicUsize>,  // TODO(tailhook) AtomicU8
    queued_at: Instant,
}

pub struct PureProto<S, C: Codec<S>> {
    writing: OutState<S, C::Future>,
    waiting: VecDeque<Waiting<C>>,
    reading: InState<S, C>,
    close: Arc<AtomicBool>,
    config: Arc<Config>,
}

/// A low-level HTTP/1.x client protocol handler
///
/// Note, most of the time you need some reconnection facility and/or
/// connection pooling on top of this interface
pub struct Proto<S, C: Codec<S>> {
    proto: PureProto<S, C>,
    handle: Handle,
    timeout: Timeout,
}


impl<S, C: Codec<S>> Proto<S, C> {
    /// Create a new protocol implementation from a TCP connection and a config
    ///
    /// You should use this protocol as a `Sink`
    pub fn new(conn: S, handle: &Handle, cfg: &Arc<Config>) -> Proto<S, C>
        where S: AsyncRead + AsyncWrite
    {
        let (cout, cin) = IoBuf::new(conn).split();
        Proto {
            proto: PureProto {
                writing: OutState::Idle(cout, Instant::now()),
                waiting: VecDeque::with_capacity(
                    cfg.inflight_request_prealloc),
                reading: InState::Idle(cin, Instant::now()),
                close: Arc::new(AtomicBool::new(false)),
                config: cfg.clone(),
            },
            handle: handle.clone(),
            timeout: Timeout::new(cfg.keep_alive_timeout, &handle)
                .expect("can always create a timeout"),
        }
    }
}

impl<C: Codec<TcpStream>> Proto<TcpStream, C> {
    /// A convenience method to establish connection and create a protocol
    /// instance
    pub fn connect_tcp(addr: SocketAddr, cfg: &Arc<Config>, handle: &Handle)
        -> Box<Future<Item=Self, Error=Error>>
    {
        let cfg = cfg.clone();
        let handle = handle.clone();
        Box::new(
            TcpStream::connect(&addr, &handle)
            .map(move |c| Proto::new(c, &handle, &cfg))
            .map_err(ErrorEnum::Io).map_err(Error::from))
        as Box<Future<Item=_, Error=_>>
    }
}

impl<S: AsyncRead + AsyncWrite, C: Codec<S>> PureProto<S, C> {
    fn poll_writing(&mut self) -> Result<bool, Error> {
        let mut progress = false;
        self.writing = match mem::replace(&mut self.writing, OutState::Void) {
            OutState::Idle(mut io, time) => {
                io.flush().map_err(ErrorEnum::Io)?;
                if time.elapsed() > self.config.keep_alive_timeout &&
                    self.waiting.len() == 0 &&
                    matches!(self.reading, InState::Idle(..))
                {
                    return Err(ErrorEnum::KeepAliveTimeout.into());
                }
                OutState::Idle(io, time)
            }
            // Note we break connection if serializer errored, because
            // we don't actually know if connection can be reused
            // safefully in this case
            OutState::Write(mut fut, start) => match fut.poll()? {
                Async::Ready(done) => {
                    let mut io = get_inner(done);
                    io.flush().map_err(ErrorEnum::Io)?;
                    progress = true;
                    OutState::Idle(io, Instant::now())
                }
                Async::NotReady => OutState::Write(fut, start),
            },
            OutState::Void => unreachable!(),
        };
        return Ok(progress);
    }
    fn poll_reading(&mut self) -> Result<bool, Error> {
        let (state, progress) =
            match mem::replace(&mut self.reading, InState::Void) {
                InState::Idle(mut io, time) => {
                    if let Some(w) = self.waiting.pop_front() {
                        let Waiting { codec: nr, state, queued_at } = w;
                        let parser = Parser::new(io, nr,
                            state, self.close.clone());
                        (InState::Read(parser, queued_at), true)
                    } else {
                        // This serves for two purposes:
                        // 1. Detect connection has been closed (i.e.
                        //    we need to call `poll_read()` every time)
                        // 2. Detect premature bytes (we didn't sent
                        //    a request yet, but there is a response)
                        if io.read().map_err(ErrorEnum::Io)? != 0 {
                            return Err(
                                ErrorEnum::PrematureResponseHeaders.into());
                        }
                        if io.done() {
                            return Err(ErrorEnum::Closed.into());
                        }
                        (InState::Idle(io, time), false)
                    }
                }
                InState::Read(mut parser, time) => {
                    match parser.poll()? {
                        Async::NotReady => {
                            (InState::Read(parser, time), false)
                        }
                        Async::Ready(Some(io)) => {
                            (InState::Idle(io, Instant::now()), true)
                        }
                        Async::Ready(None) => {
                            return Err(ErrorEnum::Closed.into());
                        }
                    }
                }
                InState::Void => unreachable!(),
            };
        self.reading = state;
        Ok(progress)
    }
}

impl<S: AsyncRead + AsyncWrite, C: Codec<S>> Sink for Proto<S, C> {
    type SinkItem = C;
    type SinkError = Error;
    fn start_send(&mut self, mut item: Self::SinkItem)
        -> StartSend<Self::SinkItem, Self::SinkError>
    {
        let old_timeout = self.proto.get_timeout();
        let res = loop {
            item = match self.proto.start_send(item)? {
                AsyncSink::Ready => break AsyncSink::Ready,
                AsyncSink::NotReady(item) => item,
            };
            let wr = self.proto.poll_writing()?;
            let rd = self.proto.poll_reading()?;
            if !wr && !rd {
                break AsyncSink::NotReady(item);
            }
        };
        let new_timeout = self.proto.get_timeout();
        let now = Instant::now();
        if new_timeout < now {
            return Err(ErrorEnum::RequestTimeout.into());
        }
        if old_timeout != new_timeout {
            self.timeout = Timeout::new(new_timeout - now, &self.handle)
                .expect("can always add a timeout");
            let timeo = self.timeout.poll()
                .expect("timeout can't fail on poll");
            match timeo {
                // it shouldn't be keep-alive timeout, but have to check
                Async::Ready(()) => {
                    match res {
                        // don't discard request
                        AsyncSink::NotReady(..) => {}
                        // can return error (can it happen?)
                        // TODO(tailhook) it's strange that this can happen
                        AsyncSink::Ready => {
                            return Err(ErrorEnum::RequestTimeout.into());
                        }
                    }
                }
                Async::NotReady => {}
            }
        }
        Ok(res)
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let old_timeout = self.proto.get_timeout();
        let res = self.proto.poll_complete()?;
        let new_timeout = self.proto.get_timeout();
        let now = Instant::now();
        if new_timeout < now {
            return Err(ErrorEnum::RequestTimeout.into());
        }
        if old_timeout != new_timeout {
            self.timeout = Timeout::new(new_timeout - now, &self.handle)
                .expect("can always add a timeout");
            let timeo = self.timeout.poll()
                .expect("timeout can't fail on poll");
            match timeo {
                // it shouldn't be keep-alive timeout, but have to check
                Async::Ready(()) => {
                    return Err(ErrorEnum::RequestTimeout.into());
                }
                Async::NotReady => {},
            }
        }
        Ok(res)
    }
}

impl<S, C: Codec<S>> PureProto<S, C> {
    fn get_timeout(&self) -> Instant {
        match self.writing {
            OutState::Idle(_, time) => {
                if self.waiting.len() == 0 {
                    match self.reading {
                        InState::Idle(.., rtime) => {
                            return max(time, rtime) +
                                self.config.keep_alive_timeout;
                        }
                        InState::Read(_, time) => {
                            return time + self.config.max_request_timeout;
                        }
                        InState::Void => unreachable!(),
                    }
                } else {
                    let req = self.waiting.get(0).unwrap();
                    return req.queued_at + self.config.max_request_timeout;
                }
            }
            OutState::Write(_, time) => {
                return time + self.config.max_request_timeout;
            }
            OutState::Void => unreachable!(),
        }
    }
}

impl<S: AsyncRead + AsyncWrite, C: Codec<S>> Sink for PureProto<S, C> {
    type SinkItem = C;
    type SinkError = Error;
    fn start_send(&mut self, mut item: Self::SinkItem)
        -> StartSend<Self::SinkItem, Self::SinkError>
    {
        if self.waiting.len() > 0 {
            if self.waiting.len() > self.config.inflight_request_limit {
                // Return right away if limit reached
                // (but limit is checked later for inflight request again)
                return Ok(AsyncSink::NotReady(item));
            }
            let last = self.waiting.get(0).unwrap();
            if last.queued_at.elapsed() > self.config.safe_pipeline_timeout {
                // Return right away if request is being waited for too long
                // (but limit is checked later for inflight request again)
                return Ok(AsyncSink::NotReady(item));
            }
        }
        if matches!(self.reading, InState::Read(_, time)
            if time.elapsed() > self.config.safe_pipeline_timeout)
        {
            // Return right away if request is being waited for too long
            return Ok(AsyncSink::NotReady(item));
        }
        let (r, st) = match mem::replace(&mut self.writing, OutState::Void) {
            OutState::Idle(mut io, time) => {
                if time.elapsed() > self.config.keep_alive_timeout &&
                    self.waiting.len() == 0 &&
                    matches!(self.reading, InState::Idle(..))
                {
                    // Too dangerous to send request now
                    (AsyncSink::NotReady(item), OutState::Idle(io, time))
                } else if self.close.load(Ordering::SeqCst) {
                    // TODO(tailhook) maybe shutdown?
                    io.flush().map_err(ErrorEnum::Io)?;
                    (AsyncSink::NotReady(item), OutState::Idle(io, time))
                } else {
                    let mut limit = self.config.inflight_request_limit;
                    if matches!(self.reading, InState::Read(..)) {
                        limit -= 1;
                    }
                    if self.waiting.len() >= limit {
                        // Note: we recheck limit here, because inflight
                        // request ifluences the limit
                        (AsyncSink::NotReady(item), OutState::Idle(io, time))
                    } else {
                        let state = Arc::new(AtomicUsize::new(0));
                        let e = encoder::new(io,
                                state.clone(), self.close.clone());
                        let fut = item.start_write(e);
                        self.waiting.push_back(Waiting {
                            codec: item,
                            state: state,
                            queued_at: Instant::now(),
                        });
                        (AsyncSink::Ready,
                         OutState::Write(fut, Instant::now()))
                    }
                }
            }
            OutState::Write(fut, start) => {
                // TODO(tailhook) should we check "close"?
                // Points:
                // * Performance
                // * Dropping future
                (AsyncSink::NotReady(item), OutState::Write(fut, start))
            }
            OutState::Void => unreachable!(),
        };
        self.writing = st;
        return Ok(r);
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        loop {
            let wr = self.poll_writing()?;
            let rd = self.poll_reading()?;
            if !wr && !rd {
                break;
            }
        }
        // Basically we return Ready when there are no in-flight requests,
        // which means we can shutdown connection safefully.
        if self.waiting.len() == 0 &&
                matches!(self.writing, OutState::Idle(..)) &&
                matches!(self.reading, InState::Idle(..))
        {
            return Ok(Async::Ready(()));
        } else {
            return Ok(Async::NotReady);
        }
    }
}
