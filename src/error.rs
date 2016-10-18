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
            display("parse error: {}", err)
            from()
        }
    }
}
