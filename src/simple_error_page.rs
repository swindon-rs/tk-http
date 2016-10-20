use std::io::Write;

use futures::Finished;
use tk_bufstream::IoBuf;
use tokio_core::io::Io;

use serve::GenericResponse;
use {Error, ResponseWriter};

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
            <p>Yours faithfully,<br>\
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

impl<S: Io> GenericResponse<S> for SimpleErrorPage {
    type Future = Finished<IoBuf<S>, Error>;
    fn into_serializer(self, mut response: ResponseWriter<S>)
        -> Self::Future
    {
        let content_length = PART1.len() + PART2.len() + PART3.len() +
            2*(4 + self.1.as_bytes().len());
        response.status(self.0, self.1);
        response.add_length(content_length as u64).unwrap();
        response.add_header("Content-Type", "text/html").unwrap();
        if response.done_headers().unwrap() {
            write!(&mut response, "\
                {p1}{code:03} {status}{p2}{code:03} {status}{p3}",
                    code=self.0, status=self.1,
                    p1=PART1, p2=PART2, p3=PART3)
                .expect("writing to a buffer always succeeds");
        }
        response.done()
    }
}

impl SimpleErrorPage {
    pub fn new(code: u16, reason: &'static str) -> SimpleErrorPage {
        SimpleErrorPage(code, reason)
    }
}
