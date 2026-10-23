# Architectural decision record (ADR) 0017: Use Cranelift for development builds

## Status

Accepted. Development builds use Cranelift and the parallel `rustc` frontend;
coverage builds use LLVM. This decision supersedes the backend exclusion in
[ADR-0016](adr-0016-adopt-a-dated-nightly-and-the-development-build-standard.md).

## Date

2026-10-23.

## Context and problem statement

ADR-0016 adopted the dated nightly, `-Zthreads=8`, and Linux `mold`, but left
Cranelift out after measuring that it cannot unwind through Cranelift-compiled
frames. The development build standard is now required to use the dev-fast
route for all development builds, with `-Zthreads=8` included. Coverage must
remain on LLVM because LLVM instrumentation is required.

The route must be consistent for bare Cargo commands and Make recipes. It must
also keep the unwind limitation visible in tests: `#[should_panic]` works under
Cranelift, but panics caught with `catch_unwind` or raised on a joined thread
do not follow LLVM's unwind behaviour.

## Decision drivers

- Development builds use the same Cranelift and `-Zthreads=8` flags through
  Cargo configuration and Make.
- Coverage retains LLVM instrumentation and runs the test that verifies mutex
  poisoning during unwind.
- Release, packaging, MSRV, beta, and Whitaker routes keep their explicit
  toolchain and warning policies.
- Build-tool checks identify a missing Cranelift component before Cargo fails.

## Decision outcome

- Add `-Zcodegen-backend=cranelift` and `-Zthreads=8` to each development
  `rustflags` source in `.cargo/config.toml` and `STANDARD_RUSTFLAGS` in the
  Makefile. Linux also selects the pinned `mold` linker.
- Define `ortho_config_dev_fast` in those same flags. The poisoned-mutex test
  that deliberately panics on a spawned thread is marked ignored only when this
  cfg is present. Coverage assigns its own `RUSTFLAGS`, so the cfg is absent
  and LLVM runs that test.
- Express the debug-assertion and empty-aggregate panic contracts with
  `#[should_panic]`, which the pinned Cranelift backend supports. Keep these
  tests active in dev-fast builds.
- Check that the pinned toolchain has a `rustc-codegen-cranelift-*` component.
  `rust-toolchain.toml` remains the source for installing the component.
- Keep all non-development routes outside the standard by assigning their
  explicit `RUSTFLAGS` as recorded in the developers' guide.

## Consequences

- The dev-fast test run does not exercise the poisoned-mutex unwind scenario.
  The LLVM coverage route does, and must stay green.
- The Cranelift limitation is not a claim that the backend has LLVM-equivalent
  unwind behaviour. Do not use this backend for release builds.
- The original full-suite measurement covered Linux only. Hosted CI validates
  the development route on the configured platforms; remeasure when the pinned
  nightly changes, following the developers' guide and issue #598.
