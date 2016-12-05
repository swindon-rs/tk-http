use std::fmt::Display;
use std::ascii::AsciiExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use tokio_core::io::Io;
use tk_bufstream::WriteBuf;

use enums::Version;
use headers::is_close;
use base_serializer::{MessageState, HeaderError};

pub enum RequestState {
    Empty = 0,
    StartedHead = 1,
    StartedNormal = 2,
}

pub struct Encoder<S: Io> {
    message: MessageState,
    buf: WriteBuf<S>,
    // TODO(tailhook) we could use smaller atomic, but they are unstable
    state: Arc<AtomicUsize>,
    close_signal: Arc<AtomicBool>,
}

pub struct EncoderDone<S: Io> {
    buf: WriteBuf<S>,
}

pub fn get_inner<S: Io>(e: EncoderDone<S>) -> WriteBuf<S> {
    e.buf
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
    pub fn request_line(&mut self, method: &str, path: &str, version: Version)
    {
        self.message.request_line(&mut self.buf.out_buf,
            method, path, version);
        let nstatus = if method.eq_ignore_ascii_case("HEAD") {
            RequestState::StartedHead as usize
        } else {
            RequestState::StartedNormal as usize
        };
        if self.state.swap(nstatus, Ordering::SeqCst) != 0 {
            // Actually this panic happens in `message.request_line` we just
            // duplicate it here for clarity
            panic!("Request line in wrong state");
        }
    }
    /// Add a header to the message.
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
    pub fn add_header<V: AsRef<[u8]>>(&mut self, name: &str, value: V)
        -> Result<(), HeaderError>
    {
        if name.eq_ignore_ascii_case("Connection") && is_close(value.as_ref())
        {
            self.close_signal.store(true, Ordering::SeqCst);
        }
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
        if name.eq_ignore_ascii_case("Connection") {
            unimplemented!();
        }
        self.message.format_header(&mut self.buf.out_buf, name, value)
    }

    /// Add a content length to the message.
    ///
    /// The `Content-Length` header is written to the output buffer
    /// immediately.  It is checked that there are no other body length
    /// headers present in the message. When the body is send the length is
    /// validated.
    ///
    /// # Panics
    ///
    /// Panics when `add_length` is called in the wrong state.
    pub fn add_length(&mut self, n: u64)
        -> Result<(), HeaderError>
    {
        self.message.add_length(&mut self.buf.out_buf, n)
    }
    /// Sets the transfer encoding to chunked.
    ///
    /// Writes `Transfer-Encoding: chunked` to the output buffer immediately.
    /// It is assured that there is only one body length header is present
    /// and the body is written in chunked encoding.
    ///
    /// # Panics
    ///
    /// Panics when `add_chunked` is called in the wrong state.
    pub fn add_chunked(&mut self)
        -> Result<(), HeaderError>
    {
        self.message.add_chunked(&mut self.buf.out_buf)
    }
    /// Closes the HTTP header
    ///
    /// Similarly to `add_header()` it's fine to `unwrap()` here, unless you're
    /// doing some proxying.
    ///
    /// # Panics
    ///
    /// Panics when the request is in a wrong state.
    pub fn done_headers(&mut self) -> Result<(), HeaderError> {
        self.message.done_headers(&mut self.buf.out_buf)
        .map(|always_support_body| assert!(always_support_body))
    }
    pub fn done(mut self) -> EncoderDone<S> {
        self.message.done(&mut self.buf.out_buf);
        EncoderDone { buf: self.buf }
    }
}

pub fn new<S: Io>(io: WriteBuf<S>,
    state: Arc<AtomicUsize>, close_signal: Arc<AtomicBool>)
    -> Encoder<S>
{
    Encoder {
        message: MessageState::RequestStart,
        buf: io,
        state: state,
        close_signal: close_signal,
    }
}
