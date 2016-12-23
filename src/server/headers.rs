use std::mem;
use std::str::from_utf8;
use std::ascii::AsciiExt;
use std::borrow::Cow;

use httparse::{self, EMPTY_HEADER, Request, Header, parse_chunk_size};
use tokio_core::io::Io;
use tk_bufstream::Buf;

use super::{Error, RequestTarget, Dispatcher};
use super::codec::BodyKind;
use super::encoder::ResponseConfig;
use headers;
use {Version};


/// Number of headers to allocate on a stack
const MIN_HEADERS: usize = 16;
/// A hard limit on the number of headers
const MAX_HEADERS: usize = 1024;


struct RequestConfig<'a> {
    body: BodyKind,
    is_head: bool,
    expect_continue: bool,
    connection_close: bool,
    connection: Option<Cow<'a, str>>,
    host: Option<&'a str>,
    target: RequestTarget<'a>,
    /// If this is true, then Host header differs from host value in
    /// request-target (first line). Note, specification allows throwing
    /// the header value by proxy in this case. But you might consider
    /// returning 400 Bad Request.
    conflicting_host: bool,
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
    method: &'a str,
    raw_target: &'a str,
    target: RequestTarget<'a>,
    host: Option<&'a str>,
    conflicting_host: bool,
    version: Version,
    headers: &'a [Header<'a>],
    body_kind: BodyKind,
    connection_close: bool,
    connection_header: Option<Cow<'a, str>>,
}

impl<'a> Head<'a> {
    pub fn method(&self) -> &str {
        self.method
    }
    pub fn raw_request_target(&self) -> &str {
        self.raw_target
    }
    /// Request-target (the middle part of the first line of request)
    pub fn request_target(&self) -> &RequestTarget<'a> {
        &self.target
    }
    /// Returns path portion of request uri
    ///
    /// Note: this may return something not starting from a slash when
    /// full uri is used as request-target
    ///
    /// If the request target is in asterisk form this returns None
    pub fn path(&self) -> Option<&str> {
        use super::RequestTarget::*;
        match self.target {
            Origin(x) => Some(x),
            Absolute { path, .. } => Some(path),
            Authority(..) => None,
            Asterisk => None,
        }
    }
    /// Return host of a request
    ///
    /// Note: this might be extracted from request-target portion of
    /// request headers (first line).
    ///
    /// If both `Host` header exists and doesn't match host in request-target
    /// then this method returns host from request-target and
    /// `has_conflicting_host()` method returns true.
    pub fn host(&self) -> Option<&str> {
        self.host
    }
    /// Returns true
    pub fn has_conflicting_host(&self) -> bool {
        self.conflicting_host
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn headers(&self) -> &'a [Header<'a>] {
        self.headers
    }
    pub fn connection_close(&self) -> bool {
        self.connection_close
    }
    pub fn connection_header(&self) -> Option<&Cow<'a, str>> {
        self.connection_header.as_ref()
    }
}


