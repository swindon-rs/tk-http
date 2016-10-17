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
#[derive(Debug,PartialEq)]
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


// /// Enum reprsenting HTTP version.
// #[derive(Debug, Clone)]
// pub enum Version {
//     Http10,
//     Http11,
// }
// impl fmt::Display for Version {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match *self {
//             Version::Http10 => f.write_str("HTTP/1.0"),
//             Version::Http11 => f.write_str("HTTP/1.1"),
//         }
//     }
// }


/// Enum Representing HTTP Request Headers.
#[derive(Debug)]
pub enum Header {
    Host,
    Connection,
    KeepAlive,
    // add some more
    ContentLength,
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
