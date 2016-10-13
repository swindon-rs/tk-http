extern crate futures;
extern crate httparse;
extern crate netbuf;
extern crate minihttp;

use std::str;
use std::io;
use std::io::{Read, Write};
use futures::Async;
use netbuf::Buf;

use minihttp::headers::Method;
use minihttp::request::Request;


#[test]
fn method_from_str() {
    assert_eq!(Method::from("GET"), Method::Get);
    assert_eq!(Method::from("get"), Method::Other("get".to_string()));
    assert_eq!(Method::from("Get"), Method::Other("Get".to_string()));

    assert_eq!(Method::from("OPTIONS"), Method::Options);
    assert_eq!(Method::from("GET"), Method::Get);
    assert_eq!(Method::from("HEAD"), Method::Head);
    assert_eq!(Method::from("POST"), Method::Post);
    assert_eq!(Method::from("PUT"), Method::Put);
    assert_eq!(Method::from("DELETE"), Method::Delete);
    assert_eq!(Method::from("TRACE"), Method::Trace);
    assert_eq!(Method::from("CONNECT"), Method::Connect);
}

#[test]
fn debug_fmt() {
    assert_eq!(format!("{:?}", Method::Get), "Get");
    assert_eq!(format!("{:?}", Method::Other("patch".to_string())),
               "Other(\"patch\")");
}


#[test]
fn request() {
    let mut buf = Buf::new();
    buf.extend(b"GET /path HTTP/1.1\r\nHost: example.com\r\n\r\n");

    let res = Request::parse_from(&buf).unwrap();
    assert!(res.is_ready());
    if let Async::Ready((req, bytes)) = res {;
        // assert_eq!(, futures::Async::Ready(()));
        assert_eq!(req.method, Method::Get);
        assert_eq!(req.path, "/path".to_string());
        assert_eq!(req.version, 1);

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

    if let Async::Ready((req, bytes)) = res {;
        assert_eq!(req.method, Method::Head);
        assert_eq!(req.path, "/path?with=query".to_string());
        assert_eq!(req.version, 1);

        assert_eq!(req.host().unwrap(), "www.example.com");
    }
}
