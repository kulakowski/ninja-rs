// For now, at least, allow dead code and unnecessary type wraps while
// we build things out.
#![allow(dead_code)]
#![cfg_attr(feature = "cargo-clippy", allow(clippy::unnecessary_wraps))]

mod arena;
mod ast;
mod blob;
mod intern;
mod lex;
mod parse;
