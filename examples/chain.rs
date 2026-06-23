//! `MainResult` with the [`Chain`] format (per-error source-chain ladder).
//!
//! Run: `cargo run --example chain`
//!
//! Output:
//!
//! ```text
//! Error: Failed to load config
//! └─ Failed to read file
//!    └─ No such file or directory (os error 2)
//! ```

use std::{fs, io};

use errortools::{Chain, MainResult};

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("Failed to load config")]
    Config(#[source] ConfigError),
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("Failed to read file")]
    Read(#[source] io::Error),
}

fn main() -> MainResult<AppError, Chain> {
    fs::read_to_string("does-not-exist.toml")
        .map_err(ConfigError::Read)
        .map_err(AppError::Config)?;
    Ok(())
}
