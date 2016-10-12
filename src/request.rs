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

    headers: Vec<(Header, String)>,

    // some known headers
    host: Option<usize>,
    content_type: Option<usize>,
    // add more known headers;
}


impl Request {

    pub fn new(parsed: httparse::Request) -> Request {
        let mut req = Request {
            method: Method::from(parsed.method.unwrap()),
            version: parsed.version.unwrap(),
            path: parsed.path.unwrap().to_string(),

            headers: Vec::with_capacity(MAX_HEADERS),

            host: None,
            content_type: None,
        };
        req.parse_headers(parsed);
        req
    }

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
        let req = Request::new(parser);
        Ok(Async::Ready((req, bytes)))
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

    fn parse_body(&mut self) -> Poll<(), Error> {
        Ok(Async::Ready(()))
    }

    // Public interface

    pub fn new_response(&self) -> Response {
        Response::new(self.version)
    }

    pub fn has_body(&self) -> bool {
        false
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
