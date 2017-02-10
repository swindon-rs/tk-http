/// This type is returned from `headers_received` handler of either
/// client client or server protocol handler
///
/// The marker is used to denote whether you want to have the whole response
/// buffered for you or read chunk by chunk.
///
/// The `Progressive` (chunk by chunk) mode is mostly useful for proxy servers.
/// Or it may be useful if your handler is able to parse data without holding
/// everything in the memory.
///
/// Otherwise, it's best to use `Buffered` mode (for example, comparing with
/// using your own buffering). We do our best to optimize it for you.
#[derive(Debug, Clone)]
pub struct RecvMode {
    mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    BufferedUpfront(usize),
    Progressive(usize),
    Hijack,
}

impl RecvMode {
    /// Download whole message body (request or response) into the memory
    /// before starting response
    ///
    /// The argument is maximum size of the body. The Buffered variant
    /// works equally well for Chunked encoding and for read-util-end-of-stream
    /// mode of HTTP/1.0, so sometimes you can't know the size of the request
    /// in advance. Note this is just an upper limit it's neither buffer size
    /// nor *minimum* size of the body.
    pub fn buffered_upfront(max_body_size: usize) -> RecvMode {
        RecvMode {
            mode: Mode::BufferedUpfront(max_body_size),
        }
    }
    /// Fetch data chunk-by-chunk.
    ///
    /// Note, your response handler can start either before or after
    /// progressive body has started or ended to read. I mean they are
    /// completely independent, and actual sequence of events depends on other
    /// requests coming in and performance of a client.
    ///
    /// The parameter denotes minimum number of bytes that may be passed
    /// to the protocol handler. This is for performance tuning (i.e. less
    /// wake-ups of protocol parser). But it's not an input buffer size. The
    /// use of `Progressive(1)` is perfectly okay (for example if you use http
    /// request body as a persistent connection for sending multiple messages
    /// on-demand)
    pub fn progressive(min_chunk_size_hint: usize) -> RecvMode {
        RecvMode {
            mode: Mode::Progressive(min_chunk_size_hint),
        }
    }
    /// Don't read request body and hijack connection after response headers
    /// are sent. Useful for connection upgrades, including websockets and
    /// for CONNECT method.
    ///
    /// Note: `data_received` method of Codec is never called for `Hijack`d
    /// connection.
    pub fn hijack() -> RecvMode {
        RecvMode { mode: Mode::Hijack }
    }
}

pub fn get_mode(mode: &RecvMode) -> Mode {
    mode.mode
}
