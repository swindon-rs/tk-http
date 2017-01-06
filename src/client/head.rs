use std::borrow::Cow;
use std::ascii::AsciiExt;
use std::slice::Iter as SliceIter;

use httparse::Header;

use enums::{Version, Status};
use client::Head;
use client::client::BodyKind;


/// Iterator over all meaningful headers for the response
///
/// This iterator is created by `Head::headers`. And iterates over all
/// headers except hop-by-hop ones.
///
/// Note: duplicate headers are not glued together neither they are sorted
pub struct HeaderIter<'a> {
    head: &'a Head<'a>,
    iter: SliceIter<'a, Header<'a>>,
}

impl<'a> Head<'a> {
    /// Returns status if it is one of the supported statuses otherwise None
    ///
    /// Note: this method does not consider "reason" string at all just
    /// status code. Which is fine as specification states.
    pub fn status(&self) -> Option<Status> {
        Status::from(self.code)
    }
    /// Returns raw status code and reason as received even
    ///
    /// This returns something even if `status()` returned `None`.
    ///
    /// Note: the reason string may not match the status code or may even be
    /// an empty string.
    pub fn raw_status(&self) -> (u16, &'a str) {
        (self.code, self.reason)
    }
    /// Iterator over the headers of HTTP request
    ///
    /// This iterator strips the following kinds of headers:
    ///
    /// 1. Hop-by-hop headers (`Connection` itself, and ones it enumerates)
    /// 2. `Content-Length` and `Transfer-Encoding`
    ///
    /// You may use `all_headers()` if you really need to access to all of
    /// them (mostly useful for debugging puproses). But you may want to
    /// consider:
    ///
    /// 1. Payload size can be fetched using `body_length()` method. Note:
    ///    this also includes cases where length is implicitly set to zero.
    /// 2. `Connection` header might be discovered with `connection_close()`
    ///    or `connection_header()`
    pub fn headers(&self) -> HeaderIter {
        HeaderIter {
            head: self,
            iter: self.headers.iter(),
        }
    }
    /// All headers of HTTP request
    ///
    /// Unlike `self.headers()` this does include hop-by-hop headers. This
    /// method is here just for completeness, you shouldn't need it.
    pub fn all_headers(&self) -> &'a [Header<'a>] {
        self.headers
    }
}


impl<'a> Iterator for HeaderIter<'a> {
    type Item = (&'a str, &'a [u8]);
    fn next(&mut self) -> Option<(&'a str, &'a [u8])> {
        while let Some(header) = self.iter.next() {
            if header.name.eq_ignore_ascii_case("Connection") ||
                header.name.eq_ignore_ascii_case("Transfer-Encoding") ||
                header.name.eq_ignore_ascii_case("Content-Length")
            {
                continue;
            }

            if let Some(ref conn) = self.head.connection_header {
                let mut conn_headers = conn.split(',').map(|x| x.trim());
                if conn_headers.any(|x| x.eq_ignore_ascii_case(header.name)) {
                    continue;
                }
            }
            return Some((header.name, header.value));
        }
        return None;
    }
}
