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
cargo orthohelp --check-agent-native --package my-cli --out-dir out
```

and receive a machine-stable JSON policy report (`out/policy-report.json`)
recording the enforcement mode in effect, the canonical vocabulary the
policy holds the project to (an explicit `vocabulary` block in the report),
any configured exceptions with their reasons, and any findings about the
configuration itself. In `deny` mode, deny-level findings cause a non-zero
exit so continuous integration can gate on the policy. The configured
policy mode and exceptions also become visible in the generated
agent-context document, so downstream agents can see which conventions a
CLI has committed to.

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
  gain additive fields; existing field *values* must not change (Decision
  D9 preserves the advertised default). Every snapshot change must be
  reviewed deliberately (never blind-accepted).
- No circular dependencies between crates. `cargo-orthohelp` already
  depends on `ortho_config`; `ortho_config` must not gain any dependency on
  `cargo-orthohelp`.
- Public API of `ortho_config` may only grow additively (new optional
  fields with `#[serde(default)]` and matching builder defaults).
- No code file may exceed 400 lines (`AGENTS.md`). `cargo-orthohelp`'s
  `src/main.rs` and `src/cli/mod.rs` both sit at 381 lines already, so new
  orchestration goes into new modules (Decision D11), not into `run()`.
- The Whitaker wrapper must never be installed, upgraded, or downgraded by
  this work (per `AGENTS.md`).
- All prose follows en-GB-oxendict spelling and the documentation style
  guide (`docs/documentation-style-guide.md`).

## Tolerances (exception triggers)

- Scope: if implementation (excluding tests, snapshots, and docs) requires
  changes to more than 18 source files or more than roughly 1,400 net lines,
  stop and escalate.
- Interface: if an existing public function or type signature in
  `ortho_config` or `cargo_orthohelp` must change incompatibly (not merely
  gain an additive field or a new builder), stop and escalate. Note: adding
  a public field to `PolicyReport` is wire-additive but source-breaking for
  downstream struct-literal construction; this specific, documented change
  is accepted because the type's rustdoc already steers construction
  through `PolicyReport::empty` and `PolicyReport::with_results`.
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
- Risk: surfacing exceptions in agent context requires additive fields in
  `ortho_config::agent_context`, which is a published schema; a mistake
  here has workspace-wide blast radius.
  Severity: medium. Likelihood: low.
  Mitigation: additive optional fields only, guarded by the existing
  wire-contract snapshot, round-trip property test, and forward-compat
  tests in `ortho_config/src/agent_context/`.
- Risk: teams may wire `--check-agent-native` into continuous integration
  while a typo in the metadata table name leaves the mode at `off`,
  producing a gate that never gates.
  Severity: medium. Likelihood: medium.
  Mitigation: Decision D13 — a loud "nothing was checked" summary line, a
  documented `jq`-based mode assertion recipe in the users' guide, and the
  documented `--policy-mode` command-line override pattern for CI.
- Risk: the deny exit shares exit code 1 with generic tool failure, so a
  CI log alone cannot distinguish policy failure from tool breakage.
  Severity: low. Likelihood: medium.
  Mitigation: the report artefact (`summary.deny`) is documented as the
  authoritative CI signal; the stderr summary prints even when the
  artefact write fails; ADR-008 records that the exit code is provisional
  until roadmap 7.2.3/7.2.5 document stable exit classes.

## Progress

- [x] Milestone 0: baseline gates recorded; branch prepared.
  Run 2026-08-09 via `scrutineer` on the clean tree (all gate logs under
  `/tmp`): `make check-fmt` PASS, `make typecheck` PASS, `make lint` PASS
  (Whitaker suite green on this baseline), `make test` PASS (55 Rust
  suites + pytest 106 passed/1 skipped), `make markdownlint` PASS (typos
  config regen: no drift), `make nixie` PASS. Verdict: green, no
  pre-existing failures. PR #416 title updated (removed the "Plan: "
  prefix); Lody session renamed to match.
- [x] Milestone 1: canonical vocabulary defaults module (red, green,
  refactor).
  - `cargo-orthohelp/src/policy/vocabulary.rs` added with
    `CANONICAL_VERBS`, `CANONICAL_FLAGS`, `is_canonical_verb`, and
    `is_canonical_flag`; 27 rstest cases green
    (`cargo test -p cargo-orthohelp policy::vocabulary`).
  - Refactor: `agent_context/mod.rs` now imports
    `crate::policy::vocabulary::CANONICAL_VERBS`; the local constant is
    removed. Agent-context unit and golden suites pass unchanged
    (snapshot-neutral).
  - Architecture discovery recorded in `Surprises & Discoveries`: the
    binary crate (`src/main.rs`) re-declares its own module tree, so the
    shared `agent_context` module needs `policy` declared in *both* the
    lib (`src/lib.rs`) and the binary (`src/main.rs`). `main.rs` gained
    `pub mod policy;`.
