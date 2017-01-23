use std::sync::Arc;

use futures::{Future, Async, Stream};
use tokio_core::io::Io;
use tk_bufstream::{ReadFramed, WriteFramed, ReadBuf, WriteBuf};

use websocket::{Frame, Config, Codec, Packet};


/// Dispatches messages received from websocket
pub trait Dispatcher {
    /// Future returned from `frame()`
    type Future: Future<Item=(), Error=()>;
    /// A frame received
    ///
    /// If backpressure is desired, method may return a future other than
    /// `futures::FutureResult`.
    fn frame(&mut self, frame: &Frame) -> Self::Future;
}


/// This is a helper for running websockets
///
/// The Loop object is a future which polls both: (1) input stream,
/// calling dispatcher on each message and a (2) channel where you can send
/// output messages to from external futures.
///
/// Also Loop object answers pings by itself and pings idle connections.
pub struct Loop<S: Io, T, D> {
    config: Arc<Config>,
    input: ReadBuf<S>,
    output: WriteBuf<S>,
    stream: T,
    dispatcher: D,
}

// TODO(tailhook) Stream::Error should be Void here
impl<S: Io, T, D> Loop<S, T, D>
    where T: Stream<Item=Packet, Error=()>,
{
    /// Create a new websocket Loop
    ///
    /// This method should be callec in `hijack` method of `server::Codec`
    pub fn new(outp: WriteFramed<S, Codec>, inp: ReadFramed<S, Codec>,
        stream: T, dispatcher: D, config: &Arc<Config>)
        -> Loop<S, T, D>
    {
        Loop {
            config: config.clone(),
            input: inp.into_inner(),
            output: outp.into_inner(),
            stream: stream,
            dispatcher: dispatcher,
        }
    }
}

impl<S: Io, T, D> Future for Loop<S, T, D>
    where T: Stream<Item=Packet, Error=()>,
          D: Dispatcher,
{
    type Item = ();  // TODO(tailhook) void?
    type Error = ();  // TODO(tailhook) show shutdown reason?

    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
