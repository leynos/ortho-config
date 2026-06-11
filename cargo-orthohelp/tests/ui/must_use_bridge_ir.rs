//! Compile-fail test: ignoring `#[must_use]` return values must produce a
//! compiler error when `unused_must_use` is denied.

#![deny(unused_must_use)]

#[must_use]
fn must_use_fn() -> u32 {
    42
}

fn main() {
    must_use_fn();
}
