//! `MainResult` with the [`Tree`] format.
//!
//! Run: `cargo run --example tree`
//!
//! Output:
//!
//! ```text
//! Error: failed to load config
//! └── failed to read file
//!     └── No such file or directory (os error 2)
//! ```

use std::{fs, io};

use errortools::{MainResult, Tree};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("failed to load config")]
    Config(#[source] ConfigError),
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("failed to read file")]
    Read(#[source] io::Error),
}

fn main() -> MainResult<AppError, Tree> {
    fs::read_to_string("does-not-exist.toml")
        .map_err(ConfigError::Read)
        .map_err(AppError::Config)?;
    Ok(())
}
