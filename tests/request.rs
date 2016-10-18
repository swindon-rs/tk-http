extern crate futures;
extern crate netbuf;
extern crate minihttp;

use std::str;
use futures::Async;
use netbuf::Buf;

use minihttp::enums::{Method, Version};
use minihttp::request::Request;


#[test]
fn request() {
    let mut buf = Buf::new();
    buf.extend(b"GET /path HTTP/1.1\r\nHost: example.com\r\n\r\n");

    let res = Request::parse_from(&buf).unwrap();
    assert!(res.is_ready());
    if let Async::Ready((req, _)) = res {;
        // assert_eq!(, futures::Async::Ready(()));
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/path".to_string());
        assert_eq!(req.version, Version::Http11);

        assert_eq!(req.host().unwrap(), "example.com");
    }
}

#[test]
fn partial_request() {
    let mut buf = Buf::new();
    buf.extend(b"HEAD /path?with=query HTTP/1.1\r\n");

    let res = Request::parse_from(&buf).unwrap();
    assert!(res.is_not_ready());

    buf.extend(b"Host: www.example.com\r\n\r\n");

    let res = Request::parse_from(&buf).unwrap();
    assert!(res.is_ready());

    if let Async::Ready((req, _)) = res {;
        assert_eq!(req.method, Method::Head);
        assert_eq!(req.path, "/path?with=query".to_string());
        assert_eq!(req.version, Version::Http11);

        assert_eq!(req.host().unwrap(), "www.example.com");
    }
}
