//! TODO(tailhook) This module should be moved into futures eventually
use std::mem;

use futures::{Async, AsyncSink, StartSend, Poll, Future, Sink};

enum State<F: Future>
    where F::Item: Sink,
          <F::Item as Sink>::SinkError: From<F::Error>,
{
    Connecting(F),
    Connected(F::Item),
    Error(<F::Item as Sink>::SinkError),
    Void,
}

pub struct Connection<F: Future>
    where F::Item: Sink,
          <F::Item as Sink>::SinkError: From<F::Error>,
{
    state: State<F>,
}

impl<F: Future> Connection<F>
    where F::Item: Sink,
          <F::Item as Sink>::SinkError: From<F::Error>,
{
    pub fn new(f: F) -> Connection<F> {
        Connection { state: State::Connecting(f) }
    }
}

impl<F: Future> Sink for Connection<F>
    where F::Item: Sink,
          <F::Item as Sink>::SinkError: From<F::Error>,
{
    type SinkItem = <F::Item as Sink>::SinkItem;
    type SinkError = <F::Item as Sink>::SinkError;
    fn start_send(&mut self, item: Self::SinkItem)
        -> StartSend<Self::SinkItem, Self::SinkError>
    {
        let (res, state) = match mem::replace(&mut self.state, State::Void) {
            State::Connecting(mut conn) => {
                match conn.poll() {
                    Ok(Async::Ready(mut conn)) => {
                        (conn.start_send(item)?, State::Connected(conn))
                    }
                    Ok(Async::NotReady) => {
                        (AsyncSink::NotReady(item), State::Connecting(conn))
                    }
                    Err(e) => {
                        (AsyncSink::NotReady(item), State::Error(e.into()))
                    }
                }
            }
            State::Connected(mut conn) => {
                (conn.start_send(item)?, State::Connected(conn))
            }
            State::Void => unreachable!(),
            State::Error(e) => (AsyncSink::NotReady(item), State::Error(e)),
        };
        self.state = state;
        return Ok(res);
    }
    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        let (res, state) = match mem::replace(&mut self.state, State::Void) {
            State::Connecting(mut conn) => {
                match conn.poll()? {
                    Async::Ready(conn) => {
                        (Async::Ready(()), State::Connected(conn))
                    }
                    Async::NotReady => {
                        (Async::NotReady, State::Connecting(conn))
                    }
                }
            }
            State::Connected(mut conn) => {
                (conn.poll_complete()?, State::Connected(conn))
            }
            State::Void => unreachable!(),
            State::Error(e) => return Err(e),
        };
        self.state = state;
        return Ok(res);
    }
}
