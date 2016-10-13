use std::io;
use std::convert::From;
use std::str;

use httparse;
use netbuf::Buf;
use futures::{Async, Poll};

use super::error::Error;
use super::response::Response;

use super::headers::{Method, Header};


const MAX_HEADERS: usize = 64;

type Slice = (usize, usize);


/// Request struct
///
/// some known headers may be moved to upper structure (ie, Host)
#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: u8,

    pub headers: Vec<(Header, String)>,

    pub body: Option<Body>,

    // some known headers
    host: Option<usize>,
    content_type: Option<usize>,
    // add more known headers;
}


impl Request {

    pub fn parse_from(buf: &Buf) -> Poll<(Request,usize), io::Error> {
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
                return Err(io::Error::new(
                    io::ErrorKind::Other, e.to_string()));
            },
        };
        let mut req = Request {
            method: Method::from(parser.method.unwrap()),
            version: parser.version.unwrap(),
            path: parser.path.unwrap().to_string(),
            headers: Vec::with_capacity(MAX_HEADERS),
            body: None,

            host: None,
            content_type: None,
        };
        req.parse_headers(parser);
        Ok(Async::Ready((req, bytes)))
    }

    pub fn parse_body<R: io::Read>(&mut self, socket: &mut R)
        -> Poll<(), io::Error>
    {
        Ok(Async::Ready(()))
    }

    fn parse_headers(&mut self, parser: httparse::Request) {
        for h in parser.headers.iter() {
            let header = Header::from(h.name);
            let value = String::from_utf8_lossy(h.value).into_owned();
            match header {
                Header::Host => {
                    self.host = Some(self.headers.len());
                },
                _ => {},
            }
            self.headers.push((header, value));
        }
    }

    // Public interface

    pub fn new_response(&self) -> Response {
        Response::new(self.version)
    }

    pub fn has_body(&self) -> bool {
        self.body.is_some()
    }

    /// Value of Host header
    pub fn host(&self) -> Option<&str> {
        match self.host {
            Some(s) => Some(self.headers[s].1.as_ref()),
            None => None,
        }
    }

    /// Value of Content-Type header
    pub fn content_type(&self) -> Option<&str> {
        match self.content_type {
            Some(s) => Some(self.headers[s].1.as_ref()),
            None => None,
        }
    }

    // interface to body

    /// Read request body into buffer.
    pub fn read_body(&mut self, buf: &mut [u8]) -> Poll<usize, Error> {
        // this must/should be hooked to underlying tcp stream
        Ok(Async::NotReady)
    }
}


#[derive(Debug)]
pub struct Body {
    data: Buf,
}

impl Body {
    pub fn new(buf: Buf) -> Body {
        Body { data: buf }
    }

    pub fn parse_from<R: io::Read>(&mut self, socket: &mut R) {
        self.data.read_from(socket);
        self.parse();
    }

    fn parse(&mut self) {
    }
}
