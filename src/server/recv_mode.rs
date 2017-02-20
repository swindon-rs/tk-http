use std::time::Duration;

use server::RecvMode;


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
            timeout: None,
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
            timeout: None,
        }
    }
    /// Don't read request body and hijack connection after response headers
    /// are sent. Useful for connection upgrades, including websockets and
    /// for CONNECT method.
    ///
    /// Note: `data_received` method of Codec is never called for `Hijack`d
    /// connection.
    pub fn hijack() -> RecvMode {
        RecvMode { mode: Mode::Hijack, timeout: None }
    }

    /// Change timeout for reading the whole request body to this value
    /// instead of configured default
    ///
    /// This might be useful if you have some specific slow routes and you
    /// can authenticate that request is valid enough. This is also useful for
    /// streaming large bodies and similar things.
    ///
    /// Or vice versa if you what shorter timeouts for suspicious host.
    pub fn body_read_timeout(mut self, duration: Duration) -> RecvMode {
        self.timeout = Some(duration);
        self
    }
}

pub fn get_mode(mode: &RecvMode) -> Mode {
    mode.mode
}
