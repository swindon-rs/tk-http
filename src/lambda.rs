use tokio_core::net::TcpStream;
use netbuf::Buf;
use futures::IntoFuture;

use {Error, ResponseWriter, GenericResponse};

pub struct ResponseFn<T>(Box<FnMut(ResponseWriter) -> T>)
    where T: IntoFuture<Item=(TcpStream, Buf), Error=Error>;

impl<T> ResponseFn<T>
    where T: IntoFuture<Item=(TcpStream, Buf), Error=Error>,
{
    pub fn new<F>(f: F) -> ResponseFn<T>
        where F: FnMut(ResponseWriter) -> T + 'static,
    {
        ResponseFn(Box::new(f))
    }
}

impl<T> GenericResponse for ResponseFn<T>
    where T: IntoFuture<Item=(TcpStream, Buf), Error=Error>,
{
    type Future = T::Future;

    fn into_serializer(mut self, writer: ResponseWriter) -> Self::Future
    {
        (self.0)(writer).into_future()
    }
}
