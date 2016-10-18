use std::fmt;
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
        } else {
            Header::Raw(val.to_string())
        }
    }
}
