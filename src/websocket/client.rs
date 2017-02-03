use tokio_core::net::Io;


pub trait Authorizer<S: Io> {
    type Result: Sized;
    fn write_headers(&mut self, e: Encoder<S>) -> EncoderDone<S> {

    }
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Result, Error>
    {

    }
}

pub struct HandshakeProto<T, S, A> {

}

impl<T, S, A> HandshakeProto<T, S, A> {
    fn new(transport: S, authorizer: A) -> HandshakeProto<T, S, A> {
        HandshakeProto {
            
        }
    }
]

impl<T, S, A> Future for Proto<T, S, A>
{
    type Item = (WriteFramed<S, WebsocketCodec>, ReadFramed<S, WebsocketCodec>,
                 T);
    type Error = Error;
    fn poll(&mut self) -> Result<Async<Self::Item>, Error> {
    }
}