- [ ] Milestone 2: policy configuration model and Cargo metadata parsing.
- [ ] Milestone 3: `--check-agent-native` CLI wiring, report emission, and
  deny-mode exit path.
- [ ] Milestone 4: policy visibility in agent-context output.
- [ ] Milestone 5a: behavioural tests and policy fixture packages.
- [ ] Milestone 5b: documentation, ADR-008, CHANGELOG, roadmap tick.

## Surprises & discoveries

- Observation: `cargo_orthohelp::policy` already exists with the complete
  report schema (`PolicyReport`, `PolicyMode`, `PolicySeverity`,
  `PolicyResult`, `SourceLocation`, `PolicySummary`, and
  `ORTHO_POLICY_REPORT_SCHEMA_VERSION = "1"`).
  Evidence: `cargo-orthohelp/src/policy/mod.rs`.
  Impact: 7.1.1 is wiring and configuration work, not schema invention. The
  plan reuses the shipped types unchanged except for additive fields.
- Observation: `ortho_config::agent_context::AgentPolicy::default()` is
  `PolicyMode::Warn`, and all three golden agent-context snapshots emit
  `"agent_native": "warn"` for fixtures with no policy configuration.
  Evidence: `ortho_config/src/agent_context/mod.rs` (default impl) and
  `cargo-orthohelp/tests/golden/agent_context__*.json.snap`.
  Impact: the checker's opt-in `off` default must not rewrite the
  advertised context default; Decision D9 separates the two.
- Observation: `cargo_orthohelp::policy::PolicyMode` and
  `ortho_config::agent_context::PolicyMode` are two distinct types (the
  mirror pattern required by ADR-003).
  Evidence: both modules define the enum independently.
  Impact: Milestone 4 needs an explicit, single-point conversion
  (Decision D12 names it) so the mirrors cannot drift silently.
- Observation: the `cargo-orthohelp` *binary* crate (`src/main.rs`)
  re-declares its own module tree (`pub mod agent_context; mod bridge;
  mod cli; ...`) rather than using the library crate's modules. Files
  under `src/` such as `agent_context/mod.rs` are therefore compiled
  twice — once in `cargo_orthohelp` (lib) and once in `cargo-orthohelp`
  (bin) — and may only reference modules present in *both* crates.
  Evidence: `src/main.rs` module declarations versus `src/lib.rs`;
  `rustc` E0433 when `agent_context/mod.rs` first tried
  `use cargo_orthohelp::policy::vocabulary::CANONICAL_VERBS` (the lib
  cannot self-reference by crate name) and when it used `crate::policy`
  (the bin had no `policy` module).
  Impact: `main.rs` must declare `pub mod policy;` so shared policy
  files (`config.rs`, `evaluate.rs`, `check.rs`, `vocabulary.rs`) resolve
  `crate::policy::*` in both crates. `check.rs` may not reference the
  bin-only `cli`/`metadata` modules; its `run_policy_check` signature
  takes `&cargo_metadata::Package` and `Option<PolicyMode>` instead of
  the bin's `Args`, and the policy table is read via a shared helper on
  `PolicyConfigMetadata` rather than through `crate::metadata`. The lib
  will gain `pub mod output;` so `check.rs` can call
  `output::write_policy_report` in both crates. This obeys the plan's
  interface list (`cargo_orthohelp::policy::check::run_policy_check` and
  `output::write_policy_report`) without a bin/lib `Args` type mismatch.

## Decision log

Decisions D1–D8 were drafted before the expert design review; D9–D14 and
the amendments to D3, D5, D6, and D7 resolve that review's findings (see
the revision note at the end of this document).

- Decision D1: the opt-in configuration surface is
  `[package.metadata.ortho_config.policy]` in the target package's
  `Cargo.toml`, parsed by extending `OrthoConfigMetadata` in
  `cargo-orthohelp/src/metadata.rs`. A dedicated policy file is deferred.
  ADR-008 explicitly reserves a `rules` key inside the policy table so
  roadmap 7.1.2/7.1.3 can add per-rule levels additively (the
  ESLint/Cargo-`[lints]` shape); `mode` is documented as a global ceiling.
  Rationale: `package.metadata.*` is Cargo's documented third-party
  extension point, the crate already parses `package.metadata.ortho_config`
  there, and prior art (cargo-dist) shows dedicated files only become
  necessary once configuration outgrows a metadata table. The 7.1.1
  configuration (one mode plus an exception list) is small.
  Date/Author: 2026-08-06, planning agent.
