use std::convert::From;
use std::ascii::AsciiExt;


/// Enum representing HTTP request methods.
///
/// ```rust,ignore
/// match req.method {
///     Method::Get => {},   // handle GET
///     Method::Post => {},  // handle POST requests
///     Method::Other(m) => { println!("Custom method {}", m); },
///     _ => {}
///     }
/// ```
#[derive(Debug, PartialEq)]
pub enum Method {
    Options,
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Trace,
    Connect,
    Other(String),
}

impl<'a> From<&'a str> for Method
{

    fn from(s: &'a str) -> Method {
        match s {
            "OPTIONS"   => Method::Options,
            "GET"       => Method::Get,
            "HEAD"      => Method::Head,
            "POST"      => Method::Post,
            "PUT"       => Method::Put,
            "PATCH"     => Method::Patch,
            "DELETE"    => Method::Delete,
            "TRACE"     => Method::Trace,
            "CONNECT"   => Method::Connect,
            s => Method::Other(s.to_string()),
        }
    }
}


/// Enum Representing HTTP Request Headers.
#[derive(Debug, PartialEq)]
pub enum Header {
    Host,
    Connection,
    KeepAlive,
    ContentLength,
    TransferEncoding,
    // add some more
    Raw(String),
}

impl<'a> From<&'a str> for Header {

    fn from(val: &'a str) -> Header {
        if val.eq_ignore_ascii_case("Host") {
            Header::Host
        } else if val.eq_ignore_ascii_case("Connection") {
            Header::Connection
        } else if val.eq_ignore_ascii_case("Keep-Alive") {
            Header::KeepAlive
        } else if val.eq_ignore_ascii_case("Content-Length") {
            Header::ContentLength
        } else if val.eq_ignore_ascii_case("Transfer-Encoding") {
            Header::TransferEncoding
        } else {
            Header::Raw(val.to_string())
        }
    }
}

impl PartialEq<str> for Header {
    fn eq(&self, other: &str) -> bool {
        use self::Header::*;
        match *self {
            Host => "Host".eq_ignore_ascii_case(other),
            Connection => "Connection".eq_ignore_ascii_case(other),
            KeepAlive => "Keep-Alive".eq_ignore_ascii_case(other),
            ContentLength => "Content-Length".eq_ignore_ascii_case(other),
            TransferEncoding => "Transfer-Encoding".eq_ignore_ascii_case(other),
            Raw(ref x) => x.eq_ignore_ascii_case(other),
        }
    }
}

impl AsRef<str> for Header {
    fn as_ref(&self) -> &str {
        use self::Header::*;
        match *self {
            Host => "Host",
            Connection => "Connection",
            KeepAlive => "Keep-Alive",
            ContentLength => "Content-Length",
            TransferEncoding => "Transfer-Encoding",
            Raw(ref x) => x,
        }
    }
}
