
use std::io;

use httparse::Error as HttpError;
use httparse::InvalidChunkSize;
use abstract_ns::Error as NsError;


quick_error! {
    #[derive(Debug)]
    /// Client request error
    pub enum Error {
        /// Scheme url is not supported, only returned by "simple" interface
        UnsupportedScheme {
            description("scheme of this url is not supported")
        }
        /// Name resolution error, only returned by "simple" interface for now
        Name(err: NsError) {
            description("name resolution error")
            display("name resolution error: {}", err)
            from()
        }
        /// I/O (basically networking) error occured during request
        Io(err: io::Error) {
            description("IO error")
            display("IO error: {}", err)
            from()
        }
        /// Bad response headers received
        Header(err: HttpError) {
            description("bad headers")
            display("bad headers: {}", err)
            from()
        }
        /// Bad chunk size received
        ChunkSize(err: InvalidChunkSize) {
            description("invalid chunk size")
            display("invalid chunk size: {}", err)
            from()
        }
        /// Bad `Content-Length` header
        BadContentLength {
            description("bad content length")
        }
        /// Duplicate `Content-Length` header
        DuplicateContentLength {
            description("duplicate content length")
        }
        /// Connection reset by peer when reading response headers
        ResetOnResponseHeaders {
            description("connection closed prematurely while reading headers")
        }
        /// Connection reset by peer when response body
        ResetOnResponseBody {
            description("connection closed prematurely while reading body")
        }
        /// Response headers are received while we had no request sent yet
        PrematureResponseHeaders {
            description("response headers received \
                         before request has been written")
        }
        /// This means connection is busy (over the limit or not yet
        /// established when trying to send request
        Busy {
            description("request can't be sent because connection is busy")
        }
        /// The channel for receiving response is canceled. This probably means
        /// that connection to server was closed before being able to fulfil
        /// the request. But it's unlikely that this error is related to this
        /// request itself.
        Canceled {
            description("request canceled")
        }
        /// Connection closed normally
        ///
        /// This error should be catched by connection poolm and not shown
        /// to the end users
        Closed {
            description("connection closed normally")
        }
    }
}
