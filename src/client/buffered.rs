//! Simple to use wrappers for dealing with fully buffered requests
//!
//! By "fully buffered" I mean two things:
//!
//! * No request or response streaming
//! * All headers and body are allocated on the heap
//!
//! Raw interface allows more granular control to make things more efficient,
//! but requires more boilerplate. You can mix and match different
//! styles on single HTTP connection.
//!
use url::Url;
use futures::Async;
use futures::future::{FutureResult, ok};
use futures::sync::oneshot::{channel, Sender, Receiver};
use tokio_core::io::Io;

use enums::Status;
use enums::Version;
use client::{Error, Codec, Encoder, EncoderDone, Head, RecvMode};

/// Fully buffered (in-memory) writing request and reading response
///
/// This coded should be used when you don't have any special needs
pub struct Buffered {
    method: &'static str,
    url: Url,
    sender: Option<Sender<Result<Response, Error>>>,
    response: Option<Response>,
    max_response_length: usize,
}

#[derive(Debug)]
/// A buffered response holds contains a body as contiguous chunk of data
pub struct Response {
    status: Status,
    headers: Vec<(String, Vec<u8>)>,
    body: Vec<u8>,
}

impl Response {
    /// Get response status
    pub fn status(&self) -> Status {
        self.status
    }
    /// Get response headers
    pub fn headers(&self) -> &[(String, Vec<u8>)] {
        &self.headers
    }
    /// Get response body
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl<S: Io> Codec<S> for Buffered {
    type Future = FutureResult<EncoderDone<S>, Error>;
    fn start_write(&mut self, mut e: Encoder<S>) -> Self::Future {
        e.request_line(self.method, self.url.path(), Version::Http11);
        self.url.host_str().map(|x| {
            e.add_header("Host", x).unwrap();
        });
        e.done_headers().unwrap();
        ok(e.done())
    }
    fn headers_received(&mut self, headers: &Head) -> Result<RecvMode, Error> {
        let status = headers.status().ok_or(Error::InvalidStatus)?;
        self.response = Some(Response {
            status: status,
            headers: headers.headers().map(|(k, v)| {
                (k.to_string(), v.to_vec())
            }).collect(),
            body: Vec::new(),
        });
        Ok(RecvMode::Buffered(self.max_response_length))
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>
    {
        assert!(end);
        let mut response = self.response.take().unwrap();
        response.body = data.to_vec();
        self.sender.take().unwrap().complete(Ok(response));
        Ok(Async::Ready(data.len()))
    }
}

impl Buffered {
    /// Fetch data from url using GET method, fully buffered
    pub fn get(url: Url) -> (Buffered, Receiver<Result<Response, Error>>) {
        let (tx, rx) = channel();
        (Buffered {
                method: "GET",
                url: url,
                sender: Some(tx),
                max_response_length: 10_485_760,
                response: None,
            },
         rx)
    }
    /// Set max response length for this buffered reader
    pub fn max_response_length(&mut self, value: usize) {
        self.max_response_length = value;
    }
}
