use std::mem;
use std::net::SocketAddr;
use std::marker::PhantomData;

use futures::Async;
use tokio_core::io::Io;

use super::{Error, Encoder, EncoderDone, Codec, Head, RecvMode};
use {OptFuture, Version};

/// Buffered request struct
///
/// some known headers may be moved to upper structure (ie, Host)
// TODO(tailhook) hide internal structure?
#[derive(Debug)]
pub struct Request {
    peer_addr: SocketAddr,
    method: String,
    path: String,
    version: Version,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

struct Headers {
    method: String,
    path: String,
    version: Version,
    headers: Vec<(String, Vec<u8>)>,
}

enum ReqState<S: Io> {
    Idle,
    Headers(Headers),
    Read(Headers, Vec<u8>),  // body is filled in
    Ready(Headers, Encoder<S>),
}

pub struct BufferedCodec<S: Io, R: Service<S>> {
    addr: SocketAddr,
    max_request_length: usize,
    service: R,
    request: ReqState<S>,
    phantom: PhantomData<S>,
}

pub trait Service<S: Io> {
    fn call(&mut self, request: Request, encoder: Encoder<S>)
        -> OptFuture<EncoderDone<S>, Error>;
}

impl<S: Io, T> Service<S> for T
    where T: FnMut(Request, Encoder<S>) -> OptFuture<EncoderDone<S>, Error>
{
    fn call(&mut self, request: Request, encoder: Encoder<S>)
        -> OptFuture<EncoderDone<S>, Error>
    {
        (self)(request, encoder)
    }
}


impl<S: Io, R: Service<S>> BufferedCodec<S, R> {
    fn new(addr: SocketAddr, service: R) -> BufferedCodec<S, R> {
        BufferedCodec {
            addr: addr,
            max_request_length: 10_485_760,
            service: service,
            request: ReqState::Idle,
            phantom: PhantomData,
        }
    }
    pub fn max_request_length(&mut self, value: usize) {
        self.max_request_length = value;
    }
}

impl<S: Io, R: Service<S>> Codec<S> for BufferedCodec<S, R> {
    fn headers_received(&mut self, headers: &Head)
        -> Result<RecvMode, Error>
    {
        self.request = ReqState::Headers(Headers {
            method: headers.method.to_string(),
            path: headers.path.to_string(),
            version: headers.version,
            headers: headers.headers.iter().map(|&header| {
                (header.name.to_string(), header.value.to_vec())
            }).collect(),
        });
        Ok(RecvMode::Buffered(self.max_request_length))
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>
    {
        use self::ReqState::*;
        assert!(end);
        self.request = match mem::replace(&mut self.request, Idle) {
            Idle => unreachable!(),
            Headers(h) => Read(h, data.to_vec()),
            Ready(h, e) => {
                let fut = self.service.call(Request {
                    peer_addr: self.addr,
                    method: h.method,
                    path: h.path,
                    version: h.version,
                    headers: h.headers,
                    body: data.to_vec(),
                }, e);
                unimplemented!();
            }
            Read(..) => unreachable!(),
        };
        Ok(Async::Ready(data.len()))
    }
    fn start_response(&mut self, e: Encoder<S>)
        -> OptFuture<EncoderDone<S>, Error>
    {
        use self::ReqState::*;
        self.request = match mem::replace(&mut self.request, Idle) {
            Idle => unreachable!(),
            Headers(h) => Ready(h, e),
            Ready(..) => unreachable!(),
            Read(h, b) => {
                return self.service.call(Request {
                    peer_addr: self.addr,
                    method: h.method,
                    path: h.path,
                    version: h.version,
                    headers: h.headers,
                    body: b,
                }, e);
            }
        };
        unimplemented!();
    }
}
