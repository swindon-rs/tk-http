//! Higher-level interface for serving fully buffered requests
//!
use std::net::SocketAddr;
use std::marker::PhantomData;

use futures::{Async, Future, IntoFuture};
use futures::future::FutureResult;
use tokio_core::io::Io;
use tokio_core::reactor::Handle;
use tk_bufstream::{ReadBuf, WriteBuf, ReadFramed, WriteFramed};

use websocket::{Codec as WebsocketCodec};
use super::{Error, Encoder, EncoderDone, Dispatcher, Codec, Head, RecvMode};
use super::{WebsocketHandshake};
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
    websocket_handshake: Option<WebsocketHandshake>,
}

pub struct BufferedDispatcher<S: Io, N: NewService<S>> {
    addr: SocketAddr,
    max_request_length: usize,
    service: N,
    handle: Handle,
    phantom: PhantomData<S>,
}

pub struct BufferedCodec<R> {
    max_request_length: usize,
    service: R,
    request: Option<Request>,
    handle: Handle,
}

pub struct WebsocketFactory<F, G> {
    service: F,
    websockets: G,
}

pub struct WebsocketService<F, G, T, U> {
    service: F,
    websockets: G,
    phantom: PhantomData<(T, U)>,
}

pub trait NewService<S: Io> {
    type Future: Future<Item=EncoderDone<S>, Error=Error>;
    type Instance: Service<S, Future=Self::Future>;
    fn new(&self) -> Self::Instance;
}

pub trait Service<S: Io> {
    type Future: Future<Item=EncoderDone<S>, Error=Error>;
    type WebsocketFuture: Future<Item=(), Error=()> + 'static;
    fn call(&mut self, request: Request, encoder: Encoder<S>) -> Self::Future;
    fn start_websocket(&mut self, output: WriteFramed<S, WebsocketCodec>,
                                  input: ReadFramed<S, WebsocketCodec>)
        -> Self::WebsocketFuture;
}

impl<F, G, H, I, T, U, S: Io> NewService<S> for WebsocketFactory<F, G>
    where F: Fn() -> H,
          H: FnMut(Request, Encoder<S>) -> T,
          G: Fn() -> I,
          I: FnMut(WriteFramed<S, WebsocketCodec>,
                   ReadFramed<S, WebsocketCodec>) -> U,
          T: Future<Item=EncoderDone<S>, Error=Error>,
          U: Future<Item=(), Error=()> + 'static,
{
    type Future = T;
    type Instance = WebsocketService<H, I, T, U>;
    fn new(&self) -> Self::Instance {
        WebsocketService {
            service: (self.service)(),
            websockets: (self.websockets)(),
            phantom: PhantomData,
        }
    }
}

impl<S: Io, H, I, T, U> Service<S> for WebsocketService<H, I, T, U>
    where H: FnMut(Request, Encoder<S>) -> T,
          I: FnMut(WriteFramed<S, WebsocketCodec>,
                   ReadFramed<S, WebsocketCodec>) -> U,
          T: Future<Item=EncoderDone<S>, Error=Error>,
          U: Future<Item=(), Error=()> + 'static,
{
    type Future = T;
    type WebsocketFuture = U;
    fn call(&mut self, request: Request, encoder: Encoder<S>) -> T {
        (self.service)(request, encoder)
    }
    fn start_websocket(&mut self, output: WriteFramed<S, WebsocketCodec>,
                                  input: ReadFramed<S, WebsocketCodec>)
        -> U
    {
        (self.websockets)(output, input)
    }
}

impl Request {
    /// Returns peer address that initiated HTTP connection
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }
    /// Returns method of a request
    pub fn method(&self) -> &str {
        &self.method
    }
    /// Returns path of a request
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Returns HTTP version used in request
    pub fn version(&self) -> Version {
        self.version
    }
    /// Returns request headers
    pub fn headers(&self) -> &[(String, Vec<u8>)] {
        &self.headers
    }
    /// Returns request body
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    /// Returns websocket handshake if exists
    pub fn websocket_handshake(&self) -> Option<&WebsocketHandshake> {
        self.websocket_handshake.as_ref()
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
    type WebsocketFuture = FutureResult<(), ()>;
    fn call(&mut self, request: Request, encoder: Encoder<S>) -> F
    {
        (self)(request, encoder)
    }
    fn start_websocket(&mut self, _output: WriteFramed<S, WebsocketCodec>,
                                  _input: ReadFramed<S, WebsocketCodec>)
        -> Self::WebsocketFuture
    {
        /// Basically no websockets
        Ok(()).into_future()
    }
}


impl<S: Io, N: NewService<S>> BufferedDispatcher<S, N> {
    pub fn new(addr: SocketAddr, handle: &Handle, service: N)
        -> BufferedDispatcher<S, N>
    {
        BufferedDispatcher {
            addr: addr,
            max_request_length: 10_485_760,
            service: service,
            handle: handle.clone(),
            phantom: PhantomData,
        }
    }
    pub fn max_request_length(&mut self, value: usize) {
        self.max_request_length = value;
    }
}

impl<S: Io, F, G, H, I, T, U> BufferedDispatcher<S, WebsocketFactory<F, G>>
    where F: Fn() -> H,
          H: FnMut(Request, Encoder<S>) -> T,
          G: Fn() -> I,
          I: FnMut(WriteFramed<S, WebsocketCodec>,
                   ReadFramed<S, WebsocketCodec>) -> U,
          T: Future<Item=EncoderDone<S>, Error=Error>,
          U: Future<Item=(), Error=()> + 'static,
{
    pub fn new_with_websockets(addr: SocketAddr, handle: &Handle,
        http: F, websockets: G)
        -> BufferedDispatcher<S, WebsocketFactory<F, G>>
    {
        BufferedDispatcher {
            addr: addr,
            max_request_length: 10_485_760,
            service: WebsocketFactory {
                service: http,
                websockets: websockets,
            },
            handle: handle.clone(),
            phantom: PhantomData,
        }
    }
}

impl<S: Io, N: NewService<S>> Dispatcher<S> for BufferedDispatcher<S, N>
    where N::Instance: 'static
{
    type Codec = BufferedCodec<N::Instance>;

    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        // TODO(tailhook) strip hop-by-hop headers
        let up = headers.get_websocket_upgrade();
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
                websocket_handshake: up.unwrap_or(None),
            }),
            handle: self.handle.clone(),
        })
    }
}

impl<S: Io, R: Service<S> + 'static> Codec<S> for BufferedCodec<R> {
    type ResponseFuture = R::Future;
    fn recv_mode(&mut self) -> RecvMode {
        if self.request.as_ref().unwrap().websocket_handshake.is_some() {
            RecvMode::Hijack
        } else {
            RecvMode::BufferedUpfront(self.max_request_length)
        }
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
    fn hijack(&mut self, write_buf: WriteBuf<S>, read_buf: ReadBuf<S>){
        let inp = read_buf.framed(WebsocketCodec);
        let out = write_buf.framed(WebsocketCodec);
        self.handle.spawn(self.service.start_websocket(out, inp));
    }
}
