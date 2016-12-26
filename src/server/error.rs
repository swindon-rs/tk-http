use std::io;
use std::convert::From;

use httparse;


quick_error! {
    /// HTTP server error
    #[derive(Debug)]
    pub enum Error {
        /// Socket IO error
        Io(err: io::Error) {
            description("I/O error")
            display("I/O error: {}", err)
            from()
        }
        /// Error parsing http headers
        ParseError(err: httparse::Error) {
            description("parse error")
            display("parse error: {:?}", err)
            from()
        }
        /// Error parsing http chunk
        ChunkParseError(err: httparse::InvalidChunkSize) {
            description("chunk size parse error")
            from()
        }
        /// Connection reset
        ConnectionReset {
            description("connection reset")
        }
        /// Bad request target (middle line of the request line)
        BadRequestTarget {
            description("error parsing request target")
        }
        /// Host header is invalid (non-utf-8 for example)
        HostInvalid {
            description("invalid host header")
        }
        /// Duplicate host header in request
        DuplicateHost {
            description("duplicate host header")
        }
        /// Connection header is invalid (non-utf-8 for example)
        ConnectionInvalid {
            description("invalid connection header")
        }
        /// Content length header is invalid (non-integer, or > 64bit)
        ContentLengthInvalid {
            description("invalid content-length header")
        }
        /// Duplicate content-length header, this is prohibited due to security
        DuplicateContentLength {
            description("duplicate content length header")
        }
        /// Unsupported kind of request body
        ///
        /// We allow CONNECT requests in the library but drop them if you
        /// don't `Hijack` connection right after headers.
        UnsupportedBody {
            description("this kind of request body is not supported (CONNECT)")
        }
        /// Request body is larger than x in `RecvMode::Buffered(x)` or >64bit
        RequestTooLong {
            description("request body is too big")
        }
    }
}
