extern crate futures;
extern crate minihttp;
extern crate tk_bufstream;

use std::sync::Arc;

use futures::{Empty, Async, Future, empty};
use tk_bufstream::{MockData, IoBuf, ReadBuf, WriteBuf};

use minihttp::server::{Proto, Config, Dispatcher, Codec};
use minihttp::server::{Head, RecvMode, Error, Encoder, EncoderDone};

struct MockDisp {
}

struct MockCodec {
}

impl Dispatcher<MockData> for MockDisp {
    type Codec = MockCodec;

    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        Ok(MockCodec {})
    }
}

impl Codec<MockData> for MockCodec {
    type ResponseFuture = Empty<EncoderDone<MockData>, Error>;
    fn recv_mode(&mut self) -> RecvMode {
        RecvMode::buffered_upfront(1024)
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>
    {
        assert!(end);
        assert_eq!(data.len(), 0);
        Ok(Async::Ready(0))
    }
    fn start_response(&mut self, e: Encoder<MockData>) -> Self::ResponseFuture
    {
        empty()
    }
    fn hijack(&mut self, write_buf: WriteBuf<MockData>,
                         read_buf: ReadBuf<MockData>){
        unimplemented!();
    }
}

#[test]
fn simple_get_request() {
    let mock = MockData::new();
    let mut proto = Proto::new(mock.clone(),
        &Arc::new(Config::new()), MockDisp {});
    proto.poll().unwrap();
    mock.add_input("GET / HTTP/1.0\r\n\r\n");
    proto.poll().unwrap();
}

#[test]
#[should_panic(expected="Version")]
fn failing_get_request() {
    let mock = MockData::new();
    let mut proto = Proto::new(mock.clone(),
        &Arc::new(Config::new()), MockDisp {});
    proto.poll().unwrap();
    mock.add_input("GET / TTMP/2.0\r\n\r\n");
    proto.poll().unwrap();
}
