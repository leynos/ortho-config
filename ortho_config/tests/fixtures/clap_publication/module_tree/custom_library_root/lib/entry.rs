//! The crate root this package reports through its `[lib] path`.
//!
//! The module below sits beside this file, not below `entry/`: a crate root
//! keeps its modules beside itself whatever the file is called. A walk that
//! recognised crate roots by the names `lib.rs` or `main.rs` would look in the
//! wrong directory and certify nothing.

mod kind;