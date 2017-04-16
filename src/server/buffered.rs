//! Higher-level interface for serving fully buffered requests
//!
use std::net::SocketAddr;
use std::sync::Arc;
use std::marker::PhantomData;

use futures::{Async, Future, IntoFuture};
use futures::future::FutureResult;
use tokio_core::reactor::Handle;
use tk_bufstream::{ReadBuf, WriteBuf, ReadFramed, WriteFramed};

use websocket::{ServerCodec as WebsocketCodec};
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

/// A dispatcher that allows to process request and return response using
/// a one single function
pub struct BufferedDispatcher<S, N: NewService<S>> {
    addr: SocketAddr,
    max_request_length: usize,
    service: N,
    handle: Handle,
    phantom: PhantomData<S>,
}

/// A codec counterpart of the BufferedDispatcher, might be used with your
/// own dispatcher too
pub struct BufferedCodec<R> {
    max_request_length: usize,
    service: R,
    request: Option<Request>,
    handle: Handle,
}

/// A helper to create a simple websocket (and HTTP) service
///
/// It's internally created by `BufferedDispatcher::new_with_websockets()`
pub struct WebsocketFactory<H, I> {
    service: Arc<H>,
    websockets: Arc<I>,
}

/// An instance of websocket factory, created by WebsocketFactory itself
pub struct WebsocketService<H, I, T, U> {
    service: Arc<H>,
    websockets: Arc<I>,
    phantom: PhantomData<(T, U)>,
}

/// A trait that you must implement to reply on requests, usually a function
pub trait NewService<S> {
    /// Future returned by the service (an actual function serving request)
    type Future: Future<Item=EncoderDone<S>, Error=Error>;
    /// Instance of the service, created for each request
    type Instance: Service<S, Future=Self::Future>;
    /// Constructor of the instance of the service, created for each request
    fn new(&self) -> Self::Instance;
}

/// An instance of a NewService for a single request, usually just a function
pub trait Service<S> {
    /// A future returned by `call()`
    type Future: Future<Item=EncoderDone<S>, Error=Error>;

    /// A future returned by `start_websocket`, it's spawned on the main loop
    /// hence needed to be static.
    type WebsocketFuture: Future<Item=(), Error=()> + 'static;

    /// A method which is called when request arrives, including the websocket
    /// negotiation request.
    ///
    /// See examples for a way to negotiate both websockets and services
    fn call(&mut self, request: Request, encoder: Encoder<S>) -> Self::Future;

    /// A method which is called when websocket connection established
    fn start_websocket(&mut self, output: WriteFramed<S, WebsocketCodec>,
                                  input: ReadFramed<S, WebsocketCodec>)
        -> Self::WebsocketFuture;
}

impl<H, I, T, U, S> NewService<S> for WebsocketFactory<H, I>
    where H: Fn(Request, Encoder<S>) -> T,
          I: Fn(WriteFramed<S, WebsocketCodec>,
                ReadFramed<S, WebsocketCodec>) -> U,
          T: Future<Item=EncoderDone<S>, Error=Error>,
          U: Future<Item=(), Error=()> + 'static,
{
    type Future = T;
    type Instance = WebsocketService<H, I, T, U>;
    fn new(&self) -> Self::Instance {
        WebsocketService {
            service: self.service.clone(),
            websockets: self.websockets.clone(),
            phantom: PhantomData,
        }
    }
}

impl<S, H, I, T, U> Service<S> for WebsocketService<H, I, T, U>
    where H: Fn(Request, Encoder<S>) -> T,
          I: Fn(WriteFramed<S, WebsocketCodec>,
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
    /// Returns the host header of a request
    pub fn host(&self) -> Option<&str> {
        self.host.as_ref().map(|s| s.as_ref())
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

impl<S, T, R> NewService<S> for T
    where T: Fn() -> R,
          R: Service<S>,
{
    type Future = R::Future;
    type Instance = R;
    fn new(&self) -> R {
        (self)()
    }
}

impl<S, T, F> Service<S> for T
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


impl<S, N: NewService<S>> BufferedDispatcher<S, N> {
    /// Create an instance of bufferd dispatcher
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
    /// Sets max request length
    pub fn max_request_length(&mut self, value: usize) {
        self.max_request_length = value;
    }
}

impl<S, H, I, T, U> BufferedDispatcher<S, WebsocketFactory<H, I>>
    where H: Fn(Request, Encoder<S>) -> T,
          I: Fn(WriteFramed<S, WebsocketCodec>,
                ReadFramed<S, WebsocketCodec>) -> U,
          T: Future<Item=EncoderDone<S>, Error=Error>,
          U: Future<Item=(), Error=()> + 'static,
{
    /// Creates a dispatcher with two functions: one serving http requests and
    /// websockets.
    pub fn new_with_websockets(addr: SocketAddr, handle: &Handle,
        http: H, websockets: I)
        -> BufferedDispatcher<S, WebsocketFactory<H, I>>
    {
        BufferedDispatcher {
            addr: addr,
            max_request_length: 10_485_760,
            service: WebsocketFactory {
                service: Arc::new(http),
                websockets: Arc::new(websockets),
            },
            handle: handle.clone(),
            phantom: PhantomData,
        }
    }
}

impl<S, N: NewService<S>> Dispatcher<S> for BufferedDispatcher<S, N> {
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
                headers: headers.headers().map(|(name, value)| {
                    (name.to_string(), value.to_vec())
                }).collect(),
                body: Vec::new(),
                websocket_handshake: up.unwrap_or(None),
            }),
            handle: self.handle.clone(),
        })
    }
}

impl<S, R: Service<S>> Codec<S> for BufferedCodec<R> {
    type ResponseFuture = R::Future;
    fn recv_mode(&mut self) -> RecvMode {
        if self.request.as_ref().unwrap().websocket_handshake.is_some() {
            RecvMode::hijack()
        } else {
            RecvMode::buffered_upfront(self.max_request_length)
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
