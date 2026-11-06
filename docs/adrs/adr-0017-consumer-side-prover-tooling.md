# Architectural decision record (ADR) 0017: Import consumer-side prover tooling

## Status

Accepted.

Date: 2026-11-06.

## Context and problem statement

OrthoConfig has no formal-methods lane. Contributors who want to check a small
pure kernel with Kani's bounded model checker, or prove an unbounded property
with Verus, must discover a verifier version, install it by hand, and invent a
command that no other repository in the estate shares. The result is
unrepeatable evidence: two developers can run "the proofs" and get different
tools, different bounds, and different claims.

Other repositories in the estate have already solved the installation and
process-execution problem. `rust-prover-tools` downloads a pinned Kani release,
verifies a pinned Verus archive against a checksum, installs both, and runs
Verus proof files. Importing its consumer-side conventions gives OrthoConfig a
reviewable lane without vendoring a downloader or adopting a second dependency
manager.

The question is what OrthoConfig should own, what the shared package should
own, and which claims the lane may make.

## Decision drivers

- Evidence must name a tool version and a bound. "The proofs pass" is not a
  claim; "Kani 0.67.0 verified this harness under unwind 5" is.
- Ordinary contributors must never install a prover to run `make all`,
  `make test`, or `make lint`.
- Pins, checksums, selected inputs, and CI belong to the consuming repository.
  Downloading, archive validation, installation, and Verus process execution
  belong to the shared package.
- A fixture success proves the runner works. It does not prove OrthoConfig
  behaviour, and it must never be presented as though it does.
- A negative control is part of the evidence: a lane that has never rejected a
  false claim has not shown that it can.
- Linux x86_64 is the only platform both verifiers publish. Other platforms
  must be recorded as unvalidated rather than silently skipped.
- Compiler admission, the shared Cargo cache, and warnings-as-errors stay
  intact.

## Options considered

### Option A: Delegate to the pinned shared package

Pin `rust-prover-tools` at a full commit, invoke it through
`uv tool run --python 3.14`, and keep repository-owned pins, commands, and CI.
Kani installation and version checking delegate to the package; Verus
installation and proof execution do too. The repository runs the selected Kani
executable directly for harness execution.

This is the accepted option.

### Option B: Vendor the downloader

Copy the package's Python into this repository. Rejected: it duplicates archive
validation and process handling that already exist, makes every future security
fix a copy task, and contradicts the estate's shared-tooling direction. It also
adds a second implementation that CI would have to test.

### Option C: Track the latest releases

Install whatever `cargo install kani-verifier` and the latest Verus archive
provide. Rejected: a verifier upgrade changes the meaning of every harness
without a reviewable diff. A pinned version and a checksum are the minimum for
reproducible evidence.

### Option D: Add provers to the ordinary test suite

Run Kani and Verus inside `make test`. Rejected: ordinary contributors would
need large binaries and a solver, and a slow admission queue would turn a unit
test run into a formal-methods install. Integration stays opt-in.

| Topic                     | A: shared, pinned | B: vendored | C: latest | D: in `make test` |
| ------------------------- | ----------------- | ----------- | --------- | ----------------- |
| Reproducible version      | Yes               | Yes         | No        | Yes               |
| Single implementation     | Yes               | No          | Yes       | Yes               |
| Ordinary gates unaffected | Yes               | Yes         | Yes       | No                |
| Reviewed upgrade          | Yes               | Yes         | No        | Yes               |

_Table 1: Comparison of prover-tooling import options._

## Decision outcome / proposed direction

### Ownership

The repository owns `tools/kani/VERSION`, `tools/verus/VERSION`, and
`tools/verus/SHA256SUMS`, the Make targets, the fixture inputs, the integration
test, and the CI lane. The shared package owns downloading, archive
verification, installation layout, and Verus execution. No package source is
vendored.

### Command surface

