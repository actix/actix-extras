# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2018-09-08
### Changed
 - Update to `actix-web = "0.7"` version

## [0.0.4] - 2018-07-01
### Fixed
 - Fix possible panic at `IntoHeaderValue` implementation for `headers::authorization::Basic`
 - Fix possible panic at `headers::www_authenticate::challenge::bearer::Bearer::to_bytes` call
