# Add an opt-in agent-native policy configuration (roadmap 7.1.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

`cargo-orthohelp` is the reference command-line tool that generates
documentation and agent-facing metadata for configuration structures built
with the OrthoConfig library. Roadmap item 7.1.1 adds the first enforcement
surface for the agent-native command-line interface (CLI) design: an opt-in
policy configuration with `off`, `warn`, and `deny` modes, canonical default
vocabulary (verbs and flags), and explicit project exceptions that are
visible in policy output.

After this change, a project maintainer can run:

```console
cargo orthohelp --check-agent-native --package my-cli
```

and receive a machine-stable JSON policy report describing the enforcement
mode in effect, the canonical vocabulary the policy will hold the project
to, any configured exceptions, and any findings about the configuration
itself. In `deny` mode, deny-level findings cause a non-zero exit so
continuous integration can gate on the policy. The configured policy mode
and exceptions also become visible in the generated agent-context document,
so downstream agents can see which conventions a CLI has committed to.

This item deliberately delivers the configuration and reporting machinery
only. The actual off-policy vocabulary diagnostics (flagging `info`, `ls`,
`--format=json`, and similar) are roadmap item 7.1.2 and are out of scope
here, except that this plan must leave 7.1.2 a clean seam to plug rules
into.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- Schema ownership follows ADR-003
  (`docs/adr-003-define-schema-ownership-for-agent-native-contracts.md`):
  the policy-report schema stays in `cargo_orthohelp::policy`; reusable
  agent-context types stay in `ortho_config::agent_context`. Do not move
  either schema between crates.
- `ORTHO_POLICY_REPORT_SCHEMA_VERSION` stays at `"1"`. Only additive,
  optional, serde-defaulted fields may be added to the policy-report types
  (per the bump-versus-additive rule in `docs/developers-guide.md`, Schema
  ownership section). The same rule applies to
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` (`"1"`) for agent-context additions.
- Existing golden snapshots for man, PowerShell, and intermediate
  representation (IR) outputs must not change. Agent-context snapshots may
  gain additive fields only, and every such change must be reviewed
  deliberately (never blind-accepted).
- No circular dependencies between crates. `cargo-orthohelp` already
  depends on `ortho_config`; `ortho_config` must not gain any dependency on
  `cargo-orthohelp`.
- Public API of `ortho_config` may only grow additively (new optional
  fields with `#[serde(default)]` and matching builder defaults).
- The Whitaker wrapper must never be installed, upgraded, or downgraded by
  this work (per `AGENTS.md`).
- All prose follows en-GB-oxendict spelling and the documentation style
  guide (`docs/documentation-style-guide.md`).

## Tolerances (exception triggers)

- Scope: if implementation (excluding tests, snapshots, and docs) requires
  changes to more than 15 source files or more than roughly 1,200 net lines,
  stop and escalate.
- Interface: if an existing public function or type signature in
  `ortho_config` or `cargo_orthohelp` must change incompatibly (not merely
  gain an additive field), stop and escalate.
- Schema: if either schema version constant appears to need a bump, stop
  and escalate — the design intent is additive-only.
- Dependencies: adding `googletest` and `pretty_assertions` as
  dev-dependencies is pre-authorized by the task brief. Any other new
  dependency (dev or otherwise) requires escalation.
- Iterations: if a gate (`make check-fmt`, `make typecheck`, `make lint`,
  `make test`) still fails after three fix attempts on the same failure,
  stop and escalate.
- Ambiguity: if the design documents and code conflict in a way not already
  covered by the Decision Log, stop and present options.

## Risks

- Risk: `make lint` (Whitaker suite) may be red on `main` for reasons
  unrelated to this change (observed on earlier branches).
  Severity: medium. Likelihood: medium.
  Mitigation: run the full gate suite on the unmodified branch first
  (Milestone 0) and record the baseline. Only failures introduced by this
  work block progress; pre-existing failures are recorded in
  `Surprises & Discoveries` and reported to the user.
- Risk: the design document's policy-report example
  (`docs/agent-native-cli-design.md` §3.3) shows `file` and `range` flat on
  each result, but the shipped `cargo_orthohelp::policy` schema nests them
  under `location`. Divergence could confuse consumers.
  Severity: low. Likelihood: certain (already observed).
  Mitigation: Decision D4 reconciles the design document to the shipped
  code, since schema version 1 is already published in the crate.
