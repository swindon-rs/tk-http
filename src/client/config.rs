use std::sync::Arc;
use std::time::Duration;

use client::{Config};

impl Config {
    /// Create a config with defaults
    pub fn new() -> Config {
        Config {
            inflight_request_limit: 1,
            inflight_request_prealloc: 1,
            keep_alive_timeout: Duration::new(4, 0),
            safe_pipeline_timeout: Duration::from_millis(300),
            max_request_timeout: Duration::new(15, 0),
        }
    }
    /// A number of inflight requests until we start returning
    /// `NotReady` from `start_send`
    ///
    /// Note we always return `NotReady` if some *request* is streaming
    /// currently. Use `Sink::buffered` to prevent that.
    ///
    /// Note 2: you might also need to tweak `safe_pipeline_timeout` to
    /// make pipelining work.
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

    /// Keep-alive timeout
    ///
    /// This is maximum time connection is kept alive when idle. We can't
    /// reliably detect when server closed connection at the remote end, in
    /// some cases (e.g. when remote server crashed).
    ///
    /// Also, there is a race condition between server closing the connection
    /// and client sending new request. So this timeout should usually be less
    /// than keep-alive timeout at server side.
    ///
    /// Note: default is very much conservative (currently 4 seconds, but we
    /// might change it).
    pub fn keep_alive_timeout(&mut self, dur: Duration) -> &mut Self {
        self.keep_alive_timeout = dur;
        self
    }

    /// Maximum time peer doesn't answer request before we consider this
    /// connection can't be used for pipelining more requests.
    ///
    /// Note: when this timeout is reached more requests can already be
    /// sent using this connection. This number only prevents further ones.
    /// You must disable pipelining at all if loosing or retrying requests is
    /// destructive for your application.
    ///
    /// Note: we have a very conservative default (currently 300 ms, but we
    /// might change it in future). There are two reasons for this, both
    /// might not apply to your specific setup:
    ///
    /// 1. We think that pipelining only makes sense in very high performance
    ///    situation where latency is comparable to network latency
    /// 2. In rust world everybody thinks peformance is fabulous
    pub fn safe_pipeline_timeout(&mut self, dur: Duration) -> &mut Self {
        self.safe_pipeline_timeout = dur;
        self
    }

    /// Absolute maximum time of whole request can work
    ///
    /// The rule of thumb: this must be the maximum time any your request
    /// can take from first byte sent to the last byte received.
    ///
    /// # Details
    ///
    /// This timeout is a subject of two contradictory goals:
    ///
    /// 1. Limit number of open (and hanging) connections
    /// 2. Tolerate peak load on the server (and don't let requests repeat,
    ///    when unneccessary)
    ///
    /// Note: while you can limit time you're waiting for each individual
    /// request by dropping a future, we don't have cancellation of already
    /// sent requests. So while your business logic will not hang for too
    /// long, connection hangs for `max_request_timeout` time and occupies
    /// connection pool's slot for this time.
    ///
    /// Default timeout is 15 seconds (which is both too large for many
    /// applications and too small for some ones)
    pub fn max_request_timeout(&mut self, dur: Duration) -> &mut Self {
        self.max_request_timeout = dur;
        self
    }

    /// Create a Arc'd config clone to pass to the constructor
    ///
    /// This is just a convenience method.
    pub fn done(&mut self) -> Arc<Config> {
        Arc::new(self.clone())
    }
}
