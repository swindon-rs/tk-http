//! Http status codes helpers
//!

/// Enum with some HTTP Status codes.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Status {
    //  1xx status codes
    Continue,                       // 100
    SwitchingProtocol,              // 101
    //  2xx status codes
    Ok,                             // 200
    Created,                        // 201
    Accepted,                       // 202
    NonAuthoritativeInformation,    // 203
    NoContent,                      // 204
    ResetContent,                   // 205
    PartialContent,                 // 206
    //  3xx status codes
    MultipleChoices,                // 300
    MovedPermanently,               // 301
    Found,                          // 302
    SeeOther,                       // 303
    NotModified,                    // 304
    UseProxy,                       // 305
    TemporaryRedirect,              // 307
    PermanentRedirect,              // 308
    //  4xx status codes
    BadRequest,                     // 400
    Unauthorized,                   // 401
    PaymentRequired,                // 402
    Forbidden,                      // 403
    NotFound,                       // 404
    MethodNotAllowed,               // 405
    NotAcceptable,                  // 406
    ProxyAuthenticationRequired,    // 407
    RequestTimeout,                 // 408
    Conflict,                       // 409
    Gone,                           // 410
    LengthRequired,                 // 411
    PreconditionFailed,             // 412
    RequestEntityTooLarge,          // 413
    RequestURITooLong,              // 414
    UnsupportedMediaType,           // 415
    RequestRangeNotSatisfiable,     // 416
    ExpectationFailed,              // 417
    UpgradeRequired,                // 426
    TooManyRequests,                // 429
    //  5xx status codes
    InternalServerError,            // 500
    NotImplemented,                 // 501
    BadGateway,                     // 502
    ServiceUnavailable,             // 503
    GatewayTimeout,                 // 504
    VersionNotSupported,            // 505
}

/// Returns reason for specified status code.
impl Status {

    /// Returns 3 digit numeric code
    pub fn code(&self) -> u16 {
        match *self {
            //  1xx Status codes
            Status::Continue                        => 100,
            Status::SwitchingProtocol               => 101,
            //  2xx status codes
            Status::Ok                              => 200,
            Status::Created                         => 201,
            Status::Accepted                        => 202,
            Status::NonAuthoritativeInformation     => 203,
            Status::NoContent                       => 204,
            Status::ResetContent                    => 205,
            Status::PartialContent                  => 206,
            //  3xx status codes
            Status::MultipleChoices                 => 300,
            Status::MovedPermanently                => 301,
            Status::Found                           => 302,
            Status::SeeOther                        => 303,
            Status::NotModified                     => 304,
            Status::UseProxy                        => 305,
            Status::TemporaryRedirect               => 307,
            Status::PermanentRedirect               => 308,
            //  4xx status codes
            Status::BadRequest                      => 400,
            Status::Unauthorized                    => 401,
            Status::PaymentRequired                 => 402,
            Status::Forbidden                       => 403,
            Status::NotFound                        => 404,
            Status::MethodNotAllowed                => 405,
            Status::NotAcceptable                   => 406,
            Status::ProxyAuthenticationRequired     => 407,
            Status::RequestTimeout                  => 408,
            Status::Conflict                        => 409,
            Status::Gone                            => 410,
            Status::LengthRequired                  => 411,
            Status::PreconditionFailed              => 412,
            Status::RequestEntityTooLarge           => 413,
            Status::RequestURITooLong               => 414,
            Status::UnsupportedMediaType            => 415,
            Status::RequestRangeNotSatisfiable      => 416,
            Status::ExpectationFailed               => 417,
            Status::UpgradeRequired                 => 426,
            Status::TooManyRequests                 => 429,
            //  5xx status codes
            Status::InternalServerError             => 500,
            Status::NotImplemented                  => 501,
            Status::BadGateway                      => 502,
            Status::ServiceUnavailable              => 503,
            Status::GatewayTimeout                  => 504,
            Status::VersionNotSupported             => 505,
        }
    }

    /// Returns title for the status code
    pub fn reason(&self) -> &'static str {
        match self.code() {
            // 1xx codes;
            100 => "Continue",
            101 => "Switching Protocol",
            //  2xx codes
            200 => "OK",
            201 => "Created",
            202 => "Accepted",
            203 => "Non-Authoriative Information",
            204 => "No Content",
            205 => "Reset Content",
            206 => "Partial Content",
            //  3xx codes
            300 => "Multiple Choice",
            301 => "Moved Permanently",
            302 => "Found",
            303 => "See Other",
            304 => "Not Modified",
            305 => "Use Proxy",
            307 => "Temporary Redirect",
            308 => "Permanent Redirect",
            //  4xx codes
            400 => "Bad Request",
            401 => "Unauthorized",
            402 => "Payment Required",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            406 => "Not Acceptable",
            407 => "Proxy Authentication Required",
            408 => "Request Timeout",
            409 => "Conflict",
            410 => "Gone",
            411 => "Length Required",
            412 => "Precondition Failed",
            413 => "Request Entity Too Large",
            414 => "Request-URI Too Long",
            415 => "Unsupported Media Type",
            416 => "Request Range Not Satisfiable",
            417 => "Expectation Failed",
            426 => "Upgrade Required",
            429 => "Too Many Requests",
            //  5xx codes
            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            505 => "HTTP Version Not Supported",
            // Custom code
            _ => "Unknown",
        }
    }

    /// Returns true if sending body is expected for such status code
    pub fn response_has_body(&self) -> bool {
        match self.code() {
            100...199 | 204 | 304 => false,
            _ => true,
        }
    }

    /// Make Status from u16 if known code is passed.
    pub fn from(code: u16) -> Option<Status> {
        use self::Status::*;
        let s = match code {
            //  1xx
            100 => Continue,
            101 => SwitchingProtocol,
            //  2xx
            200 => Ok,
            201 => Created,
            202 => Accepted,
            203 => NonAuthoritativeInformation,
            204 => NoContent,
            205 => ResetContent,
            206 => PartialContent,
            //  3xx
            300 => MultipleChoices,
            301 => MovedPermanently,
            302 => Found,
            303 => SeeOther,
            304 => NotModified,
            305 => UseProxy,
            307 => TemporaryRedirect,
            308 => PermanentRedirect,
            //  4xx
            400 => BadRequest,
            401 => Unauthorized,
            402 => PaymentRequired,
            403 => Forbidden,
            404 => NotFound,
            405 => MethodNotAllowed,
            406 => NotAcceptable,
            407 => ProxyAuthenticationRequired,
            408 => RequestTimeout,
            409 => Conflict,
            410 => Gone,
            411 => LengthRequired,
            412 => PreconditionFailed,
            413 => RequestEntityTooLarge,
            414 => RequestURITooLong,
            415 => UnsupportedMediaType,
            416 => RequestRangeNotSatisfiable,
            417 => ExpectationFailed,
            426 => UpgradeRequired,
            429 => TooManyRequests,
            //  5xx
            500 => InternalServerError,
            501 => NotImplemented,
            502 => BadGateway,
            503 => ServiceUnavailable,
            504 => GatewayTimeout,
            505 => VersionNotSupported,
            _ => return None,
        };
        Some(s)
    }
}
