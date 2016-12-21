use std::sync::Arc;
use std::collections::VecDeque;

use futures::{Future, Poll, Async};
use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::io::Io;

use super::parser::Parser;
use super::encoder::{self, get_inner};
use super::{Dispatcher, Codec, Error, EncoderDone, Config, RecvMode};


enum OutState<S: Io> {
    Idle(WriteBuf<S>),
    Write(Box<Future<Item=EncoderDone<S>, Error=Error>>),
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
        codec: C,
    },
    Void,
}

/// A low-level HTTP/1.x server protocol handler
pub struct Proto<S: Io, D: Dispatcher<S>> {
    dispatcher: D,
    inbuf: ReadBuf<S>,
    reading: InState<D::Codec>,
    waiting: VecDeque<D::Codec>,
    writing: OutState<S>,
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

impl<S: Io, D: Dispatcher<S>> Proto<S, D> {
    fn do_reads(&mut self) -> Result<bool, Error> {
        unimplemented!();
    }
    fn do_writes(&mut self) -> Result<(), Error> {
        unimplemented!();
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
        Ok(Async::NotReady)
    }
}
