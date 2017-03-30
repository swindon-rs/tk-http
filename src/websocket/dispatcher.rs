use std::fmt;
use std::sync::Arc;

use futures::{Future, Async, Stream};
use futures::future::{FutureResult, ok};
use futures::stream;
use tk_bufstream::{ReadFramed, WriteFramed, ReadBuf, WriteBuf};
use tk_bufstream::{Encode};
use tokio_io::{AsyncRead, AsyncWrite};

use websocket::{Frame, Config, Packet, Error, ServerCodec, ClientCodec};
use websocket::error::ErrorEnum;
use websocket::zero_copy::{parse_frame, write_packet, write_close};


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
pub struct Loop<S, T, D: Dispatcher> {
    config: Arc<Config>,
    input: ReadBuf<S>,
    output: WriteBuf<S>,
    stream: Option<T>,
    dispatcher: D,
    backpressure: Option<D::Future>,
    state: LoopState,
    server: bool,
}


/// A special kind of dispatcher that consumes all messages and does nothing
///
/// This is used with `Loop::closing()`.
pub struct BlackHole;

/// A displayable stream error that never happens
///
/// This is used with `Loop::closing()`.
pub struct VoidError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopState {
    Open,
    CloseSent,
    CloseReceived,
    Done,
}

// TODO(tailhook) Stream::Error should be Void here
impl<S, T, D, E> Loop<S, T, D>
    where T: Stream<Item=Packet, Error=E>,
          D: Dispatcher,
{
    /// Create a new websocket Loop (server-side)
    ///
    /// This method should be called in `hijack` method of `server::Codec`
    pub fn server(
        outp: WriteFramed<S, ServerCodec>,
        inp: ReadFramed<S, ServerCodec>,
        stream: T, dispatcher: D, config: &Arc<Config>)
        -> Loop<S, T, D>
    {
        Loop {
            config: config.clone(),
            input: inp.into_inner(),
            output: outp.into_inner(),
            stream: Some(stream),
            dispatcher: dispatcher,
            backpressure: None,
            state: LoopState::Open,
            server: true,
        }
    }
    /// Create a new websocket Loop (client-side)
    ///
    /// This method should be called after `HandshakeProto` finishes
    pub fn client(
        outp: WriteFramed<S, ClientCodec>,
        inp: ReadFramed<S, ClientCodec>,
        stream: T, dispatcher: D, config: &Arc<Config>)
        -> Loop<S, T, D>
    {
        Loop {
            config: config.clone(),
            input: inp.into_inner(),
            output: outp.into_inner(),
            stream: Some(stream),
            dispatcher: dispatcher,
            backpressure: None,
            state: LoopState::Open,
            server: false,
        }
    }
}

impl<S> Loop<S, stream::Empty<Packet, VoidError>, BlackHole>
{
    /// A websocket loop that sends failure and waits for closing handshake
    ///
    /// This method should be called instead of `new` if something wrong
    /// happened with handshake.
    ///
    /// The motivation of such constructor is: browsers do not propagate
    /// http error codes when websocket is established. This is presumed as
    /// a security feature (so you can't attack server that doesn't support
    /// websockets).
    ///
    /// So to show useful failure to websocket code we return `101 Switching
    /// Protocol` response code (which is success). I.e. establish a websocket
    /// connection, then immediately close it with a reason code and text.
    /// Javascript client can fetch the failure reason from `onclose` callback.
    pub fn closing(
        outp: WriteFramed<S, ServerCodec>,
        inp: ReadFramed<S, ServerCodec>,
        reason: u16, text: &str,
        config: &Arc<Config>)
        -> Loop<S, stream::Empty<Packet, VoidError>, BlackHole>
    {
        let mut out = outp.into_inner();
        write_close(&mut out.out_buf, reason, text, false);
        Loop {
            config: config.clone(),
            input: inp.into_inner(),
            output: out,
            stream: None,
            dispatcher: BlackHole,
            backpressure: None,
            state: LoopState::CloseSent,
            // TODO(tailhook) should we provide client-size thing?
            server: true,
        }
    }
}

