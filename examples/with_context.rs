//! Tagging file-operation errors with context via [`WithContext`].
//!
//! Errors are handled by two enums:
//!
//! - [`FsError`] distinguishes which step failed (create vs. write). The
//!   `Create` variant nests two [`WithContext`] layers — the outer
//!   [`WithPath`] tags the path, and an inner `WithContext<usize, io::Error>`
//!   from the retry loop carries the attempt number — so the chain reports
//!   `"<path>: <attempt>: <io error>"`.
//! - [`AppError`] is the top-level error in `main`. It routes [`FsError`]
//!   into `MainResult`.
//!
//! Run: `cargo run --example with_context`
//!
//! Output (the exact io message is platform-dependent):
//!
//! ```text
//! Error: An FS error happened: Failed to create file: no/such/dir/output.txt: 3: No such file or directory (os error 2)
//! ```

use std::{
    fs::File,
    io::{self, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use errortools::{MainResult, WithContext, with_context::WithPath};

/// How many times `create_with_retry` will attempt `File::create` before
/// surfacing the last error.
const RETRY_ATTEMPTS: NonZeroUsize = NonZeroUsize::new(3).unwrap();

#[derive(Debug, thiserror::Error)]
enum FsError {
    // Chain single contextualized errors with `WithContext` inline!
    #[error("Failed to create file")]
    Create(#[source] WithPath<PathBuf, WithContext<usize, io::Error>>),
    #[error("Failed to write file")]
    Write(#[source] WithPath<&'static Path, io::Error>),
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("An FS error happened")]
    Fs(#[source] FsError),
}

/// Retries `File::create` up to `attempts` times. On exhaustion, returns the
/// final attempt's error tagged with its attempt number via
/// `WithContext<usize, io::Error>`. The default `Colon` strategy renders that
/// as `"<attempt>: <io error>"` when it shows up in the chain.
fn create_with_retry(
    path: &Path,
    attempts: NonZeroUsize,
) -> Result<File, WithContext<usize, io::Error>> {
    let last = attempts.get();
    // First `last - 1` attempts: silently retry on failure.
    for _ in 1..last {
        if let Ok(file) = File::create(path) {
            return Ok(file);
        }
    }
    // Final attempt: surface the error tagged with the attempt number.
    File::create(path).map_err(|e| WithContext::new(last, e))
}

fn write_file(path: &'static Path, contents: &[u8]) -> Result<(), FsError> {
    // Single map_err: the retry-tagged error gets wrapped with the path and
    // routed into `FsError::Create` in one closure.
    let mut file = create_with_retry(path, RETRY_ATTEMPTS)
        .map_err(|e| FsError::Create(WithContext::new(path.to_path_buf(), e)))?;
    // Double map_err: tag with path first, then lift into the enum variant.
    file.write_all(contents)
        .map_err(|e| WithContext::new(path, e))
        .map_err(FsError::Write)?;
    Ok(())
}

fn main() -> MainResult<AppError> {
    // Parent directory doesn't exist, so every retry of `File::create` fails.
    // `WithContext::new(attempt, io_err)` tags the final attempt; the outer
    // `WithContext::new(path, ...)` wraps that with the path; `?` routes the
    // result through `FsError::Create` and `AppError::Fs` into `MainResult`.
    write_file(Path::new("no/such/dir/output.txt"), b"hello, errortools\n")
        .map_err(AppError::Fs)?;
    Ok(())
}
