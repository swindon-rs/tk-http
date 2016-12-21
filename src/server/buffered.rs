use std::net::SocketAddr;
use std::marker::PhantomData;

use futures::Async;
use tokio_core::io::Io;

use super::{Error, Encoder, EncoderDone, Dispatcher, Codec, Head, RecvMode};
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

pub struct BufferedDispatcher<S: Io, R: Service<S>> {
    addr: SocketAddr,
    max_request_length: usize,
    service: R,
    phantom: PhantomData<S>,
}

pub struct BufferedCodec<R> {
    max_request_length: usize,
    service: R,
    request: Option<Request>,
}

pub trait Service<S: Io> {
    fn call(&mut self, request: Request, encoder: Encoder<S>)
        -> OptFuture<EncoderDone<S>, Error>;
}

impl Request {
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn version(&self) -> Version {
        self.version
    }
    pub fn headers(&self) -> &[(String, Vec<u8>)] {
        &self.headers
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
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


impl<S: Io, R: Service<S>> BufferedDispatcher<S, R> {
    pub fn new(addr: SocketAddr, service: R) -> BufferedDispatcher<S, R> {
        BufferedDispatcher {
            addr: addr,
            max_request_length: 10_485_760,
            service: service,
            phantom: PhantomData,
        }
    }
    pub fn max_request_length(&mut self, value: usize) {
        self.max_request_length = value;
    }
}

impl<S: Io, R: Service<S> + Clone> Dispatcher<S> for BufferedDispatcher<S, R> {
    type Codec = BufferedCodec<R>;

    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        Ok(BufferedCodec {
            max_request_length: self.max_request_length,
            service: self.service.clone(),
            request: Some(Request {
                peer_addr: self.addr,
                method: headers.method.to_string(),
                path: headers.path.to_string(),
                version: headers.version,
                headers: headers.headers.iter().map(|&header| {
                    (header.name.to_string(), header.value.to_vec())
                }).collect(),
                body: Vec::new(),
            }),
        })
    }
}
impl<S: Io, R: Service<S> + Clone> Codec<S> for BufferedCodec<R> {
    fn recv_mode(&mut self) -> RecvMode {
        RecvMode::BufferedUpfront(self.max_request_length)
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>
    {
        assert!(end);
        self.request.as_mut().unwrap().body = data.to_vec();
        Ok(Async::Ready(data.len()))
    }
    fn start_response(&mut self, e: Encoder<S>)
        -> OptFuture<EncoderDone<S>, Error>
    {
        self.service.call(self.request.take().unwrap(), e)
    }
}
