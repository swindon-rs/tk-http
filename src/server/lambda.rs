use tokio_core::io::Io;
use tk_bufstream::IoBuf;
use futures::IntoFuture;

use super::{Error, ResponseWriter, GenericResponse};

pub struct ResponseFn<T, S: Io>(Box<FnMut(ResponseWriter<S>) -> T>)
    where T: IntoFuture<Item=IoBuf<S>, Error=Error>;

impl<T, S: Io> ResponseFn<T, S>
    where T: IntoFuture<Item=IoBuf<S>, Error=Error>,
{
    pub fn new<F>(f: F) -> ResponseFn<T, S>
        where F: FnMut(ResponseWriter<S>) -> T + 'static,
    {
        ResponseFn(Box::new(f))
    }
}

impl<T, S: Io> GenericResponse<S> for ResponseFn<T, S>
    where T: IntoFuture<Item=IoBuf<S>, Error=Error>,
{
    type Future = T::Future;

    fn into_serializer(mut self, writer: ResponseWriter<S>) -> Self::Future
    {
        (self.0)(writer).into_future()
    }
}
