use std::mem;
use std::sync::Arc;
use std::collections::VecDeque;

use futures::{Future, Poll, Async};
use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::io::Io;

use super::encoder::{self, get_inner, ResponseConfig};
use super::{Dispatcher, Codec, Error, EncoderDone, Config, RecvMode};
use super::headers::parse_headers;
use super::codec::BodyKind;


enum OutState<S: Io, F> {
    Idle(WriteBuf<S>),
    Write(F),
    Void,
}

// TODO(tailhook) review usizes here, probaby we may accept u64
#[derive(Debug, Clone)]
enum BodyProgress {
    Fixed(usize), // bytes left
    Chunked { buffered: usize, pending_chunk: usize, done: bool },
}

enum InState<C> {
    Headers,
    Body {
        mode: RecvMode,
        progress: BodyProgress,
        response_config: ResponseConfig,
        codec: C,
    },
    Hijack,
    Closed,
}

/// A low-level HTTP/1.x server protocol handler
pub struct Proto<S: Io, D: Dispatcher<S>> {
    dispatcher: D,
    inbuf: ReadBuf<S>,
    reading: InState<D::Codec>,
    waiting: VecDeque<(ResponseConfig, D::Codec)>,
    writing: OutState<S, <D::Codec as Codec<S>>::ResponseFuture>,
    config: Arc<Config>,
}

impl<S: Io, D: Dispatcher<S>> Proto<S, D> {
    /// Create a new protocol implementation from a TCP connection and a config
    ///
    /// You should use this protocol as a `Sink`
    pub fn new(conn: S, cfg: &Arc<Config>, dispatcher: D) -> Proto<S, D> {
        let (cout, cin) = IoBuf::new(conn).split();
        return Proto {
            dispatcher: dispatcher,
            inbuf: cin,
            reading: InState::Headers,
            waiting: VecDeque::with_capacity(cfg.inflight_request_prealloc),
            writing: OutState::Idle(cout),
            config: cfg.clone(),
        }
    }
}

fn start_reading(body_kind: BodyKind, mode: RecvMode)
    -> Result<BodyProgress, Error>
{
    use super::codec::BodyKind::*;
    use super::codec::RecvMode::*;
    use super::Error::*;

    match (body_kind, mode) {
        (Unsupported, _) => {
            Err(UnsupportedBody)
        }
        (Fixed(x), BufferedUpfront(y)) if x > y as u64 => {
            Err(RequestTooLong)
        }
        (Fixed(x), _) => {
            Ok(BodyProgress::Fixed(x as usize))
        }
        (Chunked, _) => {
            Ok(BodyProgress::Chunked { buffered: 0, pending_chunk: 0,
                                       done: false })
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
                Body { .. } => self.config.inflight_request_limit-1,
                Closed | Hijack => return Ok(changed),
            };
            if self.waiting.len() >= limit {
                break;
            }
            // TODO(tailhook) Do reads alter parse_headers() [optimization]
            self.inbuf.read()?;
            let (next, cont) = match mem::replace(&mut self.reading, Closed) {
                Headers => {
                    match parse_headers(&mut self.inbuf.in_buf,
                                        &mut self.dispatcher)?
                    {
                        Some((body, mut codec, cfg)) => {
                            let mode = codec.recv_mode();
                            if mode == RecvMode::Hijack {
                                (Hijack, true)
                            } else {
                                (Body { mode: mode,
                                        response_config: cfg,
                                        progress: start_reading(body, mode)?,
                                        codec: codec },
                                 true)
                            }
                        }
                        None => (Headers, false),
                    }
                }
                Body { .. } => unimplemented!(),
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
        use server::RecvMode::{BufferedUpfront, Progressive};
        loop {
            let (next, cont) = match mem::replace(&mut self.writing, Void) {
                Idle(io) => {
                    if let Some((rc, mut codec)) = self.waiting.pop_front() {
                        let e = encoder::new(io, rc);
                        (Write(codec.start_response(e)), true)
                    } else {
                        match self.reading {
                            Body { mode: BufferedUpfront(..), ..}
                            | Closed | Headers
                            => {
                                (Idle(io), false)
                            }
                            Body { mode: RecvMode::Hijack, ..} => {
                                unreachable!();
                            }
                            Body {
                                mode: Progressive(_),
                                codec: ref mut codec, ..}
                            => {
                                // TODO(tailhook) start writing now
                                unimplemented!();
                            }
                            Hijack => unimplemented!(),
                        }
                    }
                }
                Write(mut f) => {
                    match f.poll()? {
                        Async::Ready(x) => {
                            (Idle(get_inner(x)), true)
                        }
                        Async::NotReady => {
                            (Write(f), true)
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

impl<S: Io, D: Dispatcher<S>> Future for Proto<S, D> {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<(), Error> {
        self.do_writes()?;
        while self.do_reads()? {
            self.do_writes()?;
        }
        // TODO(tailhook) close connection on `Connection: close`
        Ok(Async::NotReady)
    }
}
