use std::mem;
use std::sync::Arc;
use std::collections::VecDeque;

use futures::{Future, Poll, Async};
use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::io::Io;

use super::encoder::{self, get_inner, ResponseConfig};
use super::{Dispatcher, Codec, Error, Config, RecvMode};
use super::headers::parse_headers;
use super::codec::BodyKind;
use chunked;
use body_parser::BodyProgress;


enum OutState<S: Io, F, C> {
    Idle(WriteBuf<S>),
    Write(F),
    Switch(F, C),
    Void,
}

struct BodyState<C> {
    mode: RecvMode,
    progress: BodyProgress,
    response_config: ResponseConfig,
    codec: C,
}

enum InState<C> {
    Headers,
    Body(BodyState<C>),
    Hijack,
    Closed,
}

/// A low-level HTTP/1.x server protocol handler
pub struct Proto<S: Io, D: Dispatcher<S>> {
    dispatcher: D,
    inbuf: Option<ReadBuf<S>>, // it's optional only for hijacking
    reading: InState<D::Codec>,
    waiting: VecDeque<(ResponseConfig, D::Codec)>,
    writing: OutState<S, <D::Codec as Codec<S>>::ResponseFuture, D::Codec>,
    config: Arc<Config>,
}

fn new_body(mode: BodyKind, recv_mode: RecvMode)
    -> Result<BodyProgress, Error>
{
    use super::codec::BodyKind as B;
    use super::RecvMode as M;
    use body_parser::BodyProgress as P;
    match (mode, recv_mode) {
        // TODO(tailhook) check size < usize
        (B::Unsupported, _) => Err(Error::UnsupportedBody),
        (B::Fixed(x), M::BufferedUpfront(b)) if x > b as u64 => {
            Err(Error::RequestTooLong)
        }
        (B::Fixed(x), _)  => Ok(P::Fixed(x as usize)),
        (B::Chunked, _) => Ok(P::Chunked(chunked::State::new())),
    }
}

impl<S: Io, D: Dispatcher<S>> Proto<S, D> {
    /// Create a new protocol implementation from a TCP connection and a config
    ///
    /// You should use this protocol as a `Sink`
    pub fn new(conn: S, cfg: &Arc<Config>, dispatcher: D) -> Proto<S, D> {
        let (cout, cin) = IoBuf::new(conn).split();
        return Proto {
            dispatcher: dispatcher,
            inbuf: Some(cin),
            reading: InState::Headers,
            waiting: VecDeque::with_capacity(cfg.inflight_request_prealloc),
            writing: OutState::Idle(cout),
            config: cfg.clone(),
        }
    }
}

impl<S: Io, D: Dispatcher<S>> Proto<S, D> {
    /// Resturns Ok(true) if new data has been read
    fn do_reads(&mut self) -> Result<bool, Error> {
        use self::InState::*;
        let mut changed = false;
        loop {
            let limit = match self.reading {
                Headers => self.config.inflight_request_limit,
                Body(..) => self.config.inflight_request_limit-1,
                Closed | Hijack => return Ok(changed),
            };
            if self.waiting.len() >= limit {
                break;
            }
            // TODO(tailhook) Do reads after parse_headers() [optimization]
            let ref mut inbuf = self.inbuf.as_mut().expect("buffer exists");
            inbuf.read()?;
            let (next, cont) = match mem::replace(&mut self.reading, Closed) {
                Headers => {
                    match parse_headers(&mut inbuf.in_buf,
                                        &mut self.dispatcher)?
                    {
                        Some((body, mut codec, cfg)) => {
                            changed = true;
                            let mode = codec.recv_mode();
                            if mode == RecvMode::Hijack {
                                self.waiting.push_back((cfg, codec));
                                (Hijack, true)
                            } else {
                                (Body(BodyState {
                                    mode: mode,
                                    response_config: cfg,
                                    progress: new_body(body, mode)?,
                                    codec: codec }),
                                 true)
                            }
                        }
                        None => (Headers, false),
                    }
                }
                Body(mut body) => {
                    body.progress.parse(inbuf)?;
                    let (bytes, done) = body.progress.check_buf(inbuf);
                    let operation = if done {
                        Some(body.codec.data_received(
                            &inbuf.in_buf[..bytes], true)?)
                    } else if inbuf.done() {
                        return Err(Error::ConnectionReset);
                    } else if matches!(body.mode, RecvMode::Progressive(x) if x <= bytes) {
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
                                (Headers, true)
                            } else {
                                (Body(body), true)
                            }
                        }
                        Some(Async::NotReady) => {
                            if matches!(body.mode, RecvMode::Progressive(x) if x > bytes) {
                                (Body(body), false)
                            } else {
                                (Body(body), true)
                            }
                        }
                        None => (Body(body), true),
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
    fn do_writes(&mut self) -> Result<bool, Error> {
        use self::OutState::*;
        use self::InState::*;
        use server::RecvMode::{BufferedUpfront, Progressive};
        loop {
            let (next, cont) = match mem::replace(&mut self.writing, Void) {
                Idle(mut io) => {
                    io.flush()?;
                    if let Some((rc, mut codec)) = self.waiting.pop_front() {
                        let e = encoder::new(io, rc);
                        if matches!(self.reading, Hijack) {
                            (Switch(codec.start_response(e), codec), true)
                        } else {
                            (Write(codec.start_response(e)), true)
                        }
                    } else {
                        match self.reading {
                            Body(BodyState { mode: BufferedUpfront(..), ..})
                            | Closed | Headers
                            => {
                                (Idle(io), false)
                            }
                            Body(BodyState { mode: RecvMode::Hijack, ..}) => {
                                unreachable!();
                            }
                            Body(BodyState {
                                mode: Progressive(_),
                                codec: ref mut _codec, ..})
                            => {
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
                            return Ok(true);
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
                return Ok(false);
            }
        }
    }
}

impl<S: Io, D: Dispatcher<S>> Future for Proto<S, D> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<(), Error> {
        if self.do_writes()? {
            return Ok(Async::Ready(()));
        }
        while self.do_reads()? {
            if self.do_writes()? {
                return Ok(Async::Ready(()));
            }
        }
        // TODO(tailhook) close connection on `Connection: close`
        Ok(Async::NotReady)
    }
}
