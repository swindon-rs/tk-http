use std::io;

use httparse::Error as HttpError;
use httparse::InvalidChunkSize;
use abstract_ns::Error as NsError;


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        UnsupportedScheme {
            description("scheme of this url is not supported")
        }
        Name(err: NsError) {
            description("name resolution error")
            display("name resolution error: {}", err)
            from()
        }
        Io(err: io::Error) {
            description("IO error")
            display("IO error: {}", err)
            from()
        }
        Header(err: HttpError) {
            description("bad headers")
            display("bad headers: {}", err)
            from()
        }
        ChunkSize(err: InvalidChunkSize) {
            description("invalid chunk size")
            display("invalid chunk size: {}", err)
            from()
        }
        BadContentLength {
            description("bad content length")
        }
        DuplicateContentLength {
            description("duplicate content length")
        }
        ResetOnResponseHeaders {
            description("connection closed prematurely while reading headers")
        }
        ResetOnResponseBody {
            description("connection closed prematurely while reading body")
        }
        PrematureResponseHeaders {
            description("response headers received \
                         before request has been written")
        }
        Closed {
            description("connection closed normally")
        }
    }
}
