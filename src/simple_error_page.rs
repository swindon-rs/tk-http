use std::io::Write;

use netbuf::Buf;
use futures::{Finished, finished};
use tokio_core::net::TcpStream;

use serve::GenericResponse;
use {Error, ResponseConfig};

const PART1: &'static str = "\
    <!DOCTYPE html>
    <html>\
        <head>\
            <title>\
    ";
const PART2: &'static str = "\
            </title>\
        </head>\
        <body>\
            <h1>\
    ";
const PART3: &'static str = concat!("\
            </h1>\
            <hr>\
            <p>Yours failthfully,<br>\
                minihttp/", env!("CARGO_PKG_VERSION"), "\
            </p>
        </body>
    </html>
    ");

/// Generates response with default error page
///
/// This module also serves as a demo of simple response writer
// TODO(tailhook) use response code enum and it's owned reason string
pub struct SimpleErrorPage(u16, &'static str);

impl GenericResponse for SimpleErrorPage {
    type Serializer = Finished<(TcpStream, Buf), Error>;
    fn make_serializer(self, sock: TcpStream, mut buf: Buf,
                             config: ResponseConfig)
        -> Self::Serializer
    {
        let content_length = PART1.len() + PART2.len() + PART3.len() +
            4 + self.1.len();
        write!(&mut buf, "{code:03} {status}\r\n\
            Content-Length: {length}\r\n\
            Connection: close\r\n\
            \r\n",
            code=self.0, status=self.1, length=content_length)
            .expect("writing to a buffer always succeeds");
        if !config.is_head {
            write!(&mut buf, "\
                {p1}{code:03} {status}{p2}{code:03} {status}{p3}",
                    code=self.0, status=self.1,
                    p1=PART1, p2=PART2, p3=PART3)
            .expect("writing to a buffer always succeeds");
        }
        finished((sock, buf))
    }
}
