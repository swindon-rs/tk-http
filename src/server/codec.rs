use futures::Async;
use httparse::Header;
use tokio_core::io::Io;

use super::{Error, Encoder, EncoderDone};
use {Version, OptFuture};


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyKind {
    Fixed(u64),
    Chunked,
}

/// This type is returned from `headers_received` handler of either
/// client client or server protocol handler
///
/// The marker is used to denote whether you want to have the whole response
/// buffered for you or read chunk by chunk.
///
/// The `Progressive` (chunk by chunk) mode is mostly useful for proxy servers.
/// Or it may be useful if your handler is able to parse data without holding
/// everything in the memory.
///
/// Otherwise, it's best to use `Buffered` mode (for example, comparing with
/// using your own buffering). We do our best to optimize it for you.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvMode {
    /// Download whole message body (request or response) into the memory
    /// before starting response
    ///
    /// The argument is maximum size of the body. The Buffered variant
    /// works equally well for Chunked encoding and for read-util-end-of-stream
    /// mode of HTTP/1.0, so sometimes you can't know the size of the request
    /// in advance. Note this is just an upper limit it's neither buffer size
    /// nor *minimum* size of the body.
    BufferedUpfront(usize),
    /// Fetch data chunk-by-chunk.
    ///
    /// Note, your response handler can start either before or after
    /// progressive body has started or ended to read. I mean they are
    /// completely independent, and actual sequence of events depends on other
    /// requests coming in and performance of a client.
    ///
    /// The parameter denotes minimum number of bytes that may be passed
    /// to the protocol handler. This is for performance tuning (i.e. less
    /// wake-ups of protocol parser). But it's not an input buffer size. The
    /// use of `Progressive(1)` is perfectly okay (for example if you use http
    /// request body as a persistent connection for sending multiple messages
    /// on-demand)
    Progressive(usize),
}

/// A borrowed structure that represents request headers
///
/// It's passed to `Codec::headers_received` and you are free to store or
/// discard any needed fields and headers from it.
///
/// Note, we don't strip hop-by-hop headers (`Connection: close`,
/// `Transfer-Encoding`) and we use them to ensure correctness of the protocol.
/// You must skip them if proxying headers somewhere.
// TODO(tailhook) hide the structure?
#[derive(Debug)]
pub struct Head<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub version: Version,
    pub host: &'a str,
    pub headers: &'a [Header<'a>],
    pub body_kind: BodyKind,
    pub close: bool,
    pub connection: &'a [u8],
}

/// This is a low-level interface to the http server
pub trait Dispatcher<S: Io> {
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

pub trait Codec<S: Io> {
    /// Return a mode which will be used to receive request body
    ///
    ///
    /// Note: this mode not only influences the size of chunks that
    /// `data_received` recieves and amount of buffering, but also it
    /// constrains
    /// sequence betwee
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
    fn start_response(&mut self, e: Encoder<S>)
        -> OptFuture<EncoderDone<S>, Error>;
}
