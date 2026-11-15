//! trybuild coverage for the artefact-lock evidence newtypes.
//!
//! `artefact_lock_evidence.rs` claims, in its own module documentation and in
//! ADR-0020, that the newtypes exist "so the compiler rejects the swap". That
//! claim is the whole justification for the vocabulary, and nothing else in the
//! suite can fail when it stops being true: the run-time tests exercise the
//! newtypes with correctly typed arguments, so they compile and pass whether or
//! not a transposed pair would.
//!
//! The fixtures cannot name `ortho_config_macros::derive::…`. This crate is
//! `proc-macro = true`, so it exports only its macros; an external crate that
//! tries to reach the types gets `E0433: cannot find 'derive' in
//! 'ortho_config_macros'`. `#[path]`-including the real source is what makes the
//! cases reach the *actual* declarations rather than a copy of them: the
//! fixture's `mod` compiles the same file the production build compiles, so a
//! newtype that loses its wrapper fails here too.
//!
//! The fixtures are Linux-gated with the types they use. `Descriptor`,
//! `DeviceNumber`, `LockFileIdentity` and `parse_lock_record` are declared under
//! `cfg(target_os = "linux")` with no `test` arm, so no other target can compile
//! a case that names one. The gate is on the harness as well as the fixtures,
//! so a non-Linux build registers no cases rather than failing to find them.

#![cfg(target_os = "linux")]

/// Compiles a fixture that transposes two evidence newtypes.
///
/// Each fixture must fail to compile with the diagnostic recorded beside it in
/// `tests/ui/`. The failure is the assertion: a fixture that started compiling
/// would report the newtypes no longer preventing the swap.
#[test]
fn transposed_evidence_newtypes_do_not_compile() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}

/// Compiles the correctly typed control.
///
/// Without this, every compile-fail case would also pass if the fixtures failed
/// for an unrelated reason — a stale `#[path]`, a renamed constructor, a missing
/// import. The control proves the fixtures reach the declarations and that the
/// calls themselves are well formed, so the only difference in the failing cases
/// is the transposition.
#[test]
fn correctly_typed_evidence_newtypes_compile() {
    trybuild::TestCases::new().pass("tests/fixtures/evidence_compiles.rs");
}
