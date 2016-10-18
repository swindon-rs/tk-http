
pub trait HttpStatus {

    fn code(&self) -> u16;

    fn reason(&self) -> &str;

    fn has_body(&self) -> bool;
}

/// Enum with some HTTP Status codes.
///
/// Contains two versions per status code, eg:
/// `Status::Http200` / `Status::Ok`
#[derive(Debug, PartialEq)]
pub enum Status {
    // custom http code
    Raw(u16, String),
    //  1xx status codes
    Http100, Continue,
    Http101, SwitchingProtocol,
    //  2xx status codes
    Http200, Ok,                             // 200
    Http201, Created,                        // 201
    Http202, Accepted,                       // 202
    Http203, NonAuthoritativeInformation,    // 203
    Http204, NoContent,                      // 204
    Http205, ResetContent,                   // 205
    Http206, PartialContent,                 // 206
    //  3xx status codes
    Http300, MultipleChoices,                // 300
    Http301, MovedPermanently,               // 301
    Http302, Found,                          // 302
    Http303, SeeOther,                       // 303
    Http304, NotModified,                    // 304
    Http305, UseProxy,                       // 305
    Http307, TemporaryRedirect,              // 307
    Http308, PermanentRedirect,              // 308
    //  4xx status codes
    Http400, BadRequest,                     // 400
    Http401, Unauthorized,                   // 401
    Http402, PaymentRequired,                // 402
    Http403, Forbidden,                      // 403
    Http404, NotFound,                       // 404
    Http405, MethodNotAllowed,               // 405
    Http406, NotAcceptable,                  // 406
    Http407, ProxyAuthenticationRequired,    // 407
    Http408, RequestTimeout,                 // 408
    Http409, Conflict,                       // 409
    Http410, Gone,                           // 410
    Http411, LengthRequired,                 // 411
    Http412, PreconditionFailed,             // 412
    Http413, RequestEntityTooLarge,          // 413
    Http414, RequestURITooLong,              // 414
    Http415, UnsupportedMediaType,           // 415
    Http416, RequestRangeNotSatisfiable,     // 416
    Http417, ExpectationFailed,              // 417
    Http426, UpgradeRequired,                // 426
    Http429, TooManyRequests,                // 429
    //  5xx status codes
    Http500, InternalServerError,            // 500
    Http501, NotImplemented,                 // 501
    Http502, BadGateway,                     // 502
    Http503, ServiceUnavailable,             // 503
    Http504, GatewayTimeout,                 // 504
    Http505, VersionNotSupported,            // 505
}

impl HttpStatus for Status {
    fn code(&self) -> u16 {
        match *self {
            //  1xx Status codes
            Status::Http100 | Status::Continue                        => 100,
            Status::Http101 | Status::SwitchingProtocol               => 101,
            //  2xx status codes
            Status::Http200 | Status::Ok                            => 200,
            Status::Http201 | Status::Created                       => 201,
            Status::Http202 | Status::Accepted                      => 202,
            Status::Http203 | Status::NonAuthoritativeInformation   => 203,
            Status::Http204 | Status::NoContent                     => 204,
            Status::Http205 | Status::ResetContent                  => 205,
            Status::Http206 | Status::PartialContent                => 206,
            //  3xx status codes
            Status::Http300 | Status::MultipleChoices               => 300,
            Status::Http301 | Status::MovedPermanently              => 301,
            Status::Http302 | Status::Found                         => 302,
            Status::Http303 | Status::SeeOther                      => 303,
            Status::Http304 | Status::NotModified                   => 304,
            Status::Http305 | Status::UseProxy                      => 305,
            Status::Http307 | Status::TemporaryRedirect             => 307,
            Status::Http308 | Status::PermanentRedirect             => 308,
            //  4xx status codes
            Status::Http400 | Status::BadRequest                    => 400,
            Status::Http401 | Status::Unauthorized                  => 401,
            Status::Http402 | Status::PaymentRequired               => 402,
            Status::Http403 | Status::Forbidden                     => 403,
            Status::Http404 | Status::NotFound                      => 404,
            Status::Http405 | Status::MethodNotAllowed              => 405,
            Status::Http406 | Status::NotAcceptable                 => 406,
            Status::Http407 | Status::ProxyAuthenticationRequired   => 407,
            Status::Http408 | Status::RequestTimeout                => 408,
            Status::Http409 | Status::Conflict                      => 409,
            Status::Http410 | Status::Gone                          => 410,
            Status::Http411 | Status::LengthRequired                => 411,
            Status::Http412 | Status::PreconditionFailed            => 412,
            Status::Http413 | Status::RequestEntityTooLarge         => 413,
            Status::Http414 | Status::RequestURITooLong             => 414,
            Status::Http415 | Status::UnsupportedMediaType          => 415,
            Status::Http416 | Status::RequestRangeNotSatisfiable    => 416,
            Status::Http417 | Status::ExpectationFailed             => 417,
            Status::Http426 | Status::UpgradeRequired               => 426,
            Status::Http429 | Status::TooManyRequests               => 429,
            //  5xx status codes
            Status::Http500 | Status::InternalServerError           => 500,
            Status::Http501 | Status::NotImplemented                => 501,
            Status::Http502 | Status::BadGateway                    => 502,
            Status::Http503 | Status::ServiceUnavailable            => 503,
            Status::Http504 | Status::GatewayTimeout                => 504,
            Status::Http505 | Status::VersionNotSupported           => 505,
            Status::Raw(code, _) => code,
        }
    }

