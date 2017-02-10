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
}
