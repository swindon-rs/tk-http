use std::mem;
use std::convert::From;
use std::str;
use std::net::SocketAddr;

use httparse;
use netbuf::Buf;
use futures::{Async, Poll};

use super::enums::{Method, Header};
use serve::ResponseConfig;
use {Version, Error};


/// Number of headers to allocate on stack
const MIN_HEADERS: usize = 16;
/// A hard limit on the number of headers
const MAX_HEADERS: usize = 1024;


/// Request struct.
///
/// some known headers may be moved to upper structure (ie, Host)
#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: Version,

    pub headers: Vec<(Header, String)>,

    pub body: Option<Body>,
    pub peer_addr: SocketAddr,

    // some known headers
    connection_close: bool,
    // TODO: get rid of this crap;
    //      must implement proper Headers structure.
    host: Option<String>,
    content_type: Option<usize>,
    content_length: Option<u64>,
    transfer_encoding: Option<String>,
    // add more known headers;
}


impl Request {

    pub fn parse_from(buf: &Buf, peer_addr: &SocketAddr)
        -> Poll<(Request, usize, BodyKind), Error>
    {
        let mut headers = [httparse::EMPTY_HEADER; MIN_HEADERS];
        let mut vec;
        let mut parser = httparse::Request::new(&mut headers);
        let mut result = parser.parse(&buf[..]);
        if matches!(result, Err(httparse::Error::TooManyHeaders)) {
            vec = vec![httparse::EMPTY_HEADER; MAX_HEADERS];
            parser = httparse::Request::new(&mut vec);
            result = parser.parse(&buf[..]);
        }
        let bytes = match result {
            Ok(httparse::Status::Complete(bytes)) => {
                bytes
            },
            Ok(httparse::Status::Partial) => {
                return Ok(Async::NotReady);
            },
            Err(e) => {
                return Err(e.into());
            },
        };
        let ver = Version::from_httparse(parser.version.unwrap());
        let mut req = Request {
            method: Method::from(parser.method.unwrap()),
            version: ver,
            path: parser.path.unwrap().to_string(),
            headers: Vec::with_capacity(parser.headers.len()),
            body: None,
            peer_addr: peer_addr.clone(),

            connection_close: ver != Version::Http11,
            host: None,
            content_type: None,
            content_length: None,
            transfer_encoding: None,
        };
        let body_kind = req.parse_headers(parser);
        Ok(Async::Ready((req, bytes, body_kind)))
    }

    fn parse_headers(&mut self, parser: httparse::Request) -> BodyKind {
        // TODO(tailhook) if there is no content_length, we chould check
        // transfer encoding, method and otherwise fail, instead of just
        // blindly assuming body is empty

        // TODO(popravich) revise body detection
        // see  http://httpwg.github.io/specs/rfc7230.html#message.body.length
        //      rotor-http/parser.rs#L86-L120
        use super::enums::headers;
        let mut body_kind = BodyKind::WithoutBody;
        let mut has_content_length = false;

        for h in parser.headers.iter() {
            match Header::from(h.name) {
                Header::Host => {
                    // TODO: check that hostname is valid (idn)
                    self.host = Some(
                        String::from_utf8_lossy(h.value).into_owned());
                }
                Header::Connection => {
                    if headers::is_close(h.value) {
                        self.connection_close = true;
                    }
                    let value = String::from_utf8_lossy(h.value).into_owned();
                    self.headers.push((Header::Connection, value));
                }
                Header::ContentLength => {
                    if has_content_length {
                        // TODO: replace with proper error;
                        panic!("duplicate content length")
                    }
                    has_content_length = true;
                    if let Some(size) = headers::content_length(h.value) {
                        self.content_length = Some(size);
                        if body_kind != BodyKind::Chunked {
                            body_kind = BodyKind::Fixed(size as usize); // XXX
                        }
                    }
                }
                Header::TransferEncoding => {
                    if headers::is_chunked(h.value) {
                        if has_content_length {
                            self.connection_close = true;
                        }
                        body_kind = BodyKind::Chunked;
                    } else {
                        // TODO: 400 Bad request;
                    }
                    let value = String::from_utf8_lossy(h.value).into_owned();
                    self.transfer_encoding = Some(value);
                }
                header => {
                    // TODO: store original bytes not converted to string
                    //      (especially 'lossy')
                    let value = String::from_utf8_lossy(h.value).into_owned();
                    self.headers.push((header, value));
                }
            }
        }
        body_kind
    }