- Risk: surfacing exceptions in agent context requires additive fields in
  `ortho_config::agent_context`, which is a published schema; a mistake
  here has workspace-wide blast radius.
  Severity: medium. Likelihood: low.
  Mitigation: additive optional fields only, guarded by the existing
  wire-contract snapshot, round-trip property test, and forward-compat
  tests in `ortho_config/src/agent_context/`.
- Risk: `--check-agent-native` needs a report output channel; choosing
  stdout versus an artefact file wrongly could constrain 7.1.2.
  Severity: low. Likelihood: medium.
  Mitigation: Decision D5 writes the JSON artefact and prints a summary;
  the expert review checkpoint validates this before implementation.

## Progress

- [ ] Milestone 0: baseline gates recorded; branch prepared.
- [ ] Milestone 1: canonical vocabulary defaults module (red, green,
  refactor).
- [ ] Milestone 2: policy configuration model and Cargo metadata parsing.
- [ ] Milestone 3: `--check-agent-native` CLI wiring, report emission, and
  deny-mode exit path.
- [ ] Milestone 4: policy visibility in agent-context output.
- [ ] Milestone 5: behavioural tests, golden snapshots, documentation, ADR,
  roadmap tick.

## Surprises & discoveries

- Observation: `cargo_orthohelp::policy` already exists with the complete
  report schema (`PolicyReport`, `PolicyMode`, `PolicySeverity`,
  `PolicyResult`, `SourceLocation`, `PolicySummary`, and
  `ORTHO_POLICY_REPORT_SCHEMA_VERSION = "1"`).
  Evidence: `cargo-orthohelp/src/policy/mod.rs`.
  Impact: 7.1.1 is wiring and configuration work, not schema invention. The
  plan reuses the shipped types unchanged except for additive fields.

## Decision log

- Decision D1: the opt-in configuration surface is
  `[package.metadata.ortho_config.policy]` in the target package's
  `Cargo.toml`, parsed by extending `OrthoConfigMetadata` in
  `cargo-orthohelp/src/metadata.rs`. A dedicated policy file is deferred.
  Rationale: `package.metadata.*` is Cargo's documented third-party
  extension point, the crate already parses `package.metadata.ortho_config`
  there, and prior art (cargo-dist) shows dedicated files only become
  necessary once configuration outgrows a metadata table. The 7.1.1
  configuration (one mode plus two exception lists) is small.
  Date/Author: 2026-08-06, planning agent.
- Decision D2: canonical vocabulary defaults live in a new
  `cargo_orthohelp::policy::vocabulary` module as public constants:
  `CANONICAL_VERBS` (`get`, `list`, `create`, `update`, `delete`, `jobs`,
  `profile`, `feedback`) and `CANONICAL_FLAGS` (`--json`, `--no-input`,
  `--force`, `--dry-run`, `--limit`, `--cursor`, `--wait`, `--profile`,
  `--deliver`). The private `CANONICAL_VERBS` constant currently in
  `cargo-orthohelp/src/agent_context/mod.rs` is removed and that module
  imports the policy constant instead, so there is exactly one source of
  truth.
  Rationale: 7.1.2 (lint rules) and the agent-context verb mapper must
  agree on the same list; duplication would drift. The full verb list
  follows design §5, which is a superset of the roadmap bullet list.
  Date/Author: 2026-08-06, planning agent.
