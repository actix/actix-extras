# actix-web-httpauth

[![Latest Version](https://img.shields.io/crates/v/actix-web-httpauth.svg)](https://crates.io/crates/actix-web-httpauth)
[![Latest Version](https://docs.rs/actix-web-httpauth/badge.svg)](https://docs.rs/actix-web-httpauth)
![Build Status](https://travis-ci.org/svartalf/actix-web-httpauth.svg?branch=master)
![Apache 2.0 OR MIT licensed](https://img.shields.io/badge/license-Apache2.0%2FMIT-blue.svg)

HTTP authentication schemes for [actix-web](https://github.com/actix/actix-web) framework.

All supported schemas are actix [Extractors](https://docs.rs/actix-web/0.6.7/actix_web/trait.FromRequest.html),
and can be used both in middlewares and request handlers, check the `examples/` folder.

## Supported schemes

 * [Basic](https://tools.ietf.org/html/rfc7617)
 * [Bearer](https://tools.ietf.org/html/rfc6750)

## Donations

If you appreciate my work and want to support me, you can do it [here](https://svartalf.info/donate/).