    // Public interface

    pub fn has_body(&self) -> bool {
        self.body.is_some()
    }

    /// Value of Host header
    pub fn host<'a>(&'a self) -> Option<&'a str> {
        match self.host {
            Some(ref s) => Some(s.as_ref()),
            None => None,
        }
    }

    /// Value of Content-Type header
    pub fn content_type(&self) -> Option<&str> {
        match self.content_type {
            Some(idx) => Some(self.headers[idx].1.as_ref()),
            None => None,
        }
    }

    /// Value of Content-Length header
    pub fn content_length(&self) -> Option<u64> {
        self.content_length.map(|x| x.clone())
    }

    /// Value of Transfer-Encoding header
    pub fn transfer_encoding(&self) -> Option<&str> {
        match self.transfer_encoding {
            Some(ref s) => Some(s.as_ref()),
            None => None,
        }
    }
}


#[derive(Debug)]
pub struct Body {
    pub data: Buf,
}

#[derive(PartialEq)]
pub enum BodyKind {
    Fixed(usize),
    Chunked,
    WithoutBody,
}


impl Body {
    pub fn new(buf: Buf) -> Body {
        Body { data: buf }
    }
}

pub fn response_config(req: &Request) -> ResponseConfig {
    ResponseConfig {
        version: req.version,
        is_head: req.method == Method::Head,
        do_close: req.connection_close,
    }
}


#[derive(Debug)]
enum ParseState {
    /// No requests parsed yet.
    Idle,
    /// Parsing Request head.
    Request,
    /// Parsing fixed-size body.
    FixedBody { size: usize },
    /// Parsing chunked body.
    ChunkedBody { end: usize },
    /// Without body.
    WithoutBody,
    /// Request & Body are parsed.
    Done,
}

pub struct RequestParser(ParseState, Option<Request>);

impl RequestParser {

    pub fn new() -> RequestParser {
        RequestParser (ParseState::Idle, None)
    }

    pub fn parse_from(&mut self, buf: &mut Buf, peer_addr: &SocketAddr)
        -> Result<bool, Error>
    {
        use self::ParseState::*;
        loop {  // transition through states until result is reached;
            match self.0 {
                Idle => {
                    self.0 = Request;
                },
                Request => {
                    match try!(self::Request::parse_from(buf, peer_addr)) {
                        Async::NotReady => break,
                        Async::Ready((req, size, body_kind)) => {
                            self.1 = Some(req);
                            buf.consume(size);
                            self.0 = match body_kind {
                                BodyKind::Fixed(size) => {
                                    FixedBody { size: size }
                                }
                                BodyKind::Chunked => {
                                    ChunkedBody { end: 0 }
                                }
                                BodyKind::WithoutBody => WithoutBody,
                            }
                        }
                    }
                },
                FixedBody { size } => {
                    if buf.len() >= size {
                        let mut req = self.1.as_mut().unwrap();
                        let mut bbuf = buf.split_off(size);
                        mem::swap(&mut bbuf, buf);
                        req.body = Some(Body::new(bbuf));
                        self.0 = ParseState::Done;
                    } else {
                        break;
                    }
                },
                ChunkedBody { mut end } => {
                    if end > buf.len() {
                        break;
                    }
                    let res = httparse::parse_chunk_size(&buf[end..]);
                    let (off, size) = match res {
                        Ok(httparse::Status::Complete(res)) => res,
                        Ok(httparse::Status::Partial) => {
                            break;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    };
                    buf.remove_range(end .. end + off as usize);
                    end += size as usize;
                    if size == 0 {
                        let mut req = self.1.as_mut().unwrap();
                        let tail = buf.split_off(end);
                        let bbuf = mem::replace(buf, tail);
                        req.body = Some(Body::new(bbuf));
                        self.0 = ParseState::Done;
                    }
                },
                WithoutBody => {
                    self.0 = ParseState::Done;
                },
                Done => {
                    return Ok(true)
                }
            }
        }
        Ok(false)
    }

    pub fn take(&mut self) -> Option<Request> {
        match self.0 {
            ParseState::Done => {
                self.0 = ParseState::Idle;
                self.1.take()
            },
            ref state => panic!("Incomplete parse state {:?}", state),
        }
    }
}
