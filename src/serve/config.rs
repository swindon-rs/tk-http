use {Version};

/// This structure contains all needed info to start response of the request
/// in a correct manner
///
/// This is ought to be used in serializer only
pub struct ResponseConfig {
    /// Whether request is a HEAD request
    pub is_head: bool,
    /// Is `Connection: close` in request or HTTP version == 1.0
    pub do_close: bool,
    /// Version of HTTP request
    pub version: Version,
}
