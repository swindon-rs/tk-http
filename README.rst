Tk-HTTP
=======

.. image:: https://travis-ci.org/swindon-rs/tk-http.svg?branch=master
   :target: https://travis-ci.org/swindon-rs/tk-http

:Status: Beta
:Documentation: https://swindon-rs.github.io/tk-http

A full-features asynchronous HTTP implementation for tokio-rs stack, including
websockets.

Features:

* HTTP 1.1 and 1.0 support (plans to support for HTTP/2 with same API)
* Flexible configuration of pipelining both for client and server
* Comprehensive configuration of timeouts both for client and server
* Strict parsing of few selected headers which influence security
* Other headers go unparsed to keep CPU usage low
* Minimum copies of data: i.e. you can decode JSON directly from network buffer
* Generic over transport (so can be used over TLS or unix sockets)


License
=======

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)
  at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