- Decision D2: canonical vocabulary defaults live in a new
  `cargo_orthohelp::policy::vocabulary` module as public slice constants:
  `CANONICAL_VERBS: &[&str]` (`get`, `list`, `create`, `update`, `delete`,
  `jobs`, `profile`, `feedback`) and `CANONICAL_FLAGS: &[&str]` (`--json`,
  `--no-input`, `--force`, `--dry-run`, `--limit`, `--cursor`, `--wait`,
  `--profile`, `--deliver`). Slices, not fixed-length arrays, so vocabulary
  growth in 7.1.3 does not change a public type. The private
  `CANONICAL_VERBS` constant currently in
  `cargo-orthohelp/src/agent_context/mod.rs` is removed and that module
  imports the policy constant instead, so there is exactly one source of
  truth.
  Rationale: 7.1.2 (lint rules) and the agent-context verb mapper must
  agree on the same list; duplication would drift. The full verb list
  follows design §5, which is a superset of the roadmap bullet list.
  Date/Author: 2026-08-06, planning agent.
- Decision D3 (amended after review): exceptions are modelled as explicit
  allowlists with mandatory reasons and an optional command scope:
  `exceptions = [{ kind = "verb"|"flag", name = "...", reason = "...",
  command_path = "..." (optional) }]`. `command_path` scopes an exception
  to one command (space-separated invocation path); when omitted the
  exception is global. Exceptions are surfaced twice: as an additive,
  serde-defaulted `exceptions` field on `PolicyReport` (full shape,
  including `reason`), and as an additive field on
  `ortho_config::agent_context::AgentPolicy` (see D12 for the wire shape).
  Rationale: roadmap 7.1.1 requires exceptions "visible in policy output";
  design §5 additionally requires them "explicit and visible in generated
  context". A mandatory reason keeps exceptions honest and reviewable
  (cargo-deny's `skip`/`allow` precedent). The optional `command_path`
  ships now because the exception shape becomes version-1 wire surface the
  moment 7.1.1 lands, and 7.1.2's realistic exceptions are scoped ("`ls`
  is allowed under `remote`"), not global.
  Date/Author: 2026-08-06, planning agent; amended same day after review.
- Decision D4: reconcile the §3.3 report example in
  `docs/agent-native-cli-design.md` to the shipped schema (nested
  `location` object) rather than changing the code, and align the §3.3
  wording ("when JSON output is requested") with the actual behaviour of
  always writing the report artefact.
  Rationale: schema version 1 already exists in the published crate; the
  design document is the cheaper thing to correct, and the nested shape is
  closer to SARIF's `physicalLocation` structure, easing a future SARIF
  export.
  Date/Author: 2026-08-06, planning agent.
- Decision D5 (amended after review): `cargo orthohelp
  --check-agent-native` always writes `policy-report.json` atomically to
  the output directory (same channel as other generator artefacts) and
  prints a short human summary to standard error that includes the report
  path and the severity counts. Standard output is reserved so a later
  item can stream the JSON report there when a structured-output flag for
  `cargo-orthohelp` itself lands (a gap recorded in design §9). The
  authority for atomic writes is the existing convention in
  `cargo-orthohelp/src/output.rs` (temp file, rename, fsync), not design
  §9, which lists atomicity as an open gap.
  Rationale: consistency with the existing artefact pipeline
  (`output::write_agent_context`) without foreclosing the tool's own
  future `--json` mode.
  Date/Author: 2026-08-06, planning agent; amended same day after review.
- Decision D6 (amended after review): deny-mode failures exit through a
  new `OrthohelpError::PolicyViolation { deny_count: usize, report_path:
  String }` variant, returned after the report artefact has been written.
  The process exits with the standard failure code (1) via `main`'s
  existing `Result` termination, which prints the Debug representation;
  the human-facing channel is therefore the D5 stderr summary, which is
  printed before the error is returned and even when the artefact write
  fails. ADR-008 records that exit code 1 is shared with generic tool
  failure, that `policy-report.json` (`summary.deny`) is the authoritative
  CI signal, and that the code is provisional until roadmap 7.2.3
  documents stable exit classes.
  Rationale: design §3.3 requires "a validation-class failure"; the
  simplest conforming behaviour is a distinct error variant. A richer
  exit-code taxonomy is roadmap 7.2.5 and must not be decided here.
  Date/Author: 2026-08-06, planning agent; amended same day after review.
- Decision D7 (amended after review): with no vocabulary lint rules in
  scope (they are 7.1.2), the evaluator emits configuration-sanity
  findings only. The error-versus-finding discriminator is: *structural*
  problems in the policy table (unknown keys, wrong TOML types, missing
  required fields such as `reason`) are hard deserialization errors in all
  modes; *semantic* problems (values that parse but cannot be honoured)
  are policy findings. The 7.1.1 findings are: `redundant_exception`
  (severity warn) — an exception naming a vocabulary item that is already
  canonical; `duplicate_exception` (severity warn) — two exceptions with
  the same kind, name, and scope; and `malformed_exception` (severity
  deny) — an exception whose name cannot match its kind's shape, defined
  precisely as: a `flag` exception whose name, after optional `--` prefix
  normalization, is empty or contains whitespace, or a `verb` exception
  whose name is empty, contains whitespace, or begins with `-`. A
  deny-severity finding in `warn` mode is reported but non-fatal, exactly
  as design §3.3 specifies mode handling; ADR-008 documents this
  explicitly so nobody mistakes warn-mode tolerance for a bug.
  Rationale: shipping a checker whose deny mode is unreachable would leave
  the exit path and report pipeline untested until 7.1.2, and the precise
  definitions prevent the implementation improvising them mid-milestone.
  Date/Author: 2026-08-06, planning agent; amended same day after review.
- Decision D8 (amended after review): adopt `googletest` and
  `pretty_assertions` as dev-dependencies of `cargo-orthohelp` for the new
  tests, as directed by the task brief; existing tests are not rewritten.
  Division of labour, to be recorded in `docs/developers-guide.md`:
  `pretty_assertions::assert_eq` for equality comparisons where a rich
  diff aids diagnosis (JSON strings, structs); `googletest` matchers for
  collection- and matcher-shaped assertions (membership, unordered
  elements); plain `assert!` for simple booleans.
  Rationale: explicit instruction in the task brief; confined to new test
  code so the change is low-risk and reversible; a stated division
  prevents style drift between two overlapping assertion crates.
  Date/Author: 2026-08-06, planning agent; amended same day after review.
- Decision D9 (from review): advertisement default and enforcement default
  are distinct. The checker's enforcement default is `off` (the feature is
  opt-in; running `--check-agent-native` with no policy table checks
  nothing). The agent context continues to advertise
  `AgentPolicy::default()` (`warn`) when no policy table is present,
  preserving the shipped version-1 wire values and design §3.2/§3.3's
  early-adoption-defaults-to-warnings intent. Only when a policy table is
  present does the generated context carry the *configured* mode. The
  transient `--policy-mode` command-line override affects the policy
  *report*'s effective mode but never the generated agent context, which
  records what the project has committed to, not what one invocation
  used. ADR-008 explains the two defaults side by side.
  Rationale: resolves the collision between the opt-in `off` default and
  the shipped `warn` advertisement without rewriting published snapshot
  values or misrepresenting a one-off CI override as a project
  commitment.
  Date/Author: 2026-08-06, planning agent, after design review.
