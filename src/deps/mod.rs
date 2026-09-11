//! Reimplementation shims for behaviour the Go original obtained from its
//! standard library.
//!
//! The upstream project has no third-party dependencies: `go.mod` declares only
//! the module path and `go 1.12`. Everything it relies on — reference-layout
//! time formatting and parsing, IANA zone lookup, `time.Date` normalisation and
//! DST disambiguation, calendar arithmetic — comes from Go's `time` package.
//!
//! None of that is available in Rust, and none of it can be approximated with a
//! package that has different semantics without changing observable behaviour,
//! so [`gotime`] reproduces exactly the parts this library's public contract
//! depends on. It is deliberately narrow: it is not a port of Go's `time`
//! package, only of the behaviour `now` exposes to its callers.

pub mod gotime;
