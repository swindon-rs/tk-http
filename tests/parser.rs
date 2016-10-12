extern crate httparse;
extern crate minihttp;

// use std::io;

// use minihttp::parser::{parse, State};
// use minihttp::request::{Request, Method};


// #[test]
// fn parse_ok() {
//     let buf = "\
//         GET /path HTTP/1.1\r\n\
//         HOST: example.com\r\n\
//         \r\n";
// 
//     let mut headers = [httparse::EMPTY_HEADER; 16];
//     let mut req = Request::new(&mut headers);
//     let res = parse(buf.as_bytes(), State::NewRequest, &mut req).unwrap();
//     assert_eq!(res, State::ProcessHeaders);
// }
// 
// #[test]
// fn parse_partial() {
//     let buf = "\
//         GET / HTTP/1.1\r\n\
//         Host: example.com\r\n\
//         \r\n";
//     let mut headers = [httparse::EMPTY_HEADER; 16];
//     let mut req = Request::new(&mut headers);
//     let bytes = buf.as_bytes();
//     let res = parse(&bytes[..bytes.len()-4], State::NewRequest, &mut req).unwrap();
//     assert_eq!(res, State::ReadingRequest);
// 
//     let res = parse(&bytes, res, &mut req).unwrap();
//     assert_eq!(res, State::ProcessHeaders);
// }
// 
// #[test]
// fn parse_request_error() {
//     let buf = "GET / HTTP\r\n\r\n";
//     let mut headers = [httparse::EMPTY_HEADER; 16];
//     let mut req = Request::new(&mut headers);
//     match parse(&buf.as_bytes(), State::NewRequest, &mut req) {
//         Err(e) => {
//             assert_eq!(e.kind(), io::ErrorKind::Other);
//             assert_eq!(e.to_string(), "invalid HTTP version".to_string());
//         },
//         Ok(_) => unreachable!(),
//     }
// }
// 
// #[test]
// fn parse_read_headers() {
//     let buf = "\
//         GET / HTTP/1.1\r\n\
//         Host: example.com:80\r\n\
//         \r\n";
//     let mut headers = [httparse::EMPTY_HEADER; 16];
//     let mut req = Request::new(&mut headers);
//     let res = parse(buf.as_bytes(), State::NewRequest, &mut req).unwrap();
// }
