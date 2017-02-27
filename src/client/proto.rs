use std::collections::VecDeque;
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::time::Instant;

use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::io::Io;
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use futures::{Future, AsyncSink, Async, Sink, StartSend, Poll};

use client::parser::Parser;
use client::encoder::{self, get_inner};
use client::{Codec, Error, Config};


enum OutState<S: Io, F> {
    Idle(WriteBuf<S>, Instant),
    Write(F),
    Void,
}

enum InState<S: Io, C: Codec<S>> {
    Idle(ReadBuf<S>),
    Read(Parser<S, C>, Instant),
    Void,
}

struct Waiting<C> {
    codec: C,
    state: Arc<AtomicUsize>,  // TODO(tailhook) AtomicU8
    queued_at: Instant,
}

/// A low-level HTTP/1.x client protocol handler
///
/// Note, most of the time you need some reconnection facility and/or
/// connection pooling on top of this interface
pub struct Proto<S: Io, C: Codec<S>> {
    writing: OutState<S, C::Future>,
    waiting: VecDeque<Waiting<C>>,
    reading: InState<S, C>,
    close: Arc<AtomicBool>,
    config: Arc<Config>,
}


impl<S: Io, C: Codec<S>> Proto<S, C> {
    /// Create a new protocol implementation from a TCP connection and a config
    ///
    /// You should use this protocol as a `Sink`
    pub fn new(conn: S, cfg: &Arc<Config>) -> Proto<S, C> {
        let (cout, cin) = IoBuf::new(conn).split();
        return Proto {
            writing: OutState::Idle(cout, Instant::now()),
            waiting: VecDeque::with_capacity(cfg.inflight_request_prealloc),
            reading: InState::Idle(cin),
            close: Arc::new(AtomicBool::new(false)),
            config: cfg.clone(),
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
        Box::new(
            TcpStream::connect(&addr, &handle)
            .map(move |c| Proto::new(c, &cfg))
            .map_err(Error::Io))
        as Box<Future<Item=_, Error=_>>
    }
}

impl<S: Io, C: Codec<S>> Sink for Proto<S, C> {
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
                    return Ok(AsyncSink::NotReady(item));
                }
                if self.close.load(Ordering::SeqCst) {
                    // TODO(tailhook) maybe shutdown?
                    io.flush()?;
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
                        (AsyncSink::Ready, OutState::Write(fut))
                    }
                }
            }
            OutState::Write(fut) => {
                // TODO(tailhook) should we check "close"?
                // Points:
                // * Performance
                // * Dropping future
                (AsyncSink::NotReady(item), OutState::Write(fut))
            }
            OutState::Void => unreachable!(),
        };
        self.writing = st;
        return Ok(r);
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.writing = match mem::replace(&mut self.writing, OutState::Void) {
            OutState::Idle(mut io, time) => {
                io.flush()?;
                if time.elapsed() > self.config.keep_alive_timeout &&
                    self.waiting.len() == 0 &&
                    matches!(self.reading, InState::Idle(..))
                {
                    return Err(Error::KeepAliveTimeout);
                }
                OutState::Idle(io, time)
            }
            // Note we break connection if serializer errored, because
            // we don't actually know if connection can be reused
            // safefully in this case
            OutState::Write(mut fut) => match fut.poll()? {
                Async::Ready(done) => {
                    let mut io = get_inner(done);
                    io.flush()?;
                    OutState::Idle(io, Instant::now())
                }
                Async::NotReady => OutState::Write(fut),
            },
            OutState::Void => unreachable!(),
        };
        loop {
            let (state, cont) =
                match mem::replace(&mut self.reading, InState::Void) {
                    InState::Idle(mut io) => {
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
                            if io.read()? != 0 {
                                return Err(Error::PrematureResponseHeaders);
                            }
                            if io.done() {
                                return Err(Error::Closed);
                            }
                            (InState::Idle(io), false)
                        }
                    }
                    InState::Read(mut parser, time) => {
                        match parser.poll()? {
                            Async::NotReady => {
                                (InState::Read(parser, time), false)
                            }
                            Async::Ready(Some(io)) => (InState::Idle(io), true),
                            Async::Ready(None) => {
                                return Err(Error::Closed);
                            }
                        }
                    }
                    InState::Void => unreachable!(),
                };
            self.reading = state;
            if !cont {
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