- Decision D10 (from review): behavioural and golden coverage uses
  dedicated policy fixture packages rather than mutating the shared
  `tests/fixtures/orthohelp_fixture/`. Two tiny packages are added:
  `tests/fixtures/orthohelp_policy_warn_fixture/` (mode `warn`, one
  redundant exception, one well-formed scoped exception) and
  `tests/fixtures/orthohelp_policy_deny_fixture/` (mode `deny`, one
  malformed exception). The existing `orthohelp_fixture` keeps no policy
  table and serves the "off by default" scenario, leaving its existing
  agent-context, roff, and PowerShell suites untouched.
  Rationale: the three behavioural scenarios need three mutually
  exclusive metadata states; the fixture package's metadata is static and
  shared by every other suite, so mutating it would poison unrelated
  tests and bridge-cache fingerprints.
  Date/Author: 2026-08-06, planning agent, after design review.
- Decision D11 (from review): pipeline placement. `--check-agent-native`
  resolves the package and parses `[package.metadata.ortho_config]` only,
  then evaluates and writes the report, *without* building the bridge
  crate or requiring `root_type`, a library target, or an `ortho_config`
  dependency. When a generator `--format` is explicitly requested in the
  same invocation, the check runs first and the generator pipeline
  follows; the default `--format ir` is treated as "not explicitly
  requested" when `--check-agent-native` is present (clap default
  detection via `ValueSource`). Orchestration lives in a new
  `cargo-orthohelp/src/policy/check.rs` exposing
  `run_policy_check(...) -> Result<PolicyCheckOutcome, OrthohelpError>`,
  called from a thin branch in `main.rs`, keeping both 381-line files
  under the 400-line cap. `--policy-mode` declares
  `requires = "check_agent_native"` so it cannot be silently ignored.
  Rationale: a policy check is most useful to packages still adopting the
  toolchain; inheriting generator preconditions (`MissingRootType`,
  bridge compilation) would make the check unusable exactly there, and an
  unconditional bridge build is wasted CI cost.
  Date/Author: 2026-08-06, planning agent, after design review.
