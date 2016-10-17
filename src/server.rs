use std::io;
// use std::io::Write;
use std::collections::VecDeque;

use futures::{Future, Poll, Async};
use tokio_core::io::{Io};
use netbuf::Buf;

use request::{Request, Body};
use response::Response;
// use error::Error;


pub type HttpError = io::Error;
pub type HttpPoll = Poll<(), HttpError>;


pub trait HttpService {
    type Request;
    type Response;
    type Error;
    type Future: Future<Item=Self::Response, Error=Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future;

    fn poll_ready(&self) -> Async<()> { Async::Ready(()) }
}

pub trait NewHandler
{
    type Handler;

    fn new_handler(&self) -> Self::Handler;
}


pub struct HttpServer<T, S>
    where S: Io,
          T: HttpService<Request=Request, Response=Response, Error=HttpError>,
{
    socket: S,
    in_buf: Buf,
    request: Option<Request>,
    out_buf: Buf,
    // out_body: Option<Buf>,
    service: T,
    in_flight: VecDeque<InFlight<T::Future>>,
}

impl<T, S> HttpServer<T, S>
    where S: Io,
          T: HttpService<Request=Request, Response=Response, Error=HttpError>,
{

    pub fn new(socket: S, service: T) -> HttpServer<T, S> {
        HttpServer {
            socket: socket,
            in_buf: Buf::new(),
            out_buf: Buf::new(),
            request: None,
            // out_body: None,
            service: service,
            in_flight: VecDeque::with_capacity(32),
        }
    }

    fn flush(&mut self) -> HttpPoll {
        match self.socket.flush() {
            Ok(_) => {
                Ok(Async::Ready(()))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Ok(Async::NotReady)
            },
            Err(e) => Err(e),
        }
    }

    fn is_done(&self) -> bool {
        false
    }

    fn read_and_process(&mut self) -> HttpPoll {
        loop {
            while !try!(self.parse_request()) {
                match try!(self.read_in()) {
                    Async::Ready(0) => return Ok(Async::Ready(())),
                    Async::Ready(_) => {},
                    Async::NotReady => return Ok(Async::NotReady),
                }
            }
            while !try!(self.parse_body()) {
                match try!(self.read_in()) {
                    Async::Ready(0) => return Ok(Async::Ready(())),
                    Async::Ready(_) => {},
                    Async::NotReady => return Ok(Async::NotReady),
                }
            }
            let req = self.request.take().unwrap();
            let waiter = self.service.call(req);
            self.in_flight.push_back(InFlight::Active(waiter));
        }
    }

    fn read_in(&mut self) -> Poll<usize, io::Error> {
        match self.in_buf.read_from(&mut self.socket) {
            Ok(size) => {
                return Ok(Async::Ready(size));
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                return Ok(Async::NotReady)
            },
            Err(e) => return Err(e),
        }
    }

    fn parse_request(&mut self) -> Result<bool, io::Error> {
        if self.request.is_none() {
            match try!(Request::parse_from(&self.in_buf)) {
                Async::NotReady => return Ok(false),
                Async::Ready((req, size)) => {
                    self.in_buf.consume(size);
                    self.request = Some(req);
                },
            }
        }
        Ok(true)
    }

    fn parse_body(&mut self) -> Result<bool, io::Error> {
        assert!(self.request.is_some());
        let mut req = self.request.as_mut().unwrap();
        match try!(Body::parse_from(&req, &mut self.in_buf)) {
            Async::Ready(size) => {
                let mut buf = Buf::new();
                buf.extend(&self.in_buf[..size]);
                self.in_buf.consume(size);
                req.body = Some(Body::new(buf));
                Ok(true)
            },
            Async::NotReady => Ok(false),
        }
    }

    fn write_and_dispose(&mut self) -> HttpPoll {
        // if future is done -> start writing response;
        // if response has body: schedule to send body;
        if let Some(res) = self.poll_waiters() {
            let resp = try!(res);
            try!(resp.write_to(&mut self.out_buf));
        };

        loop {
            match self.out_buf.write_to(&mut self.socket) {
                Ok(_) => {
                    return Ok(Async::Ready(()));
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(Async::NotReady);
                },
                Err(e) => {
                    return Err(e);
                },
            }
        }
    }

    fn poll_waiters(&mut self) -> Option<Result<T::Response, T::Error>> {
        for waiter in self.in_flight.iter_mut() {
            waiter.poll();
        }
        match self.in_flight.front() {
            Some(&InFlight::Done(_)) => {},
            _ => return None,
        };
        match self.in_flight.pop_front() {
            Some(InFlight::Done(res)) => Some(res),
            _ => None
        }
    }
}

impl<T, S> Future for HttpServer<T, S>
    where S: Io,
          T: HttpService<Request=Request, Response=Response, Error=HttpError>,
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> HttpPoll {
        try!(self.flush());

        try!(self.read_and_process());

        try!(self.write_and_dispose());

        try!(self.flush());

        if self.is_done() {
            return Ok(Async::Ready(()));
        }
        Ok(Async::NotReady)
    }
}


enum InFlight<F>
    where F: Future,
{
    Active(F),
    Done(Result<F::Item, F::Error>),
}

impl<F: Future> InFlight<F> {
    pub fn poll(&mut self) {
        let res = match *self {
            InFlight::Active(ref mut f) => {
                match f.poll() {
                    Ok(Async::Ready(res)) => Ok(res),
                    Ok(Async::NotReady) => return,
                    Err(e) => Err(e),
                }
            },
            _ => return
        };
        *self = InFlight::Done(res);
    }
}
