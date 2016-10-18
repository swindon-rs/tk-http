use std::io;
use std::convert::From;
use std::str;
use std::str::FromStr;

use httparse;
use netbuf::Buf;
use futures::{Async, Poll};

use super::response::Response;
use super::headers::{Method, Header};
use serve::ResponseConfig;
use {Version};


const MAX_HEADERS: usize = 64;

type Slice = (usize, usize);


/// Request struct
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
    host: Option<usize>,
    content_type: Option<usize>,
    content_length: Option<usize>,
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
            version: Version::from_httparse(parser.version.unwrap()),
            path: parser.path.unwrap().to_string(),
            headers: Vec::with_capacity(MAX_HEADERS),
            body: None,

            host: None,
            content_type: None,
            content_length: None,
        };
        req.parse_headers(parser);
        Ok(Async::Ready((req, bytes)))
    }

    // pub fn parse_body(&mut self, buf: &mut Buf) -> Poll<(), io::Error> {
    //     if let Some(body_size) = self.content_length() {
    //         println!("Must read {} bytes", body_size);
    //     }
    //     Ok(Async::Ready(()))
    // }

    fn parse_headers(&mut self, parser: httparse::Request) {
        for h in parser.headers.iter() {
            let header = Header::from(h.name);
            let value = String::from_utf8_lossy(h.value).into_owned();
            match header {
                Header::Host => {
                    self.host = Some(self.headers.len());
                },
                Header::ContentLength => {
                    // check if value is usize:
                    self.content_length = Some(self.headers.len());
                },
                _ => {},
            }
            self.headers.push((header, value));
        }
    }

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
}


#[derive(Debug)]
pub struct Body {
    pub data: Buf,
}

impl Body {
    pub fn new(buf: Buf) -> Body {
        Body { data: buf }
    }

    pub fn parse_from(request: &Request, buf: &Buf)
        -> Poll<Option<usize>, io::Error>
    {
        if let Some(clen) = request.content_length() {
            if buf.len() >= clen {
                return Ok(Async::Ready(Some(clen)))
            }
        // } else if Some(ctype) = request.content_type() {
        }
        Ok(Async::Ready(None))
    }
}

pub fn response_config(req: &Request) -> ResponseConfig {
    ResponseConfig {
        version: req.version,
        is_head: req.method == Method::Head,
        do_close: true, // TODO(tailhook) close
    }
}