- Decision D12 (from review): wire shapes and conversions. The agent
  context mirror type `ortho_config::agent_context::PolicyException`
  carries `kind` (string-typed on the wire for forward tolerance), `name`,
  and optional `command_path` — but *not* `reason`. Reasons are written
  for maintainers and reviewers and are published in `policy-report.json`;
  copying them into agent context, an artefact distributed to third-party
  agents, risks leaking internal context (tickets, contract names). The
  `exceptions` vector on `AgentPolicy` serializes unconditionally
  (matching the schema's house style of explicit empty collections — no
  `skip_serializing_if`), which changes existing agent-context snapshots
  by one additive field; that diff is reviewed and accepted deliberately.
  Conversions are single-point: `From<&cargo_orthohelp::policy::
  PolicyException> for ortho_config::agent_context::PolicyException` and
  `From<cargo_orthohelp::policy::PolicyMode> for
  ortho_config::agent_context::PolicyMode`, both defined at the adapter
  boundary in `cargo-orthohelp`, so mirror drift is caught at one place.
  Rationale: forward-tolerant `kind` protects old deserializing consumers
  when 7.1.3 adds kinds; omitting `reason` from the agent-distributed
  artefact satisfies design §5's "explicit and visible" with the
  exception's identity while containing the leak surface.
  Date/Author: 2026-08-06, planning agent, after design review.
- Decision D13 (from review): the off-mode failure path is made loud and
  documented. When the effective mode is `off`, the stderr summary reads
  distinctly, for example: `policy mode off (no
  [package.metadata.ortho_config.policy] table found); nothing was
  checked`, and the users' guide documents a CI recipe asserting the mode
  (`jq -e '.mode != "off"' policy-report.json`) plus the `--policy-mode
  warn|deny` override as a "fail if unconfigured" pattern. ADR-008 also
  records the residual typo gap honestly: strict unknown-key handling
  applies *inside* the policy table; a misspelt table name still resolves
  to `off`, which is why the loud summary and CI recipe exist.
  Rationale: the designed default failure mode of CI gating is a
  never-gating gate; prevention is cheap wording and documentation now.
  Date/Author: 2026-08-06, planning agent, after design review.
- Decision D14 (from review): the evaluator seam is
  `evaluate(config: &PolicyConfig, inputs: &PolicyInputs) -> PolicyReport`
  where `PolicyInputs` is a `#[non_exhaustive]` struct that is empty in
  7.1.1 (constructed via `PolicyInputs::default()`). The report gains an
  additive, serde-defaulted `vocabulary` block (`verbs: Vec<String>`,
  `flags: Vec<String>`) populated from the D2 constants, and a
  `PolicyReport::with_details(mode, results, exceptions, vocabulary)`
  constructor so `evaluate` never mutates public fields post-construction
  (the type's rustdoc already warns that direct mutation breaks the
  summary invariant). ADR-008 notes that 7.1.2 will pass the bridge IR
  through `PolicyInputs` (an additive field) and will add an additive
  `replacement: Option<String>` to `PolicyResult` so canonical
  replacements stay machine-readable.
  Rationale: 7.1.2's rules need the command tree; growing `PolicyInputs`
  additively avoids the incompatible signature change this plan's own
  tolerances forbid. The `vocabulary` block closes the gap between the
  roadmap bullet ("provide canonical defaults") and what a report consumer
  can actually see.
  Date/Author: 2026-08-06, planning agent, after design review.

## Outcomes & retrospective

To be completed at milestones and at the end of the work.

## Context and orientation

The workspace (`/` refers to the repository root) contains, among others:

- `ortho_config/` — the library crate. `ortho_config/src/agent_context/`
  owns the agent-context schema (`AgentContext`, `AgentPolicy { agent_native:
  PolicyMode }`, `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION = "1"`), guarded by
  insta wire-contract snapshots, rstest table tests, a proptest round-trip
  test, and a forward-compatibility test. `AgentPolicy::default()` is
  `Warn` (see D9).
