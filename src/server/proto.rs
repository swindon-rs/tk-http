use futures::Future;
use tk_bufstream::{IoBuf, WriteBuf, ReadBuf};
use tokio_core::io::Io;

use super::parser::Parser;
use super::encoder::{self, get_inner};
use super::{Codec, Error, EncoderDone, Config, RecvMode};


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

enum InState {
    Headers,
    Body {
        mode: RecvMode,
        progress: BodyProgress,
    },
    Void,
}

/// A low-level HTTP/1.x server protocol handler
pub struct Proto<S: Io, C: Codec<S>> {
    codec: C,
    inbuf: ReadBuf<S>,
    reading: InState,
    waiting: VecDeque<(C, Arc<AtomicUsize>)>,
    writing: OutState<S>,
    config: Arc<Config>,
}

impl<S: Io, C: Codec<S>> Proto<S, C> {
    /// Create a new protocol implementation from a TCP connection and a config
    ///
    /// You should use this protocol as a `Sink`
    pub fn new(conn: S, cfg: &Arc<Config>) -> Proto<S, C> {
        let (cout, cin) = IoBuf::new(conn).split();
        return Proto {
            reading: InState::Idle(cin),
            waiting: VecDeque::with_capacity(cfg.inflight_request_prealloc),
            writing: OutState::Idle(cout),
            config: cfg.clone(),
        }
    }
}

impl<S: Io, C: Codec<S>> Proto<S, C> {
    fn do_reads(&mut self) -> Poll<(), Error> {
        unimplemented!();
    }
    fn do_writes(&mut self) -> Poll<(), Error> {
        unimplemented!();
    }
}

impl<S: Io, C: Codec<S>> Proto<S, C> {
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
