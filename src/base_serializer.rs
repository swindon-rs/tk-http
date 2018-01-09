//! This contains common part of serializer between client and server
//! implementation

use std::fmt::Display;
use std::io::Write;
#[allow(unused_imports)]
use std::ascii::AsciiExt;

use tk_bufstream::Buf;


use enums::Version;

quick_error! {
    #[derive(Debug)]
    pub enum HeaderError {
        DuplicateContentLength {
            description("Content-Length is added twice")
        }
        DuplicateTransferEncoding {
            description("Transfer-Encoding is added twice")
        }
        InvalidHeaderName {
            description("Header name contains invalid characters")
        }
        InvalidHeaderValue {
            description("Header value contains invalid characters")
        }
        TransferEncodingAfterContentLength {
            description("Transfer encoding added when Content-Length is \
                already specified")
        }
        ContentLengthAfterTransferEncoding {
            description("Content-Length added after Transfer-Encoding")
        }
        CantDetermineBodySize {
            description("Neither Content-Length nor Transfer-Encoding \
                is present in the headers")
        }
        BodyLengthHeader {
            description("Content-Length and Transfer-Encoding must be set \
                using the specialized methods")
        }
        RequireBodyless {
            description("This message must not contain body length fields.")
        }
    }
}

/// This is a state of message that is fine both for requests and responses
///
/// Note: while we pass buffer to each method, we expect that the same buffer
/// is passed each time
#[derive(Debug)]
pub enum MessageState {
    /// Nothing has been sent.
    ResponseStart { version: Version, body: Body, close: bool },
    /// A continuation line has been sent.
    FinalResponseStart { version: Version, body: Body, close: bool },
    /// Nothing has been sent.
    #[allow(dead_code)] // until we implement client requests
    RequestStart,
    /// Status line is already in the buffer.
    Headers { body: Body, close: bool },
    /// The message contains a fixed size body.
    FixedHeaders { is_head: bool, close: bool, content_length: u64 },
    /// The message contains a chunked body.
    ChunkedHeaders { is_head: bool, close: bool },
    /// The message contains no body.
    ///
    /// A request without a `Content-Length` or `Transfer-Encoding`
    /// header field contains no body.
    ///
    /// All 1xx (Informational), 204 (No Content),
    /// and 304 (Not Modified) responses do not include a message body.
    Bodyless,
    /// The message contains a body with the given length.
    FixedBody { is_head: bool, content_length: u64 },
    /// The message contains a chunked body.
    ChunkedBody { is_head: bool },
    /// A message in final state.
    Done,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Body {
    /// Message contains a body.
    Normal,
    /// Message body is ignored: responses to HEAD requests.
    Head,
    /// Message must not have a body: all 1xx (Informational),
    /// 204 (No Content), and 304 (Not Modified) responses
    Denied,
    /// The message is a request and always contains a body (maybe empty).
    #[allow(dead_code)] // until we implement client requests
    Request,
}

fn invalid_header(value: &[u8]) -> bool {
    return value.iter().any(|&x| x == b'\r' || x == b'\n')
}

impl MessageState {
    /// Write status line.
    ///
    /// This puts status line into a buffer immediately. If you don't
    /// continue with request it will be sent to the network shortly.
    ///
    /// # Panics
    ///
    /// When status line is already written. It's expected that your request
    /// handler state machine will never call the method twice.
    ///
    /// When the status code is 100 (Continue). 100 is not allowed
    /// as a final status code.
    pub fn response_status(&mut self, buf: &mut Buf, code: u16, reason: &str) {
        use self::Body::*;
        use self::MessageState::*;
        match *self {
            ResponseStart { version, mut body, close } |
            FinalResponseStart { version, mut body, close } => {
                // 100 (Continue) interim status code is not allowed as
                // a final response status.
                assert!(code != 100);
                write!(buf, "{} {} {}\r\n",
                    version, code, reason).unwrap();
                // Responses without body:
                //
                // * 1xx (Informational)
                // * 204 (No Content)
                // * 304 (Not Modified)
                if (code >= 100 && code < 200) || code == 204 || code == 304 {
                    body = Denied
                }
                *self = Headers { body: body, close: close };
            }
            ref state => {
                panic!("Called response_status() method on response \
                    in state {:?}", state)
            }
        }
    }