- `cargo-orthohelp/` — the generator binary and library. Key modules:
  `src/cli/mod.rs` (clap definitions: `Cli` → `CargoSubcommand::Orthohelp`
  → `Args` with `--package`, `--format`, `--out-dir`, and others; 381
  lines), `src/metadata.rs` (parses `[package.metadata.ortho_config]` into
  `OrthoConfigMetadata`; `select_package` enforces generator preconditions
  such as `root_type` presence), `src/bridge.rs` (builds and runs an
  ephemeral bridge crate that emits the target CLI's `DocMetadata` IR
  JSON), `src/schema/mod.rs` (`DocMetadata`, `FieldMetadata`,
  `CliMetadata` — where command paths and flag longs live),
  `src/agent_context/mod.rs` (`bridge_ir_to_agent_context`, currently
  holding a private `CANONICAL_VERBS`), `src/policy/mod.rs` (the shipped
  report schema), `src/output.rs` (atomic artefact writers: temp file,
  rename, fsync), `src/error.rs` (`OrthohelpError`), and `src/main.rs`
  (pipeline orchestration; 381 lines; `fn main() ->
  Result<(), OrthohelpError>` exits 1 on error printing the Debug
  representation).
- `tests/fixtures/orthohelp_fixture/` — a fixture package exercised by
  golden and behavioural tests (`SimpleFixtureConfig`, `FixtureConfig`,
  `NestedFixtureConfig`). Its metadata is shared by every suite, hence
  Decision D10's dedicated policy fixtures.
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
`pub const CANONICAL_VERBS: &[&str]` and
`pub const CANONICAL_FLAGS: &[&str]` with rustdoc linking design §5, plus
`pub fn is_canonical_verb(&str) -> bool` and
`pub fn is_canonical_flag(&str) -> bool` (flag matching accepts the long
name with or without the `--` prefix; document the normalization).

Red: add rstest table tests (in a `policy/vocabulary/tests.rs` or a
`#[cfg(test)]` module) asserting membership for every canonical item and
non-membership for `info`, `ls`, `--format`, `--output`,
`--skip-confirmations`, using the D8 assertion division. Run the focused
test and observe failure (the module does not exist yet, so the red stage
is the compile failure of the test target followed by first failing
assertions once stubs exist).

Green: implement the module minimally. Refactor: replace the private
`CANONICAL_VERBS` in `cargo-orthohelp/src/agent_context/mod.rs` with an
import of the new constant; confirm the existing agent-context unit,
property, and golden tests still pass unchanged (this refactor is
snapshot-neutral).

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
    pub command_path: Option<String>,
}
```

Extend `OrthoConfigMetadata` in `cargo-orthohelp/src/metadata.rs` with
`policy: Option<PolicyConfigMetadata>` deserialized from
`[package.metadata.ortho_config.policy]`, with
`mode = "off" | "warn" | "deny"` and an `exceptions` array of tables.
Unknown keys inside the policy table are a deserialization error
(strict), per the D7 discriminator: structural configuration problems
fail in all modes. ADR-008 records the strictness trade-off and its
version-skew consequence (an older pinned tool hard-fails on newer policy
keys; 7.1.2+ additions must note a minimum tool version), plus the
reserved `rules` key (D1).

Red: rstest cases over TOML fragments — absent table (defaults to `off`),
each mode value, exceptions with and without `reason` (missing reason is
an error), with and without `command_path`, unknown key (error). Property
test (proptest): serialize an arbitrary `PolicyConfig` to the metadata
TOML shape and parse it back; the round-trip must be lossless, and
`PolicySummary::from_results` invariants (`total == off + warn + deny`)
hold for arbitrary result vectors. Green: implement. Refactor: extract
shared parsing helpers if duplication with existing metadata parsing
appears.

### Milestone 3 — CLI wiring, evaluation, report emission, deny exit

Add to `Args` in `cargo-orthohelp/src/cli/mod.rs`:
`--check-agent-native` (bool) and `--policy-mode <off|warn|deny>`
(optional override of the configured mode; command-line wins over
metadata for the *report*, mirroring the `--root-type` precedent; clap
`requires = "check_agent_native"` per D11).

Add `cargo-orthohelp/src/policy/evaluate.rs` implementing D14's
`evaluate(config, inputs) -> PolicyReport` with the D7 findings, the D3
exceptions attached, and the D14 `vocabulary` block populated; extend
`policy/mod.rs` with the additive `exceptions` and `vocabulary` report
fields (all `#[serde(default)]`, doc-commented as schema-version-1
additive) and the `with_details` constructor.

