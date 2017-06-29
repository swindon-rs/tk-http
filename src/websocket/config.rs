use std::time::Duration;
use std::sync::Arc;

use websocket::{Config};

impl Config {
    /// Create a config with defaults
    pub fn new() -> Config {
        Config {
            ping_interval: Duration::new(10, 0),
            inactivity_timeout: Duration::new(30, 0),
            max_packet_size: 10 << 20,
        }
    }
    /// Set ping interval
    ///
    /// Default is 10 seconds.
    ///
    /// If no messages have been received within this interval, we send
    /// a ping message. Only full messages are accounted. If some large
    /// frame is being received for this long, we still send ping.
    ///
    /// Note: you can't remove the interval, but you can set it to
    /// a sufficiently large value.
    ///
    /// Note 2: you may also need to tune inactivity timeout if you change
    /// this value.
    pub fn ping_interval(&mut self, dur: Duration) -> &mut Self {
        self.ping_interval = dur;
        self
    }

    /// Set inactivity timeout
    ///
    /// Default is 25 seconds.
    ///
    /// A connection is shut down if no messages were received during this
    /// interval.
    ///
    /// Note: only full frames are accounted. If some very large frame is
    /// being sent too long, we drop the connection. So be sure to set this
    /// value large enough so that slowest client can send largest frame and
    /// another ping.
    ///
    /// There are two use cases for this interval:
    ///
    /// 1. Make it 2.5x the ping_interval to detect connections which
    ///    don't have destination host alive
    ///
    /// 2. Inactivity interval that is smaller than `ping_interval` will
    ///    detect connections which are alive but do not send any messages.
    ///    This is similar to how HTTP servers shutdown inactive connections.
    ///
    /// Note: you may also need to tune ping interval if you change
    /// this value.
    pub fn inactivity_timeout(&mut self, dur: Duration) -> &mut Self {
        self.inactivity_timeout = dur;
        self
    }

    /// Maximum packet size
    ///
    /// If some frame declares size larger than this, we immediately abort
    /// the connection
    pub fn max_packet_size(&mut self, size: usize) -> &mut Self {
        self.max_packet_size = size;
        self
    }

    /// Create a Arc'd config clone to pass to the constructor
    ///
    /// This is just a convenience method.
    pub fn done(&mut self) -> Arc<Config> {
        Arc::new(self.clone())
    }
}
