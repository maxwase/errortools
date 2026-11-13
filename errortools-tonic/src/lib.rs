#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod client;
mod code;
mod mapping;
mod tree;

pub use client::ErrorStatusExt;
pub use code::{Code, DetachedErrors, detach_leaves};
pub use mapping::{DEFAULT_TREE_CAP, Detail, IntoStatus, Mapping, TREE_TRAILER};

#[cfg(test)]
mod tests;
