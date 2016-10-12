use std::io;
use std::fmt;
use std::io::Write;

use futures;
use futures::{Poll, Async};
use netbuf::Buf;


#[derive(Debug, PartialEq)]
enum ResponseState {
    CollectHeaders,
    WriteBody,
}

#[derive(Debug)]
pub struct Response {
    version: u8,
    code: u16,
    reason: String,

    headers: Vec<(String, String)>,
}

impl Response {

    pub fn new(version: u8) -> Response {
        Response {
            code: 200,
            reason: "OK".to_string(),
            version: version,

            headers: Vec::with_capacity(16*2),
        }
    }
    pub fn set_status(&mut self, code: u16) -> &mut Response {
        self.code = code;
        // TODO: change to proper reason if code is known
        self
    }
    pub fn set_reason(&mut self, reason: String) -> &mut Response {
        self.reason = reason;
        self
    }

    pub fn header(&mut self, header: &str, value: &str) -> &mut Response {
        self.headers.push((header.to_string(), value.to_string()));
        self
    }

    pub fn write_to(&self, buf: &mut Buf) -> io::Result<()> {
        try!(self.write_status(buf));
        try!(self.write_headers(buf));
        try!(buf.write(b"\r\n"));
        Ok(())
    }

    fn write_status(&self, buf: &mut Buf) -> io::Result<()> {
        write!(buf, "HTTP/1.{} {} {}\r\n",
               self.version, self.code, self.reason)
    }

    fn write_headers(&self, buf: &mut Buf) -> io::Result<()> {
        for &(ref h, ref v) in self.headers.iter() {
            try!(write!(buf, "{}: {}\r\n", h, v));
        }
        Ok(())
    }
}