Add `cargo-orthohelp/src/policy/check.rs` with `run_policy_check`
implementing D11's placement (package + metadata resolution only, no
bridge), D5's report write via a new `output::write_policy_report`, the
D5/D13 stderr summary (including the report path; loud wording when the
mode is `off`), and the D6 deny return
(`OrthohelpError::PolicyViolation { deny_count, report_path }`). Wire a
thin branch into `main.rs::run` honouring the D11 `--format` interaction.

Red first: unit tests for `evaluate` (empty config → empty report with
vocabulary block; each sanity finding, including the exact D7 boundary
cases; summary counts), rstest cases for mode resolution (metadata only,
flag only, flag overrides metadata; `--policy-mode` without
`--check-agent-native` is a clap error), and an insta snapshot of a
representative `policy-report.json` (explicit snapshot name,
`tests/golden/policy_report_tests.rs`, following the agent-context golden
pattern). Then green, then refactor.

### Milestone 4 — policy visibility in agent context

In `ortho_config/src/agent_context/mod.rs`, extend `AgentPolicy` with an
additive `#[serde(default)]` `exceptions: Vec<PolicyException>` using the
D12 wire shape (string `kind`, `name`, optional `command_path`, no
`reason`; serialized unconditionally). In `cargo-orthohelp`, implement
the D12 single-point conversions and populate the generated context per
D9: advertised default (`warn`) when no policy table exists; the
configured mode when one does; never the `--policy-mode` override.

Red: extend the `ortho_config` wire-contract snapshot and round-trip
property strategy to cover the new field; add a forward-compat assertion
that a document omitting the field still deserializes, and one showing an
unrecognized `kind` string survives round-tripping. Extend the golden
suite with an agent-context run against the D10 warn fixture showing mode
and exceptions in context. Existing `orthohelp_fixture` snapshots change
only by the additive empty `exceptions` field; review those diffs
deliberately. Then green, then refactor.

### Milestone 5a — behavioural tests and policy fixtures

Add the D10 fixture packages (workspace members with minimal
`OrthoConfig` root types mirroring `orthohelp_fixture`'s simple config,
plus the policy metadata tables). New
`cargo-orthohelp/tests/features/orthohelp_policy.feature` with step
module `tests/rstest_bdd/behaviour/steps_policy.rs`, wired in
`scenarios.rs`. Scenarios (final wording refined during implementation):

```gherkin
Feature: Agent-native policy check
  Scenario: Warn mode reports findings without failing
    Given the policy warn fixture package
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report lists one warning with code "redundant_exception"
    And the policy report lists the configured exceptions
    And the policy report lists the canonical vocabulary

  Scenario: Deny mode fails on deny findings
    Given the policy deny fixture package
    When cargo orthohelp runs with --check-agent-native
    Then the command fails with a policy violation
    And the policy report summary counts one deny finding

  Scenario: Off mode suppresses checking
    Given a fixture package with no policy table
    When cargo orthohelp runs with --check-agent-native
    Then the command succeeds
    And the policy report records mode "off" and no findings
    And standard error notes that nothing was checked

  Scenario: Command-line mode override wins for the report
    Given the policy warn fixture package
    When cargo orthohelp runs with --check-agent-native --policy-mode deny
    Then the policy report records mode "deny"
```

These are end-to-end: they execute the compiled binary against fixture
packages, matching the existing agent-context behavioural tests.

### Milestone 5b — documentation, ADR, roadmap

- New ADR-008 (`docs/adr-008-agent-native-policy-configuration.md`, house
  template): records D1 (metadata table surface, reserved `rules` key),
  D3/D12 (exception shape, reason confinement), D5/D6 (report channel,
  shared exit code, authoritative signal), D7 (discriminator and finding
  codes), D9 (advertisement versus enforcement defaults), D11 (pipeline
  placement), D13 (off-mode loudness and residual typo gap), and D14
  (seam and planned 7.1.2 additive fields); listed in `docs/contents.md`
  under Decisions and archives.
- `docs/agent-native-cli-design.md`: reconcile §3.3 per D4; note the
  configuration surface, the two defaults (D9), and exception visibility.
