//! Custom [`Format`] strategy.
//!
//! Run: `cargo run --example custom_format`
//!
//! Output:
//! ```text
//! Outer -> Middle -> inner
//! { "message": "Outer -> Middle -> inner" }
//! ```

use core::{error::Error, fmt};
use std::{io, marker::PhantomData};

use errortools::{Format, FormatError, chain};
use itertools::Itertools;

struct Arrow;

impl<E: Error + ?Sized> Format<E> for Arrow {
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", chain(&error).format(" -> "))
    }
}

/// You can also implement your own custom format strategy that uses another format strategy to render the error message.
/// This is useful for rendering errors in a structured format like JSON or OpenTelemetry.
struct Json<F>(PhantomData<fn() -> F>);

impl<E, F> Format<E> for Json<F>
where
    E: std::error::Error + ?Sized,
    F: for<'a> Format<&'a E>,
{
    fn fmt(error: &E, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{ \"message\": \"{}\" }}", error.formatted::<F>())
    }
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("Outer")]
    Outer(#[source] MidError),
}

#[derive(Debug, thiserror::Error)]
enum MidError {
    #[error("Middle")]
    Middle(#[source] io::Error),
}

fn main() {
    let err = AppError::Outer(MidError::Middle(io::Error::other("inner")));
    println!("{}", err.formatted::<Arrow>());
    println!("{}", err.formatted::<Json<Arrow>>());
}