`make install-kani` installs Kani into the active `CARGO_HOME`;
`make kani-check` verifies that executable against the pin.
`make install-verus` installs the pinned Verus release under `.verus/`. Proof
execution is opt-in: `make kani` runs the registered smoke harnesses,
`make kani-full` runs every registered harness, and `make verus` runs explicit
proof files. An empty production inventory fails with `FORMAL-EMPTY` rather
than reporting success.

### Solver coupling

Verus enforces an exact Z3 version. The shared package's standalone
`z3 install` pins 4.16.0, which the pinned Verus 0.2026.01.30.44ebdee rejects
with "expected z3 version 4.12.5, found 4.16.0". The Make boundary therefore
sets `VERUS_Z3_PATH` to the release's bundled solver and installs no separate
Z3. `VERUS_Z3_PATH` remains overridable.

### Evidence and negative controls

`tests/fixtures/prover_tools/` holds a Kani package and two Verus files. The
Kani fixtures verify three true harnesses and reject a mutant that omits the
last input. The Verus fixtures verify a bounded inductive count and reject a
strict bound that the empty sequence refutes. The opt-in target
`make test-prover-tools-integration` executes them and writes transcripts plus
`target/prover-tools-fixtures/prover-evidence.json`.

### CI lane

`.github/workflows/prover-tools.yml` runs on relevant pull-request paths, a
manual dispatch, and a weekly schedule. It installs, checks, and runs the
fixtures on Linux x86_64 under a 30-minute ceiling, holds `contents: read`
only, pins every action to a full commit, and uploads the evidence on success
or failure. It does not run in the ordinary `ci.yml` matrix. sccache is
disabled for the job: Kani drives its own compiler, which sccache cannot
cache, so the shared setup action must not export a rustc wrapper that would
intercept it.

## Goals and non-goals

- Goals:
  - Give contributors one repeatable command for each verifier.
  - Keep pins, checksums, inputs, and CI reviewable in this repository.
  - Establish genuine acceptance and rejection evidence for both runners.
  - Keep ordinary gates free of prover installation and execution.
- Non-goals:
  - Register production harnesses or proofs; those remain feature-owned.
  - Validate macOS, Windows, or non-x86_64 Linux for this lane.
  - Cache installations, vendor a downloader, or change compiler admission.
  - Present fixture evidence as a proof of configuration behaviour.

## Migration plan

The lane is additive. No runtime API, Cargo dependency, or schema changes. The
first production harness or proof arrives in its own feature pull request with
its owning feature, bounds, assumptions, and negative control recorded there.

## Known risks and limitations

- Kani passes are bounded by their unwind depth; a pass says nothing beyond
  that bound.
- A Verus proof can reason about a model that is not refined against runtime
  code. The refinement obligation stays with the feature that adds the proof.
- The pinned Verus release bundles a specific Z3 build; a future Verus pin may
  change the solver path. `make install-verus` and the integration target would
  fail loudly rather than verify with the wrong solver.
- Only Linux x86_64 is exercised. The workflow names that platform so the gap
  is visible.
- Installation is uncached. A cold CI run pays the download cost; that is
  accepted until there is measured benefit and a single-owner cache design.

## Outstanding decisions

- None for this import. Production proof obligations, caching, and additional
  platforms each require their own decision.

## Architectural rationale

The split keeps the repository's reviewable surface small and its evidence
explicit. The consumer owns everything a reviewer must see; the package owns
everything a reviewer should not have to re-implement. Opt-in execution
protects the ordinary contributor. Negative controls keep the lane honest.
Fixtures prove the runner, not the product, and the documentation says so.

## References

- [Execplan: Import Kani and Verus tooling](../execplans/import-prover-tools.md).
- [Developers' guide: Kani and Verus verification tools](../developers-guide.md).
- [Kani documentation](https://model-checking.github.io/kani/).
- [Verus guide](https://verus-lang.github.io/verus/guide/).
- [Pinned shared tool](https://github.com/leynos/rust-prover-tools/blob/592558f7183b554fc749696246984c5d7300d029/README.md).
