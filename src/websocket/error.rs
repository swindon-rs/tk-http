use std::io;
use std::str::{Utf8Error};

use httparse;

quick_error! {
    /// Websocket Error works both for client and server connections
    #[derive(Debug)]
    pub enum Error wraps pub ErrorEnum {
        /// Socket IO error
        Io(err: io::Error) {
            description("IO error")
            display("IO error: {}", err)
            from()
        }
        /// Error when polling timeout future (unreachable)
        Timeout {
            description("Timeout error (unreachable)")
            display("Timeout error (unreachable)")
        }
        /// Text frame can't be decoded
        InvalidUtf8(err: Utf8Error) {
            description("Error decoding text frame")
            display("Error decoding text frame: {}", err)
            from()
        }
        /// Got websocket message with wrong opcode
        InvalidOpcode(code: u8) {
            description("Opcode of the frame is invalid")
            display("Opcode of the frame is invalid: {}", code)
            from()
        }
        /// Got unmasked frame
        Unmasked {
            description("Received unmasked frame")
        }
        /// Got fragmented frame (fragmented frames are not supported yet)
        Fragmented {
            description("Received fragmented frame")
        }
        /// Received frame that is longer than configured limit
        TooLong {
            description("Received frame that is too long")
        }
        /// Currently this error means that channel to/from websocket closed
        ///
        /// In future we expect this condition (processor dropping channel) to
        /// happen when we forced killing connection by backend, so processor
        /// got rid of all object that refer to the connection.
        ///
        /// Another case: we are trying to use RemoteReplier for connection
        /// that already closed
        Closed {
            description("Forced connection close")
        }
        /// Error parsing http headers
        HeaderError(err: httparse::Error) {
            description("parse error")
            display("parse error: {:?}", err)
            from()
        }
        PrematureResponseHeaders {
            description("response headers before request are sent")
        }
        Custom(err: Box<::std::error::Error + Send + Sync>) {
            description("custom error")
            display("custom error: {}", err)
            cause(&**err)
        }
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
    send_sync(Error::from(ErrorEnum::TooLong));
}
