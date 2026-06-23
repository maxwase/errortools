//! `FormatError` extension trait for ad-hoc formatting (e.g. logging).
//!
//! Run: `cargo run --example format_error`

use std::io;

use errortools::FormatError;

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("Failed to load config")]
    Config(#[source] io::Error),
}

fn main() {
    let err = AppError::Config(io::Error::new(io::ErrorKind::NotFound, "missing.toml"));

    println!("one line: {}", err.one_line());
    println!();
    println!("chain:\n{}", err.chain());

    let dyn_err: &dyn core::error::Error = &err;
    println!();
    println!("dyn: {}", dyn_err.one_line());
}
