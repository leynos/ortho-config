# Import Kani and Verus tooling

Status: APPROVED — implementation authorized on 2026-11-03.

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone.

## Purpose / big picture

Provide reproducible installation and explicit execution of Kani, a bounded
Rust model checker, and Verus, a deductive verifier. Contributors should be
able to install the pinned tools, execute repository-selected verification
inputs, and distinguish a successful proof from a missing tool, empty suite,
failed obligation, or interrupted run. Continuous integration (CI) should
exercise the same entrypoints and retain enough evidence to diagnose failures.

This change imports integration patterns from Netsuke and Chutoro, using the
shared `rust-prover-tools` package rather than copying their installers.
Success means real positive and negative integration fixtures exercise both
verifiers. It does not mean that OrthoConfig's runtime has been formally
verified. Future features must supply their own substantive obligations and
explicit relationship to production code.

The missing-required-values plan is a motivating consumer, not an instruction
to implement roadmap 7.3.1 here. Its reviewed design currently selects property
tests and negative controls and defers Verus until a non-trivial kernel needs
an exhaustive guarantee.[^1] This import removes the tooling obstacle without
reversing that design decision or marking its roadmap work complete.

## Constraints

- This publication changes only this plan and its documentation-index entry.
  Implementation starts only after explicit approval of the plan.
- Preserve runtime behaviour, public APIs, Cargo dependency surfaces, and the
  stable toolchain in `rust-toolchain.toml`. Do not import Netsuke IR or
  Chutoro graph, minimum-spanning-tree, or SIMD algorithms and proofs.
- Keep production code independent of prover runners and infrastructure.
  Kani harnesses eventually exercise real pure kernels behind `cfg(kani)`;
  Verus files remain outside the normal Cargo workspace. No new runtime port,
  facade, or compatibility alias is justified by this tooling task.
- Use `rust-prover-tools` at a full commit pin. Keep verifier versions and
  Verus archive checksums in the consuming repository. Never substitute an
  unpinned latest release, ignore checksum failures, or treat absence as proof.
- Keep prover targets outside `make all`, `make test`, and `make lint`.
  Ordinary contributors must not install provers to run ordinary gates.
- Preserve `RUSTC_WRAPPER`, `RUSTDOC`, compiler admission, and the shared
  default Cargo cache. Do not create a local isolated Cargo home, override
  admission, modify shared services, or build under `/tmp`.
- Keep all candidate gates sequential. A queued compiler is not a failure;
  report unhealthy admission without attempting host recovery. Stop if disk
  fills. Never kill another agent's processes.
- Preserve warnings-as-errors. Do not import Chutoro's relaxed Kani warning
  policy or Netsuke-specific compiler flags. Follow repository Python scripting
  standards for any new executable script.

## Tolerances (exception triggers)

Stop implementation, record the proposed deviation, set status to BLOCKED, and
request direction if the work needs a runtime API change, a production
algorithm extraction, a new Cargo dependency, a copied downloader, or a change
to another feature's proof obligations. The only new external tool dependency
approved by this proposal is the pinned `rust-prover-tools` invocation and its
required Python interpreter; existing test dependencies should suffice.

The initial implementation budget is 24 non-ExecPlan files and 1,800 net lines
of code and other documentation. ExecPlan files are excluded from both counts.
Exceeding either bound requires a scope decision. Three unsuccessful repairs of
the same contract failure require reassessment before further edits.

Initial integration limits are one proof process at a time, 120 seconds per
small fixture after tool installation, and 30 minutes per CI job including
installation. These are resource limits, not a time limit on the engineer. A
fixture exceeding them requires measurement and a revised bounded fixture or an
approved limit; never silently reduce assertions or disable unwind checks.
Failure of the pinned tools to run with host admission intact is an impasse to
report, not permission to bypass it.

## Risks

- High impact, medium likelihood: the Kani bundled compiler cannot consume
  current dependency versions or host flags. Use a standalone integration
  fixture first, retain the stable production compiler, and explicitly leave
  production-crate compatibility unclaimed until a feature adds a real harness.
