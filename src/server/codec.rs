use futures::{Async, Future};
use tokio_core::io::Io;
use tk_bufstream::{ReadBuf, WriteBuf};

use super::{Error, Encoder, EncoderDone, Head};
use super::RecvMode;


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyKind {
    Fixed(u64),
    Chunked,
    Unsupported,
}

/// This is a low-level interface to the http server
pub trait Dispatcher<S: Io> {
    /// The codec type  for this dispatcher
    ///
    /// In many cases the type is just `Box<Codec<S>>`, but it left as
    /// associated type make different types of middleware cheaper.
    type Codec: Codec<S>;

    /// Received headers of a request
    ///
    /// At this point we already extracted all the headers and other data
    /// that we need to ensure correctness of the protocol. If you need
    /// to handle some data from the headers you need to store them somewhere
    /// (for example on `self`) for further processing.
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>;
}

/// The type represents a consumer of a single request and yields a writer of
/// a response (the latter is a ``ResponseFuture``
pub trait Codec<S: Io> {
    /// This is a future returned by `start_response`
    ///
    /// It's fine if it's just `Box<Future<Item=EncoderDone<S>, Error>>` in
    /// most cases.
    type ResponseFuture: Future<Item=EncoderDone<S>, Error=Error>;

    /// Return a mode which will be used to receive request body
    ///
    ///
    /// Note: this mode not only influences the size of chunks that
    /// `data_received` recieves and amount of buffering, but also it
    /// constrains the sequence between calls of `start_response()`
    /// and `data_received()`.
    ///
    /// Called once, right after `headers_received`
    fn recv_mode(&mut self) -> RecvMode;

    /// Chunk of the response body received
    ///
    /// `end` equals to `true` for the last chunk of the data.
    ///
    /// Method returns `Async::Ready(x)` to denote that it has consumed `x`
    /// bytes. If there are some bytes left in the buffer they will be passed
    /// again on the call.
    ///
    /// If the response is empty, or last chunk arrives later and it's empty
    /// we call `c.data_received(b"", true)` on every wakeup,
    /// until `Async::Ready(0)` is returned (this helps to drive future that
    /// might complete on response completion without spawning another ones,
    /// but note that next response can't start writing in the meantime).
    ///
    /// Protocol panics if returned number of bytes larger than `data.len()`.
    ///
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>;

    /// Start writing a response
    ///
    /// This method is called when there all preceding requests are either
    /// send to the network or already buffered. It can be called before
    /// `data_received()` but not before `headers_received()` (that would not
    /// make sense).
    ///
    /// Everything you write into a buffer might be flushed to the network
    /// immediately (or as fast as you yield to main loop). On the other
    /// hand we might buffer/pipeline multiple responses at once.
    fn start_response(&mut self, e: Encoder<S>) -> Self::ResponseFuture;

    /// Called after future retunrted by `start_response` done if recv mode
    /// is `Hijack`
    ///
    /// Note: both input and output buffers can contain some data.
    fn hijack(&mut self, _output: WriteBuf<S>,  _input: ReadBuf<S>) {
        panic!("`Codec::recv_mode` returned `Hijack` but \
            no hijack() method implemented");
    }
}

impl<S: Io, F> Codec<S> for Box<Codec<S, ResponseFuture=F>>
    where F: Future<Item=EncoderDone<S>, Error=Error>,
{
    type ResponseFuture = F;
    fn recv_mode(&mut self) -> RecvMode {
        (**self).recv_mode()
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>
    {
        (**self).data_received(data, end)
    }
    fn start_response(&mut self, e: Encoder<S>) -> Self::ResponseFuture {
        (**self).start_response(e)
    }
    fn hijack(&mut self, output: WriteBuf<S>,  input: ReadBuf<S>) {
        (**self).hijack(output, input)
    }
}
