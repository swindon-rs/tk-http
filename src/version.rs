use std::fmt;

/// Enum reprsenting HTTP version.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Version {
    Http10,
    Http11,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Version::Http10 => f.write_str("HTTP/1.0"),
            Version::Http11 => f.write_str("HTTP/1.1"),
        }
    }
}