    /// Write request line.
    ///
    /// This puts request line into a buffer immediately. If you don't
    /// continue with request it will be sent to the network shortly.
    ///
    /// # Panics
    ///
    /// When request line is already written. It's expected that your request
    /// handler state machine will never call the method twice.
    pub fn request_line(&mut self, buf: &mut Buf,
        method: &str, path: &str, version: Version)
    {
        use self::Body::*;
        use self::MessageState::*;
        match *self {
            RequestStart => {
                write!(buf, "{} {} {}\r\n",
                    method, path, version).unwrap();
                // All requests may contain a body although it is uncommon for
                // GET and HEAD requests to contain one.
                *self = Headers { body: Request, close: false };
            }
            ref state => {
                panic!("Called request_line() method on request in state {:?}",
                       state)
            }
        }
    }

    /// Write a 100 (Continue) response.
    ///
    /// A server should respond with the 100 status code if it receives a
    /// 100-continue expectation.
    ///
    /// # Panics
    ///
    /// When the response is already started. It's expected that your response
    /// handler state machine will never call the method twice.
    pub fn response_continue(&mut self, buf: &mut Buf) {
        use self::MessageState::*;
        match *self {
            ResponseStart { version, body, close } => {
                write!(buf, "{} 100 Continue\r\n\r\n", version).unwrap();
                *self = FinalResponseStart { version: version,
                                            body: body,
                                            close: close }
            }
            ref state => {
                panic!("Called continue_line() method on response in state {:?}",
                       state)
            }
        }
    }

    fn write_header(&mut self, buf: &mut Buf, name: &str, value: &[u8])
        -> Result<(), HeaderError>
    {
        if invalid_header(name.as_bytes()) {
            return Err(HeaderError::InvalidHeaderName);
        }
        let start = buf.len();
        buf.write_all(name.as_bytes()).unwrap();
        buf.write_all(b": ").unwrap();

        let value_start = buf.len();
        buf.write_all(value).unwrap();
        if invalid_header(&buf[value_start..]) {
            buf.remove_range(start..);
            return Err(HeaderError::InvalidHeaderValue);
        }

        buf.write_all(b"\r\n").unwrap();
        Ok(())
    }

