# Architectural decision record (ADR) 0016: Adopt a dated nightly and the development build standard

## Status

Accepted. The repository pins a dated nightly toolchain for development and
formatting, takes the parallel `rustc` frontend and the `mold` linker as Cargo
defaults, and keeps every non-development route off the standard. The Cranelift
development backend was measured and is not adopted.

## Date

2026-10-16.

## Context and problem statement

The repository pinned stable 1.99 in `rust-toolchain.toml` and carried no Cargo
configuration. The Makefile assigned `RUSTFLAGS="-D warnings"` in `test` and
`typecheck`. The estate build standard asks for the parallel `rustc` frontend,
the `mold` linker (Linux only), the Cranelift backend where the suite passes, a
dated nightly kept within 56 days, and an exercised minimum supported Rust
version (MSRV).

Adopting the standard raises three questions. Where do the flags live, given
that Cargo selects one `rustflags` source and does not merge them, and that an
assigned `RUSTFLAGS` outranks every `rustflags` table in the configuration?
Which routes must stay outside the standard? How does a contributor or a CI job
obtain the tools the standard needs?

## Decision drivers

- The parallel frontend is nightly-only, so the pin must be a nightly, and a
  dated one so builds are reproducible.
- A bare `cargo build` in a checkout should get the standard without being
  asked.
- A recipe that assigns `RUSTFLAGS` must not silently drop the standard and
  still report success.
- Stable and beta compilers refuse nightly-only flags, so the MSRV check, the
  beta lane, and `cargo +stable install` from a checkout must keep working.
- `mold` ships for Linux only; macOS and Windows must build with the platform
  linker.
- Published binaries should be built with production flags only.

## Options considered

### Option A: select the backend through a profile key

A `[profile.dev] codegen-backend` key would be a natural home for the Cranelift
backend. Every non-nightly Cargo refuses it with "feature `codegen-backend` is
required", which was measured against 1.89.0, beta, and 1.99. No environment
variable overrides it, so the MSRV check, the beta lane, and
`cargo +stable install` run from the checkout would all fail.

### Option B: select the backend through `rustflags`

A `rustflags` entry is displaced by an assigned `RUSTFLAGS`, which those routes
already use, so they keep working. This is the only viable form of backend
selection. The measurement that decided against adopting a backend is recorded
in the developers' guide.[^1]

### Option C: one global linker table under `cfg(unix)`

A single table would be shorter, but macOS builds also match `cfg(unix)` and
macOS ships no `mold`. Rejected; the linker flag sits under
`cfg(target_os = "linux")` only.

### Option D: a second pinned toolchain for maintenance

A separate toolchain would keep the repository nightly out of release and
maintenance work, at the cost of a second pin to maintain and a second install
step. Rejected in favour of one repository nightly, with the non-development
routes displacing the configuration instead.

## Decision outcome

- Pin a dated nightly in `rust-toolchain.toml`, with the `rustfmt`, `clippy`,
  `rust-analyzer`, `rustc-codegen-cranelift-preview`, and `llvm-tools-preview`
  components and the `minimal` profile. It is the newest nightly that offers
  every component on Linux (x86_64 and aarch64), Windows MSVC, and macOS,
  checked against the rustup manifests. It carries the date 2026-10-16 at the
  time of writing.
- Put the standard in `.cargo/config.toml` as Cargo defaults: `-Zthreads=8` in
  `[build]`, and `-Zthreads=8 -Clink-arg=-fuse-ld=mold` under
  `[target.'cfg(target_os = "linux")']` only. The Makefile restates the same
  flags wherever a recipe assigns `RUSTFLAGS`: `$(GATE_RUSTFLAGS)` for a gate
  and `$(DEBUG_RUSTFLAGS)` for a debug build. `BUILD_STANDARD=0` switches the
  standard off for one invocation; only the advisory beta lane uses it.