    fn reason<'a>(&'a self) -> &'a str {
        match *self {
            //  1xx Status codes
            Status::Http100 | Status::Continue                        => "Continue",
            Status::Http101 | Status::SwitchingProtocol               => "Switching Protocol",
            //  2xx status codes
            Status::Http200 | Status::Ok                            => "OK",
            Status::Http201 | Status::Created                       => "Created",
            Status::Http202 | Status::Accepted                      => "Accepted",
            Status::Http203 | Status::NonAuthoritativeInformation   => "Non-Authoriative Information",
            Status::Http204 | Status::NoContent                     => "No Content",
            Status::Http205 | Status::ResetContent                  => "Reset Content",
            Status::Http206 | Status::PartialContent                => "Partial Content",
            //  3xx status codes
            Status::Http300 | Status::MultipleChoices               => "Multiple Choice",
            Status::Http301 | Status::MovedPermanently              => "Moved Permanently",
            Status::Http302 | Status::Found                         => "Found",
            Status::Http303 | Status::SeeOther                      => "See Other",
            Status::Http304 | Status::NotModified                   => "Not Modified",
            Status::Http305 | Status::UseProxy                      => "Use Proxy",
            Status::Http307 | Status::TemporaryRedirect             => "Temporary Redirect",
            Status::Http308 | Status::PermanentRedirect             => "Permanent Redirect",
            //  4xx status codes
            Status::Http400 | Status::BadRequest                    => "Bad Request",
            Status::Http401 | Status::Unauthorized                  => "Unauthorized",
            Status::Http402 | Status::PaymentRequired               => "Payment Required",
            Status::Http403 | Status::Forbidden                     => "Forbidden",
            Status::Http404 | Status::NotFound                      => "Not Found",
            Status::Http405 | Status::MethodNotAllowed              => "Method Not Allowed",
            Status::Http406 | Status::NotAcceptable                 => "Not Acceptable",
            Status::Http407 | Status::ProxyAuthenticationRequired   => "Proxy Authentication Required",
            Status::Http408 | Status::RequestTimeout                => "Request Timeout",
            Status::Http409 | Status::Conflict                      => "Conflict",
            Status::Http410 | Status::Gone                          => "Gone",
            Status::Http411 | Status::LengthRequired                => "Length Required",
            Status::Http412 | Status::PreconditionFailed            => "Precondition Failed",
            Status::Http413 | Status::RequestEntityTooLarge         => "Request Entity Too Large",
            Status::Http414 | Status::RequestURITooLong             => "Request-URI Too Long",
            Status::Http415 | Status::UnsupportedMediaType          => "Unsupported Media Type",
            Status::Http416 | Status::RequestRangeNotSatisfiable    => "Request Range Not Satisfiable",
            Status::Http417 | Status::ExpectationFailed             => "Expectation Failed",
            Status::Http426 | Status::UpgradeRequired               => "Upgrade Required",
            Status::Http429 | Status::TooManyRequests               => "Too Many Requests",
            //  5xx status codes
            Status::Http500 | Status::InternalServerError           => "Internal Server Error",
            Status::Http501 | Status::NotImplemented                => "Not Implemented",
            Status::Http502 | Status::BadGateway                    => "Bad Gateway",
            Status::Http503 | Status::ServiceUnavailable            => "Service Unavailable",
            Status::Http504 | Status::GatewayTimeout                => "Gateway Timeout",
            Status::Http505 | Status::VersionNotSupported           => "HTTP Version Not Supported",
            Status::Raw(_, ref reason) => reason,
        }
    }

    fn has_body(&self) -> bool {
        match *self {
            _ => true,
        }
    }
}
