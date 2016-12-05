extern crate url;
extern crate minihttp;
extern crate argparse;
extern crate env_logger;
extern crate tokio_core;
#[macro_use] extern crate log;

use std::io::{self, Write};
use std::env;
use std::fs::File;
use std::path::{PathBuf, Path};
use std::str::FromStr;

use url::Url;
use argparse::{ArgumentParser, Store, ParseOption};


pub struct Options {
    pub url: Url,
    pub dump_header: Option<PathBuf>,
}


pub fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "warn");
    }
    env_logger::init().unwrap();

    let mut opt = Options {
        url: Url::from_str("http://localhost").unwrap(),
        dump_header: None,
    };
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut opt.url)
            .add_argument("url", Store, "
                Fetch specified url
            ").required();
        ap.refer(&mut opt.dump_header)
            .add_option(&["-D", "--dump-header"], ParseOption,
                "Header is already added");
        ap.parse_args_or_exit();
    }

    let mut lp = tokio_core::reactor::Core::new().expect("loop created");
    let handle = lp.handle();

    let response = lp.run(
        minihttp::client::fetch(opt.url, &handle)
    ).unwrap();
    if let Some(filename) = opt.dump_header {
        let mut out: Box<io::Write> = if filename == Path::new("-") {
            Box::new(io::stdout())
        } else {
            Box::new(
                File::create(filename).expect("can't open file for headers"))
        };
        writeln!(&mut out, "HTTP/1.x {} {}",
            response.code(), response.reason()).unwrap();
        for &(ref k, ref v) in response.headers() {
            writeln!(&mut out,
                "{}: {}", k, String::from_utf8_lossy(v)).unwrap();
        }
        writeln!(&mut out, "").unwrap();
    }
    response.body().map(|data| {
        io::stdout().write_all(data).unwrap();
    });
}