- Do not adopt backend selection. The Cranelift backend was requested, measured,
  and refused; the evidence is in the developers' guide.[^1] The exception is
  tracked by [issue #598](https://github.com/leynos/ortho-config/issues/598),
  which requires a re-measurement by 2027-04-18.
- Keep explicit non-development routes outside the standard. `make release`
  uses `$(RELEASE_RUSTFLAGS)`, which assigns an empty `RUSTFLAGS` on purpose to
  displace the configuration. Coverage and packaging assign `RUSTFLAGS` in
  their workflows. `make lint-whitaker` assigns `-D warnings` alone, because
  Whitaker runs on its own toolchain. `make msrv-build` assigns `-D warnings`
  and selects `+$(MSRV)`, a stable toolchain whose version is read from
  `Cargo.toml` (currently 1.89.0). The beta lane sets `BUILD_STANDARD=0`.
- Provision the build tools explicitly. `make install-build-tools` installs the
  pinned `mold` release, verified by SHA-256 against `tools/mold/SHA256SUMS`,
  and then the toolchain that `rust-toolchain.toml` names; there is no
  source-build fallback. `make check-build-tools` runs before `test`,
  `typecheck`, `lint-clippy` (hence `lint`), and the debug `build`, and every
  failure tells the contributor to run `make install-build-tools`. CI installs
  the same pinned `mold` through the shared `setup-rust` action's
  `install-mold` input in `build-test` and in the coverage publisher job.
- Add an MSRV build job to CI: a matrix job keyed by `toolchain`, with one
  quoted leg equal to the manifest's `rust-version` as major.minor, that runs
  `make msrv-build MSRV=${{ matrix.toolchain }}`. It runs `cargo build` over
  every target with all features on the declared MSRV, never a test run. The
  estate rule RT-026 states a cost rule of `cargo check --all-targets`; the
  maintainer chose a build instead, which is stronger because it also links and
  is still not a test run. The numeric leg is what RT-026 detects.
- Keep releases on the pinned nightly. `rust-build-release` resolves the
  toolchain from `rust-toolchain.toml`, and `release.yml` names no toolchain,
  so the Dependabot-maintained pin governs releases with production flags only.
  No workflow names a dated nightly other than the pin (estate rule RT-027).
- Add a daily `rust-toolchain` entry to `.github/dependabot.yml` so the pin
  stays within the 56-day window.

The build-standard contracts under `tests/workflow_contracts/` judge
`make --dry-run <target> CARGO=probe-cargo` output, so a recipe that loses the
standard, or a release, coverage, or Whitaker route that gains it, fails
`make test-workflow-contracts`. Estate-wide policy is audited by Concordat and
is not replicated here.

## Consequences

- Dependabot moves the pin daily. Each bump needs the `trybuild` snapshots
  regenerated from the compiler and the Cranelift measurement re-read.
- A source builder on Linux needs `mold` and the pinned nightly: a bare
  `cargo build` in a checkout reads the configuration and links with `mold`.
  Windows and macOS need neither `mold` nor any tool beyond the pinned nightly.
- Nightly Cargo warns `cargo::unused_workspace_package_fields` for four
  `[workspace.package]` fields that the release scripts nevertheless read.
- Releases built by `rust-build-release` resolve their toolchain from
  `rust-toolchain.toml`, so published binaries are built by the pinned nightly,
  by decision, with production flags only: LLVM, the platform linker, and
  `-D warnings`.
- Mutation testing relies on the shared `setup-rust` default `RUSTFLAGS` to
  displace the configuration, and no repository contract covers that.
- The Windows coverage legs use the runner's drive root as `CARGO_TARGET_DIR`,
  because Cargo repeats the target root across one compiler command line often
  enough to reach the 32,767-character `CreateProcess` limit. The Linux legs
  keep cargo-llvm-cov's default. See
  [The Windows coverage command line](../developers-guide.md#the-windows-coverage-command-line).

[^1]: Developers' guide, "Cranelift qualification" section.
