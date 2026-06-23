//! `#[error(transparent)]` for pass-through variants.
//!
//! When an inner error already carries full context, the outer variant adds
//! nothing useful. `transparent` forwards `Display` and `source` straight
//! through, and `#[from]` is appropriate here because no context is dropped.
//!
//! Run: `cargo run --example transparent`
//!
//! Output:
//!
//! ```text
//! Error: Failed to load config: Failed to read file: missing
//! ```
//!
//! Note `Io` does not appear in the chain — the `transparent` variant is
//! invisible.

use std::io;

use errortools::MainResult;

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Io(#[from] ConfigError),
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("Failed to load config")]
    Load(#[source] FileError),
}

#[derive(Debug, thiserror::Error)]
enum FileError {
    #[error("Failed to read file")]
    Read(#[source] io::Error),
}

fn load() -> Result<(), ConfigError> {
    Err(ConfigError::Load(FileError::Read(io::Error::new(
        io::ErrorKind::NotFound,
        "missing",
    ))))
}

fn main() -> MainResult<AppError> {
    // `#[from]` on the transparent variant lets the call site lift
    // `ConfigError` into `AppError` with `.map_err(AppError::from)`.
    load().map_err(AppError::from)?;
    Ok(())
}
