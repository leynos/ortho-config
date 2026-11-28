//! Fixture crate root that redirects its module through a raw spelling.
//!
//! `#[r#path = "decoy.rs"]` loads `decoy.rs` exactly as `#[path]` does, and
//! `error.rs` sits beside this file so a default-name walk certifies it. An
//! attribute check that needs the plain spelling misses the redirect and counts
//! the file the build never compiles.

#[r#path = "decoy.rs"]
mod error;
