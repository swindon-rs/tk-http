use std::mem;

use futures::{Future, Poll};
use futures::Async::{Ready, NotReady};


/// Optional future
///
/// This is a future that can hold either a result directly, similarly to
/// a `futures::done` or a real future. Future is stored in a `Box`ed form, to
/// keep signature of this structure simpler.
///
/// This works in the cases where we can check if we have a hot path where we
/// almost never return a future. So the consumer of the future can check
/// enum and do something immediately on the fast path, and proceed the long
/// path with a boxed future otherwise.
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
