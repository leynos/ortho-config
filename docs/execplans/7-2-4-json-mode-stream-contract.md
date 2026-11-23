# Add a JSON mode stream contract (7.2.4)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review and user approval)

Branch: `7-2-4-json-mode-stream-contract`

Roadmap item: `7.2.4` in `docs/roadmap.md`.

Implementation must not begin until the user explicitly approves this plan.

## Purpose / big picture

OrthoConfig lets a command-line application describe itself to coding agents. A
derive macro reads attributes on a command's arguments struct, emits a
documentation intermediate representation (IR, the versioned JSON model behind
man pages and help), and `cargo-orthohelp` turns that IR into a compact "agent
context" JSON document that tells an agent how to invoke each command.

Roadmap item 7.2.3 taught that metadata to say whether a command supports
`--json`, what its stdout means, whether stderr carries diagnostics, and which
exit classes it documents. It deliberately stopped short of saying how JSON
mode uses the two output streams. An agent that runs `app report --json`
therefore still cannot tell whether stdout will hold one JSON document, a
stream of JSON lines, or a pointer to a file; whether a failure arrives as one
JSON diagnostic on stderr or as several; or whether a child process such as a
compiler or `git` may spray its own text into stdout and corrupt the parse.
Cargo's own documentation warns its JSON consumers to expect non-JSON lines on
stdout from other processes; this item exists so an OrthoConfig application can
promise and be linted against the opposite.

After this change an author can write:

```rust,no_run
#[derive(OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    output(
        supports_json = true,
        stdout = "data",
        stderr_diagnostics = true,
        json_mode(
            success_stdout = "one_json_document",
            failure_stderr = "one_json_diagnostic",
            subprocess_output_policy = "capture_preview_and_log"
        )
    )
)]
struct ReportArgs { /* ... */ }
```

and the generated agent context for that command will contain:

```json
{
  "output": {
    "supports_json": true,
    "stdout": "data",
    "stderr_diagnostics": true,
    "json_mode": {
      "success_stdout": "one_json_document",
      "failure_stderr": "one_json_diagnostic",
      "subprocess_output_policy": "capture_preview_and_log"
    }
  }
}
```

A new reusable policy API,
`cargo_orthohelp::policy::rules::json_mode::check_json_mode`, reports every
command that claims JSON support but cannot state where every byte of stdout
and stderr goes, and every command whose stream contract contradicts its other
declarations (for example, a machine renderer that lets child output pass
straight through while JSON mode promises to capture it).

Existing behaviour is preserved: undeclared commands acquire no stream
contract, no claim is inferred from names, verbs, flags, renderers, or locales,
the documentation IR stays at version `2.0`, the agent-context schema stays at
version `1`, and the policy-report schema stays at version `1`.

