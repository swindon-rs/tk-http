use std::io;
use std::fmt;

use bytes::buf::BlockBuf;
use httparse;
use tokio_proto::Parse;
use tokio_proto::pipeline::Frame;

use super::response::Response;

/// Enum representing HTTP request methods.
///
/// ```rust
/// match req.method {
///     Method::GET => {},   // handle GET
///     Method::POST => {},  // handle POST requests
///     Method::Other(m) => { println!("Custom method {}", m); },
///     _ => {}
///     }
/// ```
#[derive(Debug)]
pub enum Method {
    DELETE,
    HEAD,
    GET,
    OPTIONS,
    PATCH,
    POST,
    PUT,
    Other(String),
}

/// Enum reprsenting HTTP version.
#[derive(Debug, Clone)]
pub enum Version {
    HTTP10,
    HTTP11,
}

/// Request struct
#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: Version,

    // TODO: implement
    // headers: Vec<(str, str)>,

    // data: &'a str,
}

pub struct Parser;


impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Version::HTTP10 => f.write_str("HTTP/1.0"),
            Version::HTTP11 => f.write_str("HTTP/1.1"),
        }
    }
}

impl Request {

    pub fn new_response(&self) -> Response {
        Response::new(self.version.clone())
    }
}

impl Parse for Parser {
    type Out = Frame<Request, (), io::Error>;

    fn parse(&mut self, buf: &mut BlockBuf) -> Option<Self::Out> {
        // Only compact if needed
        if !buf.is_compact() {
            buf.compact();
        }

        let mut n = 0;

        let res = {
            // TODO: we should grow this headers array if parsing fails and asks for
            //       more headers
            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut r = httparse::Request::new(&mut headers);
            let status = match r.parse(buf.bytes().expect("buffer not compact")) {
                Ok(status) => status,
                Err(e) => {
                    println!("Got error: {}", e);
                    return Some(Frame::Error(
                        io::Error::new(
                            io::ErrorKind::Other,
                            format!("failed to parse http request: {}", e)
                        )));
                }
            };

            match status {
                httparse::Status::Complete(amt) => {
                    n = amt;
                    let method = match r.method.unwrap().to_uppercase().as_str() {
                        "GET" => Method::GET,
                        "HEAD" => Method::HEAD,
                        "POST" => Method::POST,
                        "PUT" => Method::PUT,
                        "DELETE" => Method::DELETE,
                        m => Method::Other(m.to_string()),
                    };
                    let version = match r.version.unwrap() {
                        0 => Version::HTTP10,
                        _ => Version::HTTP11,
                    };

                    Some(Frame::Message(Request {
                        method: method,
                        path: r.path.unwrap().to_string(),
                        version: version,
                        //headers: r.headers
                        //    .iter()
                        //    .map(|h| (toslice(h.name.as_bytes()), toslice(h.value)))
                        //    .collect(),
                        //data: None,
                    }))
                }
                httparse::Status::Partial => {
                    None
                }
            }
        };

        match res {
            Some(Frame::Message(_)) => {
                buf.shift(n);
            }
            _ => {}
        };
        res
    }

    fn done(&mut self, buf: &mut BlockBuf) -> Option<Self::Out> {
        Some(Frame::Done)
        // TODO: must check if request body is fully read;
    }
}