fn scan_headers<'x>(raw_request: &'x Request)
    -> Result<RequestConfig<'x>, Error>
{
    // Implements the body length algorithm for requests:
    // http://httpwg.github.io/specs/rfc7230.html#message.body.length
    //
    // The length of a request body is determined by one of the following
    // (in order of precedence):
    //
    // 1. If the request contains a valid `Transfer-Encoding` header
    //    with `chunked` as the last encoding the request is chunked
    //    (3rd option in RFC).
    // 2. If the request contains a valid `Content-Length` header
    //    the request has the given length in octets
    //    (5th option in RFC).
    // 3. If neither `Transfer-Encoding` nor `Content-Length` are
    //    present the request has an empty body
    //    (6th option in RFC).
    // 4. In all other cases the request is a bad request.
    use super::codec::BodyKind::*;
    use super::Error::*;
    use super::RequestTarget::*;

    let is_head = raw_request.method.unwrap() == "HEAD";
    let mut has_content_length = false;
    let mut close = raw_request.version.unwrap() == 0;
    let mut expect_continue = false;
    let mut body = Fixed(0);
    let mut connection = None::<Cow<_>>;
    let mut host_header = false;
    let target = RequestTarget::parse(raw_request.path.unwrap())
        .ok_or(BadRequestTarget)?;
    let mut conflicting_host = false;
    let mut host = match target {
        RequestTarget::Authority(x) => Some(x),
        RequestTarget::Absolute { authority, .. } => Some(authority),
        _ => None,
    };
    for header in raw_request.headers.iter() {
        if header.name.eq_ignore_ascii_case("Transfer-Encoding") {
            if let Some(enc) = header.value.split(|&x| x == b',').last() {
                if headers::is_chunked(enc) {
                    if has_content_length {
                        // override but don't allow keep-alive
                        close = true;
                    }
                    body = Chunked;
                }
            }
        } else if header.name.eq_ignore_ascii_case("Content-Length") {
            if has_content_length {
                // duplicate content_length
                return Err(DuplicateContentLength);
            }
            has_content_length = true;
            if body != Chunked {
                let s = from_utf8(header.value)
                    .map_err(|_| ContentLengthInvalid)?;
                let len = s.parse().map_err(|_| ContentLengthInvalid)?;
                body = Fixed(len);
            } else {
                // transfer-encoding has preference and don't allow keep-alive
                close = true;
            }
        } else if header.name.eq_ignore_ascii_case("Connection") {
            let strconn = from_utf8(header.value)
                .map_err(|_| ConnectionInvalid)?.trim();
            connection = match connection {
                Some(x) => Some(x + ", " + strconn),
                None => Some(strconn.into()),
            };
            // TODO(tailhook) capture connection header(s) itself
            if header.value.split(|&x| x == b',').any(headers::is_close) {
                close = true;
            }
        } else if header.name.eq_ignore_ascii_case("Host") {
            if host_header {
                return Err(DuplicateHost);
            }
            host_header = true;
            let strhost = from_utf8(header.value)
                .map_err(|_| HostInvalid)?.trim();
            if host.is_none() {  // if host is not in uri
                // TODO(tailhook) additional validations for host
                host = Some(strhost);
            } else if host != Some(strhost) {
                conflicting_host = true;
            }
        } else if header.name.eq_ignore_ascii_case("Expect") {
            if headers::is_continue(header.value) {
                expect_continue = true;
            }
        }
    }
    Ok(RequestConfig {
        body: body,
        is_head: is_head,
        expect_continue: expect_continue,
        connection: connection,
        host: host,
        target: target,
        connection_close: close,
        conflicting_host: conflicting_host,
    })
}

fn parse_headers<S, D>(buffer: &mut Buf, disp: &mut D)
    -> Result<Option<(D::Codec, ResponseConfig)>, Error>
    where S: Io,
          D: Dispatcher<S>,
{
    let (codec, cfg, bytes) = {
        let mut vec;
        let mut headers = [EMPTY_HEADER; MIN_HEADERS];

        let mut raw = Request::new(&mut headers);
        let mut result = raw.parse(&buffer[..]);
        if matches!(result, Err(httparse::Error::TooManyHeaders)) {
            vec = vec![EMPTY_HEADER; MAX_HEADERS];
            raw = Request::new(&mut vec);
            result = raw.parse(&buffer[..]);
        }
        match result? {
            httparse::Status::Complete(bytes) => {
                let cfg = scan_headers(&raw)?;
                let ver = raw.version.unwrap();
                let head = Head {
                    method: raw.method.unwrap(),
                    raw_target: raw.path.unwrap(),
                    target: cfg.target,
                    version: if ver == 1
                        { Version::Http11 } else { Version::Http10 },
                    host: cfg.host,
                    conflicting_host: cfg.conflicting_host,
                    headers: raw.headers,
                    body_kind: cfg.body,
                    // For HTTP/1.0 we could implement
                    // Connection: Keep-Alive but hopefully it's rare
                    // enough to ignore nowadays
                    connection_close: cfg.connection_close || ver == 0,
                    connection_header: cfg.connection,
                };
                let codec = disp.headers_received(&head)?;
                let response_config = ResponseConfig::from(&head);
                (codec, response_config, bytes)
            }
            _ => return Ok(None),
        }
    };
    buffer.consume(bytes);
    Ok(Some((codec, cfg)))
}