This work closes the outstanding decision recorded in ADR-0023 ("Stream framing
and the `JsonModeContract`-style shape remain with roadmap 7.2.4") and the §6.2
"JSON mode must be stricter than mostly JSON" contract in
`docs/agent-native-cli-design.md`.

## Constraints

- Implementation begins only after the user approves this plan. Until then
  only this ExecPlan may change.
- Preserve the no-inference rule (`docs/agent-native-cli-design.md` §8.1 and
  ADR-0023). Never derive a stream contract, JSON support, or a subprocess
  policy from a command name, verb, flag, output format token, renderer
  default, locale, or parent command. A child never inherits its parent's
  `json_mode` record.
- Preserve schema ownership (ADR-0003): `ortho_config::docs` owns the strict
  IR; `ortho_config::agent_context` owns the tolerant wire;
  `cargo-orthohelp/src/schema` owns the IR mirror; the bridge mapping in
  `cargo-orthohelp/src/agent_context/` is the only code that crosses from
  mirror to wire. No schema module imports another.
- Keep `ORTHO_DOCS_IR_VERSION` at `"2.0"` (`ortho_config/src/docs/mod.rs`
  and `cargo-orthohelp/src/schema/mod.rs`),
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` at `"1"`, and
  `ORTHO_POLICY_REPORT_SCHEMA_VERSION` at `"1"`. The addition is optional and
  additive.
- Do not change the meaning, serialization, or defaults of any existing field,
  including `output.supports_json`, `output.stdout`,
  `output.stderr_diagnostics`, `output.exit_classes`, every `renderer` member,
  and `output_modes`.
- Do not change the `agent-native.output.json-missing` rule, its predicate,
  its messages, or `check_output`'s signature and report.
- Do not wire the new rules into `cargo orthohelp --check-agent-native`. That
  path is bridge-free and has no command metadata (ADR-0023 and the 7.2.3
  plan's decision on bridge builds). Composition into the future
  `cargo orthohelp check` belongs to roadmap 7.2.8.
- Do not implement `cargo-orthohelp --json` (roadmap 8.1.1), numeric exit-code
  mappings or diagnostic class/code consistency lints (roadmap 7.2.5), or a
  runtime checker that inspects real process output. This item models and lints
  declarations; a declaration is not a certificate.
- Protocol tokens (`one_json_document`, `jsonl_stream`, `artifact_path`,
  `one_json_diagnostic`, `jsonl_diagnostics`, `capture_preview_and_log`,
  `inherit_only_in_human_mode`, `forbidden`) are ASCII, snake case, and never
  localized. `artifact_path` keeps the design's American spelling because it is
  a wire token; prose uses "artefact".
- Public API change authorized: `OutputMetadata` (IR) and `OutputContract`
  (wire) each gain `json_mode: Option<…>`; new public types listed under
  `Interfaces and dependencies`; new public function `check_json_mode`. These
  are pre-1.0 APIs, so every exhaustive struct literal in the workspace,
  fixtures, examples, and doctests is updated in the same commit. No alias,
  facade, or deprecated entry point is added.
- Authorized dependencies: none at runtime. `googletest` and
  `pretty_assertions` (workspace versions `0.14.3` and `1.4.1`, caret
  requirements, `workspace = true`) may be added as dev-dependencies to
  `ortho_config_macros` if its new tests use them. No other dependency.
- Repository limits (`AGENTS.md`): no code file above 400 lines; every module
  starts with a `//!` comment; every `assert!`/`assert_eq!`/`assert_ne!`
  carries a diagnostic message; no `.expect()` outside tests; lint suppressions
  only as item-scoped `#[expect(…, reason = "…")]` as a last resort;
  en-GB-oxendict spelling in prose and comments.
- Tests inject the environment; no new process-wide environment mutation, no
  `serial_test`, no isolated `CARGO_HOME`.
- Completed ExecPlans (including
  `docs/execplans/7-2-3-structured-output-and-exit-metadata.md`) are historical
  and must not be edited.
- Run formatting, linting, and tests sequentially; never bypass compile
  admission (`RUSTC_WRAPPER`); never build in `/tmp`.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` (currently
`6ec295f4730003a74b131a40526ea47548d3f1db`). Count net lines (additions minus
deletions) with:

```sh
git diff --numstat "$(git merge-base origin/main HEAD)" -- . \
  ':(exclude)docs/execplans/7-2-4-json-mode-stream-contract.md'
```

Classify each file by purpose, whole-file:

- Code: `*.rs` (production and tests, including trybuild fixtures),
  `*.stderr`, `*.snap`, `*.feature`, and `proptest-regressions/*`.
- Configuration: `Cargo.toml`, `Cargo.lock`, `typos.local.toml`, generated
  `typos.toml`, and anything under `.github/` or `.config/`.
- Documentation: every other `*.md` file. This ExecPlan is excluded from every
  count.

Budget survey. The closest precedent is 7.2.3, which added the same kind of
record across the same layers and squash-merged as `ef46b48d`. Measured with
the classification above, it changed 97 files: about 1,006 net lines of
production Rust, 3,236 of test Rust, 205 of snapshots, 28 of fixtures, 15 of
configuration, and 763 of documentation (excluding its plan). Its plan set no
numeric budget, so no revision history exists. 7.2.1 is the only plan in this
family with a measured overrun: it set 30 files and about 1,500 net Rust lines,
then delivered 89 files and 2,838 net Rust lines, with tests at 73 percent of
the Rust. That overrun was missed scope (tests and trybuild fixtures were not
estimated), not approved growth. 7.2.2 recorded no size. This plan therefore
budgets tests explicitly and sets limits near the 7.2.3 measurement, with
contingency for the extra policy rule and the exhaustive table test.

The 7.2.4 estimate is: about 700 net production lines (three enums and one
record in each of three schema owners, derive parsing and emission, bridge
mapping, and one rule module) and about 2,800 net test lines (parser tables,
wire and IR decoding, bridge mapping, property tests, the exhaustive rule
table, BDD steps, the end-to-end test, snapshots, and one trybuild fixture).

- Files: if more than 90 non-ExecPlan files change, stop and escalate.
- Code: if net code lines exceed 4,600 in total, or net production Rust
  (files outside `tests/`, `*_tests.rs`, `*_proptests*.rs`, `tests_*.rs`,
  `tests/` modules, and `#[cfg(test)]`-only files) exceeds 1,200, stop and
  escalate.
- Configuration: if net configuration lines exceed 60, stop and escalate.
- Documentation: if net documentation lines exceed 1,000, stop and escalate.
- Interface: if a public API change beyond those authorized in `Constraints`
  is needed (for example, changing `check_output`, adding a field to
  `AgentCommand` or `DocMetadata` directly, or bumping a schema version), stop
  and escalate.
- Dependencies: if any dependency beyond those authorized in `Constraints` is
  needed, stop and escalate.
- Architecture: if the work requires a bridge build in policy-only execution,
  wiring rules into `--check-agent-native`, changing renderer semantics, or a
  runtime stream checker, stop and escalate. If roadmap 7.2.8 has landed an
  orchestrator that composes `check_output` before this work is implemented,
  stop and agree with the user whether this item also composes
  `check_json_mode` there.
- Iterations: if the same gate still fails after three diagnosed repair
  attempts, stop and escalate with the captured log.
- Time: no wall-clock limit applies (`AGENTS.md`); record timestamps in
  `Progress` so slow milestones are visible.
- Ambiguity: if review or implementation exposes more than one defensible
  meaning for a token (see `D3`–`D6`), stop and present the options.

Each category is enforced independently; spare capacity in one cannot offset
another.

## Risks

- Risk: the new subprocess policy duplicates 7.2.2's per-renderer
  `subprocess_inheritance` (`allowed | forbidden`), creating two sources of
  truth. Severity: high. Likelihood: high. Mitigation: `D6` and `D9` give the
  two fields distinct jobs (the renderer says whether child output passes
  through in each renderer; `json_mode` says where it goes in JSON mode) and
  the inconsistency rule reports explicit disagreements. Neither field is
  defaulted from the other.
- Risk: `artifact_path` overlaps 7.2.3's `stdout = "path"`. Severity: medium.
  Likelihood: medium. Mitigation: `D3` defines `artifact_path` as stream
  framing (the result document lives in a file; stdout carries one JSON
  manifest) and leaves `stdout` as semantic purpose. The ADR states the
  combinations explicitly.
- Risk: the machine renderer is not necessarily the JSON renderer, so the
  renderer reconciliation in `D9` could flag a command whose machine renderer
  is, say, tab-separated text. Severity: medium. Likelihood: low. Mitigation:
  the design review must confirm or reject this reading of ADR-0016; until then
  the plan treats a declared JSON mode as a machine rendering. If rejected,
  drop conditions C4 and C5 and record the change.
- Risk: struct-literal sweep misses a target that compiles only in some
  configurations (7.2.2 missed fixture literals; 7.2.3 missed a doctest).
  Severity: medium. Likelihood: medium. Mitigation: run
  `cargo test --workspace --doc` and `cargo test --all-targets --all-features`
  in EP-M1, and search with
  `grep -rn "OutputMetadata {\|OutputContract {" --include=*.rs --include=*.md`.
- Risk: near-cap files. `cargo-orthohelp/src/policy/rules/output_tests.rs`
  (385 lines), `ortho_config/tests/docs_ir_output.rs` (376),
  `cargo-orthohelp/src/schema/tests/mod.rs` (352), and
  `ortho_config/src/agent_context/mod.rs` (345). Severity: low. Likelihood:
  high. Mitigation: put every new type and test in new sibling modules named in
  `Plan of work`; add at most a `mod` line and a `pub use` line to near-cap
  roots, and check `wc -l` before each commit.
- Risk: fixture churn. Adding stream contracts to the nested fixture changes
  golden agent-context snapshots in `cargo-orthohelp/tests/golden/`, which
  other crates' tests also read. Severity: low. Likelihood: high. Mitigation:
  review every snapshot diff line by line; accept only diffs that add the
  declared `json_mode` blocks.
- Risk: the rolling Whitaker suite or compile-admission saturation moves a
  gate verdict without a source change. Severity: medium. Likelihood: medium.
  Mitigation: record the suite revision and `build-limits-status` beside any
  surprising verdict; treat nested-Cargo deadline expiries as load until proven
  otherwise.
- Risk: an exhaustive enumeration test is slow. Severity: low. Likelihood:
  low. The domain is 261,792 pure evaluations of small structs; measure it in
  EP-M2 and split by mode only if it exceeds a few seconds in a debug build.

## Progress

- [x] (2026-11-23 10:20Z) Branch renamed to
  `7-2-4-json-mode-stream-contract`; Lody session titled.
- [x] (2026-11-23 10:50Z) Reconnaissance by a Wyvern team: 7.2.3 output
  block, policy rules and 7.2.8 status, renderer and prover infrastructure, and
  budget history.
- [x] (2026-11-23 11:00Z) Prior-art research: JSON Lines, RFC 7464 JSON text
  sequences, the Command Line Interface Guidelines, Cargo
  `--message-format json`, and rustc `--error-format=json`.
- [x] (2026-11-23 11:40Z) Initial draft written.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` JSON-mode stream contract carried from derive to agent context.
- [ ] `EP-M2` byte-accounting policy rules delivered as a library API.
- [ ] `EP-M3` documentation reconciled and roadmap 7.2.4 marked done.

## Surprises & discoveries

- Observation: `NestedGreetCommand` in
  `tests/fixtures/orthohelp_fixture/src/lib.rs` already declares
  `supports_json = true` and a machine renderer with
  `subprocess_inheritance = "allowed"`. Evidence: lines 164–211 at `6ec295f4`.
  Impact: once it gains a stream contract it is a natural, realistic witness
  for the renderer inconsistency condition C4; its fixture comment explains
  that its human and machine inheritance values deliberately differ, so they
  must not be "fixed" to silence the finding.
- Observation: `cargo-orthohelp` already keeps every child's stdout off its
  own stdout: Cargo's stdout is piped into the protocol reader
  (`cargo-orthohelp/src/bridge_build_process.rs`, `spawn_cargo`), Cargo's
  stderr is inherited to stderr, and the bridge executable's streams are
  captured with `Command::output()` (`bridge_execution.rs`, `run_bridge`).
  Evidence: Wyvern policy reconnaissance. Impact: roadmap 8.1.1 can declare
  `capture_preview_and_log` or a stricter policy truthfully; nothing in this
  item changes that code.
- Observation: no Verus or Kani proof exists in the repository; the
  `make verus` and `make kani` targets exit 2 with `FORMAL-EMPTY` when no proof
  files are named. Evidence: `Makefile` lines 266–291 and
  `docs/developers-guide.md` "Production proof inventory". Impact: selecting
  Verus here would create the first production proof; see the formal
  verification decision.

## Decision log

- D1. Decision: extend 7.2.3's `output` block with one nested optional record,
  `json_mode`, rather than flat keys on `output` or new keys on `renderer`.
  Rationale: ADR-0023 reserved room for exactly this inside `output` and gave
  it a single presence rule. A nested record keeps "no JSON-mode contract
  declared" distinct from "contract partly declared", which flat keys cannot
  express, and it keeps framing separate from the renderer's presentation
  vocabulary (ADR-0016). Rejected: renderer keys (would make the renderer's
  `stdout` mean two things); flat `output` keys (no presence rule for the
  group). Date/Author: 2026-11-23, planning agent.
- D2. Decision: use the design's member names and tokens verbatim:
  `success_stdout` ∈ {`one_json_document`, `jsonl_stream`, `artifact_path`},
  `failure_stderr` ∈ {`one_json_diagnostic`, `jsonl_diagnostics`},
  `subprocess_output_policy` ∈ {`capture_preview_and_log`,
  `inherit_only_in_human_mode`, `forbidden`}. Rationale: the tokens are already
  published in `docs/agent-native-cli-design.md` §6.2 and allow-listed for
  spelling; renaming them would create drift with no benefit. Date/Author:
  2026-11-23, planning agent.
- D3. Decision: `artifact_path` means the command writes its JSON result
  document to a file artefact and writes exactly one JSON manifest document to
  stdout naming each delivered artefact path. On failure stdout is empty unless
  at least one artefact was delivered before the failure, in which case stdout
  holds exactly one manifest naming only the delivered artefacts. Rationale:
  this keeps the invariant "in JSON mode stdout contains only complete JSON
  documents" uniform across all three tokens, and it makes the design's
  exception ("failure writes no stdout unless an explicit artefact has already
  been delivered") precise. Rejected: a bare path line on stdout (would make
  stdout non-JSON in JSON mode). The manifest's field names are
  application-owned; OrthoConfig does not fix them. Date/Author: 2026-11-23,
  planning agent; open to design review.
- D4. Decision: `jsonl_stream` follows JSON Lines (UTF-8, one complete JSON
  value per line, each terminated by LF, no blank lines). On success stdout
  holds zero or more records; on failure stdout may hold the records already
  streamed, but never a partial (torn) record. Rationale: a streaming command
  cannot retract delivered records, and JSON Lines is the convention Cargo and
  rustc already use. RFC 7464 record-separator framing was considered and
  rejected because no tool in the estate emits it and line tools cannot split
  it. Date/Author: 2026-11-23, planning agent.
- D5. Decision: under `one_json_diagnostic`, success writes nothing to stderr
  and failure writes exactly one JSON diagnostic document followed by LF. Under
  `jsonl_diagnostics`, stderr is a JSON Lines stream on both outcomes, success
  may carry zero or more records (for example warnings), and failure carries at
  least one. Rationale: this matches the design's default contract and rustc's
  JSON-lines-on-stderr precedent. Diagnostic record fields (class and code) are
  7.2.5's concern. Date/Author: 2026-11-23, planning agent.
- D6. Decision: the subprocess tokens state where child stdout and stderr go
  in JSON mode. `capture_preview_and_log`: the parent captures child output; a
  bounded preview may appear only as an escaped string inside the result or
  diagnostic document, and the full output may be written only to a log
  artefact whose path appears inside that document (the "agreed artefact path"
  of the roadmap). `inherit_only_in_human_mode`: in human mode the child
  inherits the parent's streams; in JSON mode child output is discarded.
  `forbidden`: child output never reaches the parent's streams in any mode.
  Under every token, raw child bytes never appear on the parent's stdout or
  stderr in JSON mode. Rationale: each token then names a destination for every
  child byte, which is what the design's "state where every byte of stdout and
  stderr goes" lint needs. Date/Author: 2026-11-23, planning agent; open to
  design review.
- D7. Decision: deliver the new checks as a separate public function,
  `check_json_mode(context: &AgentContext, mode: PolicyMode) -> PolicyReport`,
  in a new module `cargo-orthohelp/src/policy/rules/json_mode.rs`, rather than
  adding them to `check_output`. Rationale: `check_output` is 7.2.3's stable
  consumer interface for 7.2.8; changing its report would invalidate 7.2.3's
  exhaustive table and the 7.2.8 cross-over contract. The split matches the
  existing `check_behaviour`/`check_output` precedent. The cost is one extra
  composition call in 7.2.8, recorded in
  `docs/cargo-orthohelp-check-gen-design.md`. Date/Author: 2026-11-23, planning
  agent.
- D8. Decision: rule `agent-native.output.json-mode-incomplete`, code
  `json_mode_contract_incomplete`, applies only when
  `output.supports_json == Some(true)`. It fires when `json_mode` is absent, or
  when any of its three members is undeclared or is the wire `unknown`
  fallback. One finding per command occurrence names every missing member in
  declaration order. Rationale: only an explicit JSON claim makes the
  byte-accounting question applicable (no inference); an unrecognized token
  cannot be shown to account for every byte, so it counts as unstated.
  Date/Author: 2026-11-23, planning agent.
- D9. Decision: rule `agent-native.output.json-mode-inconsistent`, code
  `json_mode_contract_inconsistent`, fires once per command occurrence when any
  of these explicit declarations disagree, listing every condition that holds
  in this order: C1 `supports_json == Some(false)` and `json_mode` declares any
  member; C2 `stdout == Some(none)` and `success_stdout` is `one_json_document`
  or `artifact_path`; C3 `stderr_diagnostics == Some(false)` and
  `failure_stderr` is declared and known; C4
  `renderer.machine.subprocess_inheritance == Some(allowed)` and
  `subprocess_output_policy` is declared and known; C5
  `subprocess_output_policy == forbidden` and
  `renderer.human.subprocess_inheritance == Some(allowed)`; C6
  `subprocess_output_policy == inherit_only_in_human_mode` and
  `renderer.human.subprocess_inheritance == Some(forbidden)`. Undeclared values
  and `unknown` tokens never trigger a condition. Rationale: each condition is
  a pair of explicit claims that cannot both be true under `D3`–`D6` and
  ADR-0016/ADR-0023 semantics, so the rule never infers. Date/Author:
  2026-11-23, planning agent; C4 and C5 depend on the reading recorded in
  `Risks`.
- D10. Decision: the derive performs no cross-field validation of
  `json_mode`; it validates grammar only (unknown keys, unknown tokens,
  repeated leaves, wrong literal kinds). Rationale: ADR-0023 assigns claim
  consistency to policy, and a derive error cannot be downgraded to a warning
  by the project's policy mode. Date/Author: 2026-11-23, planning agent.
- D11. Decision: no schema version bumps. Rationale: the field is optional
  and additive under the §8.2 additive-field policy; 7.2.2 and 7.2.3 made the
  same choice. Date/Author: 2026-11-23, planning agent.
- D12. Decision: record the accepted surface in a new ADR (the next free
  number; `docs/adrs/adr-0026-json-mode-stream-contract.md` at the base
  revision) and add an `Amended by ADR-0026` banner plus an `## Amendments`
  section to ADR-0023 recording that its stream-framing outstanding decision is
  resolved. Rationale: the token semantics are substantive and hard to reverse
  once agents depend on them; the style guide's amendment process preserves
  ADR-0023's text. Date/Author: 2026-11-23, planning agent.
- D13. Decision: no Verus or Kani milestone. See
  `Formal verification decision`. Date/Author: 2026-11-23, planning agent.
- D14. Decision: no runtime stream conformance checker in this item.
  Rationale: the roadmap asks to model the contract; the first executable
  consumer is roadmap 8.1.1 (`cargo-orthohelp --json`), whose end-to-end tests
  will verify real streams. A reusable checker needs a concrete second consumer
  before it earns a public API (`AGENTS.md` abstraction policy). Date/Author:
  2026-11-23, planning agent; open to design review.

## Outcomes & retrospective

Not started. Record outcomes at each milestone boundary and reconcile every
discovery with the artefacts in `Conformance basis` before marking the plan
`COMPLETE`.

## Context and orientation

Inspected base revision: `6ec295f4730003a74b131a40526ea47548d3f1db` on `main`.
Line numbers below refer to that revision; relocate symbols with
`grep -rn "<symbol>" --include=*.rs`.

The workspace has four crates relevant here. `ortho_config` is the runtime
library and owns two schemas: the documentation IR under
`ortho_config/src/docs/` and the agent-context wire format under
`ortho_config/src/agent_context/`. `ortho_config_macros` is the derive macro
that parses `#[ortho_config(...)]` attributes and emits IR-building code.
`cargo-orthohelp` is a Cargo subcommand that builds a small "bridge" binary
against a target package, runs it to obtain the IR as JSON, decodes that JSON
into its own mirror of the IR (`cargo-orthohelp/src/schema/`), and maps the
mirror into agent context (`cargo-orthohelp/src/agent_context/`). Its `policy`
module evaluates agent-native lint rules. `tests/fixtures/` holds fixture
packages that the end-to-end tests compile.

The 7.2.3 output pipeline that this item extends:

- Parsing: `ortho_config_macros/src/derive/parse/doc_attrs.rs:77` routes the
  `output` key to `parse_output_meta` in
  `ortho_config_macros/src/derive/parse/output_attrs.rs` (263 lines). That file
  defines `STDOUT_VALUES` (line 27), `group_is_empty` (line 81, accepts
  `output()`), `unknown_output_key` (line 92), `parse_exit_class` (line 108),
  and `set_once` (line 249, rejects repeated leaves across groups). Leaf helpers
  `parse_bool` and `parse_token` live in
  `ortho_config_macros/src/derive/parse/renderer_values.rs`.
- Parsed types: `OutputAttrs` and `ExitClassAttr` in
  `ortho_config_macros/src/derive/parse/doc_types.rs` (253 lines, lines 40 and
  53).
- Emission: `build_output_metadata` and `block_tokens` in
  `ortho_config_macros/src/derive/generate/docs/output.rs` (155 lines), using
  `enum_option_tokens` and `bool_option_tokens` from
  `generate/docs/renderer.rs`.
- IR: `OutputMetadata`, `StdoutPurpose`, `ExitClass` in
  `ortho_config/src/docs/ir/output.rs` (80 lines), attached as
  `DocMetadata::output` (`ortho_config/src/docs/ir/mod.rs:76`). The IR enum has
  no `Unknown` variant: unknown tokens are decode errors. IR `null` reads as
  absence.
- Mirror: `cargo-orthohelp/src/schema/output.rs` (52 lines) must serialize
  exactly like the IR.
- Wire: `OutputContract`, `StdoutPurpose` (`#[non_exhaustive]`, with
  `#[serde(other)] Unknown`), `ExitClass`, `deserialize_present`, and
  `deserialize_contract` in `ortho_config/src/agent_context/output.rs` (125
  lines), attached as `AgentCommand::output`
  (`ortho_config/src/agent_context/mod.rs:179`) and re-exported at line 44. A
  present `null` is a decode error in agent context.
- Bridge: `map_output` and `map_stdout` (one explicit arm per variant) in
  `cargo-orthohelp/src/agent_context/output.rs` (48 lines), called from
  `cargo-orthohelp/src/agent_context/mod.rs:243`.
- Policy: `check_output` in `cargo-orthohelp/src/policy/rules/output.rs`
  (155 lines), rule `agent-native.output.json-missing`, code
  `json_support_missing`, severity `warn` in warn mode and `deny` otherwise,
  stable `sort_by` on command path, `location: None`, report built with
  `PolicyReport::with_details(mode, results, Vec::new(), Vocabulary::canonical())`.
  It is a library API: no production path calls it. Shared rule-test helpers
  live in `cargo-orthohelp/src/policy/rules/tests.rs` (`context_with`,
  `command`, `ctx_for_command`).

Renderer metadata (7.2.2, ADR-0016) already carries, per renderer role (`human`
and `machine`), `stdout` and `stderr` content categories
(`none | text | structured | mixed`) and `progress_inheritance` and
`subprocess_inheritance` (`allowed | forbidden`, wire `unknown` fallback). The
wire types are `RendererContract` and `RendererRecord` with `InheritancePolicy`
in `ortho_config/src/agent_context/renderer.rs` (`InheritancePolicy` at line
290). `AgentCommand::renderer` is not optional: it carries role defaults, but
`subprocess_inheritance` itself stays `None` unless declared.

`cargo-orthohelp`'s own stdout writers are `write_json_stdout`
(`cargo-orthohelp/src/output/mod.rs:104`) for the policy report and the agent
context. The `--check-agent-native` path (`main.rs:112`,
`policy/check/mod.rs:71`) runs configuration-sanity rules only and never calls
`policy::rules`. Neither `check`/`gen` (roadmap 7.2.8) nor `--json` (roadmap
8.1.1) exists yet; `docs/cargo-orthohelp-check-gen-design.md` §§4 and 6
describe how 7.2.8 will compose rule reports and states that 7.2.8 does not
implement stream framing or new completeness rules.

Tests and tooling. Workspace dev-dependencies (`Cargo.toml` lines 29–38):
`rstest 0.28.0`, `rstest-bdd 0.6.0`, `rstest-bdd-macros 0.6.0`,
`proptest 1.11.0`, `googletest 0.14.3`, `pretty_assertions 1.4.1`.
`ortho_config` and `cargo-orthohelp` also use `insta` and `trybuild`;
`ortho_config_macros` lacks `googletest`, `pretty_assertions`, and `insta`.
`make test` runs `cargo test --all-targets --all-features` (not nextest) and
then pytest; it does not run doctests, so doctests need
`cargo test --workspace --doc` explicitly. BDD scenarios for `cargo-orthohelp`
live in `cargo-orthohelp/tests/features/` and bind through
`cargo-orthohelp/tests/rstest_bdd.rs` with step modules under
`cargo-orthohelp/tests/rstest_bdd_support/` (7.2.3's
`structured_output.feature` is bound in
`rstest_bdd_support/behaviour/scenarios.rs` around line 56). Compile-fail tests
run `ortho_config/tests/compile_fail.rs` over `tests/ui/*.rs` with committed
`.stderr` files. Golden agent-context snapshots live in
`cargo-orthohelp/tests/golden/` (for example
`agent_context__nested_fixture.json.snap`). The real-binary end-to-end
precedent is `cargo-orthohelp/tests/agent_context_output_end_to_end.rs` with
helpers in `cargo-orthohelp/tests/fixtures/output_assertions.rs`. Prover targets
`make kani`, `make kani-full`, and `make verus VERUS_PROOF_FILES='…'` exist
but have no production inputs.

Relevant documentation and skills. Read `docs/agent-native-cli-design.md`
§§6.2, 6.2.1, 6.2.2, 8.1, and 8.2; ADR-0003, ADR-0016, and ADR-0023;
`docs/cargo-orthohelp-check-gen-design.md` §§4–6; `docs/design.md`;
`docs/cargo-orthohelp-design.md`; `docs/users-guide.md` sections "Declare
structured output and exit classes" (line 2397) and "Lint declared structured
output" (line 2780); `docs/developers-guide.md` "Structured output metadata
ownership" (line 847); `docs/rust-testing-with-rstest-fixtures.md`;
`docs/rstest-bdd-users-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/documentation-style-guide.md` (ADR amendments, line 545). The
localization documents (`docs/cli-localization-design.md` and
`docs/localizable-rust-libraries-with-fluent.md`) matter only to confirm that
tokens stay non-localized. `docs/clap-dispatch-and-ortho-config-integration.md`
and `docs/scripting-standards.md` need no change. Load the `rust-router` skill
first, then `rust-types-and-apis` (public types), `rust-unit-testing` (rstest,
googletest, insta), `proptest`, `rust-verification` (method selection),
`arch-decision-records` (ADR-0026), `execplans`, and `en-gb-oxendict-style` for
prose. Load `verus` only if the formal verification decision is reopened.

## Conformance basis

Upstream artefacts, all at revision `6ec295f4`:

- `docs/roadmap.md` item 7.2.4 and its three bullets.
- `docs/agent-native-cli-design.md` §6.2 (structured output and the
  `JsonModeContract`-style shape), §6.2.1 (renderer metadata), §8.1
  (defaulting), §8.2 (schema compatibility).
- ADR-0003 (schema ownership), ADR-0016 (renderer metadata), ADR-0023
  (structured-output declarations, outstanding decision on stream framing).
- `docs/cargo-orthohelp-check-gen-design.md` §§4–6 (7.2.8 composition).
- `AGENTS.md` and the user's planning request (testing, documentation, and
  verification requirements).

There is no task-specific terms-of-reference document.

Requirements derived from those artefacts:

- `REQ-success-stdout`: a command can declare that its successful JSON-mode
  stdout is exactly one JSON result document, or one of the design's two
  alternatives (`jsonl_stream`, `artifact_path`).
- `REQ-failure-stderr`: a command can declare that its failing JSON-mode
  stderr is exactly one JSON diagnostic document, or a JSON Lines diagnostic
  stream.
- `REQ-subprocess`: a command can declare a subprocess output policy under
  which child output never reaches stdout in JSON mode except through an agreed
  artefact path.
- `REQ-byte-accounting`: the lint policy rejects commands that claim JSON mode
  but cannot state where every byte of stdout and stderr goes (§6.2).
- `REQ-no-inference`: nothing is inferred or inherited (§8.1, ADR-0023).
- `REQ-ownership`: schema ownership and mirror parity are preserved
  (ADR-0003).
- `REQ-additive`: wire versions stay unchanged and older documents decode
  without acquiring claims (§8.2).
- `REQ-nonlocalized`: protocol tokens are never localized (§6.2).

Gaps observed at the base revision:

- `GAP-fields`: `OutputMetadata` and `OutputContract` have no stream
  contract members.
- `GAP-rule`: no rule checks byte accounting or stream-contract consistency.
- `GAP-renderer`: no rule reconciles renderer `subprocess_inheritance` with a
  JSON-mode subprocess policy.
- `GAP-docs`: §6.2, §8.1, §8.2, ADR-0023, the users' guide, and the
  developers' guide describe stream contracts as future work; §8.1 still lists
  the delivered renderer rows as "planned".

Trace links:

```plaintext
roadmap 7.2.4 bullet 1 -> REQ-success-stdout -> GAP-fields -> EP-M1 -> agent_context_json_mode_end_to_end::declared_stream_contract_survives_to_both_artefacts
roadmap 7.2.4 bullet 2 -> REQ-failure-stderr -> GAP-fields -> EP-M1 -> tests_json_mode_wire::known_tokens_decode
roadmap 7.2.4 bullet 3 -> REQ-subprocess -> GAP-fields, GAP-renderer -> EP-M1, EP-M2 -> json_mode_table_tests::rules_match_the_specification_exhaustively
design §6.2 lint sentence -> REQ-byte-accounting -> GAP-rule -> EP-M2 -> json_mode_stream_contract.feature
ADR-0023 no-inference -> REQ-no-inference -> EP-M1, EP-M2 -> json_mode_tests::undeclared_command_is_not_inferred
ADR-0003 -> REQ-ownership -> EP-M1 -> schema json_mode mirror parity snapshots
design §8.2 -> REQ-additive -> EP-M1 -> legacy context without json_mode decodes unchanged
design §6.2 -> REQ-nonlocalized -> EP-M1 -> end-to-end test in two locales
ADR-0023 outstanding decision -> GAP-docs -> EP-M3 -> ADR-0026 and amended ADR-0023
```

## Verification plan

### Abstract model

A command's JSON-mode stream contract is a partial record `J = (σ, φ, π)` where
each member is either undeclared (written `⊥`), a known token, or (in agent
context only) `unknown`:

- `σ`, the success stdout framing, is one of `one_json_document`,
  `jsonl_stream`, or `artifact_path`;
- `φ ∈ {one_json_diagnostic, jsonl_diagnostics}` (failure stderr);
- `π ∈ {capture_preview_and_log, inherit_only_in_human_mode, forbidden}`
  (subprocess output policy).

`J` itself may be absent. The surrounding 7.2.3 declarations are
`S ∈ {⊥, false, true}` (`supports_json`),
`P ∈ {⊥, data, path, summary, none, unknown}` (`stdout`), and
`D ∈ {⊥, false, true}` (`stderr_diagnostics`). The 7.2.2 renderer supplies
`H, M ∈ {⊥, allowed, forbidden, unknown}`, the human and machine
`subprocess_inheritance` values.

Semantics (normative for the ADR, not executed by this item). Let `enc(v)` be
the compact JSON encoding of a value `v`, `LF` a line feed, and `ε` the empty
byte string. In JSON mode, for outcome `o ∈ {success, failure}`:

- `σ = one_json_document`: success stdout is `enc(v)·LF` for exactly one `v`;
  failure stdout is `ε`.
- `σ = jsonl_stream`: success stdout is `enc(v₁)·LF·…·enc(vₙ)·LF` with
  `n ≥ 0` and no `LF` inside any `enc(vᵢ)`; failure stdout is such a sequence
  of already-delivered records, never ending in a partial record.
- `σ` is `artifact_path`: the result document is written to an artefact file;
  success stdout is `enc(m)·LF` for one manifest `m` naming at least one
  delivered artefact; failure stdout is `ε` if nothing was delivered, otherwise
  `enc(m)·LF` naming only delivered artefacts.
- `φ = one_json_diagnostic`: success stderr is `ε`; failure stderr is
  `enc(d)·LF` for exactly one diagnostic `d`.
- `φ = jsonl_diagnostics`: stderr is a JSON Lines sequence; on failure it has
  at least one record.
- For every `π`, raw child bytes never appear on the parent's stdout or
  stderr in JSON mode. Under `capture_preview_and_log` they may appear only as
  an escaped string inside a document or in a log artefact whose path a
  document names; under the other two tokens they are discarded in JSON mode.
  `inherit_only_in_human_mode` lets the child inherit streams in human mode;
  `forbidden` never connects child output to the parent's streams.

`J` is complete when it is present and all three members are known tokens.

The policy specification, written independently of the implementation (`m` is
the policy mode):

```plaintext
applicable(c)  = S(c) = true
incomplete(c)  = applicable(c) and not complete(J(c))
C1(c) = S = false and J present with any member not ⊥
C2(c) = P = none and σ ∈ {one_json_document, artifact_path}
C3(c) = D = false and φ is a known token
C4(c) = M = allowed and π is a known token
C5(c) = π = forbidden and H = allowed
C6(c) = π = inherit_only_in_human_mode and H = forbidden
inconsistent(c) = C1 or C2 or C3 or C4 or C5 or C6
results(off, cs)  = []
results(warn, cs) = for each c in stable path order:
                      [incomplete finding if incomplete(c)]
                      ++ [inconsistent finding if inconsistent(c)],
                    each with severity warn
results(deny, cs) = same, severity deny
```

Malformed authoring input (unknown key, unknown token, wrong literal kind,
repeated leaf) is a derive error with a span. Malformed IR JSON (unknown token)
is a decode error. Malformed agent-context JSON (present `null`) is a decode
error; an unknown token decodes to `unknown`.

### Invariants and lemmas

- `INV-preserve` (REQ-success-stdout, REQ-failure-stderr, REQ-subprocess,
  REQ-ownership): for every command independently, the declared `json_mode`
  members survive derive → IR → mirror → wire unchanged, an absent record stays
  absent, an empty `json_mode()` stays a present empty record, and no command
  acquires a parent's record.
- `INV-presence` (REQ-additive): an agent context or IR document without
  `json_mode` decodes exactly as before; serialization omits absent members and
  the absent record; a present `null` is rejected in agent context and read as
  absence in the IR.
- `INV-unknown` (REQ-additive): each wire enum maps an unrecognized token to
  `unknown` and leaves the rest of the document readable; each IR enum and each
  derive token list rejects an unrecognized token.
- `INV-nonlocalized` (REQ-nonlocalized): the stream contract is identical in
  agent context generated under two locales.
- `L-incomplete` (REQ-byte-accounting): `check_json_mode` emits an
  `json_mode_contract_incomplete` finding for a command occurrence if and only
  if `m ≠ off` and `incomplete(c)`.
- `L-inconsistent` (REQ-byte-accounting, REQ-subprocess): it emits a
  `json_mode_contract_inconsistent` finding if and only if `m ≠ off` and
  `inconsistent(c)`, and the message lists exactly the conditions that hold.
- `INV-report` (REQ-byte-accounting): findings follow stable command-path
  order, equal paths are not deduplicated, reports are additive over disjoint
  command sets, checking does not mutate the context, and off-mode and empty
  reports still advertise the canonical vocabulary.
- `INV-no-inference` (REQ-no-inference): changing a command's path, verb,
  flags, `output_modes`, locale, or any renderer member other than
  `subprocess_inheritance` never changes the report.
- `L-no-leak` (REQ-subprocess, semantic): for every complete `J`, outcome,
  and child stream, the model routes no raw child byte to stdout in JSON mode,
  and stdout holds only complete JSON documents. This is a property of the
  token definitions in ADR-0026, not of executable code in this item.

### Axioms (trusted, not verified here)

- `AXIOM-serde`: Serde's documented behaviour for `default`,
  `skip_serializing_if`, `deserialize_with`, `rename_all`, and
  `#[serde(other)]`. Exercised by the round-trip and decode tests.
- `AXIOM-syn`: `syn` parses nested attribute groups as documented, including
  rejection of empty groups before the callback (handled by the existing
  `group_is_empty`). Exercised by parser tests and the trybuild fixture.
- `AXIOM-bridge`: the bridge binary transports the IR faithfully as JSON.
  Exercised by the real-binary end-to-end test.
- `AXIOM-jsonlines`: JSON Lines as specified at jsonlines.org (UTF-8, one
  value per line, `LF` terminator). Used only in ADR prose.
- `AXIOM-declaration`: a downstream application's declarations are claims.
  Nothing here verifies that a real process honours them.

### Methods per obligation

- Obligation: `INV-preserve`.
  Method: `rstest` tables for the grammar plus `proptest` round trips.
  Rationale: the input space (three optional enums inside an optional record,
  nested command trees) is broad but shallow; this mirrors 7.2.3's proven
  approach. Domain: every token of every member, absent/empty/partial/full
  records, bounded command trees of depth 0–4 with up to 32 commands. Artefacts:
  `ortho_config_macros/src/derive/parse/tests/json_mode_attrs.rs`,
  `ortho_config/tests/docs_ir_json_mode.rs`,
  `cargo-orthohelp/src/agent_context/json_mode_tests.rs`, and
  `cargo-orthohelp/src/agent_context/json_mode_proptests.rs`. Evidence:
  `cargo test -p ortho_config_macros json_mode`,
  `cargo test -p ortho_config json_mode`,
  `cargo test -p cargo-orthohelp json_mode` fail before EP-M1 (missing symbols)
  and pass after. Non-vacuity: generators classify and assert that each token,
  the absent record, the empty record, and at least one partial record occur
  (`prop_assert!` on counters or `proptest`'s `prop_oneof!` weights with a
  coverage test); negative control: temporarily map `forbidden` to
  `inherit_only_in_human_mode` in the bridge, or copy a parent's record into a
  child; a named test must fail, then restore with `git checkout -p` after
  copying the probe aside, and delete any new `proptest-regressions` entries
  the probe created.
- Obligation: `INV-presence` and `INV-unknown`.
  Method: named `rstest` cases. Rationale: each rule is a finite case.
  Artefacts: `ortho_config/src/agent_context/tests_json_mode_wire.rs`,
  `ortho_config/tests/docs_ir_json_mode.rs`,
  `cargo-orthohelp/src/schema/tests/json_mode_snapshots.rs`, and the trybuild
  fixture `ortho_config/tests/ui/output_invalid_json_mode.rs` with its
  `.stderr`. Evidence: wire tests show `unknown` for a future token in each
  member, rejection of present `null` for the record and each member, and an
  unchanged decode of a 7.2.3-era context; IR tests reject an unknown token;
  the trybuild `.stderr` pins the error naming the valid tokens. Non-vacuity:
  each test asserts the decoded value, not merely `is_ok()`; the legacy test
  uses a document copied from the current golden snapshot.
- Obligation: `L-incomplete` and `L-inconsistent`.
  Method: exhaustive enumeration in an ordinary unit test. Rationale: the input
  domain that the rules read is finite: `S` (3) × `P` (6) × `D` (3) × `J`
  (absent, or 5 × 4 × 5 member states = 101) × `H` (4) × `M` (4) × mode (3) =
  261,792 combinations. Enumerating all of them against an independent oracle
  is an exhaustive check of the whole domain, which is the strongest evidence
  available for a finite function and needs no extra tool. Artefact:
  `cargo-orthohelp/src/policy/rules/json_mode_table_tests.rs`
  (`rules_match_the_specification_exhaustively`). The oracle is written from
  the specification above as six named condition functions over plain enums
  defined in the test module; it does not call production predicates. Evidence:
  the test fails before EP-M2 (missing module) and passes after; it also
  asserts the total count of evaluated combinations equals 261,792 and the
  counts of incomplete and inconsistent findings equal the oracle's counts.
  Non-vacuity: assert that each condition C1–C6 and the incomplete predicate is
  true for at least one combination and false for at least one, and that at
  least one combination triggers several inconsistency conditions at once, so
  the message-ordering contract is exercised. Seeded faults, each run then
  reverted: drop C4; treat `unknown` as complete; make `S = ⊥` applicable; emit
  in off mode. Each must fail the enumeration with a counterexample naming the
  combination.
- Obligation: named specification examples for `L-incomplete` and
  `L-inconsistent`. Method: `rstest` table with one readable row per condition
  and per missing member, plus `insta` snapshots of the off, warn, and deny
  reports for a three-command context. Artefact:
  `cargo-orthohelp/src/policy/rules/json_mode_tests.rs`. Rationale: the
  enumeration proves correctness; named rows document intent and lock message
  text. Evidence: snapshots reviewed, never blindly accepted; `googletest`
  matchers assert rule IDs, codes, severities, and `location: None`.
- Obligation: `INV-report` and `INV-no-inference`.
  Method: `proptest` over generated command vectors of length 0–64, copying
  7.2.3's `output_proptests.rs` properties (permutation invariance for distinct
  paths, stable ties, additivity, non-mutation) and varying unrelated fields.
  Artefact: `cargo-orthohelp/src/policy/rules/json_mode_proptests.rs`.
  Non-vacuity: generated vectors must contain both offending and clean
  commands; assert class counts. Negative control: replace the stable sort with
  `sort_unstable_by` or deduplicate equal paths; a property must fail.
- Obligation: `INV-nonlocalized` and the executable boundary.
  Method: real-binary end-to-end test. Artefact:
  `cargo-orthohelp/tests/agent_context_json_mode_end_to_end.rs`, reusing the
  supervised fixture helpers of `agent_context_output_end_to_end.rs`. It runs
  `cargo orthohelp --format agent-context` on `orthohelp_fixture` under two
  locales, compares the `json_mode` blocks in `agent-context.json` and stdout,
  decodes the context, and calls `check_json_mode` in deny mode. Expected: the
  witnesses named in `EP-M1` produce exactly the predicted findings.
  Non-vacuity: the fixture contains a complete witness, an incomplete witness,
  and an inconsistent witness; the test asserts each, so a bridge that dropped
  `json_mode` would turn the complete witness into an incomplete finding and
  fail.
- Obligation: `L-no-leak`.
  Method: design review of the ADR-0026 token table, plus the completeness
  rule. Rationale: the property holds by definition of the vocabulary; there is
  no executable routing code in this item to verify. Residual gap: runtime
  conformance is unverified until roadmap 8.1.1 adds real stream tests for
  `cargo-orthohelp --json`.

### Formal verification decision

Verus and Kani are not selected. Every executable obligation introduced here is
a total function over a finite domain (at most 261,792 inputs), and the plan
enumerates that domain completely in an ordinary test against an independent
oracle; Kani would re-establish the same exhaustiveness with more tooling, and
Verus is designed for properties over unbounded domains. The only unbounded
objects in this item are the byte streams in the abstract model, and no code
here parses or produces them. A Verus proof about a detached model of stream
framing would not verify shipped code, which `AGENTS.md` forbids presenting as
evidence. This matches the 7.2.2 and 7.2.3 decisions, and 7.2.2's stated
reconsideration trigger (cross-field enforcement) is met here by exhaustive
enumeration rather than by proof because the cross-field domain is finite.

Reconsideration trigger: if design review or later work adds a runtime stream
conformance checker (for example `admits(contract, outcome, stdout, stderr)`),
its framing kernel has an unbounded domain. Before implementing it, revise this
plan with a proof milestone over a pure framing function, with these candidate
lemmas (to be refined when verification conditions appear):
`L-frame-roundtrip`, splitting the concatenation of `LF`-free records each
followed by `LF` returns exactly those records; `L-no-torn`, an accepted stream
is empty or ends with `LF`; `L-one-document`, `one_json_document` acceptance
holds exactly when splitting yields one record. The acceptance command would be
`make verus VERUS_PROOF_FILES='<production proof path>'` beside the ordinary
gates, and the proof must reject a seeded fault. No `assume`, `admit`, or
trusted body may be introduced.

### Suggested BDD features

Proposed feature file:
`cargo-orthohelp/tests/features/json_mode_stream_contract.feature`, bound in
`cargo-orthohelp/tests/rstest_bdd_support/behaviour/scenarios.rs` with step
definitions in a new
`cargo-orthohelp/tests/rstest_bdd_support/json_mode_policy.rs`. It covers
`REQ-byte-accounting`, `REQ-subprocess`, and `REQ-no-inference` in `EP-M2`.

```gherkin
Feature: JSON mode stream contract
  Scenario: A JSON command states where every byte goes
    Given a command that supports JSON with a complete stream contract and subprocess policy "forbidden"
    When the JSON mode policy is checked in "deny" mode
    Then the JSON mode policy report has no findings

  Scenario: A JSON command has no stream contract
    Given a command that supports JSON with no stream contract
    When the JSON mode policy is checked in "deny" mode
    Then exactly one "json_mode_contract_incomplete" finding has severity "deny"
    And the finding names "success_stdout", "failure_stderr", and "subprocess_output_policy"

  Scenario: A JSON command omits its subprocess policy
    Given a command that supports JSON with success stdout "jsonl_stream" and failure stderr "jsonl_diagnostics"
    When the JSON mode policy is checked in "warn" mode
    Then exactly one "json_mode_contract_incomplete" finding has severity "warn"
    And the finding names only "subprocess_output_policy"

  Scenario: The machine renderer lets child output through
    Given a command that supports JSON with a complete stream contract and subprocess policy "capture_preview_and_log"
    And its machine renderer allows subprocess inheritance
    When the JSON mode policy is checked in "deny" mode
    Then exactly one "json_mode_contract_inconsistent" finding has severity "deny"

  Scenario: A stream contract contradicts a JSON refusal
    Given a command that declines JSON support but declares success stdout "one_json_document"
    When the JSON mode policy is checked in "deny" mode
    Then exactly one "json_mode_contract_inconsistent" finding has severity "deny"

  Scenario: Enforcement is disabled
    Given a command that supports JSON with no stream contract
    When the JSON mode policy is checked in "off" mode
    Then the JSON mode policy report has no findings

  Scenario: JSON mode is not inferred from a verb
    Given a command named "get" with no output declaration
    When the JSON mode policy is checked in "deny" mode
    Then the JSON mode policy report has no findings
```

Runner: `cargo test -p cargo-orthohelp --test rstest_bdd json_mode`. The
scenarios must fail before `check_json_mode` exists (red: unresolved step
module or missing function) and pass after. Remember that rstest-bdd's
`scenarios!` input is not tracked by Cargo: after editing the `.feature` file,
touch the binding module or the test binary will be stale.

## Plan of work

Work proceeds in three milestones. Each starts red, ends green, passes the full
gate sequence, and is committed as a coherent unit.

### `EP-M1` carry the JSON-mode stream contract from derive to agent context

Requirements: `REQ-success-stdout`, `REQ-failure-stderr`, `REQ-subprocess`
(declaration only), `REQ-no-inference`, `REQ-ownership`, `REQ-additive`,
`REQ-nonlocalized`; gap `GAP-fields`; decisions `D1`–`D6`, `D10`–`D12`.

Red. Add, before any production change:

1. `ortho_config_macros/src/derive/parse/tests/json_mode_attrs.rs` (registered
   beside the existing `output_attrs` test module): a full declaration, an empty
   `json_mode()`, composition of members across two `output(...)` groups,
   every token of every member (`rstest` cases), and rejection of an unknown
   member key, an unknown token for each member, a non-string literal, a
   repeated member in one group and across groups, and `json_mode` outside
   `output(...)`.
2. `ortho_config/tests/docs_ir_json_mode.rs`: IR decode and encode for absent,
   `null`, empty, partial, and full records; unknown-token rejection; derive
   emission for a declared, empty, and undeclared record.
3. `ortho_config/src/agent_context/tests_json_mode_wire.rs` (registered with
   `#[path]` in `ortho_config/src/agent_context/tests.rs` beside
   `tests_output_wire.rs`): omission, present-`null` rejection for the record
   and each member, empty record, each known token, a future token per member
   decoding to `unknown` and serializing as `"unknown"`, tolerance of a future
   member key, and an unchanged decode of a context copied from
   `cargo-orthohelp/tests/golden/agent_context__nested_fixture.json.snap`.
4. `cargo-orthohelp/src/agent_context/json_mode_tests.rs`: explicit mapping of
   every token, absent and empty records, per-command independence, and
   localizer independence.
5. `ortho_config/tests/ui/output_invalid_json_mode.rs` with an unknown
   `success_stdout` token. Generate its `.stderr` only after the parser exists
   and review it.

Run the focused commands in `Concrete steps`; they must fail on missing symbols
or modules, not on fixture or tool errors.

Green. Implement:

1. Derive parsing in a new
   `ortho_config_macros/src/derive/parse/json_mode_attrs.rs`
   (`parse_json_mode_meta`), dispatched from `parse_output_meta` in
   `output_attrs.rs` for the key `json_mode`. Reuse `group_is_empty`,
   `set_once`, and `parse_token`. Add `JsonModeAttrs` (three `Option<String>`
   members) to `doc_types.rs` and a `json_mode: Option<JsonModeAttrs>` member to
   `OutputAttrs`. Token lists are constants beside the parser, as
   `STDOUT_VALUES` is.
2. Emission in `ortho_config_macros/src/derive/generate/docs/output.rs`
   (or a sibling `json_mode.rs` if the file would pass about 250 lines),
   emitting `json_mode: Some(JsonModeMetadata { … })` or `None` with
   `enum_option_tokens`.
3. IR types in a new `ortho_config/src/docs/ir/json_mode.rs`:
   `JsonModeMetadata`, `SuccessStdout`, `FailureStderr`,
   `SubprocessOutputPolicy` (strict enums, no `Unknown`), with
   `OutputMetadata::json_mode: Option<JsonModeMetadata>` using the same serde
   attributes as `DocMetadata::output`. Re-export from `ortho_config::docs`.
4. Mirror types in a new `cargo-orthohelp/src/schema/json_mode.rs`, wired
   into `cargo-orthohelp/src/schema/output.rs`.
5. Wire types in a new `ortho_config/src/agent_context/json_mode.rs`:
   `JsonModeContract` with each member using `deserialize_present`, and the
   three enums marked `#[non_exhaustive]` with
   `#[default] #[serde(other)] Unknown`. Add `OutputContract::json_mode` with a
   `deserialize_with` function that rejects a present `null`, mirroring
   `deserialize_contract`. Re-export the four types from
   `ortho_config::agent_context` (one `mod` line and one `pub use` line in the
   345-line `mod.rs`).
6. Bridge mapping: `map_json_mode` plus one explicit-arm mapping function per
   enum, in `cargo-orthohelp/src/agent_context/output.rs` or a sibling
   `json_mode.rs`, called from `map_output`.
7. Update every exhaustive `OutputMetadata { … }` and `OutputContract { … }`
   literal in the workspace, fixtures, examples, doctests, and Markdown
   examples with `json_mode: None`.
8. Fixture witnesses in `tests/fixtures/orthohelp_fixture/src/lib.rs`:
   - `NestedGreetCommand` gains a complete stream contract inside its
     existing `output(...)` group. Its machine renderer already declares
     `subprocess_inheritance = "allowed"`, so this command is the
     inconsistent (C4) witness; extend its doc comment to say so;
   - `NestedPruneCommand` gains a JSON claim with a complete stream contract
     as the clean witness;
   - `NestedGrantAccessCommand` gains a JSON claim whose stream contract
     omits the subprocess policy, as the incomplete witness.

   ```rust,no_run
   // NestedGreetCommand: added inside the existing output(...) group.
   json_mode(
       success_stdout = "one_json_document",
       failure_stderr = "one_json_diagnostic",
       subprocess_output_policy = "capture_preview_and_log"
   )

   // NestedPruneCommand: clean, complete witness.
   output(
       supports_json = true,
       stdout = "summary",
       json_mode(
           success_stdout = "artifact_path",
           failure_stderr = "one_json_diagnostic",
           subprocess_output_policy = "forbidden"
       )
   )

   // NestedGrantAccessCommand: incomplete witness.
   output(
       supports_json = true,
       stdout = "summary",
       json_mode(success_stdout = "jsonl_stream", failure_stderr = "jsonl_diagnostics")
   )
   ```

   If a fixture comment or another test contradicts one of these choices, pick
   the nearest command without that conflict and record it in `Decision log`.
9. Add `cargo-orthohelp/tests/agent_context_json_mode_end_to_end.rs`
   (structure modelled on `agent_context_output_end_to_end.rs`) covering the
   declared blocks in file and stdout under two locales. Its policy half is
   added in EP-M2.
10. Add schema snapshots in
    `cargo-orthohelp/src/schema/tests/json_mode_snapshots.rs` (empty, full,
    each token) and review the regenerated golden agent-context snapshots.

Refactor. Keep every touched file under 400 lines, share helpers rather than
duplicating 7.2.3 test setup, and run `make lint` locally through the gate
sequence (Clippy finds `missing_const_for_fn` and similar issues that
`cargo test` does not).

Documentation in the same milestone: write ADR-0026 (status `Accepted` on
approval, date, context, drivers, options A–D as in `D1`, the token table with
`D3`–`D6` semantics, wire representation and defaults, migration, risks,
outstanding decisions), amend ADR-0023 per `D12`, add ADR-0026 to
`docs/contents.md`, and add the derive syntax to `docs/users-guide.md` "Declare
structured output and exit classes" with an explicit statement that the
contract is a declaration, not a runtime guarantee.

### `EP-M2` lint byte accounting as a library API

Requirements: `REQ-byte-accounting`, `REQ-subprocess`, `REQ-no-inference`; gaps
`GAP-rule`, `GAP-renderer`; decisions `D7`–`D9`.

Red. Add `cargo-orthohelp/src/policy/rules/json_mode_table_tests.rs`
(exhaustive enumeration), `json_mode_tests.rs` (named rows and report
snapshots), `json_mode_proptests.rs` (report properties), the BDD feature and
step module, and the policy half of the end-to-end test. Register the test
modules in `cargo-orthohelp/src/policy/rules/mod.rs` under `#[cfg(test)]`. They
must fail because `check_json_mode` does not exist.

Green. Add `cargo-orthohelp/src/policy/rules/json_mode.rs` with a `//!` module
comment, `pub mod json_mode;` in `rules/mod.rs`, and:

```rust
pub fn check_json_mode(context: &AgentContext, mode: PolicyMode) -> PolicyReport
```

following `check_output`'s structure: return
`PolicyReport::with_details(mode, Vec::new(), Vec::new(), Vocabulary::canonical())`
in off mode; otherwise collect commands, `sort_by` path (stable), and for each
command push an incomplete finding then an inconsistent finding when their
predicates hold. Keep predicates as small private functions (`is_incomplete`,
`inconsistency_conditions` returning the ordered list of condition labels) so
each stays below the cognitive-complexity threshold. Messages name the command
path, the missing members or the conflicting declarations, and the remedy
(declare the member, or correct whichever declaration is untrue). Add a Rustdoc
example that decodes a small context with `serde_json::from_value` and shows
one incomplete finding in deny mode, as `check_output`'s example does.

Refactor. Share the severity projection with `check_output` only if both
modules can use one private helper without changing `check_output`'s behaviour;
otherwise leave the duplicated three-line function and record why. Run the
seeded-fault negative controls listed in `Verification plan`, restore the code,
and delete any probe-created regression files.

Documentation in the same milestone: add "Lint the JSON mode stream contract" to
`docs/users-guide.md` after "Lint declared structured output", mirror the
Rustdoc example in `docs/cargo-orthohelp-api-guide.md` beside the
structured-output rule, and add a short paragraph to
`docs/cargo-orthohelp-check-gen-design.md` §6 recording that the 7.2.8
orchestrator composes `check_json_mode` beside `check_output` when both exist,
without changing 7.2.8's acceptance gate.

### `EP-M3` documentation reconciliation and delivery closure

Requirements: all, through `GAP-docs`.

Update `docs/agent-native-cli-design.md`: §6.2 replaces "remain roadmap 7.2.4
work" with the delivered shape and links ADR-0026; §8.1 adds rows for
`output.json_mode` and its three members (absent, `v1`, no inference) and
corrects the stale "planned" renderer rows; §8.2 records the additive
`json_mode` history and the unknown-token fallback; §9 removes JSON-mode stream
contracts from the gap list. Update `docs/design.md` (the deferral at about
line 1399) and `docs/cargo-orthohelp-design.md` (deferral notes near lines 254,
815, and 948). Update `docs/developers-guide.md` "Structured output metadata
ownership" with the new modules, the exhaustive-enumeration testing pattern,
and the statement that no production proof was added and why. Add a
`CHANGELOG.md` `## Unreleased` entry noting the new metadata, the new rule API,
and the breaking struct-literal change. Search for stale claims with:

```sh
grep -rn "7\.2\.4\|JsonModeContract\|stream framing\|stream contract" docs README.md CHANGELOG.md \
  | grep -v "docs/execplans/"
```

and fix every live document it reports (completed ExecPlans are excluded). Mark
roadmap 7.2.4 and its three bullets `[x]` in `docs/roadmap.md` only after all
evidence passes. Run all gates, update this plan's living sections, and set the
status to `COMPLETE`.

## Milestones and plateaus

- Identifier and outcome: `EP-M1`, authors can declare a JSON-mode stream
  contract and see it in IR, mirror, and agent context.
  - Requirements and gaps: `REQ-success-stdout`, `REQ-failure-stderr`,
    `REQ-subprocess` (declaration), `REQ-no-inference`, `REQ-ownership`,
    `REQ-additive`, `REQ-nonlocalized`; `GAP-fields`.
  - Acceptance evidence: focused EP-M1 tests, the real-binary end-to-end
    test's declaration half, reviewed snapshots, and the full gate sequence.
  - Conformance check:
    - declared members round-trip unchanged per command;
    - ADR-0003 ownership and mirror parity hold;
    - no version constant changed;
    - only the authorized public API changed;
    - no trust boundary or persisted-format change beyond the additive field;
    - trace links in `Conformance basis` still name real tests.
  - Recovery: revert the milestone commit; nothing else depends on it.
  - Remaining gaps: `GAP-rule`, `GAP-renderer`, most of `GAP-docs`.
  - Compatibility decision: none. Older IR and agent-context JSON must decode
    unchanged (a wire-format requirement, not a source shim).
- Identifier and outcome: `EP-M2`, `check_json_mode` reports incomplete and
  inconsistent stream contracts.
  - Requirements and gaps: `REQ-byte-accounting`, `REQ-subprocess`,
    `REQ-no-inference`; `GAP-rule`, `GAP-renderer`.
  - Acceptance evidence: exhaustive table, named rows, snapshots, property
    tests, BDD scenarios, end-to-end policy half, doctest, negative controls,
    and the full gate sequence.
  - Conformance check:
    - specification above matches the implementation exactly;
    - `check_output` unchanged (its tests and snapshots unchanged);
    - no CLI wiring and no bridge build in policy-only paths;
    - only `check_json_mode` added to the public API;
    - trace links current.
  - Recovery: revert the milestone commit; `EP-M1` remains coherent.
  - Remaining gaps: the rest of `GAP-docs`.
  - Compatibility decision: none.
- Identifier and outcome: `EP-M3`, live documentation describes the delivered
  contract and roadmap 7.2.4 is done.
  - Requirements and gaps: `GAP-docs`.
  - Acceptance evidence: stale-claim search clean, Markdown gates, full gate
    sequence, roadmap checkboxes.
  - Conformance check: every upstream artefact reconciled; ADR-0023 history
    preserved.
  - Recovery: revert the documentation commit.
  - Remaining gaps: runtime conformance (8.1.1), numeric exit codes and
    diagnostic class/code consistency (7.2.5), CLI composition (7.2.8).
  - Compatibility decision: none.

## Concrete steps

Working directory: the repository root on branch
`7-2-4-json-mode-stream-contract`. Start each session with `git status --short`
and `git log --oneline -3`.

EP-M1 focused commands (red then green):

```sh
cargo test -p ortho_config_macros json_mode
cargo test -p ortho_config json_mode
cargo test -p ortho_config --test compile_fail
cargo test -p cargo-orthohelp json_mode
cargo test -p cargo-orthohelp --test agent_context_json_mode_end_to_end
cargo test -p cargo-orthohelp --test golden_tests
cargo test --workspace --doc
```

Expected red: compile errors naming the missing `json_mode` member, module, or
type. Expected green: all named tests pass; record the counts here. A filter
that matches zero tests is not acceptance.

EP-M2 focused commands:

```sh
cargo test -p cargo-orthohelp json_mode
cargo test -p cargo-orthohelp --test rstest_bdd json_mode
cargo test -p cargo-orthohelp --test agent_context_json_mode_end_to_end
cargo test -p cargo-orthohelp --doc
```

Gate sequence after each milestone, run sequentially (preferably delegated to
`scrutineer`), each with `pipefail` and a log:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-ortho-config-7-2-4-json-mode-stream-contract.out
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-7-2-4-json-mode-stream-contract.out
make lint 2>&1 | tee /tmp/lint-ortho-config-7-2-4-json-mode-stream-contract.out
make test 2>&1 | tee /tmp/test-ortho-config-7-2-4-json-mode-stream-contract.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-7-2-4-json-mode-stream-contract.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-7-2-4-json-mode-stream-contract.out
```

Add a milestone suffix (for example `-m1`) to log names so a later run does not
overwrite failing evidence. Read a failing log before rerunning anything.
Proceed only when every command exits zero. If `make spelling` regenerates
unrelated `typos.toml` content, follow the repository's separate-base-PR policy.

For this planning change (documentation only), run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`; the Rust gates are not claimed for
unimplemented work.

## Validation and acceptance

After `EP-M1`: running `cargo orthohelp --format agent-context` against
`tests/fixtures/orthohelp_fixture` writes an `agent-context.json` whose `greet`
command contains
`"json_mode":{"success_stdout":"one_json_document","failure_stderr":"one_json_diagnostic","subprocess_output_policy":"capture_preview_and_log"}`,
whose `version` command has no `output` block, and whose stdout carries the
same document. The test
`agent_context_json_mode_end_to_end::declared_stream_contract_survives_to_both_artefacts`
fails before and passes after.

After `EP-M2`: decoding that context and calling
`check_json_mode(&context, PolicyMode::Deny)` yields exactly one
`json_mode_contract_inconsistent` finding for the greet command (condition C4),
one `json_mode_contract_incomplete` finding for the grant-access command naming
`subprocess_output_policy`, and none for prune, audit, or version;
`summary.deny` is 2. `check_output` on the same context is unchanged from its
pre-change result. The exhaustive table evaluates 261,792 combinations with no
mismatch.

Quality criteria:

- Tests: `make test` passes, plus `cargo test --workspace --doc`.
- Verification: `INV-*` and `L-*` obligations discharged as specified, with
  the negative controls observed and reverted.
- Lint and format: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- Performance: the exhaustive table completes in a few seconds in a debug
  build; record its time.
- Security: no new input surface beyond derive attributes and JSON decoding;
  no secrets in snapshots or logs.

## Idempotence and recovery

All focused tests and gates can be rerun safely. Snapshot updates are reviewed
with `cargo insta review` rather than accepted wholesale. Seeded fault probes
are applied to a copied file, observed, and reverted by copying back; never use
`git checkout --` on a file with unstaged work. Delete any
`proptest-regressions` entries created by a probe in the same step. Commit each
green milestone with `git commit -F` using an inspected message file; roll back
with a reviewed `git revert` rather than a reset. If a nested-Cargo test times
out under compile-admission load, check `build-limits-status` and rerun the
single test before treating it as a defect.

## Artefacts and notes

Reconnaissance facts recorded at planning time (base `6ec295f4`):

```plaintext
7.2.3 squash merge ef46b48d: 97 files, +6844 -225
  prod-rs  23 files net 1006
  test-rs  50 files net 3236
  snap      8 files net  205
  fixture   3 files net   28
  config    2 files net   15
  docs     10 files net  763 (excluding its ExecPlan)
```

Prior art consulted:

- JSON Lines (jsonlines.org): UTF-8, each line one JSON value, `LF`
  terminator, no blank lines.
- RFC 7464 JSON text sequences: record-separator prefix framing,
  `application/json-seq`; considered for `jsonl_stream` and rejected (`D4`).
- Command Line Interface Guidelines (clig.dev): primary and machine-readable
  output on stdout; messages, logs, and errors on stderr; JSON when `--json` is
  passed.
- Cargo external tools, `--message-format json`: JSON objects one per line on
  stdout, with a caveat that other processes may print non-JSON lines to
  stdout; this item's subprocess policy exists to rule that out.
- rustc `--error-format=json`: diagnostics emitted one JSON object per line
  on stderr, the precedent for `jsonl_diagnostics`.

## Interfaces and dependencies

In `ortho_config/src/docs/ir/json_mode.rs` (re-exported from
`ortho_config::docs`):

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonModeMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_stdout: Option<SuccessStdout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_stderr: Option<FailureStderr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subprocess_output_policy: Option<SubprocessOutputPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuccessStdout { OneJsonDocument, JsonlStream, ArtifactPath }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureStderr { OneJsonDiagnostic, JsonlDiagnostics }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubprocessOutputPolicy { CapturePreviewAndLog, InheritOnlyInHumanMode, Forbidden }
```

and `OutputMetadata` gains
`#[serde(default, skip_serializing_if = "Option::is_none")] pub json_mode: Option<JsonModeMetadata>`.

In `cargo-orthohelp/src/schema/json_mode.rs`: identical mirror types that
serialize exactly as the IR does; `cargo-orthohelp/src/schema/output.rs`'s
`OutputMetadata` gains the same member.

In `ortho_config/src/agent_context/json_mode.rs` (re-exported from
`ortho_config::agent_context`): `JsonModeContract` with the same three members,
each
`#[serde(default, deserialize_with = "deserialize_present",
skip_serializing_if = "Option::is_none")]`,
and the three enums with the same variants plus
`#[default] #[serde(other)] Unknown`, each enum `#[non_exhaustive]`.
`OutputContract` gains `json_mode: Option<JsonModeContract>` with a
present-`null`-rejecting deserializer. The new structs stay exhaustive,
matching ADR-0023's treatment of `OutputContract` until roadmap 13.1 adds
constructors.

In `cargo-orthohelp/src/policy/rules/json_mode.rs`:

```rust
/// Stable rule identifiers and codes.
const INCOMPLETE_RULE_ID: &str = "agent-native.output.json-mode-incomplete";
const INCOMPLETE_CODE: &str = "json_mode_contract_incomplete";
const INCONSISTENT_RULE_ID: &str = "agent-native.output.json-mode-inconsistent";
const INCONSISTENT_CODE: &str = "json_mode_contract_inconsistent";

#[must_use]
pub fn check_json_mode(context: &AgentContext, mode: PolicyMode) -> PolicyReport;
```

Reuse policy: the IR, mirror, and wire types are owned by their schema modules
and constructed only by the derive, decoding, and the bridge respectively.
`check_json_mode` is the only owner of the two new predicates; 7.2.8 and any
CLI must call it rather than reimplementing them. No new helper abstraction is
introduced beyond private functions inside these modules; existing helpers
(`group_is_empty`, `set_once`, `parse_token`, `enum_option_tokens`,
`deserialize_present`, `PolicyReport::with_details`, `Vocabulary::canonical`)
are reused.

Dependencies: no runtime dependency. Dev-dependencies `googletest` and
`pretty_assertions` may be added to `ortho_config_macros` with
`workspace = true`; nothing else.

## Revision note

- 2026-11-23: initial draft from Wyvern reconnaissance and prior-art
  research, before community-of-experts review.
