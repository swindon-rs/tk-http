use std::io::{self, Write};
use std::collections::VecDeque;

use futures::{Future, Poll, Async};
use tokio_core::net::TcpStream;
use tokio_service::Service;
use netbuf::Buf;

use request::{Request, Body, response_config};
use serve::{ResponseConfig, ResponseWriter};
use {GenericResponse, Error};


enum InFlight<F, R>
    where F: Future<Item=R>,
          R: GenericResponse,
{
    Service(ResponseConfig, F),
    Waiting(ResponseConfig, R),
    Responding(R::Future),
}


pub struct HttpServer<T>
    where T: Service<Request=Request, Error=Error>,
          T::Response: GenericResponse,
{
    /// Socket and output buffer, it's None when connection is borrowed by
    ///
    conn: Option<(TcpStream, Buf)>,
    in_buf: Buf,
    request: Option<Request>,
    service: T,
    in_flight: VecDeque<InFlight<T::Future, T::Response>>,
    done: bool,
}

impl<T> HttpServer<T>
    where T: Service<Request=Request, Error=Error>,
          T::Response: GenericResponse,
{

    pub fn new(socket: TcpStream, service: T) -> HttpServer<T> {
        HttpServer {
            conn: Some((socket, Buf::new())),
            in_buf: Buf::new(),
            request: None,
            // out_body: None,
            service: service,
            in_flight: VecDeque::with_capacity(32),
            done: false,
        }
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn flush(&mut self) -> Poll<(), Error> {
        if let Some((ref mut sock, ref mut buf)) = self.conn {
            loop {
                match buf.write_to(sock) {
                    Ok(_) => break,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock
                    => {
                        break;
                    }
                    Err(e) => {
                        return Err(e.into());
                    },
                }
            }
            match sock.flush() {
                Ok(_) => {
                    Ok(Async::Ready(()))
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    Ok(Async::NotReady)
                },
                Err(e) => Err(e.into()),
            }
        } else {
            Ok(Async::Ready(()))
        }
    }

    fn read_and_process(&mut self) -> Poll<(), Error> {
        loop {
            while !try!(self.parse_request()) {
                match try!(self.read_in()) {
                    Async::Ready(0) => {
                        self.done = true;
                        return Ok(Async::Ready(()))
                    },
                    Async::Ready(_) => {},
                    Async::NotReady => return Ok(Async::NotReady),
                }
            }
            while !try!(self.parse_body()) {
                match try!(self.read_in()) {
                    Async::Ready(0) => {
                        self.done = true;
                        return Ok(Async::Ready(()))
                    },
                    Async::Ready(_) => {},
                    Async::NotReady => return Ok(Async::NotReady),
                }
            }
            let req = self.request.take().unwrap();
            let cfg = response_config(&req);
            let waiter = self.service.call(req);
            self.in_flight.push_back((InFlight::Service(cfg, waiter)));
        }
    }

    fn read_in(&mut self) -> Poll<usize, io::Error> {
        if let Some((ref mut sock, _)) = self.conn {
            match self.in_buf.read_from(sock) {
                Ok(size) => {
                    return Ok(Async::Ready(size));
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(Async::NotReady)
                },
                Err(e) => return Err(e),
            }
        } else {
            Ok(Async::NotReady)
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
        if self.request.as_ref().unwrap().body.is_some() {
            return Ok(true)
        }
        let mut req = self.request.as_mut().unwrap();
        match try!(Body::parse_from(&req, &mut self.in_buf)) {
            Async::Ready(None) => Ok(true),
            Async::Ready(Some(size)) => {
                let mut buf = Buf::new();
                buf.extend(&self.in_buf[..size]);
                self.in_buf.consume(size);
                req.body = Some(Body::new(buf));
                Ok(true)
            },
            Async::NotReady => Ok(false),
        }
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
                    match try!(fut.poll()) {
                        Async::Ready((sock, buf)) => {
                            self.conn = Some((sock, buf));
                        }
                        Async::NotReady => return Ok(()),
                    }
                }
                Some(&mut InFlight::Waiting(..)) => {}
                _ => return Ok(()),
            };
            let (sock, buf) = self.conn.take().expect("connection is owned");
            match self.in_flight.pop_front() {
                Some(InFlight::Responding(_)) => continue,
                Some(InFlight::Waiting(cfg, response)) => {
                    self.in_flight.push_front(InFlight::Responding(
                        response.make_serializer(ResponseWriter::new(sock, buf,
                            cfg.version, cfg.is_head, cfg.do_close))));
                }
                _ => unreachable!(),
            };
        }
    }
}

impl<T> Future for HttpServer<T>
    where T: Service<Request=Request, Error=Error>,
          T::Response: GenericResponse,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<(), Error> {
        try!(self.flush());

        try!(self.read_and_process());

        try!(self.poll_waiters());

        try!(self.flush());

        if self.is_done() {
            return Ok(Async::Ready(()));
        }
        Ok(Async::NotReady)
    }
}
