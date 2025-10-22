//! Code generation entry points for the derive system.
//!
//! Re-exports declarative merge emitters along with struct and trait
//! generators so the macro entrypoint can assemble complete outputs while
//! keeping the implementation cohesive.

pub(crate) mod declarative;
pub(crate) mod ortho_impl;
pub(crate) mod structs;
