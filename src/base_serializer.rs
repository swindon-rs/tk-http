//! This contains common part of serializer between client and server
//! implementation

use std::io::Write;
use std::ascii::AsciiExt;

use netbuf::Buf;

use version::Version;

quick_error! {
    #[derive(Debug)]
    pub enum HeaderError {
        DuplicateContentLength {
            description("Content-Length is added twice")
        }
        DuplicateTransferEncoding {
            description("Transfer-Encoding is added twice")
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

#[derive(Debug)]
pub enum MessageState {
    /// Nothing has been sent.
    ResponseStart { version: Version, body: Body, close: bool },
    /// A continuation line has been sent.
    FinalResponseStart { version: Version, body: Body, close: bool },
    /// Nothing has been sent.
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
    Request,
}

/// Represents both request messages and response messages.
///
/// Specific wrappers are exposed in `server` and `client` modules.
/// This type is private for the crate.
pub struct Message(pub Buf, pub MessageState);

impl Message {
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
    pub fn response_status(&mut self, code: u16, reason: &str) {
        use self::Body::*;
        use self::MessageState::*;
        match self.1 {
            ResponseStart { version, mut body, close } |
            FinalResponseStart { version, mut body, close } => {
                // 100 (Continue) interim status code is not allowed as
                // a final response status.
                assert!(code != 100);
                write!(self.0, "{} {} {}\r\n", version, code, reason).unwrap();
                // Responses without body:
                //
                // * 1xx (Informational)
                // * 204 (No Content)
                // * 304 (Not Modified)
                if (code >= 100 && code < 200) || code == 204 || code == 304 {
                    body = Denied
                }
                self.1 = Headers { body: body, close: close };
            }
            ref state => {
                panic!("Called response_status() method on response in state {:?}",
                       state)
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
    pub fn request_line(&mut self, method: &str, path: &str, version: Version)
    {
        use self::Body::*;
        use self::MessageState::*;
        match self.1 {
            RequestStart => {
                write!(self.0, "{} {} {}\r\n", method, path, version).unwrap();
                // All requests may contain a body although it is uncommon for
                // GET and HEAD requests to contain one.
                self.1 = Headers { body: Request, close: false };
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
    pub fn response_continue(&mut self) {
        use self::MessageState::*;
        match self.1 {
            ResponseStart { version, body, close } => {
                write!(self.0, "{} 100 Continue\r\n\r\n", version).unwrap();
                self.1 = FinalResponseStart { version: version,
                                              body: body,
                                              close: close }
            }
            ref state => {
                panic!("Called continue_line() method on response in state {:?}",
                       state)
            }
        }
    }

    fn write_header(&mut self, name: &str, value: &[u8]) {
        self.0.write_all(name.as_bytes()).unwrap();
        self.0.write_all(b": ").unwrap();
        self.0.write_all(value).unwrap();
        self.0.write_all(b"\r\n").unwrap();
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
    pub fn add_header(&mut self, name: &str, value: &[u8])
        -> Result<(), HeaderError>
    {
        use self::MessageState::*;
        use self::HeaderError::*;
        if name.eq_ignore_ascii_case("Content-Length")
            || name.eq_ignore_ascii_case("Transfer-Encoding") {
            return Err(BodyLengthHeader)
        }
        match self.1 {
            Headers { .. } | FixedHeaders { .. } | ChunkedHeaders { .. } => {
                self.write_header(name, value);
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
    pub fn add_length(&mut self, n: u64)
        -> Result<(), HeaderError> {
        use self::MessageState::*;
        use self::HeaderError::*;
        use self::Body::*;
        match self.1 {
            FixedHeaders { .. } => Err(DuplicateContentLength),
            ChunkedHeaders { .. } => Err(ContentLengthAfterTransferEncoding),
            Headers { body: Denied, .. } => Err(RequireBodyless),
            Headers { body, close } => {
                self.write_header("Content-Length",
                                  &n.to_string().into_bytes()[..]);
                self.1 = FixedHeaders { is_head: body == Head,
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
    pub fn add_chunked(&mut self)
        -> Result<(), HeaderError> {
            use self::MessageState::*;
            use self::HeaderError::*;
            use self::Body::*;
            match self.1 {
                FixedHeaders { .. } => Err(TransferEncodingAfterContentLength),
                ChunkedHeaders { .. } => Err(DuplicateTransferEncoding),
                Headers { body: Denied, .. } => Err(RequireBodyless),
                Headers { body, close } => {
                    self.write_header("Transfer-Encoding", b"chunked");
                    self.1 = ChunkedHeaders { is_head: body == Head,
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
        !matches!(self.1,
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
    pub fn done_headers(&mut self) -> Result<bool, HeaderError> {
        use self::Body::*;
        use self::MessageState::*;
        if matches!(self.1,
                    Headers { close: true, .. } |
                    FixedHeaders { close: true, .. } |
                    ChunkedHeaders { close: true, .. }) {
            self.add_header("Connection", b"close").unwrap();
        }
        let expect_body = match self.1 {
            Headers { body: Denied, .. } => {
                self.1 = Bodyless;
                false
            }
            Headers { body: Request, .. } => {
                self.1 = FixedBody { is_head: false, content_length: 0 };
                true
            }
            Headers { body: Normal, .. } => {
                return Err(HeaderError::CantDetermineBodySize);
            }
            FixedHeaders { is_head, content_length, .. } => {
                self.1 = FixedBody { is_head: is_head,
                                     content_length: content_length };
                !is_head
            }
            ChunkedHeaders { is_head, .. } => {
                self.1 = ChunkedBody { is_head: is_head };
                !is_head
            }
            ref state => {
                panic!("Called done_headers() method on  in state {:?}",
                       state)
            }
        };
        self.0.write(b"\r\n").unwrap();
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
    pub fn write_body(&mut self, data: &[u8]) {
        use self::MessageState::*;
        match self.1 {
            Bodyless => panic!("Message must not contain body."),
            FixedBody { is_head, ref mut content_length } => {
                if data.len() as u64 > *content_length {
                    panic!("Fixed size response error. \
                        Bytes left {} but got additional {}",
                        content_length, data.len());
                }
                if !is_head {
                    self.0.write(data).unwrap();
                }
                *content_length -= data.len() as u64;
            }
            ChunkedBody { is_head } => if !is_head && data.len() > 0 {
                write!(self.0, "{:x}\r\n", data.len()).unwrap();
                self.0.write(data).unwrap();
                self.0.write(b"\r\n").unwrap();
            },
            ref state => {
                panic!("Called write_body() method on message \
                    in state {:?}", state)
            }
        }
    }

    /// Returns true if `done()` method is already called-
    pub fn is_complete(&self) -> bool {
        matches!(self.1, MessageState::Done)
    }

    /// Writes needed finalization data into the buffer and asserts
    /// that response is in the appropriate state for that.
    ///
    /// The method may be called multiple times.
    ///
    /// # Panics
    ///
    /// When the message is in the wrong state or the body is not finished.
    pub fn done(&mut self) {
        use self::MessageState::*;
        match self.1 {
            Bodyless => self.1 = Done,
            // Don't check for responses to HEAD requests if body was actually sent.
            FixedBody {is_head: true, .. } |
            ChunkedBody { is_head: true } => self.1 = Done,
            FixedBody { is_head: false, content_length: 0 } => self.1 = Done,
            FixedBody { is_head: false, content_length } =>
                panic!("Tried to close message with {} bytes remaining.",
                       content_length),
            ChunkedBody { is_head: false } => {
                self.0.write(b"0\r\n\r\n").unwrap();
                self.1 = Done;
            }
            Done => {}  // multiple invocations are okay.
            ref state => {
                panic!("Called done() method on response in state {:?}",
                       state);
            }
        }
    }

    pub fn state(self) -> MessageState {
        self.1
    }

    pub fn decompose(self) -> (Buf, MessageState) {
        (self.0, self.1)
    }
}

#[cfg(test)]
mod test {
    use netbuf::Buf;
    use super::{Message, MessageState, Body};
    use version::Version;

    #[test]
    fn message_size() {
        // Just to keep track of size of structure
        assert_eq!(::std::mem::size_of::<MessageState>(), 16);
    }

    fn do_request<F: FnOnce(Message)>(fun: F) -> Buf {
        let mut buf = Buf::new();
        fun(Message(buf, MessageState::RequestStart));
        return buf;
    }
    fn do_response10<F: FnOnce(Message)>(fun: F) -> Buf {
        let mut buf = Buf::new();
        fun(MessageState::ResponseStart {
            version: Version::Http10,
            body: Body::Normal,
            close: false,
        }.with(&mut buf));
        return buf;
    }
    fn do_response11<F: FnOnce(Message)>(close: bool, fun: F) -> Buf {
        let mut buf = Buf::new();
        fun(MessageState::ResponseStart {
            version: Version::Http11,
            body: Body::Normal,
            close: close,
        }.with(&mut buf));
        return buf;
    }

    fn do_head_response11<F: FnOnce(Message)>(close: bool, fun: F) -> Buf {
        let mut buf = Buf::new();
        fun(MessageState::ResponseStart {
            version: Version::Http11,
            body: Body::Head,
            close: close,
        }.with(&mut buf));
        return buf;
    }

    #[test]
    fn minimal_request() {
        assert_eq!(&do_request(|mut msg| {
            msg.request_line("GET", "/", Version::Http10);
            msg.done_headers().unwrap();
            msg.done();
        })[..], "GET / HTTP/1.0\r\n\r\n".as_bytes());
    }

    #[test]
    fn minimal_response() {
        assert_eq!(&do_response10(|mut msg| {
            msg.response_status(200, "OK");
            msg.add_length(0).unwrap();
            msg.done_headers().unwrap();
            msg.done();
        })[..], "HTTP/1.0 200 OK\r\nContent-Length: 0\r\n\r\n".as_bytes());
    }

    #[test]
    fn minimal_response11() {
        assert_eq!(&do_response11(false, |mut msg| {
            msg.response_status(200, "OK");
            msg.add_length(0).unwrap();
            msg.done_headers().unwrap();
            msg.done();
        })[..], "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".as_bytes());
    }

    #[test]
    fn close_response11() {
        assert_eq!(&do_response11(true, |mut msg| {
            msg.response_status(200, "OK");
            msg.add_length(0).unwrap();
            msg.done_headers().unwrap();
            msg.done();
        })[..], concat!("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n",
                        "Connection: close\r\n\r\n").as_bytes());
    }

    #[test]
    fn head_request() {
        assert_eq!(&do_request(|mut msg| {
            msg.request_line("HEAD", "/", Version::Http11);
            msg.add_length(5).unwrap();
            msg.done_headers().unwrap();
            msg.write_body(b"Hello");
            msg.done();
        })[..], "HEAD / HTTP/1.1\r\nContent-Length: 5\r\n\r\nHello".as_bytes());
    }

    #[test]
    fn head_response() {
        // The response to a HEAD request may contain the real body length.
        assert_eq!(&do_head_response11(false, |mut msg| {
            msg.response_status(200, "OK");
            msg.add_length(500).unwrap();
            msg.done_headers().unwrap();
            msg.done();
        })[..], "HTTP/1.1 200 OK\r\nContent-Length: 500\r\n\r\n".as_bytes());
    }

    #[test]
    fn informational_response() {
        // No response with an 1xx status code may contain a body length.
        assert_eq!(&do_response11(false, |mut msg| {
            msg.response_status(142, "Foo");
            msg.add_length(500).unwrap_err();
            msg.done_headers().unwrap();
            msg.done();
        })[..], "HTTP/1.1 142 Foo\r\n\r\n".as_bytes());
    }
}
