use std::fmt;
use std::sync::Arc;

use futures::{Future, Async, Stream};
use futures::stream::Fuse;
use tokio_core::io::Io;
use tk_bufstream::{ReadFramed, WriteFramed, ReadBuf, WriteBuf};
use tk_bufstream::{Decode, Encode};

use websocket::{Frame, Config, Codec, Packet, Error};
use websocket::zero_copy::parse_frame;


/// Dispatches messages received from websocket
pub trait Dispatcher {
    /// Future returned from `frame()`
    type Future: Future<Item=(), Error=Error>;
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
pub struct Loop<S: Io, T, D: Dispatcher> {
    config: Arc<Config>,
    input: ReadBuf<S>,
    output: WriteBuf<S>,
    stream: Fuse<T>,
    dispatcher: D,
    backpressure: Option<D::Future>,
}

// TODO(tailhook) Stream::Error should be Void here
impl<S: Io, T, D, E> Loop<S, T, D>
    where T: Stream<Item=Packet, Error=E>,
          D: Dispatcher,
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
            stream: stream.fuse(),
            dispatcher: dispatcher,
            backpressure: None,
        }
    }
}

impl<S: Io, T, D, E> Loop<S, T, D>
    where T: Stream<Item=Packet, Error=E>,
          D: Dispatcher,
{
    fn read_stream(&mut self) -> Result<(), E> {
        // For now we assume that there is no useful backpressure can
        // be applied to a stream, so we read everything from the stream
        // and put it into a buffer
        while let Async::Ready(value) = self.stream.poll()? {
            match value {
                Some(pkt) => {
                    Codec.encode(pkt, &mut self.output.out_buf);
                }
                None => break,
            }
        }
        Ok(())
    }
}

impl<S: Io, T, D, E> Future for Loop<S, T, D>
    where T: Stream<Item=Packet, Error=E>,
          D: Dispatcher,
          E: fmt::Display,
{
    type Item = ();  // TODO(tailhook) void?
    type Error = Error;

    fn poll(&mut self) -> Result<Async<()>, Error> {
        self.read_stream()
            .map_err(|e| error!("Can't read from stream: {}", e)).ok();
        self.output.flush()?;

        if let Some(mut back) = self.backpressure.take() {
            match back.poll()? {
                Async::Ready(()) => {}
                Async::NotReady => {
                    self.backpressure = Some(back);
                    return Ok(Async::NotReady);
                }
            }
        }

        loop {
            while self.input.in_buf.len() > 0 {
                let (mut fut, nbytes) = match
                    parse_frame(&mut self.input.in_buf,
                                self.config.max_packet_size)?
                {
                    Some((frame, nbytes)) => {
                        (self.dispatcher.frame(&frame), nbytes)
                    }
                    None => break,
                };
                self.input.in_buf.consume(nbytes);
                match fut.poll()? {
                    Async::Ready(()) => {},
                    Async::NotReady => {
                        self.backpressure = Some(fut);
                        return Ok(Async::NotReady)
                    }
                }
            }
            match self.input.read()? {
                0 => {
                    if self.input.done() {
                        return Ok(Async::Ready(()));
                    } else {
                        return Ok(Async::NotReady);
                    }
                }
                _ => continue,
            }
        }
        unreachable!();
    }
}
