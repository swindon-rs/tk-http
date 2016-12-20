use std::collections::VecDeque;
use std::net::SocketAddr;

use tk_bufstream::IoBuf;
use futures::{Future, Poll, Async};
//use futures::Async::{Ready};
use tokio_core::io::Io;
use tokio_service::Service;

use request::{Request, RequestParser, response_config};
use serve::{ResponseConfig, ResponseWriter};
use {GenericResponse, Error};


enum InFlight<F, R, S: Io>
    where F: Future<Item=R>,
          R: GenericResponse<S>,
{
    Service(ResponseConfig, F),
    Waiting(ResponseConfig, R),
    Responding(R::Future),
}


pub struct HttpServer<T, S>
    where T: Service<Request=Request, Error=Error>,
          T::Response: GenericResponse<S>,
          S: Io,
{
    /// Socket and output buffer, it's None when connection is borrowed by
    ///
    conn: Option<IoBuf<S>>,
    peer_addr: SocketAddr,
    request_parser: RequestParser,
    service: T,
    in_flight: VecDeque<InFlight<T::Future, T::Response, S>>,
}

impl<T, S> HttpServer<T, S>
    where T: Service<Request=Request, Error=Error>,
          T::Response: GenericResponse<S>,
          S: Io
{

    pub fn new(socket: S, service: T, peer_addr: SocketAddr)
        -> HttpServer<T, S>
    {
        HttpServer {
            conn: Some(IoBuf::new(socket)),
            peer_addr: peer_addr,
            request_parser: RequestParser::new(),
            service: service,
            in_flight: VecDeque::with_capacity(32),
        }
    }

    fn read_and_process(&mut self) -> Result<(), Error> {
        if let Some(ref mut conn) = self.conn {
            loop {
                while !self.request_parser.parse_from(
                    &mut conn.in_buf, &self.peer_addr)?
                {
                    if conn.read()? == 0 {
                        return Ok(());
                    }
                }
                let req = self.request_parser.take().unwrap();
                let cfg = response_config(&req);
                let waiter = self.service.call(req);
                self.in_flight.push_back((InFlight::Service(cfg, waiter)));
            }
        }
        Ok(())
    }

    fn poll_waiters(&mut self) -> Result<(), Error> {
        for waiter in self.in_flight.iter_mut() {
            let waiting = match waiter {
                &mut InFlight::Service(cfg, ref mut f) => {
                    match f.poll() {
                        Ok(Async::Ready(res)) => Some((cfg, res)),
                        Ok(Async::NotReady) => None,
                        Err(e) => return Err(e),
                    }
                },
                _ => None
            };
            if let Some((cfg, value)) = waiting {
                *waiter = InFlight::Waiting(cfg, value);
            }
        }
        loop {
            match self.in_flight.front_mut() {
                Some(&mut InFlight::Responding(ref mut fut)) => {
                    match fut.poll()? {
                        Async::Ready(conn) => {
                            self.conn = Some(conn);
                        }
                        Async::NotReady => return Ok(()),
                    }
                }
                Some(&mut InFlight::Waiting(..)) => {}
                _ => return Ok(()),
            };
            match self.in_flight.pop_front() {
                Some(InFlight::Responding(_)) => continue,
                Some(InFlight::Waiting(cfg, response)) => {
                    let conn = self.conn.take().expect("connection is owned");
                    self.in_flight.push_front(InFlight::Responding(
                        response.into_serializer(ResponseWriter::new(conn,
                            cfg.version, cfg.is_head, cfg.do_close))));
                }
                _ => unreachable!(),
            };
        }
    }
}

impl<T, S> Future for HttpServer<T, S>
    where T: Service<Request=Request, Error=Error>,
          T::Response: GenericResponse<S>,
          S: Io,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<(), Error> {
        if let Some(ref mut conn) = self.conn {
            conn.flush()?;
        }
        self.read_and_process()?;

        self.poll_waiters()?;

        if let Some(ref mut conn) = self.conn {
            conn.flush()?;
            if conn.done() {
                return Ok(Async::Ready(()));
            }
        }
        Ok(Async::NotReady)
    }
}
