use std::mem;
use std::convert::From;
use std::str;
use std::str::FromStr;
use std::ascii::AsciiExt;

use httparse;
use netbuf::Buf;
use futures::{Async, Poll};

use super::enums::{Method, Header};
use serve::ResponseConfig;
use {Version, Error};


const MAX_HEADERS: usize = 64;


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

    // some known headers
    connection_close: bool,
    host: Option<usize>,
    content_type: Option<usize>,
    content_length: Option<usize>,
    transfer_encoding: Option<usize>,
    // add more known headers;
}


impl Request {

    pub fn parse_from(buf: &Buf) -> Poll<(Request,usize,BodyKind), Error> {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut parser = httparse::Request::new(&mut headers);
        let bytes = match parser.parse(&buf[..]) {
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
            headers: Vec::with_capacity(MAX_HEADERS),
            body: None,

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
        let mut body_kind = BodyKind::WithoutBody;

        for h in parser.headers.iter() {
            let header = Header::from(h.name);
            let value = String::from_utf8_lossy(h.value).into_owned();
            match header {
                Header::Host => {
                    self.host = Some(self.headers.len());
                },
                Header::Connection => {
                    if value.split(',')
                        .any(|token| token.eq_ignore_ascii_case("close"))
                    {
                        self.connection_close = true;
                    }
                },
                Header::ContentLength => {
                    // check if value is usize:
                    self.content_length = Some(self.headers.len());
                    match usize::from_str(value.as_str()) {
                        Ok(size) => {
                            if body_kind != BodyKind::Chunked {
                                body_kind = BodyKind::Fixed(size);
                            }
                        },
                        _ => {},
                    }
                },
                Header::TransferEncoding => {
                    self.transfer_encoding = Some(self.headers.len());
                    match value.split(|c| c == ',').last() {
                        Some("chunked") => {
                            body_kind = BodyKind::Chunked;
                        }
                        _ => {},
                    }
                    // is chunked
                }
                _ => {},
            }
            self.headers.push((header, value));
        }
        body_kind
    }

    // Public interface

    pub fn has_body(&self) -> bool {
        self.body.is_some()
    }

    /// Value of Host header
    pub fn host(&self) -> Option<&str> {
        match self.host {
            Some(idx) => Some(self.headers[idx].1.as_ref()),
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
    pub fn content_length(&self) -> Option<usize> {
        match self.content_length {
            None => None,
            Some(idx) => {
                match usize::from_str(self.headers[idx].1.as_ref()) {
                    Ok(size) => Some(size),
                    Err(_) => None,
                }
            },
        }
    }

    /// Value of Transfer-Encoding header
    pub fn transfer_encoding(&self) -> Option<&str> {
        match self.transfer_encoding {
            None => None,
            Some(idx) => {
                Some(self.headers[idx].1.as_ref())
            }
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
    ChunkedBody { next_chunk_offset: usize },
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

    pub fn parse_from(&mut self, buf: &mut Buf) -> Result<bool, Error> {
        loop {  // transition through states until result is reached;
            match self.0 {
                ParseState::Idle => {
                    self.0 = ParseState::Request;
                },
                ParseState::Request => {
                    match try!(Request::parse_from(buf)) {
                        Async::NotReady => break,
                        Async::Ready((req, size, body_kind)) => {
                            buf.consume(size);
                            self.1 = Some(req);
                            self.0 = match body_kind {
                                BodyKind::Fixed(size) => ParseState::FixedBody{size: size},
                                BodyKind::Chunked => ParseState::ChunkedBody{next_chunk_offset: 0},
                                BodyKind::WithoutBody => ParseState::WithoutBody,
                            };
                        },
                    }
                },
                ParseState::FixedBody { size } => {
                    if buf.len() >= size {
                        let mut req = self.1.take().unwrap();
                        let mut bbuf = buf.split_off(size);
                        mem::swap(&mut bbuf, buf);
                        req.body = Some(Body::new(bbuf));
                        self.1 = Some(req);
                        self.0 = ParseState::Done;
                    } else {
                        break;
                    }
                },
                ParseState::ChunkedBody { mut next_chunk_offset } => {
                    if next_chunk_offset > buf.len() {
                        break;
                    }
                    let (off, size) = match httparse::parse_chunk_size(&buf[next_chunk_offset..]) {
                        Ok(httparse::Status::Complete(res)) => res,
                        Ok(httparse::Status::Partial) => {
                            break;
                        },
                        Err(e) => {
                            return Err(e.into());
                        },
                    };
                    buf.remove_range(next_chunk_offset .. next_chunk_offset + off as usize);
                    next_chunk_offset += size as usize;
                    if size == 0 {
                        let mut req = self.1.take().unwrap();
                        let tail = buf.split_off(next_chunk_offset);
                        let bbuf = mem::replace(buf, tail);
                        req.body = Some(Body::new(bbuf));
                        self.1 = Some(req);
                        self.0 = ParseState::Done;
                    }
                },
                ParseState::WithoutBody => {
                    self.0 = ParseState::Done;
                },
                ParseState::Done => {
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
