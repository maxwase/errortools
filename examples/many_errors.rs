//! Demonstrates `ManyErrors` with nested groups and multiple rendering shapes.
//!
//! Run: `cargo run --example many_errors`

use std::io;

use errortools::many_errors::Ascii;
use errortools::{Formatted, ManyErrors, many_errors::Tree};

#[derive(Debug, thiserror::Error)]
enum DeployError {
    #[error("deploy failed")]
    Failed(#[source] ManyErrors<&'static str, RegionError>),
}

#[derive(Debug, thiserror::Error)]
enum RegionError {
    #[error("connection refused")]
    Refused,
    #[error("timed out")]
    Timeout(#[source] io::Error),
}

fn main() {
    // Build nested ManyErrors: two regions, one with two sub-errors.
    let mut east = ManyErrors::new();
    east.push("i-0a1", RegionError::Refused);
    east.push(
        "i-0b2",
        RegionError::Timeout(io::Error::other("network partition")),
    );

    let mut all: ManyErrors<&str, RegionError> = ManyErrors::new();
    all.push_group("us-east-1", east);
    all.push("eu-west-1", RegionError::Refused);

    // Default Display = Tree with Unicode connectors and count header
    println!("=== Default (Tree / Unicode) ===");
    println!("{all}");

    println!();
    println!("=== List ===");
    println!("{}", all.list());

    println!();
    println!("=== Bullets ===");
    println!("{}", all.bullets());

    println!();
    println!("=== Inline ===");
    println!("{}", all.one_line());

    println!();
    println!("=== ASCII connectors, no header ===");
    println!("{}", Formatted::<_, Tree<Ascii, false>>::new(&all));

    // Show it as a source in a top-level error
    let err = DeployError::Failed(all);
    println!();
    println!("=== As thiserror source ===");
    println!("{err}");
}