    fn write_formatted<D: Display>(&mut self, buf: &mut Buf,
        name: &str, value: D)
        -> Result<(), HeaderError>
    {
        if invalid_header(name.as_bytes()) {
            return Err(HeaderError::InvalidHeaderName);
        }
        let start = buf.len();
        buf.write_all(name.as_bytes()).unwrap();
        buf.write_all(b": ").unwrap();

        let value_start = buf.len();
        write!(buf, "{}", value).unwrap();
        if invalid_header(&buf[value_start..]) {
            buf.remove_range(start..);
            return Err(HeaderError::InvalidHeaderValue);
        }

        buf.write_all(b"\r\n").unwrap();
        Ok(())
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
    pub fn add_header(&mut self, buf: &mut Buf, name: &str, value: &[u8])
        -> Result<(), HeaderError>
    {
        use self::MessageState::*;
        use self::HeaderError::*;
        if name.eq_ignore_ascii_case("Content-Length")
            || name.eq_ignore_ascii_case("Transfer-Encoding") {
            return Err(BodyLengthHeader)
        }
        match *self {
            Headers { .. } | FixedHeaders { .. } | ChunkedHeaders { .. } => {
                self.write_header(buf, name, value)?;
                Ok(())
            }
            ref state => {
                panic!("Called add_header() method on a message in state {:?}",
                       state)
            }
        }
    }

    /// Same as `add_header` but allows value to be formatted directly into
    /// the buffer
    ///
    /// Useful for dates and numeric headers, as well as some strongly typed
    /// wrappers
    pub fn format_header<D: Display>(&mut self, buf: &mut Buf,
        name: &str, value: D)
        -> Result<(), HeaderError>
    {
        use self::MessageState::*;
        use self::HeaderError::*;
        if name.eq_ignore_ascii_case("Content-Length")
            || name.eq_ignore_ascii_case("Transfer-Encoding") {
            return Err(BodyLengthHeader)
        }
        match *self {
            Headers { .. } | FixedHeaders { .. } | ChunkedHeaders { .. } => {
                self.write_formatted(buf, name, value)?;
                Ok(())
            }
            ref state => {
                panic!("Called add_header() method on a message in state {:?}",
                       state)
            }
        }
    }

    /// Add a content length to the message.
    ///
    /// The `Content-Length` header is written to the output buffer immediately.
    /// It is checked that there are no other body length headers present in the
    /// message. When the body is send the length is validated.
    ///
    /// # Panics
    ///
    /// Panics when `add_length` is called in the wrong state.
    pub fn add_length(&mut self, buf: &mut Buf, n: u64)
        -> Result<(), HeaderError> {
        use self::MessageState::*;
        use self::HeaderError::*;
        use self::Body::*;
        match *self {
            FixedHeaders { .. } => Err(DuplicateContentLength),
            ChunkedHeaders { .. } => Err(ContentLengthAfterTransferEncoding),
            Headers { body: Denied, .. } => Err(RequireBodyless),
            Headers { body, close } => {
                self.write_formatted(buf, "Content-Length", n)?;
                *self = FixedHeaders { is_head: body == Head,
                                        close: close,
                                        content_length: n };
                Ok(())
            }
            ref state => {
                panic!("Called add_length() method on message in state {:?}",
                       state)
            }
        }
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
    pub fn add_chunked(&mut self, buf: &mut Buf)
        -> Result<(), HeaderError> {
            use self::MessageState::*;
            use self::HeaderError::*;
            use self::Body::*;
            match *self {
                FixedHeaders { .. } => Err(TransferEncodingAfterContentLength),
                ChunkedHeaders { .. } => Err(DuplicateTransferEncoding),
                Headers { body: Denied, .. } => Err(RequireBodyless),
                Headers { body, close } => {
                    self.write_header(buf, "Transfer-Encoding", b"chunked")?;
                    *self = ChunkedHeaders { is_head: body == Head,
                                              close: close };
                    Ok(())
                }
            ref state => {
                panic!("Called add_chunked() method on message in state {:?}",
                       state)
            }
        }
    }

    /// Returns true if at least `status()` method has been called
    ///
    /// This is mostly useful to find out whether we can build an error page
    /// or it's already too late.
    pub fn is_started(&self) -> bool {
        !matches!(*self,
            MessageState::RequestStart |
            MessageState::ResponseStart { .. } |
            MessageState::FinalResponseStart { .. })
    }

    /// Closes the HTTP header and returns `true` if entity body is expected.
    ///
    /// Specifically `false` is returned when status is 1xx, 204, 304 or in
    /// the response to a `HEAD` request but not if the body has zero-length.
    ///
    /// Similarly to `add_header()` it's fine to `unwrap()` here, unless you're
    /// doing some proxying.
    ///
    /// # Panics
    ///
    /// Panics when the response is in a wrong state.
    pub fn done_headers(&mut self, buf: &mut Buf)
        -> Result<bool, HeaderError>
    {
        use self::Body::*;
        use self::MessageState::*;
        if matches!(*self,
                    Headers { close: true, .. } |
                    FixedHeaders { close: true, .. } |
                    ChunkedHeaders { close: true, .. }) {
            self.add_header(buf, "Connection", b"close").unwrap();
        }
        let expect_body = match *self {
            Headers { body: Denied, .. } => {
                *self = Bodyless;
                false
            }
            Headers { body: Request, .. } => {
                *self = FixedBody { is_head: false, content_length: 0 };
                true
            }
            Headers { body: Normal, .. } => {
                return Err(HeaderError::CantDetermineBodySize);
            }
            FixedHeaders { is_head, content_length, .. } => {
                *self = FixedBody { is_head: is_head,
                                     content_length: content_length };
                !is_head
            }
            ChunkedHeaders { is_head, .. } => {
                *self = ChunkedBody { is_head: is_head };
                !is_head
            }
            ref state => {
                panic!("Called done_headers() method on  in state {:?}",
                       state)
            }
        };
        buf.write(b"\r\n").unwrap();
        Ok(expect_body)
    }

    /// Write a chunk of the message body.
    ///
    /// Works both for fixed-size body and chunked body.
    ///
    /// For the chunked body each chunk is put into the buffer immediately
    /// prefixed by chunk size. Empty chunks are ignored.
    ///
    /// For both modes chunk is put into the buffer, but is only sent when
    /// rotor-stream state machine is reached. So you may put multiple chunks
    /// into the buffer quite efficiently.
    ///
    /// You may write a body in responses to HEAD requests just like in real
    /// requests but the data is not sent to the network. Of course it is
    /// more efficient to not construct the message body at all.
    ///
    /// # Panics
    ///
    /// When response is in wrong state. Or there is no headers which
    /// determine response body length (either Content-Length or
    /// Transfer-Encoding).
    pub fn write_body(&mut self, buf: &mut Buf, data: &[u8]) {
        use self::MessageState::*;
        match *self {
            Bodyless => panic!("Message must not contain body."),
            FixedBody { is_head, ref mut content_length } => {
                if data.len() as u64 > *content_length {
                    panic!("Fixed size response error. \
                        Bytes left {} but got additional {}",
                        content_length, data.len());
                }
                if !is_head {
                    buf.write(data).unwrap();
                }
                *content_length -= data.len() as u64;
            }
            ChunkedBody { is_head } => if !is_head && data.len() > 0 {
                write!(buf, "{:x}\r\n", data.len()).unwrap();
                buf.write(data).unwrap();
                buf.write(b"\r\n").unwrap();
            },
            ref state => {
                panic!("Called write_body() method on message \
                    in state {:?}", state)
            }
        }
    }
    /// Returns true if headers are already sent (buffered)
    pub fn is_after_headers(&self) -> bool {
        use self::MessageState::*;
        matches!(*self, Bodyless | Done |
            FixedBody {..} | ChunkedBody {..})
    }

    /// Returns true if `done()` method is already called-
    pub fn is_complete(&self) -> bool {
        matches!(*self, MessageState::Done)
    }

    /// Writes needed finalization data into the buffer and asserts
    /// that response is in the appropriate state for that.
    ///
    /// The method may be called multiple times.
    ///
    /// # Panics
    ///
    /// When the message is in the wrong state or the body is not finished.
    pub fn done(&mut self, buf: &mut Buf) {
        use self::MessageState::*;
        match *self {
            Bodyless => *self = Done,
            // Don't check for responses to HEAD requests if body was actually sent.
            FixedBody { is_head: true, .. } |
            ChunkedBody { is_head: true } => *self = Done,
            FixedBody { is_head: false, content_length: 0 } => *self = Done,
            FixedBody { is_head: false, content_length } =>
                panic!("Tried to close message with {} bytes remaining.",
                       content_length),
            ChunkedBody { is_head: false } => {
                buf.write(b"0\r\n\r\n").unwrap();
                *self = Done;
            }
            Done => {}  // multiple invocations are okay.
            ref state => {
                panic!("Called done() method on response in state {:?}",
                       state);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use tk_bufstream::{Buf};

    use super::{MessageState, Body};
    use enums::Version;

    #[test]
    fn message_size() {
        // Just to keep track of size of structure
        assert_eq!(::std::mem::size_of::<MessageState>(), 16);
    }

    fn do_request<F>(fun: F) -> Buf
        where F: FnOnce(MessageState, &mut Buf)
    {
        let mut buf = Buf::new();
        fun(MessageState::RequestStart, &mut buf);
        buf
    }
    fn do_response10<F>(fun: F) -> Buf
        where F: FnOnce(MessageState, &mut Buf)
    {
        let mut buf = Buf::new();
        fun(MessageState::ResponseStart {
            version: Version::Http10,
            body: Body::Normal,
            close: false,
        }, &mut buf);
        buf
    }
    fn do_response11<F>(close: bool, fun: F) -> Buf
        where F: FnOnce(MessageState, &mut Buf)
    {
        let mut buf = Buf::new();
        fun(MessageState::ResponseStart {
            version: Version::Http11,
            body: Body::Normal,
            close: close,
        }, &mut buf);
        buf
    }

    fn do_head_response11<F>(close: bool, fun: F)
        -> Buf
        where F: FnOnce(MessageState, &mut Buf)
    {
        let mut buf = Buf::new();
        fun(MessageState::ResponseStart {
            version: Version::Http11,
            body: Body::Head,
            close: close,
        }, &mut buf);
        buf
    }

    #[test]
    fn minimal_request() {
        assert_eq!(&do_request(|mut msg, buf| {
            msg.request_line(buf, "GET", "/", Version::Http10);
            msg.done_headers(buf).unwrap();
        })[..], "GET / HTTP/1.0\r\n\r\n".as_bytes());
    }

    #[test]
    fn minimal_response() {
        assert_eq!(&do_response10(|mut msg, buf| {
            msg.response_status(buf, 200, "OK");
            msg.add_length(buf, 0).unwrap();
            msg.done_headers(buf).unwrap();
        })[..], "HTTP/1.0 200 OK\r\nContent-Length: 0\r\n\r\n".as_bytes());
    }

    #[test]
    fn minimal_response11() {
        assert_eq!(&do_response11(false, |mut msg, buf| {
            msg.response_status(buf, 200, "OK");
            msg.add_length(buf, 0).unwrap();
            msg.done_headers(buf, ).unwrap();
        })[..], "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".as_bytes());
    }

    #[test]
    fn close_response11() {
        assert_eq!(&do_response11(true, |mut msg, buf| {
            msg.response_status(buf, 200, "OK");
            msg.add_length(buf, 0).unwrap();
            msg.done_headers(buf).unwrap();
        })[..], concat!("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n",
                        "Connection: close\r\n\r\n").as_bytes());
    }

    #[test]
    fn head_request() {
        assert_eq!(&do_request(|mut msg, buf| {
            msg.request_line(buf, "HEAD", "/", Version::Http11);
            msg.add_length(buf, 5).unwrap();
            msg.done_headers(buf, ).unwrap();
            msg.write_body(buf, b"Hello");
        })[..], "HEAD / HTTP/1.1\r\nContent-Length: 5\r\n\r\nHello".as_bytes());
    }

    #[test]
    fn head_response() {
        // The response to a HEAD request may contain the real body length.
        assert_eq!(&do_head_response11(false, |mut msg, buf| {
            msg.response_status(buf, 200, "OK");
            msg.add_length(buf, 500).unwrap();
            msg.done_headers(buf).unwrap();
        })[..], "HTTP/1.1 200 OK\r\nContent-Length: 500\r\n\r\n".as_bytes());
    }

    #[test]
    fn informational_response() {
        // No response with an 1xx status code may contain a body length.
        assert_eq!(&do_response11(false, |mut msg, buf| {
            msg.response_status(buf, 142, "Foo");
            msg.add_length(buf, 500).unwrap_err();
            msg.done_headers(buf).unwrap();
        })[..], "HTTP/1.1 142 Foo\r\n\r\n".as_bytes());
    }
}
