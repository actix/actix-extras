# actix-web-httpauth

![Build Status](https://travis-ci.org/svartalf/actix-web-httpauth.svg?branch=master)
![Docs](https://docs.rs/actix-web-httpauth/badge.svg)
![Crates.io](https://img.shields.io/crates/v/actix-web-httpauth.svg)

HTTP authentication schemes for [actix-web](https://github.com/actix/actix-web) framework.

All supported schemas are actix [Extractors](https://docs.rs/actix-web/0.6.7/actix_web/trait.FromRequest.html),
and can be used both in middlewares and request handlers, check the `examples/` folder.

## Supported schemes

 * [Basic](https://tools.ietf.org/html/rfc7617)
