use std::io;
use std::convert::From;

use httparse;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            description("I/O error")
            display("I/O error: {}", err)
            from()
        }
        ParseError(err: httparse::Error) {
            description("parse error")
            display("parse error: {:?}", err)
            from()
        }
        ChunkParseError(err: httparse::InvalidChunkSize) {
            description("chunk size parse error")
            from()
        }
        BadRequestTarget {
            description("error parsing request target")
        }
        HostInvalid {
            description("invalid host header")
        }
        DuplicateHost {
            description("duplicate host header")
        }
        ConnectionInvalid {
            description("invalid connection header")
        }
        ContentLengthInvalid {
            description("invalid content-length header")
        }
        DuplicateContentLength {
            description("duplicate content length header")
        }
    }
}
