use std::ascii::AsciiExt;
use std::fmt::Display;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

use futures::{Future, Async};
use tk_bufstream::{WriteBuf, WriteFramed, ReadFramed};
use tokio_core::io::Io;

use base_serializer::{MessageState, HeaderError};
// TODO(tailhook) change the error
use client::{Head, Error};
use enums::Version;
use headers::is_close;
use websocket::Codec as WebsocketCodec;

/// This a request writer that you receive in `Codec`
///
/// Methods of this structure ensure that everything you write into a buffer
/// is consistent and valid protocol
pub struct Encoder<S: Io> {
    message: MessageState,
    buf: WriteBuf<S>,
}

/// This structure returned from `Encoder::done` and works as a continuation
/// that should be returned from the future that writes request.
pub struct EncoderDone<S: Io> {
    buf: WriteBuf<S>,
}

/// Authorizer sends all the necessary headers and checks response headers
/// to establish websocket connection
///
/// The `SimpleAuthorizer` implementation is good enough for most cases, but
/// custom authorizer may be helpful for `Cookie` or `Authorization` header.
pub trait Authorizer<S: Io> {
    /// The type that may be returned from a `header_received`. It should
    /// encompass everything parsed from input headers.
    type Result: Sized;
    /// Write request headers
    ///
    /// Websocket-specific headers like `Connection`, `Upgrade`, and
    /// `Sec-Websocket-Key` are written automatically. But optional things
    /// like `User-Agent` must be written by this method, as well as
    /// path encoded in request-line.
    fn write_headers(&mut self, e: Encoder<S>) -> EncoderDone<S>;
    /// A handler of response headers
    ///
    /// It's called when websocket has been sucessfully connected or when
    /// server returned error, check that response code equals 101 to make
    /// sure response is established.
    ///
    /// Anyway, handler may be skipped in case of invalid response headers.
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Result, Error>;
}

pub struct HandshakeProto<S, A> {
    transport: S,
    authorizer: A,
}


pub struct SimpleAuthorizer {
    path: String,
}

impl<S: Io> Authorizer<S> for SimpleAuthorizer {
    type Result = ();
    fn write_headers(&mut self, mut e: Encoder<S>) -> EncoderDone<S> {
        e.request_line(&self.path);
        e.add_header("User-Agent", concat!("minihttp/",
            env!("CARGO_PKG_VERSION"))).unwrap();
        e.done()
    }
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Result, Error>
    {
        Ok(())
    }
}

fn check_header(name: &str) {
    if name.eq_ignore_ascii_case("Connection") ||
        name.eq_ignore_ascii_case("Upgrade") ||
        name.eq_ignore_ascii_case("Sec-Websocket-Key")
    {
        panic!("You shouldn'set connection header yourself");
    }
}

impl<S: Io> Encoder<S> {
    /// Write request line.
    ///
    /// This puts request line into a buffer immediately. If you don't
    /// continue with request it will be sent to the network shortly.
    ///
    /// # Panics
    ///
    /// When request line is already written. It's expected that your request
    /// handler state machine will never call the method twice.
    pub fn request_line(&mut self, path: &str) {
        self.message.request_line(&mut self.buf.out_buf,
            "GET", path, Version::Http11);
    }
    /// Add a header to the websocket authenticatin data
    ///
    /// Header is written into the output buffer immediately. And is sent
    /// as soon as the next loop iteration
    ///
    /// `Content-Length` header must be send using the `add_length` method
    /// and `Transfer-Encoding: chunked` must be set with the `add_chunked`
    /// method. These two headers are important for the security of HTTP.
    ///
    /// Note that there is currently no way to use a transfer encoding other
    /// than chunked.
    ///
    /// We return Result here to make implementing proxies easier. In the
    /// application handler it's okay to unwrap the result and to get
    /// a meaningful panic (that is basically an assertion).
    ///
    /// # Panics
    ///
    /// Panics when `add_header` is called in the wrong state.
    ///
    /// When you add a special header `Connection`, `Upgrade`,
    /// `Sec-Websocket-*`, because they must be set with special methods
    pub fn add_header<V: AsRef<[u8]>>(&mut self, name: &str, value: V)
        -> Result<(), HeaderError>
    {
        check_header(name);
        self.message.add_header(&mut self.buf.out_buf, name, value.as_ref())
    }

    /// Same as `add_header` but allows value to be formatted directly into
    /// the buffer
    ///
    /// Useful for dates and numeric headers, as well as some strongly typed
    /// wrappers
    pub fn format_header<D: Display>(&mut self, name: &str, value: D)
        -> Result<(), HeaderError>
    {
        check_header(name);
        self.message.format_header(&mut self.buf.out_buf, name, value)
    }
    /// Finish writing headers and return `EncoderDone` which can be moved to
    ///
    /// # Panics
    ///
    /// Panics when the request is in a wrong state.
    pub fn done(mut self) -> EncoderDone<S> {
        self.message.done_headers(&mut self.buf.out_buf)
            .map(|never_support_body| assert!(!never_support_body)).unwrap();
        self.message.done(&mut self.buf.out_buf);
        EncoderDone { buf: self.buf }
    }
}

fn encoder<S: Io>(io: WriteBuf<S>) -> Encoder<S> {
    Encoder {
        message: MessageState::RequestStart,
        buf: io,
    }
}

impl<S, A> HandshakeProto<S, A> {
    pub fn new(transport: S, authorizer: A) -> HandshakeProto<S, A> {
        HandshakeProto {
            authorizer: authorizer,
            transport: transport,
        }
    }
}

impl<S: Io, A> Future for HandshakeProto<S, A>
    where A: Authorizer<S>
{
    type Item = (WriteFramed<S, WebsocketCodec>, ReadFramed<S, WebsocketCodec>,
                 A::Result);
    type Error = Error;
    fn poll(&mut self) -> Result<Async<Self::Item>, Error> {
        unimplemented!();
    }
}
