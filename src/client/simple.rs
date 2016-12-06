use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::Arc;

use url::{Url, Host};
use futures::{IntoFuture, Future, Sink};
use abstract_ns::{Resolver, Error as NsError};
use futures_cpupool::CpuPool;
use ns_std_threaded::ThreadedResolver;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;

use {OptFuture};
use client::errors::Error;
use client::proto::Proto;
use client::buffered::{Buffered, Response};
use client::Config;


/// This is a simplistic function to just do a GET request for an url
/// and return result.
///
/// This function:
/// * Starts `ThreadedResolver` with one thread
/// * Resolves a name
/// * Connects to a random peer
/// * Fetches result into a buffer with maximum size of 10Mb
/// * Closes everything
///
/// So it's very inefficient but good enough for command-line tools or example
/// scripts.
pub fn fetch_once_buffered(url: Url, handle: &Handle)
    -> Box<Future<Item=Response, Error=Error>>
{
    let handle = handle.clone();
    if !url.has_host() || url.scheme() != "http" {
        return Box::new(Err(Error::UnsupportedScheme).into_future());
    }
    let port = url.port().unwrap_or(80);
    Box::new(match url.host().unwrap() {
        Host::Domain(dom) => {
            let ns = ThreadedResolver::new(CpuPool::new(1));
            OptFuture::Future(ns.resolve(&format!("{}:{}", dom, port))
                .map_err(Error::Name).boxed())
        }
        Host::Ipv4(addr) => {
            OptFuture::Value(Ok([
                SocketAddr::V4(SocketAddrV4::new(addr, port))
            ].iter().cloned().collect()))
        }
        Host::Ipv6(addr) => {
            OptFuture::Value(Ok([
                SocketAddr::V6(SocketAddrV6::new(addr, port, 0, 0))
            ].iter().cloned().collect()))
        }
    }.and_then(|addr| {
        addr.pick_one().ok_or(NsError::NameNotFound).map_err(Error::Name)
    }).and_then(move |addr| {
        TcpStream::connect(&addr, &handle).map_err(Error::Io)
    }).and_then(|sock| {
        let (codec, receiver) = Buffered::get(url);
        let proto = Proto::new(sock, &Arc::new(Config::new()));
        proto.send(codec)
        .map(|_| -> Response { unreachable!() })
        .select(receiver.map_err(|_| -> Error { unimplemented!() }))
        .map(|(response, _)| {
            response
        })
        .map_err(|(e, _)| e)
    })) as Box<Future<Item=Response, Error=Error>>
}
