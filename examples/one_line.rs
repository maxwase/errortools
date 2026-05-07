//! `MainResult` with the default one-line format.
//!
//! Run: `cargo run --example one_line`
//!
//! Output:
//!
//! ```text
//! Error: failed to load config: failed to read file: No such file or directory (os error 2)
//! ```

use std::{fs, io};

use errortools::MainResult;

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

fn main() -> MainResult<AppError> {
    fs::read_to_string("does-not-exist.toml")
        .map_err(ConfigError::Read)
        .map_err(AppError::Config)?;
    Ok(())
}