- `docs/cargo-orthohelp-design.md`: new §6 subsection describing the
  policy pipeline stage (mirroring §6.3.1's agent-context precedent) and
  a §12 note on policy-report additive fields.
- `docs/users-guide.md`: replace the "policy checking remains a future
  command surface" paragraph (Documentation and agent contracts section)
  with the new command, configuration table, modes, exceptions, the D13
  CI recipe, and a note that exception reasons are published in
  `policy-report.json` but not in agent context.
- `docs/developers-guide.md`: record the vocabulary single-source
  convention (D2), the assertion-crate division (D8), and the policy test
  layout.
- `CHANGELOG.md`: entry under Unreleased for the new command surface and
  the additive schema fields.
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

1. Against the warn fixture (policy table with `mode = "warn"`, one
   redundant exception, one scoped exception), running
   `cargo orthohelp --check-agent-native --package
   orthohelp_policy_warn_fixture --out-dir <tmp>` exits 0, writes
   `<tmp>/policy-report.json` whose `version` is `"1"`, `mode` is
   `"warn"`, `summary.warn` counts the redundant-exception finding, whose
   `exceptions` array lists both configured exceptions with reasons, and
   whose `vocabulary` block lists the canonical verbs and flags.
2. Against the deny fixture (`mode = "deny"`, one malformed exception),
   the same command exits non-zero with a policy-violation error after
   writing the report, and `summary.deny` is 1; stderr carries the
   summary including the report path.
3. Against `orthohelp_fixture` (no policy table), the run exits 0, the
   report records mode `"off"` with zero findings, and stderr states that
   nothing was checked (opt-in default honoured).
4. `--policy-mode deny` on the command line overrides a `warn` metadata
   mode in the report; `--policy-mode` without `--check-agent-native` is
   rejected by clap.
5. The generated agent context for the warn fixture shows
   `policy.agent_native` = `"warn"` (the configured mode) and lists the
   exceptions (kind, name, scope — no reasons); contexts generated for
   packages with no policy table still advertise `"warn"` per D9, and
   existing snapshots change only by the additive `exceptions` field.
6. The policy check against a package lacking `root_type` succeeds
   without building the bridge crate (D11).
7. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
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
  is_canonical_verb, is_canonical_flag}` (public; slice constants).
- `cargo_orthohelp::policy::{PolicyConfig, PolicyException, ExceptionKind,
  PolicyInputs}` and
  `evaluate(config: &PolicyConfig, inputs: &PolicyInputs) -> PolicyReport`
  (public seam for 7.1.2; `PolicyInputs` is `#[non_exhaustive]` and empty
  in 7.1.1).
- `PolicyReport.exceptions` and `PolicyReport.vocabulary` (additive,
  serde-defaulted) plus `PolicyReport::with_details(...)`.
- `cargo_orthohelp::policy::check::run_policy_check` and
  `output::write_policy_report` (atomic).
- `OrthohelpError::PolicyViolation { deny_count: usize, report_path:
  String }`.
- `ortho_config::agent_context::AgentPolicy` gains
  `exceptions: Vec<PolicyException>` (additive, defaulted, always
  serialized; string-typed `kind`; no `reason` field).
- Single-point `From` conversions in `cargo-orthohelp` for the mode and
  exception mirrors (D12).
- CLI: `--check-agent-native` and `--policy-mode` (with `requires`) on
  `cargo orthohelp`.
- Fixture packages `orthohelp_policy_warn_fixture` and
  `orthohelp_policy_deny_fixture` under `tests/fixtures/`.
- Dev-dependencies added to `cargo-orthohelp`: `googletest`,
  `pretty_assertions` (caret requirements).

No new runtime dependencies. `kani`/`verus` are not warranted: the only
invariants (summary counts, config round-trip) are ranges over generated
inputs, which proptest covers; there is no contractual lemma requiring
exhaustive proof.

## Revision note (2026-08-06)

Revised after a six-lens expert design review (structural integrity,
contracts, alternatives, forward pressure, failure modes, viability).
What changed: D3, D5, D6, D7, and D8 were amended (scoped exceptions,
report-path in summary and error, precise finding definitions, assertion
division); new decisions D9–D14 resolve the review's blocking findings —
the advertised-versus-enforced default collision with the shipped
`AgentPolicy::default() == Warn` and existing snapshots (D9), the fixture
strategy for mutually exclusive metadata states (D10), pipeline placement
that avoids generator preconditions and the unconditional bridge build
(D11), wire shapes with reason confinement and forward-tolerant `kind`
(D12), off-mode loudness against never-gating CI (D13), and an extensible
evaluator seam plus a report `vocabulary` block so canonical defaults are
actually visible in policy output (D14). Milestone 5 was split into 5a
(tests and fixtures) and 5b (documentation) for atomic commits; the
`CHANGELOG.md` entry was added to the documentation list. Why: the review
found two design flaws (default collision; vocabulary promised but not
delivered) and several contract details that were cheap to fix before
implementation and costly after. Effect on remaining work: milestone
count grows by one; scope tolerance raised from 15 files/1,200 lines to
18 files/1,400 lines to cover the two fixture packages. The plan remains
DRAFT pending user approval.