- Decision D3: exceptions are modelled as explicit allowlists with
  mandatory reasons:
  `exceptions = [{ kind = "verb"|"flag", name = "...", reason = "..." }]`.
  Exceptions are surfaced twice: as an additive, serde-defaulted
  `exceptions: Vec<PolicyException>` field on `PolicyReport`, and as an
  additive optional field on `ortho_config::agent_context::AgentPolicy`.
  Rationale: roadmap 7.1.1 requires exceptions "visible in policy output";
  design §5 additionally requires them "explicit and visible in generated
  context". A mandatory reason keeps exceptions honest and reviewable
  (cargo-deny's `skip`/`allow` precedent).
  Date/Author: 2026-08-06, planning agent.
- Decision D4: reconcile the §3.3 report example in
  `docs/agent-native-cli-design.md` to the shipped schema (nested
  `location` object) rather than changing the code.
  Rationale: schema version 1 already exists in the published crate; the
  design document is the cheaper thing to correct, and the nested shape is
  closer to SARIF's `physicalLocation` structure, easing a future SARIF
  export.
  Date/Author: 2026-08-06, planning agent.
- Decision D5: `cargo orthohelp --check-agent-native` writes
  `policy-report.json` atomically to the output directory (same channel as
  other generator artefacts) and prints a short human summary to standard
  error. Standard output is reserved so a later item can stream the JSON
  report there when a structured-output flag for `cargo-orthohelp` itself
  lands (design §9 records that gap).
  Rationale: consistency with the existing artefact pipeline
  (`output::write_agent_context`) and with the atomic-write requirement in
  design §9, without foreclosing the tool's own future `--json` mode.
  Date/Author: 2026-08-06, planning agent.
- Decision D6: deny-mode failures exit through a new
  `OrthohelpError::PolicyViolation { deny_count: usize }` variant mapped to
  the process's standard failure exit (code 1), after the report artefact
  has been written.
  Rationale: design §3.3 requires "a validation-class failure"; the
  simplest conforming behaviour is a distinct error variant with a clear
  message. A richer exit-code taxonomy is roadmap 7.2.5 and must not be
  decided here.
  Date/Author: 2026-08-06, planning agent.
- Decision D7: with no vocabulary lint rules in scope (they are 7.1.2), the
  evaluator in this milestone emits configuration-sanity findings only:
  an exception naming a vocabulary item that is already canonical (code
  `redundant_exception`, severity warn) and an exception whose `kind` and
  `name` cannot ever match policy vocabulary shape (code
  `malformed_exception`, severity deny). This gives warn and deny paths
  real, testable findings and gives 7.1.2 a working rule seam.
  Rationale: shipping a checker whose deny mode is unreachable would leave
  the exit path and report pipeline untested until 7.1.2.
  Date/Author: 2026-08-06, planning agent.
- Decision D8: adopt `googletest` and `pretty_assertions` as
  dev-dependencies of `cargo-orthohelp` for the new tests, as directed by
  the task brief; existing tests are not rewritten.
  Rationale: explicit instruction in the task brief; confined to new test
  code so the change is low-risk and reversible.
  Date/Author: 2026-08-06, planning agent.

## Outcomes & retrospective

To be completed at milestones and at the end of the work.

## Context and orientation

The workspace (`/` refers to the repository root) contains, among others:

- `ortho_config/` — the library crate. `ortho_config/src/agent_context/`
  owns the agent-context schema (`AgentContext`, `AgentPolicy { agent_native:
  PolicyMode }`, `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"`), guarded by
  insta wire-contract snapshots, rstest table tests, a proptest round-trip
  test, and a forward-compatibility test.
- `cargo-orthohelp/` — the generator binary and library. Key modules:
  `src/cli/mod.rs` (clap definitions: `Cli` → `CargoSubcommand::Orthohelp`
  → `Args` with `--package`, `--format`, `--out-dir`, and others),
  `src/metadata.rs` (parses `[package.metadata.ortho_config]` into
  `OrthoConfigMetadata`), `src/bridge.rs` (builds and runs an ephemeral
  bridge crate that emits the target CLI's `DocMetadata` IR JSON),
  `src/schema/mod.rs` (`DocMetadata`, `FieldMetadata`, `CliMetadata` —
  where command paths and flag longs live), `src/agent_context/mod.rs`
  (`bridge_ir_to_agent_context`, currently holding a private
  `CANONICAL_VERBS`), `src/policy/mod.rs` (the shipped report schema),
  `src/output.rs` (atomic artefact writers), `src/error.rs`
  (`OrthohelpError`), and `src/main.rs` (pipeline orchestration).
- `tests/fixtures/orthohelp_fixture/` — a fixture package exercised by
  golden and behavioural tests (`SimpleFixtureConfig`, `FixtureConfig`,
  `NestedFixtureConfig`).
- `cargo-orthohelp/tests/` — `features/*.feature` files with rstest-bdd
  step modules under `tests/rstest_bdd/behaviour/`, wired by
  `scenarios!` in `behaviour/scenarios.rs`; golden snapshot tests under
  `tests/golden/` using insta with explicit snapshot names.

Terms: "agent context" is the compact machine-oriented JSON document
describing how to invoke a CLI (design §3.2). The "policy report" is the
machine-readable output of a policy check run (design §3.3). "IR" is the
documentation intermediate representation emitted by the bridge crate.

Relevant skills for the implementer: `leta` (code navigation),
`rust-router` → `rust-types-and-apis` (additive schema fields, newtype
choices) and `rust-unit-testing` (rstest fixtures, insta, assertions),
`proptest` (round-trip property tests), `commit-message` and
`comenq-coderabbit` (gating workflow). Relevant documents:
`docs/agent-native-cli-design.md` §3.2, §3.3, §5;
`docs/cargo-orthohelp-design.md` §6 and §10; `docs/developers-guide.md`
(Schema ownership, Behavioural test layout, Snapshot tests);
`docs/rust-testing-with-rstest-fixtures.md`;
`docs/rtest-bdd-users-guide.md`; `docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`.

## Plan of work

### Milestone 0 — baseline

Run the full gate suite unchanged (`make check-fmt`, `make typecheck`,
`make lint`, `make test`, `make markdownlint`, `make nixie`), via the
`scrutineer` subagent, and record pass/fail per gate in `Progress`. Any
pre-existing failure is recorded and reported, not fixed here.

### Milestone 1 — canonical vocabulary defaults (red, green, refactor)

Create `cargo-orthohelp/src/policy/vocabulary.rs` declaring
`pub const CANONICAL_VERBS: [&str; 8]` and
`pub const CANONICAL_FLAGS: [&str; 9]` with rustdoc linking design §5, plus
`pub fn is_canonical_verb(&str) -> bool` and
`pub fn is_canonical_flag(&str) -> bool` (flag matching accepts the long
name with or without the `--` prefix; document the normalization).

Red: add rstest table tests (in `policy/vocabulary/tests.rs` or a
`#[cfg(test)]` module) asserting membership for every canonical item and
non-membership for `info`, `ls`, `--format`, `--output`,
`--skip-confirmations`; use `googletest` matchers or `pretty_assertions`
for the equality assertions. Run the focused test and observe failure
(module does not exist yet, so the red stage is the compile failure of the
test target followed by first failing assertions once stubs exist).

Green: implement the module minimally. Refactor: replace the private
`CANONICAL_VERBS` in `cargo-orthohelp/src/agent_context/mod.rs` with an
import of the new constant; confirm the existing agent-context unit,
property, and golden tests still pass unchanged.

### Milestone 2 — policy configuration model and metadata parsing

In `cargo-orthohelp/src/policy/config.rs` define:

```rust
pub struct PolicyConfig {
    pub mode: PolicyMode,          // default: PolicyMode::Off (opt-in)
    pub exceptions: Vec<PolicyException>,
}

pub struct PolicyException {
    pub kind: ExceptionKind,       // Verb | Flag, snake_case on the wire
    pub name: String,
    pub reason: String,
}
```

Extend `OrthoConfigMetadata` in `cargo-orthohelp/src/metadata.rs` with
`policy: Option<PolicyConfigMetadata>` deserialized from
`[package.metadata.ortho_config.policy]`, with
`mode = "off" | "warn" | "deny"` and an `exceptions` array of tables.
Unknown keys inside the policy table are a deserialization error (strict),
because policy configuration that silently ignores a typo is worse than a
failure; record this as strictness intent in the ADR.

Red: rstest cases over TOML fragments — absent table (defaults to `off`),
each mode value, exceptions with and without `reason` (missing reason is
an error), unknown key (error). Property test (proptest): serialize an
arbitrary `PolicyConfig` to the metadata TOML shape and parse it back;
round-trip must be lossless, and `PolicySummary::from_results` invariants
(`total == off + warn + deny`) hold for arbitrary result vectors. Green:
implement. Refactor: extract shared parsing helpers if duplication with
existing metadata parsing appears.

### Milestone 3 — CLI wiring, evaluation, report emission, deny exit

Add to `Args` in `cargo-orthohelp/src/cli/mod.rs`:
`--check-agent-native` (bool) and `--policy-mode <off|warn|deny>`
(optional override of the configured mode; command-line wins over
metadata, mirroring the `--root-type` precedent in `metadata.rs`).

Add `cargo-orthohelp/src/policy/evaluate.rs` with
`pub fn evaluate(config: &PolicyConfig) -> PolicyReport`, implementing the
Decision D7 configuration-sanity findings and attaching the exceptions
list to the report (additive `exceptions` field on `PolicyReport`,
`#[serde(default)]`, doc-commented as schema-version-1 additive).

Wire `main.rs`: when `--check-agent-native` is set, resolve the effective
mode (`off` short-circuits with an empty report and a note on standard
error), run evaluation against the parsed configuration, write
`policy-report.json` via a new `output::write_policy_report`, print a
one-line summary (mode, counts) to standard error, and in deny mode with
deny-level findings return `OrthohelpError::PolicyViolation`. The check
runs without requiring a `--format` generator pass, but composes with one.

Red first: unit tests for `evaluate` (empty config → empty report; each
sanity finding; summary counts), rstest cases for mode resolution
(metadata only, flag only, flag overrides metadata), and an insta snapshot
of a representative `policy-report.json` (explicit snapshot name,
`tests/golden/policy_report_tests.rs`, following the agent-context golden
pattern). Then green, then refactor.

### Milestone 4 — policy visibility in agent context

In `ortho_config/src/agent_context/mod.rs`, extend `AgentPolicy` with an
additive `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
`exceptions: Vec<PolicyExceptionRef>` (kind, name, reason — a small
mirror type owned by `ortho_config`, since agent context must not depend
on `cargo_orthohelp`). Update `bridge_ir_to_agent_context` (or its caller
in `main.rs`) to populate `policy.agent_native` from the effective
configured mode and to copy exceptions in.

Red: extend the `ortho_config` wire-contract snapshot and round-trip
property strategy to cover the new field; add a forward-compat assertion
that documents omitting the field still deserializes. Extend one
`cargo-orthohelp` golden agent-context fixture (the fixture package gains
a `[package.metadata.ortho_config.policy]` table) so the generated
context shows the mode and exceptions. Review the snapshot diffs
deliberately. Then green, then refactor.

### Milestone 5 — behavioural tests, documentation, roadmap

Behavioural coverage: new `cargo-orthohelp/tests/features/
orthohelp_policy.feature` with step module
`tests/rstest_bdd/behaviour/steps_policy.rs`, wired in `scenarios.rs`.
Scenarios (final wording refined during implementation):

```gherkin
Feature: Agent-native policy check
  Scenario: Warn mode reports findings without failing
    Given a fixture package with policy mode "warn" and a redundant exception
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report lists one warning with code "redundant_exception"
    And the policy report lists the configured exceptions

  Scenario: Deny mode fails on deny findings
    Given a fixture package with policy mode "deny" and a malformed exception
    When cargo orthohelp runs with --check-agent-native
    Then the command fails with a policy violation
    And the policy report summary counts one deny finding

  Scenario: Off mode suppresses checking
    Given a fixture package with no policy table
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report records mode "off" and no findings
```

These are end-to-end: they execute the compiled binary against the fixture
package, matching the existing agent-context behavioural tests.

Documentation:

- New ADR-008 (`docs/adr-008-agent-native-policy-configuration.md`,
  house template): records D1 (metadata table surface), D3 (exception
  shape with mandatory reasons), D5/D6 (report channel and deny exit), D7
  (sanity findings), and strict unknown-key handling; listed in
  `docs/contents.md` under Decisions and archives.
- `docs/agent-native-cli-design.md`: reconcile the §3.3 example to the
  nested `location` shape (D4); note the configuration surface and
  exception visibility.
- `docs/cargo-orthohelp-design.md`: new §6 subsection describing the
  policy pipeline stage (mirroring §6.3.1's agent-context precedent) and a
  §12 note on policy-report additive fields.
- `docs/users-guide.md`: replace the "policy checking remains a future
  command surface" paragraph (Documentation and agent contracts section)
  with the new command, configuration table, modes, and exceptions.
- `docs/developers-guide.md`: record the vocabulary single-source
  convention (D2) and the policy test layout.
- `docs/roadmap.md`: tick 7.1.1 and its three sub-bullets as done.

Finally update this ExecPlan's living sections and mark COMPLETE.

### Gating cadence (every milestone)

After each milestone: run `make check-fmt`, `make typecheck`, `make lint`,
`make test` sequentially (delegated to `scrutineer`; docs-only milestones
also need `make markdownlint` and `make nixie`), commit with a
file-based imperative-mood message, then request a CodeRabbit review via
`coderabbit review --agent` (delegated to `scrutineer`) and clear all
concerns before starting the next milestone. Gates must be green before
CodeRabbit is asked to look.

## Concrete steps

All commands run at the repository root.

```console
# Baseline (Milestone 0) — via scrutineer, logs under /tmp
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-7-1-1.out
make typecheck 2>&1 | tee /tmp/typecheck-ortho-config-7-1-1.out
make lint      2>&1 | tee /tmp/lint-ortho-config-7-1-1.out
make test      2>&1 | tee /tmp/test-ortho-config-7-1-1.out

# Focused red/green loops, e.g. Milestone 1
cargo test -p cargo-orthohelp policy::vocabulary 2>&1 | tee /tmp/test-vocab.out

# Behavioural suite only
cargo test -p cargo-orthohelp --test rstest_bdd 2>&1 | tee /tmp/test-bdd.out

# Golden snapshots (review diffs deliberately; never blind-accept)
cargo insta pending-snapshots
```

Expected red-stage transcript shape (Milestone 1, before implementation):

```plaintext
error[E0432]: unresolved import `cargo_orthohelp::policy::vocabulary`
```

followed, once stubs exist, by named assertion failures; after green the
focused command reports `test result: ok`.

## Validation and acceptance

Acceptance is behavioural:

1. In a fixture package with `[package.metadata.ortho_config.policy]`
   setting `mode = "warn"` and one redundant exception, running
   `cargo orthohelp --check-agent-native --package orthohelp_fixture
   --out-dir <tmp>` exits 0, writes `<tmp>/policy-report.json` whose
   `version` is `"1"`, `mode` is `"warn"`, `summary.warn` is 1, and whose
   `exceptions` array lists the configured exception with its reason.
2. The same run with `mode = "deny"` and a malformed exception exits
   non-zero with a policy-violation error after writing the report, and
   `summary.deny` is 1.
3. With no policy table, the run exits 0 and the report records mode
   `"off"` with zero findings (opt-in default honoured).
4. `--policy-mode deny` on the command line overrides a `warn` metadata
   mode.
5. The generated agent context for a fixture with a policy table shows
   `policy.agent_native` matching the configured mode and lists the
   exceptions.
6. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
   pass (relative to the Milestone 0 baseline), as do `make markdownlint`
   and `make nixie` after documentation milestones.

Red-Green-Refactor evidence is recorded per milestone in `Progress` with
the focused command, the observed red failure, and the green pass.

## Idempotence and recovery

Every milestone is an atomic commit; `git revert` of the latest milestone
commit restores a green tree. Snapshot updates are committed together with
the change that caused them. Generator runs write to temporary directories
in tests, so re-runs are clean. If a gate fails mid-milestone, fix forward
within the iteration tolerance or escalate.

## Interfaces and dependencies

At completion the following must exist:

- `cargo_orthohelp::policy::vocabulary::{CANONICAL_VERBS, CANONICAL_FLAGS,
  is_canonical_verb, is_canonical_flag}` (public).
- `cargo_orthohelp::policy::{PolicyConfig, PolicyException, ExceptionKind}`
  and `evaluate(config: &PolicyConfig) -> PolicyReport` (public seam for
  7.1.2).
- `PolicyReport.exceptions: Vec<PolicyException>` (additive, defaulted).
- `OrthohelpError::PolicyViolation { deny_count: usize }`.
- `ortho_config::agent_context::AgentPolicy` gains
  `exceptions: Vec<PolicyExceptionRef>` (additive, defaulted).
- CLI: `--check-agent-native` and `--policy-mode` on `cargo orthohelp`.
- Dev-dependencies added to `cargo-orthohelp`: `googletest`,
  `pretty_assertions` (caret requirements).

No new runtime dependencies. `kani`/`verus` are not warranted: the only
invariants (summary counts, config round-trip) are ranges over generated
inputs, which proptest covers; there is no contractual lemma requiring
exhaustive proof.