- High impact, medium likelihood: a conflicting executable wins on `PATH`.
  Test default and custom Cargo homes, spaces, apostrophes, and the
  source-build fallback through the real command boundary before enabling CI.
- High impact, low likelihood: a proof fixture or empty registry gives false
  assurance. Require discovered counts, reachable cases, negative controls, and
  separate labels for tooling fixtures and production obligations.
- Medium impact, medium likelihood: upstream integration remains under review.
  Netsuke PR #1063 was open at inspection. Reconcile its changes before copying
  patterns; the inspected head is evidence, not an assertion of approval.
- Medium impact, medium likelihood: Python 3.14 is unavailable or tooling
  downloads are interrupted. Let `uv` provision the tool interpreter, report
  actionable failures, and test restart without claiming a partial install.
- Medium impact, low likelihood: caching introduces competing owners or stale
  binaries. Start uncached; retain timings and disk measurements before any
  later proposal to import cache actions.

## Progress

- [x] (2026-11-03) Read repository instructions and requested skills; rename
  the branch to `import-prover-tools`.
- [x] (2026-11-03) Inspect pinned Netsuke, Chutoro, and shared-tool sources,
  and reconcile the linked missing-values plan's deferred proof scope.
- [x] (2026-11-03) Draft the import contract, milestones, and verification
  obligations; add the plan to `docs/contents.md`.
- [x] (2026-11-03) Publish the validated plan in draft PR #651.
- [x] (2026-11-03) Receive explicit implementation approval from the user.
- [x] (2026-11-03) M1 baseline workflow contracts passed at `2ad6f261`:
  366 passed, 1 skipped
  (`/tmp/test-workflow-contracts-ortho-config-import-prover-tools-baseline.out`).
- [x] (2026-11-03) M1 red contracts fail on the expected missing pins/targets:
  6 failed, including absent targets and the Kani pin
  (`/tmp/prover-tools-m1-red.out`).
- [x] (2026-11-06) M1: the pinned `install-kani`, `kani-check`,
  `install-verus`, `kani`, `kani-full`, and `verus` targets and the
  command-boundary contracts are implemented. `make check-fmt`, `make lint`, and
  `make typecheck` passed on the implementation tree.
- [x] (2026-11-06) M2: `tests/fixtures/prover_tools/` holds the Kani package
  and both Verus files, and `make test-prover-tools-integration` runs them
  against the installed pins with acceptance and rejection evidence.
- [x] (2026-11-06) M3: `.github/workflows/prover-tools.yml`,
  `tests/workflow_contracts/prover_tools_workflow_test.py`, ADR-0017, and the
  developers' guide, design, layout, and index updates are implemented.
- [x] (2026-11-06) M3: the first hosted `prover-tools` run failed one
  integration test because Verus requires Rust 1.93.0 and the runner only
  installed the repository's 1.99 pin. `make install-verus` now installs the
  toolchain named in the release's `version.json`, with a contract test for
  both the install and the missing-name failure.
