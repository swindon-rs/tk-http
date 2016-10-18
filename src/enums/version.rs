use std::fmt;

/// Enum reprsenting HTTP version.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Version {
    Http10,
    Http11,
}

impl Version {

    pub fn from_httparse(v: u8) -> Version {
        match v {
            0 => Version::Http10,
            1 => Version::Http11,
            // TODO(tailhook) figure out if httparse validates this number
            x => panic!("Unknown http version {:?}", x),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Version::Http10 => f.write_str("HTTP/1.0"),
            Version::Http11 => f.write_str("HTTP/1.1"),
        }
    }
}

