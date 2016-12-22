use std::ascii::AsciiExt;

use httparse::{EMPTY_HEADER, Request, parse_chunk_size};

use super::{Error, RequestTarget};
use super::codec::BodyKind;


/// Number of headers to allocate on a stack
const MIN_HEADERS: usize = 16;
/// A hard limit on the number of headers
const MAX_HEADERS: usize = 1024;


struct RequestConfig<'a> {
    body: BodyKind,
    is_head: bool,
    expect_continue: bool,
    connection_close: bool,
    connection: Cow<'a, str>,
    host: Option<&'a str>,
    path: RequestTarget<'a>,
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
    path: &'a str,
    version: Version,
    host: &'a str,
    headers: &'a [Header<'a>],
    body_kind: BodyKind,
    connection_close: bool,
    connection: &'a [u8],
}


fn scan_headers(raw_request: &Request) -> Result<RequestConfig, Error> {
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
    use super::Error::{};
    let is_head = raw_request.method.unwrap() == "HEAD";
    let mut has_content_length = false;
    let mut close = raw_request.version.unwrap() == 0;
    let mut expect_continue = false;
    let mut body = Fixed(0);
    let mut connection = None;
    let mut host_header = false;
    let (mut host, path) = {
        if raw.path.contains("://") {
        }
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
                let s = try!(from_utf8(header.value));
                let len = try!(s.parse().map_err(BadContentLength));
                body = Fixed(len);
            } else {
                // transfer-encoding has preference and don't allow keep-alive
                close = true;
            }
        } else if header.name.eq_ignore_ascii_case("Close") {
            // TODO(tailhook) capture connection header(s) itself
            if header.value.split(|&x| x == b',').any(headers::is_close) {
                close = true;
            }
        } else if header.name.eq_ignore_ascii_case("Host") {
            if host_header {
                return Err(DulicateHost);
            }
            host_header = true;
            if host.is_none() {  // if host is not in uri
                // TODO(tailhook) additional validations for host
                host = from_utf8(header.value).ok_or(HostInvalid)?;
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
        connection_close: close,
    })
}

fn parse_headers<S, D>(buffer: &mut Buf, disp: &mut D, is_head: bool)
    -> Result<D::Codec, Error>
    where S: Io,
          D: Dispatcher<S>,
{
    let mut vec;
    let mut headers = [httparse::EMPTY_HEADER; MIN_HEADERS];
    let (head, cfg) = {
        let mut raw = httparse::Request::new(&mut headers);
        let mut result = raw.parse(&buffer[..]);
        if matches!(result, Err(httparse::Error::TooManyHeaders)) {
            vec = vec![httparse::EMPTY_HEADER; MAX_HEADERS];
            raw = httparse::Request::new(&mut vec);
            result = raw.parse(&buffer[..]);
        }
        match result? {
            httparse::Status::Complete(bytes) => {
                let cfg = scan_headers(&raw)?;
                let head = Head {
                    method = raw.method.unwrap(),
                    path: raw.path.unwrap(),
                    version: if raw.version.unwrap() == 1
                        { Version::Http11 } else { Version::Http10 },
                    headers: headers,
                    body_kind: body,
                    // For HTTP/1.0 we could implement Connection: Keep-Alive
                    // but hopefully it's rare enough to ignore nowadays
                    close: close || ver == 0,
                };
            }
            _ => return Ok(None),
        }
    };
    let mode = codec.headers_received(&head)?;
    (mode, body, close, bytes)
}
