use std::time::Duration;
use std::sync::Arc;

use server::{Config};

impl Config {
    /// Create a config with defaults
    pub fn new() -> Config {
        Config {
            inflight_request_limit: 2,
            inflight_request_prealloc: 0,
            first_byte_timeout: Duration::new(5, 0),
            keep_alive_timeout: Duration::new(90, 0),
            headers_timeout: Duration::new(10, 0),
            input_body_byte_timeout: Duration::new(15, 0),
            input_body_whole_timeout: Duration::new(3600, 0),
            output_body_byte_timeout: Duration::new(15, 0),
            output_body_whole_timeout: Duration::new(300, 0),
        }
    }
    /// A number of inflight requests until we stop reading more requests
    pub fn inflight_request_limit(&mut self, value: usize) -> &mut Self {
        self.inflight_request_limit = value;
        self
    }
    /// Size of the queue that is preallocated for holding requests
    ///
    /// Should be smaller than `inflight_request_limit`.
    pub fn inflight_request_prealoc(&mut self, value: usize) -> &mut Self {
        self.inflight_request_prealloc = value;
        self
    }
    /// Create a Arc'd config clone to pass to the constructor
    ///
    /// This is just a convenience method.
    pub fn done(&mut self) -> Arc<Config> {
        Arc::new(self.clone())
    }
    /// Timeout receiving very first byte over connection
    pub fn first_byte_timeout(&mut self, value: Duration) -> &mut Self {
        self.first_byte_timeout = value;
        self
    }
    /// Timeout of idle connection (when no request has been sent yet)
    pub fn keep_alive_timeout(&mut self, value: Duration) -> &mut Self {
        self.keep_alive_timeout = value;
        self
    }
    /// Timeout of receiving whole request headers
    ///
    /// This timeout starts when first byte of headers is received
    pub fn headers_timeout(&mut self, value: Duration) -> &mut Self {
        self.headers_timeout = value;
        self
    }
    /// Maximum delay between any two bytes of input request received
    pub fn input_body_byte_timeout(&mut self, value: Duration) -> &mut Self {
        self.input_body_byte_timeout = value;
        self
    }
    /// Timeout of whole request body received
    ///
    /// This timeout might be adjusted on per-request basis in
    /// `headers_received`.
    pub fn input_body_whole_timeout(&mut self, value: Duration) -> &mut Self {
        self.input_body_whole_timeout = value;
        self
    }
    /// Maximum delay between any two bytes of the output request could be
    /// sent
    pub fn output_body_byte_timeout(&mut self, value: Duration) -> &mut Self {
        self.output_body_byte_timeout = value;
        self
    }
    /// Timeout for the whole response body to be send to the client
    ///
    /// This timeout is taken literally for any response, so it must be
    /// as large as needed for slowest client fetching slowest file. I.e.
    /// it's fine if it's
    pub fn output_body_whole_timeout(&mut self, value: Duration) -> &mut Self {
        self.output_body_whole_timeout = value;
        self
    }
}
