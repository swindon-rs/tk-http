use std::io;

use futures::{Future, Poll, Async};
use tokio_core::io::{Io};
use netbuf::Buf;


/// Http Server handler.
///
/// Handles single HTTP connection.
/// It responsible for:
/// * reading incoming data into buf;
/// * parsing it using httparse;
/// * passing Request to upper Service;
/// * waiting for response;
/// * serializing into buffer;
/// * treating connection well;
pub struct HttpServer<S> {
    stream: S,
    inbuf: Buf,
    outbuf: Buf,
    // TODO: add service
}


impl<S> HttpServer<S>
    where S: Io,
{

    pub fn new(stream: S) -> HttpServer<S> {
        HttpServer {
            stream: stream,
            inbuf: Buf::new(),
            outbuf: Buf::new(),
        }
    }
}


impl<S> Future for HttpServer<S>
    where S: Io,
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        // TODO:
        //  if there read data -> parse it
        //  if no read data -> read it into buffer [and parse it?]
        //  If there is response -> serialize it to out buffer;
        //  if there is data to be written -> write it;

        // Loop until we block; otherwise we'd end task and close connection;
        loop {
            let mut not_ready = false;

            // Try flush pending writes;
            match self.stream.flush() {
                Ok(_) => {},
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    not_ready = true;
                },
                Err(e) => return Err(e.into()),
            }

            // Try read raw data into buffer;
            let read = match self.inbuf.read_from(&mut self.stream) {
                Ok(bytes) => Some(bytes),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    not_ready = true;
                    None
                },
                Err(e) => return Err(e.into()),
            };
            match read {
                Some(0) => return Ok(Async::Ready(())),
                Some(_) | None => {},
            }
            println!("read {:?} bytes", read);

            // Now we have to (a) parse it (b) process it and (c) serialize it;
            // fake it by echoing:
            if self.inbuf.len() > 0 {   // TODO: fix netbuf::Buf.write([]) empty buf
                match self.inbuf.write_to(&mut self.outbuf) {
                    Ok(b) => {println!("Copied {} bytes", b);},
                    Err(e) => return Err(e.into()),
                }
            }

            // Try write out buffer;
            if self.outbuf.len() > 0 {
                let written = match self.outbuf.write_to(&mut self.stream) {
                    Ok(bytes) => bytes,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        not_ready = true;
                        0
                    },
                    Err(e) => return Err(e.into()),
                };
                println!("written {} bytes", written);
            }

            if not_ready {
                return Ok(Async::NotReady);
            }
            // TODO: add exit condition
        }
    }
}
