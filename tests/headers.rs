extern crate minihttp;

use minihttp::enums::Method;
use minihttp::enums::Header;


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
fn header_from_str() {
    assert_eq!(Header::from("host"), Header::Host);
    assert_eq!(Header::from("Host"), Header::Host);
    assert_eq!(Header::from("Connection"), Header::Connection);

    assert_eq!(Header::from("X-Some-Header"), Header::Raw("X-Some-Header".into()));
}
