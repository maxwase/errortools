//! Custom [`Format`] strategy.
//!
//! Run: `cargo run --example custom_format`
//!
//! Output: `outer -> middle -> inner`

use core::{error::Error, fmt};
use std::io;

use errortools::{Format, FormatError, chain};
use itertools::Itertools;

struct Arrow;

impl<E: Error + ?Sized> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(&error).format(" -> "))
    }
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("outer")]
    Outer(#[source] MidError),
}

#[derive(Debug, thiserror::Error)]
enum MidError {
    #[error("middle")]
    Middle(#[source] io::Error),
}

fn main() {
    let err = AppError::Outer(MidError::Middle(io::Error::other("inner")));
    println!("{}", err.formatted::<Arrow>());
}