- [x] (2026-11-06) M3 / E4: hosted run
  [42217419559](https://github.com/leynos/ortho-config/actions/runs/42217419559)
  passed on `602f9e06`. Every step ran, the log shows
  `rustup toolchain install '1.93.0-x86_64-unknown-linux-gnu'`, the fixture
  suite reported `11 passed in 7.91s`, and artefact 13502306744 holds the
  transcripts and `prover-evidence.json` with the accepted and rejected
  controls.
- [x] (2026-11-06) M3 review follow-up: `make verus` now passes
  `--no-install-missing` and `--no-ensure-toolchain`. The shared package treats
  them as independent options, so the earlier single flag still allowed a proof
  run to install a Rust toolchain. A contract assertion covers both flags, and
  the developers' guide and ADR-0017 record that execution never installs.

## Surprises & discoveries

The target feature plan explicitly rejects proving a trivial Boolean filter and
requires real-provider and schema-fidelity evidence instead. Its D4 and
verification sections defer a Verus milestone until a later design justifies
one. No feature theorem should be invented to make this tooling import appear
complete.

Both donor repositories pin the shared package at
`592558f7183b554fc749696246984c5d7300d029`. Its `pyproject.toml` requires
Python 3.14, whereas OrthoConfig's Makefile uses Python 3.13 for existing
scripts. A dedicated tool interpreter avoids an unrelated script migration.
Both donors pin Kani 0.67.0, but their Verus versions differ. Chutoro supplies
an inspected checksum for its older release, so it is the conservative initial
Verus baseline; upgrading it is separate work.

The baseline Makefile, Cargo manifest, and workflows contain no Kani or Verus
integration. Existing `tests/workflow_contracts/makefile_support.py` and
`workflow_reading.py` provide reusable recipe and workflow readers. Reuse these
before adding helpers. CodeGraph indexing timed out after 300 seconds during
planning; targeted source-file inspection supplies the evidence for the build
interfaces. The official Kani documentation confirms `#[kani::proof]` harness
selection, successful/failed verification results, Cargo integration, and Linux
x86_64 prebuilt support.[^5][^6][^7] The current Verus guide describes static
verification against user specifications over all executions; the guide is not
tied to the imported binary pin, so the pinned runner remains the acceptance
authority.[^8] Re-reading the live Netsuke PR found that its Kani command
quoting still breaks on apostrophes, and installer subprocesses can select an
earlier conflicting `cargo-kani`; both defects are covered by the new red
command-boundary contract.[^9] The shared tool can install a standalone Z3
binary under `.cache/z3/4.16.0/z3` and forwards `VERUS_Z3_PATH` to Verus, but
that solver is not interchangeable with a Verus release: executing the pinned
Verus 0.2026.01.30.44ebdee with Z3 4.16.0 aborts with "expected z3 version
4.12.5, found 4.16.0", while the release's bundled `.verus/<version>/verus/z3`
reports 4.12.5 and verifies successfully. D5 is therefore revised below.[^10]

The pinned Verus release embeds Rust 1.93.0 and refuses to run under another
toolchain, but the repository pins 1.99. A workstation that happens to have
1.93.0 installed hides the gap; a clean CI runner does not, and the first
hosted run failed at `verus --version` with "required rust toolchain
1.93.0-x86_64-unknown-linux-gnu not found". The release records that name in
its own `version.json`, so installation can read it rather than duplicating a
pin that would drift at the next Verus bump.

## Decision log

D1 (2026-11-03): import consumer-side conventions and delegate installation to
shared tooling. The repository owns pins, selected inputs, commands, and CI;
the package owns downloading, archive validation, installation, and Verus
process execution. Do not vendor its Python implementation.

D2 (2026-11-03): begin with Linux x86_64 verification, Kani 0.67.0, and Verus
0.2026.01.30.44ebdee from the inspected Chutoro pin set. Record other platforms
as unvalidated for this new lane; leave ordinary cross-platform CI intact. Use
Python 3.14 only for the prover tool. Do not raise the existing script baseline
or import donor compatibility aliases.

D3 (2026-11-03): start with uncached installation, following Chutoro's measured
choice rather than Netsuke's multi-directory cache. Local Cargo caches remain
shared. Any later cache adoption needs measured benefit, a single owner per
path, complete version keys, repair tests, and trusted-event write rules.

D4 (2026-11-03): integration fixtures establish runner correctness only. Empty
production suites fail with an explicit diagnostic. A fixture success must
never be presented as a proof of configuration loading or missing-value
enumeration. Production proofs remain feature-owned and approval-gated.

D5 (2026-11-03, revised 2026-11-06): `install-verus` runs only
`prover-tools verus install`, and the Make boundary sets `VERUS_Z3_PATH` to the
release's bundled `<repository>/.verus/<version>/verus/z3`. Verus enforces an
exact solver version: with the separate `prover-tools z3 install` solver
(4.16.0) the pinned Verus 0.2026.01.30.44ebdee aborts with "expected z3 version
4.12.5, found 4.16.0", while the bundled solver verifies the positive fixture
(4 verified, 0 errors) and rejects the negative one (postcondition not
satisfied). The repository therefore ignores `.verus/` only and does not keep a
`.cache/z3/` installation. `VERUS_Z3_PATH` remains an override.

D6 (2026-11-06): `install-verus` also ensures the Rust toolchain the pinned
Verus release requires. After the shared package installs the release, a
sub-make reads the `toolchain` field from
`<repository>/.verus/<version>/verus/version.json` and runs
`rustup toolchain install <name> --profile minimal`. Reading the installed
release's own manifest avoids a second hardcoded pin. A missing name fails with
`VERUS-TOOLCHAIN` rather than skipping the step, and `VERUS_TOOLCHAIN_FILE`
exists so the contract test can supply the manifest.

## Outcomes & retrospective

The user approved implementation on 2026-11-03. The plan was published in draft
PR #651 and implementation is now authorized within its constraints and
tolerances. The remaining outcome is a tested integration, reached through
M1–M3. Kani 0.67.0 and Verus 0.2026.01.30.44ebdee are installed locally; all
three positive Kani harnesses verify, the mutant harness fails with a
counterexample, the positive Verus fixture reports 4 verified with 0 errors,
and the negative fixture reports a postcondition failure. The opt-in
integration target passes all ten tests in 16.67 seconds on a warm tree.

## Context and orientation

OrthoConfig is a Rust workspace with runtime configuration logic, derive
macros, localization crates, and the `cargo-orthohelp` documentation tool.
`docs/contents.md` indexes its design and contributor guides. `Makefile` owns
formatting, Rust lint/typecheck/test commands, Python tests, and workflow
contracts. `.github/workflows/ci.yml` owns the existing multi-platform checks.
`tests/workflow_contracts/` uses pytest and includes property tests for build
configuration. `scripts/requirements-test.txt` supplies script test tools.

A harness is a function submitted to Kani with symbolic inputs. Its loop unwind
bound limits the executions explored; a pass makes no claim beyond those
bounds. A Verus proof discharges logical obligations over a specified model. A
separate proof model needs an explicit refinement relationship to runtime code
before it can support a runtime correctness claim. A negative control is an
intentionally false assertion that the verifier must reject.

## Conformance basis

The baseline is OrthoConfig commit `2760c618b4d76d04caacc2482c7ca4630381dbff`,
including `AGENTS.md`, `Makefile`, `docs/design.md`, `docs/developers-guide.md`,
`docs/scripting-standards.md`, and `docs/documentation-style-guide.md`. There
is no separate approved terms of reference or prover-specific architecture
decision record (ADR). This is a proposal; M3 records the accepted tooling
boundary in the design and a new ADR using the next available number.

Source evidence, inspected on 2026-11-03:

- Netsuke PR #1063 head `15f413bac8fcda6a96c344261125aeddbfd2f774`:
  `Makefile`, `.github/actions/kani-cache/action.yml`,
  `.github/workflows/ci.yml`, and its changed-file inventory.[^2]
- Chutoro `0a0713af4a3452b0297dcb8d3a70f23196c1daef`: `Makefile`,
  `tools/kani/VERSION`, `tools/verus/{VERSION,SHA256SUMS}`,
  `docs/verus-toolchain.md`, `.github/workflows/kani-pr.yml`,
  `.github/workflows/nightly-kani.yml`, and the `verus-proofs` CI job.[^3]
- Shared package `592558f7183b554fc749696246984c5d7300d029`:
  `README.md` and `pyproject.toml` establish command contracts and Python
  requirements.[^4]
- Missing-values plan at `b04aae967dd9436348e93edce453aa2bb46c4f7c`:
  D4 and its verification section establish conditional future adoption.[^1]

R1 is reproducible installation and version selection, mapped to M1/E1. R2 is
faithful execution and failure reporting, mapped to M1–M2/E2. R3 is observable,
non-vacuous verifier integration, mapped to M2/E3. R4 is bounded CI and
documented feature adoption without runtime coupling, mapped to M3/E4. All four
derive from the requested tooling import and the repository verification and
architecture rules. Roadmap 7.3.1 remains a consumer, not a completed
requirement of this plan.

## Interfaces and dependencies

Add `tools/kani/VERSION` with `0.67.0` and `tools/verus/VERSION` with
`0.2026.01.30.44ebdee`. Copy Chutoro's matching checksum entry verbatim:

```plaintext
0f693297869f8804c08c1726c8b4e80cacb97741f66380ffdf5f5885dabec376  verus-0.2026.01.30.44ebdee-x86-linux.zip
```

Pin `PROVER_TOOLS_SOURCE` to the full shared-package revision above. Invoke it
through `uv tool run --python 3.14 --from ... prover-tools`, with the existing
repository-local uv cache/tool directories. Add Make targets `install-kani`,
`kani-check`, `install-verus`, `kani`, `kani-full`, `verus`, and
`test-prover-tools-integration`. Pass `--repo-root` explicitly. Kani
installation and version checking delegate to `prover-tools kani install` and
`prover-tools kani check-version`; proof execution uses the same selected
Cargo-home executable directly. Verus execution delegates to
`prover-tools verus run` with repeated `--proof-file` arguments. Verus setup
installs only the pinned release and points `VERUS_Z3_PATH` at the solver
bundled inside it, as revised in D5.

Expose `KANI`, `KANI_FLAGS`, `KANI_INSTALL_FLAGS`, `VERUS_FLAGS`, and
`VERUS_INSTALL_FLAGS` as documented developer overrides. Select the default
Kani executable from the active Cargo home without replacing the caller's Cargo
home. Keep a shell-safe command representation when forwarding it to
`--kani-command`; test both shell quoting and the package's command parsing.
Apply any required executable precedence to installer subprocesses too,
including `--compile`, without changing global linker precedence.

`kani` selects an explicit smoke inventory; `kani-full` selects every
registered production harness. Both check the selected Kani version first.
Until a feature registers a real harness, these targets must fail with
`FORMAL-EMPTY: no production Kani harnesses registered`. `verus` similarly
fails with `FORMAL-EMPTY: no production Verus proofs registered` when neither
an explicit proof argument nor a production inventory exists. Never inherit the
package's Chutoro-specific default proof path. An explicit `VERUS_FLAGS` proof
list must run in order and stop at its first failure.

Keep real-tool canaries in `tests/fixtures/prover_tools/`, with an independent
minimal Kani Cargo manifest declaring its own workspace and `cfg(kani)` lint
recognition. Its build output belongs under the repository `target/` tree, not
`/tmp`; its registry cache is the default shared Cargo cache. Positive and
negative Verus fixtures stay outside ordinary Rust compilation. These are
integration-test inputs, not production proof inventory entries.

The existing pytest framework should own Make boundary tests in
`tests/workflow_contracts/prover_tools_test.py` and real-tool tests in
`scripts/tests/test_prover_tools_integration.py`. Mark real-tool tests for
explicit execution so ordinary `make test` neither downloads tools nor fails
because a prover is absent. The explicit integration target must fail if the
tools are absent; it cannot skip. Extend existing readers only for demonstrated
reuse and document their test-only ownership in the developer's guide.

## Verification plan

V1 / E1: installed identity must match repository pins, and installation,
version checking, and execution must select the same executable. Parameterize
Make boundary tests over default/custom Cargo homes, a conflicting earlier
`PATH` entry, spaces and apostrophes, missing executables, malformed pin files,
and version mismatch. Fakes record argument vectors, not just shell text.
`make test-workflow-contracts` must reject missing targets before M1 and pass
these cases afterwards. Seed a wrong-executable selection to establish that
these tests fail. A real install followed by `make kani-check` is the external
boundary evidence; checksum and installer correctness remain upstream axioms.

V2 / E2: ordered proof execution preserves argument boundaries and fails on any
failed invocation. Use pytest parameterization for zero, one, and multiple
proof files, quoted paths, extra flags, installer failure, missing proof,
version mismatch, and non-zero verifier exit. Add a property test over short
argument lists with spaces and quotes to check faithful forwarding. Record
child status in diagnostics; Make may map a child failure to its own non-zero
exit code, so do not assert numeric equality between those two layers. Run a
batch with a false second proof and a third marker fixture: the first must run,
the second must fail, and the third must not run. A seeded continue-after-error
defect must fail this contract.

V3 / E3: real verifiers must accept valid input and reject false assertions.
Create a tiny fixture-only bounded counter kernel over four symbolic Boolean
inputs, counting selected inputs into a `u8`. Kani checks the result against an
independently expressed sum, the range 0–4, and reachable zero/full/mixed
cases, with `#[kani::unwind(5)]`. A separately selected mutant omits the last
input and must produce a counterexample with that input selected. No assumption
excludes that witness. This proves fixture behaviour within four inputs, not
production code or unbounded behaviour.

For Verus, use a fixture-only recursive count over a finite Boolean sequence.
Prove the non-trivial bound `0 <= count(s) <= s.len()` by induction on length,
with empty and append cases, rather than assuming the conclusion. Include
inhabited empty, singleton, and mixed witnesses. A separate negative fixture
claims `count(s) < s.len()` without a precondition excluding all-true or empty
sequences and must fail verification. No `assume`, `admit`, or external proof
body may close these obligations. `make test-prover-tools-integration` must
observe successful verification and a genuine assertion failure, not a compiler
error or timeout, for each tool. Record verifier versions, fixture names, proof
counts, bounds, and negative-control diagnostics.

V4 / E4: CI actually executes the intended inputs within limits. Workflow
contracts inspect triggers, job names, complete action pins, read-only
permissions, timeout, command invocations, and retention on failure. A mutated
workflow that omits the integration invocation must fail the contract. The
hosted run must show a non-zero fixture count and both accepted and rejected
controls, with no skipped required step. A cancelled or timed-out job leaves
acceptance incomplete. Local tests alone do not establish hosted execution.

External axioms are the pinned verifier/compiler/solver semantics, the shared
package's documented CLI and checksum enforcement, `uv` and `rustup` tool
resolution, and the operating system's process/exit behaviour. This task tests
repository-owned wiring at those boundaries, not third-party internals. Real
production Kani compatibility, model-to-runtime refinement, schema fidelity,
and unbounded configuration properties remain outside the fixture evidence.

## Plan of work and milestones

### M1 — pinned installation and command boundary

After approval, re-read the donor files at the recorded revisions and inspect
subsequent PR #1063 fixes, especially Cargo-home quoting and source-install
precedence. Reconcile necessary fixes in this plan without silently changing
tool pins. Run baseline workflow contracts. Add the V1/V2 boundary tests first
and demonstrate missing-target or wrong-command failures before editing the
Makefile. Use existing fixture/readers and injected executable fakes.

Add pins, Make variables and targets, and narrow ignore rules for `.verus/` and
fixture build artefacts if existing rules do not cover them. Empty production
inventories must fail explicitly. Add contributor guidance for installation and
the tool-only Python version. Pass the boundary tests and all code commit gates
before committing this coherent installation plateau. E1/E2 establish R1 and
the command part of R2; real verifier operation and CI remain outstanding.
Recovery is a targeted revert of this atomic commit; no shared installations or
unrelated cache directories may be deleted.

### M2 — real-tool integration and future proof registration

Add the fixture package, Verus files, and explicit pytest integration target.
First demonstrate the runner tests failing because wiring or fixtures are
absent, then establish the negative controls with genuine verifier rejection.
Implement the smallest runner-test wiring and make both positive controls pass.
Keep all mutations isolated to fixture inputs; do not mutate the production
tree or install a fake tool over a real executable.

Install and run the pinned tools sequentially, capturing versions and counts.
Test a repeated install and interrupted/missing-executable repair through the
shared tool's interface. Do not copy its downloader tests wholesale. Verify that
`make test` skips only the explicitly marked real-tool tests while the
explicit target runs all of them and fails on missing tools.

Document the production inventory format in `docs/developers-guide.md`: each
entry names its owning feature, source function/model bridge, harness or proof
file, inputs, bounds, assumptions, expected count, and smoke/full selection. No
production entries exist at this plateau. E3 and completed E2 establish R2 and
R3 after all code gates pass. Recovery removes only fixture-owned outputs;
installation failures are retried without changing the pin. Feature proofs and
hosted integration remain outside this milestone.

### M3 — CI and contributor adoption

Add `.github/workflows/prover-tools.yml` with a Linux x86_64 integration job on
relevant pull-request paths, manual dispatch, and a weekly schedule. Paths must
include this workflow, Makefile, pin files, integration fixtures/tests, and
workflow-contract readers. Use repository-standard pinned actions and
runner/fork conventions, `contents: read`, no secrets, and the 30-minute limit.
Run installs, checks, and the explicit integration target in order. Keep `uv`
cache ownership with exactly one existing setup step; do not add a prover
archive cache. Upload fixture logs and version/count summaries on success and
failure with explicit 14-day retention. Disable sccache for the job: Kani
drives its own compiler, which sccache cannot cache, so the shared setup action
must not export a rustc wrapper into the fixture build. The repository's
sccache-wiring contract records this job as needing no backend.

Do not introduce always-green production proof jobs. Document the later
extension: a feature with approved production obligations registers them, adds
a bounded Kani pull-request smoke lane and a full scheduled lane, and adds
scheduled/manual Verus execution. Proof-related path triggers must cover the
production dependencies as well as harnesses; an empty selected inventory
fails. Verus promotion to required pull-request checks needs measured stability
and an explicit later decision.

Update `docs/developers-guide.md`, `docs/repository-layout.md`, and
`docs/design.md`; add the next available ADR for the tooling boundary and index
it in `docs/contents.md`. Include command examples, platform limits, installer
side effects, empty-suite semantics, failure interpretation, fixture versus
production claims, and the feature-adoption checklist. No user's-guide
behaviour change is implied by maintainer tooling. E4 requires workflow
contracts and a hosted integration run, followed by all local code gates. A
scheduled run cannot be observed before merge; an equivalent manual run
establishes execution while schedule configuration is checked statically.
Record any hosted evidence still pending rather than declaring completion.

At each plateau, review R1–R4, actual dependency directions, public/API and
persisted-format changes, pins, and the target feature's deferred scope. Update
trace links and discoveries before committing. No compatibility layer is
needed: these are new internal tooling interfaces with no deployed consumer.
Any invalidated assumption or architecture departure requires an explicit
accepted deviation before proceeding. Complete M3 only after documentation,
ADR, plan, and actual evidence agree; rollback removes the new workflow while
preserving existing CI.

## Concrete steps and validation

Run commands from the repository root, individually, stopping at the first
unexpected failure. Use `set -o pipefail` so `tee` cannot hide a failing gate.
For this plan-only publication, run:

```bash
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-ortho-config-import-prover-tools.out
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-import-prover-tools.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-import-prover-tools.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-import-prover-tools.out
git diff --check
```

For each implementation plateau, run relevant baseline and red tests first,
then their green counterparts, followed sequentially by `make fmt`,
`make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make test-workflow-contracts`, `make markdownlint`, `make nixie`,
`mbake validate Makefile`, and `git diff --check`. Log each gate under `/tmp`
with the same action/project/branch naming. Formatting edits require checking
the final candidate again. Prefer the scrutineer agent for full gate runs.
Never commit a red or incomplete gate.

After M2 the following additional commands must run sequentially:

```bash
make install-kani
make kani-check
make install-verus
make test-prover-tools-integration
```

Expected results are a matching Kani version, successful pinned Verus setup,
and a non-empty passing integration suite whose internal negative controls were
rejected for their intended assertions. Tool downloads, version strings, or a
zero-test exit are insufficient evidence. `make kani`, `make kani-full`, and
unconfigured `make verus` intentionally fail with the documented `FORMAL-EMPTY`
messages until production obligations are registered. Explicit Verus fixture
paths are accepted only as tooling integration evidence.

## Idempotence and recovery

Installation must be repeatable with the same pins; an existing installation
still needs identity checking. A failed download or checksum validation must
leave a failure status and permit a clean retry through the shared installer.
Never delete the shared Cargo or Rust installation to recover. Record a
conflicting existing Kani version before installation and document that Kani
uses the active Cargo-home binaries, while Verus uses repository-local storage.

Keep normal builds and fixture build trees under `target/`. Remove only
identified fixture outputs after tests and retain failure logs. Revert a
milestone through its atomic commit if necessary. New caches, platform support,
production proof kernels, or changes to external assumptions require updating
this plan and obtaining approval for a material deviation.

## Artefacts and notes

Retain the red/green contract evidence, real-tool version output, positive
proof counts, negative counterexamples, resource measurements, final candidate
commit, and hosted run URL in Progress or the implementation PR. Distinguish
source failures from unavailable infrastructure. Publication of this draft is
not implementation approval and does not discharge E1–E4.

[^1]: [Missing-required-values plan at the inspected revision](https://github.com/leynos/ortho-config/blob/b04aae967dd9436348e93edce453aa2bb46c4f7c/docs/execplans/7-3-1-enumerating-missing-required-values.md).

[^2]: [Netsuke PR #1063](https://github.com/leynos/netsuke/pull/1063)
    and [inspected Makefile](https://github.com/leynos/netsuke/blob/15f413bac8fcda6a96c344261125aeddbfd2f774/Makefile).

[^3]: [Chutoro source revision](https://github.com/leynos/chutoro/tree/0a0713af4a3452b0297dcb8d3a70f23196c1daef)
    and [Verus guide](https://github.com/leynos/chutoro/blob/0a0713af4a3452b0297dcb8d3a70f23196c1daef/docs/verus-toolchain.md).

[^4]: [Shared prover tools at the proposed pin](https://github.com/leynos/rust-prover-tools/tree/592558f7183b554fc749696246984c5d7300d029).

[^5]: [Kani attributes](https://model-checking.github.io/kani/reference/attributes.html)
    explains that `#[kani::proof]` identifies a harness and is the smallest
    verification unit.

[^6]: [Using Kani](https://model-checking.github.io/kani/usage.html)
    recommends `cargo kani` for Cargo packages and describes harness selection.

[^7]: [Kani installation guide](https://model-checking.github.io/kani/install-guide.html)
    lists `x86_64-unknown-linux-gnu` as a prebuilt platform.

[^8]: [Verus guide](https://verus-lang.github.io/verus/guide/)
    describes static verification against user specifications over executions.

[^9]: [Netsuke PR #1063 at its live head](https://github.com/leynos/netsuke/pull/1063)
    remains open at `15f413bac8fcda6a96c344261125aeddbfd2f774`; its latest
    CodeRabbit follow-ups identify shell quoting and installer PATH precedence.

[^10]: [Pinned shared-tool README](https://github.com/leynos/rust-prover-tools/blob/592558f7183b554fc749696246984c5d7300d029/README.md)
    documents the separate Z3 install and `VERUS_Z3_PATH`; its source installs
    Z3 under `.cache/z3/4.16.0/z3`. Measured on 2026-11-06, the pinned Verus
    release rejects that solver and accepts only its bundled 4.12.5 build.

Revision note (2026-11-03): initial proposal separates installation and runner
integration from feature-owned proof obligations, preserving the reviewed
missing-values design and recording immutable donor evidence.

Revision note (2026-11-03): mark the plan APPROVED following the user’s
explicit instruction to begin implementation; M1–M3 remain outstanding.

Revision note (2026-11-06): record the D5 revision after measuring the solver
mismatch, add the real-tool integration target and evidence file, and bound one
fixture invocation at 240 seconds so the first invocation may compile the
fixture crate; a warm run of all ten integration tests took 16.67 seconds.

Revision note (2026-11-03): record the approved M1 baseline and official
Kani/Verus documentation consulted through Firecrawl; this constrains fixture
claims to runner behaviour and leaves pinned-binary execution as acceptance.
