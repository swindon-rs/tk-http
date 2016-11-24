use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::clone::Clone;
use std::mem;

use futures::{Future, BoxFuture};
use futures::finished;
use tokio_core::reactor::Handle;
use tokio_curl::Session;
use curl::easy::{Easy, List};
use netbuf::Buf;
use httparse;

use super::enums::{Method, Header, Version, Status};
use super::enums::headers;


/// HttpClient based on tokio-curl.
///
pub struct HttpClient(Session, ClientState, Option<Easy>);


// TODO: replace by minihttp::base_serializer::Message;
enum ClientState {
    RequestStart,
    Headers { headers: List, body: Body, is_head: bool },
    FixedBody { is_head: bool },
    ChunkedBody { is_head: bool  },
    Bodyless { is_head: bool  },
    Done,
}

enum Body {
    Fixed,
    Chunked,
    NoBody,
}

impl HttpClient {
    pub fn new(handle: Handle) -> HttpClient {
        HttpClient(Session::new(handle), ClientState::RequestStart, None)
    }

    /// Set request method and URL.
    pub fn request(&mut self, method: Method, url: &str) {
        use self::ClientState::*;
        match self.1 {
            RequestStart => {
                let mut curl = Easy::new();
                let mut is_head = false;
                match method {
                    Method::Get => curl.get(true).unwrap(),
                    Method::Post => curl.post(true).unwrap(),
                    Method::Head => {
                        is_head = true;
                        curl.nobody(true).unwrap();
                        curl.custom_request("HEAD").unwrap();
                    }
                    Method::Patch => curl.custom_request("PATCH").unwrap(),
                    Method::Put => curl.custom_request("PUT").unwrap(),
                    Method::Delete => curl.custom_request("DELETE").unwrap(),
                    Method::Other(m) => {
                        curl.custom_request(m.as_str()).unwrap();
                    }
                    m => panic!("Method {:?} not implemented", m)
                };
                curl.url(url).unwrap();
                self.1 = Headers {
                    headers: List::new(),
                    body: Body::NoBody,
                    is_head: is_head,
                };
                self.2 = Some(curl);
            }
            _ => panic!("Called request() method in wrong state")
        }
    }

    /// Add Request Header.
    pub fn add_header(&mut self, header: Header, value: &str) {
        use self::ClientState::*;
        if header == Header::ContentLength {
            self.add_length(value.parse().unwrap());
            return
        } else if header == Header::TransferEncoding {
            assert_eq!(value, "chunked");
            self.add_chunked();
            return
        }
        match self.1 {
            Headers { ref mut headers, .. } => {
                let name = match header {
                    Header::Host => "Host",
                    Header::Raw(ref name) => name,
                    _ => return
                };
                headers.append(
                    format!("{}: {}", name, value).as_str()).unwrap();
            }
            _ => panic!("Called add_header() method in wrong state")
        }
    }

    /// Add Content-Length header and expect fixed-size body.
    ///
    /// # Panics
    ///
    pub fn add_length(&mut self, size: u64) {
        use self::ClientState::*;
        use self::Body::*;
        match self.1 {
            Headers { ref mut headers, ref mut body, .. } => {
                match body {
                    &mut NoBody => {
                        headers.append(
                            format!("Content-Length: {}", size).as_str())
                            .unwrap();
                        *body = Fixed;
                    }
                    &mut Fixed => panic!("Body length already set"),
                    &mut Chunked => panic!("Chunked body expected"),
                }
            }
            _ => panic!("Called add_length() method in wrong state")
        };
    }


    pub fn add_chunked(&mut self) {
        use self::ClientState::*;
        use self::Body::*;
        match self.1 {
            Headers { ref mut headers, ref mut body, .. } => {
                match body {
                    &mut NoBody => {
                        headers.append("Transfer-Encoding: chunked").unwrap();
                        *body = Chunked;
                    }
                    &mut Fixed => panic!("Fixed body expected"),
                    &mut Chunked => panic!("Chunked already set"),
                }
            }
            _ => panic!("Called add_chunked() method in wrong state")
        }
    }

