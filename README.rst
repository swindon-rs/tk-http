Tk-HTTP
=======

.. image:: https://travis-ci.org/swindon-rs/tk-http.svg?branch=master
   :target: https://travis-ci.org/swindon-rs/tk-http

:Status: Beta
:Documentation: https://docs.rs/tk-http/

Features:

* HTTP 1.1 and 1.0 support (plans to support for HTTP/2 with same API)
* Flexible configuration of pipelining both for client and server
* Comprehensive configuration of timeouts both for client and server
* Strict parsing of few selected headers which influence security
* Other headers go unparsed to keep CPU usage low
* Minimum copies of data: i.e. you can decode JSON directly from network buffer
* Generic over transport (so can be used over TLS or unix sockets)
