use std::mem;

use futures::{Future, Poll};
use futures::Async::{Ready, NotReady};


pub enum OptFuture<I, E> {
    Future(Box<Future<Item=I, Error=E>>),
    Value(Result<I, E>),
    #[doc(hidden)]
    Done,
}


impl<I, E> Future for OptFuture<I, E> {
    type Item = I;
    type Error = E;
    fn poll(&mut self) -> Poll<I, E> {
        use self::OptFuture::*;
        let future = match mem::replace(self, Done) {
            Future(mut f) => match f.poll()? {
                Ready(v) => return Ok(Ready(v)),
                NotReady => f,
            },
            Value(v) => {
                return Ok(Ready(v?))
            }
            Done => unreachable!(),
        };
        *self = OptFuture::Future(future);
        Ok(NotReady)
    }
}