    /// Finish writing headers
    pub fn done_headers(&mut self) {
        use self::ClientState::*;
        let next = match self.1 {
            Headers { ref body, is_head, .. } => {
                match body {
                    &Body::Fixed => {
                        FixedBody { is_head: is_head }
                    }
                    &Body::Chunked => {
                        ChunkedBody { is_head: is_head }
                    }
                    &Body::NoBody => {
                        Bodyless { is_head: is_head }
                    }
                }
            }
            _ => panic!("Called done_headers() method in wrong state")
        };
        match mem::replace(&mut self.1, next) {
            Headers { headers, .. } => {
                let mut curl = self.2.as_mut().unwrap();
                curl.http_headers(headers).unwrap();
            }
            _ => {}
        }
    }

    /// Set request body from netbuf::Buf.
    pub fn body_from_buf(&mut self, mut body: Buf) {
        use self::ClientState::*;
        match self.1 {
            FixedBody { .. } => {
                let mut curl = self.2.as_mut().unwrap();
                curl.read_function(move |mut buf| {
                    let bytes = buf.write(&body[..]).expect("Body written");
                    body.consume(bytes);
                    Ok(bytes)
                }).unwrap()
            }
            ChunkedBody { .. } => {
                let mut curl = self.2.as_mut().unwrap();
                curl.read_function(move |mut buf| {
                    let a = buf.write(
                        format!("{}\r\n", body.len()).as_bytes())
                        .expect("Chunk length written");
                    let b = buf.write(&body[..])
                        .expect("Chunk body written");
                    body.consume(b);
                    Ok(a + b)
                }).unwrap()
            }
            _ => panic!("Called body_from_buf() method in wrong state")
        }
    }

    /// Write body
    pub fn write_body(&mut self, body: &[u8]) {
        use self::ClientState::*;
        match self.1 {
            FixedBody { .. } | ChunkedBody { .. } => {
                let mut curl = self.2.as_mut().unwrap();
                curl.post_fields_copy(body).unwrap();
            }
            _ => panic!("Called write_body() method in wrong state")
        }
    }

    /// Finish writing request. Execute it.
    pub fn done(&mut self) -> BoxFuture<Response, io::Error> {
        use self::ClientState::*;
        let is_head = match self.1 {
            FixedBody { is_head } |
            ChunkedBody { is_head } |
            Bodyless { is_head } => {
                self.1 = Done;
                is_head
            }
            _ => panic!("Called done() method in wrong state")
        };

        let resp_body = Arc::new(Mutex::new(Buf::new()));
        let headers = resp_body.clone();

        let mut curl = self.2.take().unwrap();
        // NOTE: close connections because of curl bug;
        curl.forbid_reuse(true).unwrap();

        curl.header_function(move |line| {
            headers.lock().unwrap()
            .write(line)
            .map(|_| true)
            .unwrap_or(false)
        }).unwrap();

        if !is_head {
            // TODO: create buf for body
            let body = resp_body.clone();
            curl.write_function(move |buf| {
                body.lock().unwrap()
                .write(buf)
                .map_err(|e| panic!("Can't write body: {:?}", e))
            }).unwrap();
        }

        self.0.perform(curl)
        .map_err(|e| e.into_error())
        .and_then(move |_resp| {
            // Response must collect headers and body
            let resp = parse_response(&mut resp_body.lock().unwrap());
            finished(resp)
        }).boxed()
    }
}

impl Clone for HttpClient {
    fn clone(&self) -> Self {
        HttpClient(self.0.clone(), ClientState::RequestStart, None)
    }
}

#[derive(Debug)]
pub struct Response {
    pub version: Version,
    pub status: Status,
    pub reason: String,
    pub headers: Vec<(Header, String)>,
    pub body: Option<Buf>,

    content_length: Option<u64>,
}


impl Response {
    pub fn content_length(&self) -> Option<u64> {
        self.content_length.map(|x| x.clone())
    }
}


