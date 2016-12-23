use std::net::SocketAddr;
use std::marker::PhantomData;

use futures::{Async, Future};
use tokio_core::io::Io;

use super::{Error, Encoder, EncoderDone, Dispatcher, Codec, Head, RecvMode};
use {Version};

/// Buffered request struct
///
/// some known headers may be moved to upper structure (ie, Host)
// TODO(tailhook) hide internal structure?
#[derive(Debug)]
pub struct Request {
    peer_addr: SocketAddr,
    method: String,
    path: String,
    host: Option<String>,
    version: Version,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

pub struct BufferedDispatcher<S: Io, N: NewService<S>> {
    addr: SocketAddr,
    max_request_length: usize,
    service: N,
    phantom: PhantomData<S>,
}

pub struct BufferedCodec<R> {
    max_request_length: usize,
    service: R,
    request: Option<Request>,
}

pub trait NewService<S: Io> {
    type Future: Future<Item=EncoderDone<S>, Error=Error>;
    type Instance: Service<S, Future=Self::Future>;
    fn new(&self) -> Self::Instance;
}

pub trait Service<S: Io> {
    type Future: Future<Item=EncoderDone<S>, Error=Error>;
    fn call(&mut self, request: Request, encoder: Encoder<S>) -> Self::Future;
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

impl<S: Io, T, R> NewService<S> for T
    where T: Fn() -> R,
          R: Service<S>,
{
    type Future = R::Future;
    type Instance = R;
    fn new(&self) -> R {
        (self)()
    }
}

impl<S: Io, T, F> Service<S> for T
    where T: Fn(Request, Encoder<S>) -> F,
        F: Future<Item=EncoderDone<S>, Error=Error>,
{
    type Future = F;
    fn call(&mut self, request: Request, encoder: Encoder<S>) -> F
    {
        (self)(request, encoder)
    }
}


impl<S: Io, N: NewService<S>> BufferedDispatcher<S, N> {
    pub fn new(addr: SocketAddr, service: N) -> BufferedDispatcher<S, N> {
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

impl<S: Io, N: NewService<S>> Dispatcher<S> for BufferedDispatcher<S, N> {
    type Codec = BufferedCodec<N::Instance>;

    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        // TODO(tailhook) strip hop-by-hop headers
        Ok(BufferedCodec {
            max_request_length: self.max_request_length,
            service: self.service.new(),
            request: Some(Request {
                peer_addr: self.addr,
                method: headers.method().to_string(),
                // TODO(tailhook) process other forms of path
                path: headers.path().unwrap().to_string(),
                host: headers.host().map(|x| x.to_string()),
                version: headers.version(),
                headers: headers.headers().iter().map(|&header| {
                    (header.name.to_string(), header.value.to_vec())
                }).collect(),
                body: Vec::new(),
            }),
        })
    }
}

impl<S: Io, R: Service<S>> Codec<S> for BufferedCodec<R> {
    type ResponseFuture = R::Future;
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
    fn start_response(&mut self, e: Encoder<S>) -> R::Future {
        self.service.call(self.request.take().unwrap(), e)
    }
}
