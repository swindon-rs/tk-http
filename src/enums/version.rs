use std::fmt;

/// Enum reprsenting HTTP version.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Version {
    /// Version 1.0 of the HTTP protocol
    Http10,
    /// Version 1.1 of the HTTP protocol
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