// TODO: move this parsing to single Request / Response parser;

fn parse_response(buf: &mut Buf)
    -> Response
{
    use httparse::Status::*;
    use httparse::Error::TooManyHeaders;

    // TODO: parse this properly
    if matches!(&buf[..25], b"HTTP/1.1 100 Continue\r\n\r\n") {
        buf.consume(25);
    }

    let (mut response, size, kind) = {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut vec;
        let mut parser = httparse::Response::new(&mut headers);
        let mut result = parser.parse(&buf[..]);
        if matches!(result, Err(TooManyHeaders)) {
            vec = vec![httparse::EMPTY_HEADER; 1024];
            parser = httparse::Response::new(&mut vec);
            result = parser.parse(&buf[..]);
        }
        if let Ok(Complete(bytes)) = result {
            let mut response = Response {
                version: Version::from_httparse(parser.version.unwrap()),
                status: Status::from(parser.code.unwrap()).unwrap(),
                reason: parser.reason.unwrap().to_string(),
                headers: Vec::with_capacity(parser.headers.len()),
                body: None,

                content_length: None,
            };
            // TODO: parse headers & body
            let body_kind = parse_headers(&parser, &mut response);
            (response, bytes, body_kind)
        } else {
            panic!("Expected complete response");
        }
    };
    buf.consume(size);
    let body = parse_body(buf, kind);
    response.body = body;
    response
}

#[derive(PartialEq)]
enum BodyKind {
    WithoutBody,
    Fixed(usize),
    Chunked,
}

fn parse_headers(parser: &httparse::Response, response: &mut Response)
    -> BodyKind
{
    let mut has_content_length = false;
    let mut body_kind = BodyKind::WithoutBody;
    for h in parser.headers.iter() {
        match Header::from(h.name) {
            Header::Connection => {
            }
            Header::ContentLength => {
                if has_content_length {
                    panic!("Duplicate Content-Length header");
                }
                has_content_length = true;
                if let Some(size) = headers::content_length(h.value) {
                    response.content_length = Some(size);
                    if body_kind != BodyKind::Chunked {
                        body_kind = BodyKind::Fixed(size as usize);
                    }
                }
            }
            Header::TransferEncoding => {
                if headers::is_chunked(h.value) {
                    if has_content_length {
                        //
                    }
                    body_kind = BodyKind::Chunked;
                }
                let value = String::from_utf8_lossy(h.value).into_owned();
                response.headers.push((Header::TransferEncoding, value));
            }
            header => {
                // ignore
                let value = String::from_utf8_lossy(h.value).into_owned();
                response.headers.push((header, value));
            },
        }
    }
    body_kind
}

/// Response body parser.
///
/// Expectes whole body is in buffer
/// (yet, only for curl case).
fn parse_body(buf: &mut Buf, body_kind: BodyKind)
    -> Option<Buf>
{
    match body_kind {
        BodyKind::WithoutBody => {
            None
        }
        BodyKind::Fixed(size) => {
            assert!(size >= buf.len());
            let bbuf = buf.split_off(size as usize);
            let bbuf = mem::replace(buf, bbuf);
            Some(bbuf)
        }
        BodyKind::Chunked => {
            Some(mem::replace(buf, Buf::new()))
        }
    }
}

#[cfg(test)]
mod test {
    use netbuf::Buf;
    use std::str;
    use super::parse_body;
    use super::BodyKind;

    #[test]
    fn parse_chunked_body() {
        let mut buf = Buf::new();
        buf.extend(b"\
            5\r\nHello\r\n\
            7\r\n World!\r\n\
            1a\r\n\nTransfer encoding checked\r\n\
            0\r\n\r\n"
            );
        let res = parse_body(&mut buf, BodyKind::Chunked);
        assert!(res.is_some());
        let body = res.unwrap();
        assert_eq!(str::from_utf8(&body[..]).unwrap(),
            "Hello World!\nTransfer encoding checked");
    }
}
