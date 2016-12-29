use std::io;
use std::str::{from_utf8, Utf8Error};


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            description("IO error")
            display("IO error: {}", err)
            from()
        }
        InvalidUtf8(err: Utf8Error) {
            description("Error decoding text frame")
            display("Error decoding text frame: {}", err)
            from()
        }
        InvalidOpcode(code: u8) {
            description("Opcode of the frame is invalid")
            display("Opcode of the frame is invalid: {}", code)
            from()
        }
        Unmasked {
            description("Received unmasked frame")
        }
        Fragmented {
            description("Received fragmented frame")
        }
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
    }
}
