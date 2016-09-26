use std::io;
use std::fmt::Write;

use bytes::buf::{BlockBuf, Fmt};
use bytes::MutBuf;
use tokio_proto::Serialize;
use tokio_proto::pipeline::Frame;

use super::request::Version;

#[derive(Debug)]
pub struct Response {
    version: Version,
}

pub struct Serializer;

impl Response {

    pub fn new(v: Version) -> Response {
        Response {
            version: v,
        }
    }
}

impl Serialize for Serializer {
    type In = Frame<Response, (), io::Error>;

    fn serialize(&mut self, msg: Self::In, buf: &mut BlockBuf) {
        println!("msg: {:?}", msg);
        match msg {
            Frame::Message(resp) => {
                write!(Fmt(buf), "{} 204 OK\r\n", resp.version).unwrap();
                buf.write_slice(b"Content-Length: 0\r\n");
                buf.write_slice(b"\r\n");
            },
            _ => {},
        };
    }
}
