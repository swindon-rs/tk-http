use std::error;
use std::fmt;
use std::io;
use std::convert::From;
use std::cmp::PartialEq;

use httparse;


#[derive(Debug)]
pub enum Error {
    ReadError(io::Error),
    ParseError(httparse::Error),
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::ReadError(ref e) => e.description(),
            Error::ParseError(_) => "httparse error",
        }
    }
}

impl PartialEq for Error {

    fn eq(&self, other: &Error) -> bool {
        match (self, other) {
            (&Error::ParseError(a), &Error::ParseError(b)) => {
                a == b
            },
            (&Error::ReadError(ref a), &Error::ReadError(ref b)) => {
                a.kind() == b.kind()
            }
            _ => false
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::ReadError(ref err) => {
                write!(f, "read error: {}", err)
            },
            Error::ParseError(ref err) => {
                write!(f, "parse error: {}", err)
            },
        }
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        match err {
            Error::ParseError(e) => {
                io::Error::new(io::ErrorKind::Other, e.to_string())
            },
            Error::ReadError(e) => e
        }
    }
}

#[cfg(test)]
mod test {
    use httparse;
    use error::Error as MyError;
    use std::error::Error;

    #[test]
    fn test_parse_error() {

        let e = MyError::ParseError(httparse::Error::HeaderName);
        assert_eq!(e.description(), "httparse error");
        assert!(e.cause().is_none());
        assert_eq!(format!("{}", e), "parse error: invalid header name");
        assert_eq!(format!("{:?}", e), "ParseError(HeaderName)");
    }
}
