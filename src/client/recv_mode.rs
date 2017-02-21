use client::RecvMode;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Buffered(usize),
    Progressive(usize),
}


impl RecvMode {
    /// Download whole message body (request or response) into the memory.
    ///
    /// The argument is maximum size of the body. The Buffered variant
    /// works equally well for Chunked encoding and for read-util-end-of-stream
    /// mode of HTTP/1.0, so sometimes you can't know the size of the request
    /// in advance. Note this is just an upper limit it's neither buffer size
    /// nor *minimum* size of the body.
    ///
    /// Note the buffer size is asserted on if it's bigger than max buffer size
    pub fn buffered(maximum_size_of_body: usize) -> RecvMode {
        RecvMode {
            mode: Mode::Buffered(maximum_size_of_body),
        }
    }
    /// Fetch data chunk-by-chunk.
    ///
    /// The parameter denotes minimum number of bytes that may be passed
    /// to the protocol handler. This is for performance tuning (i.e. less
    /// wake-ups of protocol parser). But it's not an input buffer size. The
    /// use of `Progressive(1)` is perfectly okay (for example if you use http
    /// request body as a persistent connection for sending multiple messages
    /// on-demand)
    pub fn progressive(min_bytes_hint: usize) -> RecvMode {
        RecvMode {
            mode: Mode::Progressive(min_bytes_hint),
        }
    }
}