impl<S, T, D, E> Loop<S, T, D>
    where T: Stream<Item=Packet, Error=E>,
          D: Dispatcher,
{
    fn read_stream(&mut self) -> Result<(), E> {
        if self.state == LoopState::CloseSent {
            return Ok(());
        }
        // For now we assume that there is no useful backpressure can
        // be applied to a stream, so we read everything from the stream
        // and put it into a buffer
        if let Some(ref mut stream) = self.stream {
            loop {
                match stream.poll()? {
                    Async::Ready(value) => match value {
                        Some(pkt) => {
                            if self.server {
                                ServerCodec.encode(pkt,
                                    &mut self.output.out_buf);
                            } else {
                                ClientCodec.encode(pkt,
                                    &mut self.output.out_buf);
                            }
                        }
                        None => {
                            match self.state {
                                LoopState::Open => {
                                    // send close
                                    write_close(&mut self.output.out_buf,
                                                1000, "", !self.server);
                                    self.state = LoopState::CloseSent;
                                }
                                LoopState::CloseReceived => {
                                    self.state = LoopState::Done;
                                }
                                _ => {}
                            }
                            break;
                        }
                    },
                    Async::NotReady => {
                        return Ok(());
                    }
                }
            }
        }
        self.stream = None;
        Ok(())
    }
}

impl<S, T, D, E> Future for Loop<S, T, D>
    where T: Stream<Item=Packet, Error=E>,
          D: Dispatcher,
          E: fmt::Display,
          S: AsyncRead + AsyncWrite,
{
    type Item = ();  // TODO(tailhook) void?
    type Error = Error;

    fn poll(&mut self) -> Result<Async<()>, Error> {
        self.read_stream()
            .map_err(|e| error!("Can't read from stream: {}", e)).ok();
        self.output.flush().map_err(ErrorEnum::Io)?;
        if self.state == LoopState::Done {
            return Ok(Async::Ready(()));
        }

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
                let (fut, nbytes) = match
                    parse_frame(&mut self.input.in_buf,
                                self.config.max_packet_size, self.server)?
                {
                    Some((frame, nbytes)) => {
                        let fut = match frame {
                            Frame::Ping(data) => {
                                trace!("Received ping {:?}", data);
                                write_packet(&mut self.output.out_buf,
                                             0xA, data, !self.server);
                                None
                            }
                            Frame::Pong(data) => {
                                trace!("Received pong {:?}", data);
                                None
                            }
                            Frame::Close(code, reply) => {
                                debug!("Websocket closed by peer [{}]{:?}",
                                    code, reply);
                                self.state = LoopState::CloseReceived;
                                Some(self.dispatcher.frame(
                                    &Frame::Close(code, reply)))
                            }
                            pkt @ Frame::Text(_) | pkt @ Frame::Binary(_) => {
                                Some(self.dispatcher.frame(&pkt))
                            }
                        };
                        (fut, nbytes)
                    }
                    None => break,
                };
                self.input.in_buf.consume(nbytes);
                if self.state == LoopState::Done {
                    return Ok(Async::Ready(()));
                }
                if let Some(mut fut) = fut {
                    match fut.poll()? {
                        Async::Ready(()) => {},
                        Async::NotReady => {
                            self.backpressure = Some(fut);
                            return Ok(Async::NotReady);
                        }
                    }
                }
            }
            match self.input.read().map_err(ErrorEnum::Io)? {
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
    }
}

impl Dispatcher for BlackHole {
    type Future = FutureResult<(), Error>;
    fn frame(&mut self, _frame: &Frame) -> Self::Future {
        ok(())
    }
}

impl fmt::Display for VoidError {
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        unreachable!();
    }
}
