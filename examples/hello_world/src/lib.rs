//! Library facade for the `hello_world` example so integration tests can reuse
//! configuration types and helpers.

pub mod cli;
pub mod error;
pub mod message;

#[cfg(test)]
pub mod test_support;
