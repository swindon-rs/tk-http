use std::io;
use std::convert::From;

use futures::sync::mpsc::SendError;
use httparse::Error as HttpError;
use httparse::InvalidChunkSize;


quick_error! {
    #[derive(Debug)]
    /// HTTP client error
    pub enum Error wraps pub ErrorEnum {
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
        /// Invalid URL specified
        InvalidUrl {
            description("requesting an invalid url")
        }
        /// Error sending a request to a connection pool
        PoolError {
            description("error sending a request to a connection pool")
        }
        /// Request body is too big (happens only in buffered mode)
        ResponseBodyTooLong {
            description("response body too long")
        }
        /// Connection header is invalid
        ConnectionInvalid {
            description("invalid connection header in response")
        }
        /// Unsupported status returned by server
        ///
        /// You have to write your own Codec to handle unsupported status codes
        InvalidStatus {
            description("unsupported status")
        }
        /// Request timed out
        RequestTimeout {
            description("request timed out")
        }
        /// Connection timed out on keep alive
        KeepAliveTimeout {
            description("connection timed out being on keep-alive")
        }
        Custom(err: Box<::std::error::Error + Send + Sync>) {
            description("custom error")
            display("custom error: {}", err)
            cause(&**err)
        }
    }
}

impl<T> From<SendError<T>> for ErrorEnum {
    fn from(_: SendError<T>) -> ErrorEnum {
        ErrorEnum::PoolError
    }
}

impl Error {
    /// Create an error instance wrapping custom error
    pub fn custom<E: Into<Box<::std::error::Error + Send + Sync>>>(err: E)
        -> Error
    {
        Error(ErrorEnum::Custom(err.into()))
    }
}

#[test]
fn send_sync() {
    fn send_sync<T: Send+Sync>(_: T) {}
    send_sync(Error::from(ErrorEnum::Canceled));
}
