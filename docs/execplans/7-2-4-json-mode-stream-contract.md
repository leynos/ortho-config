# Add a JSON mode stream contract (7.2.4)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (community-of-experts design review applied; awaiting user
approval of the plan and of decisions A1–A5)

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
stream of JSON lines, or a reference to a file; whether a failure arrives as
one JSON diagnostic on stderr or as several; or whether a child process such as
a compiler may write its own text into stdout and corrupt the parse. Cargo's
own documentation warns consumers of its JSON messages to expect non-JSON lines
on stdout from other processes. This item lets an OrthoConfig application
promise the opposite and be linted against that promise.

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

The three members together say where every byte of stdout and stderr goes in
JSON mode: the stdout framing on success, the stderr framing on failure, and
where any child process's output goes instead of stdout.

A new reusable policy API,
`cargo_orthohelp::policy::rules::json_mode::check_json_mode`, reports every
command that claims (or implies) JSON support but cannot state where every byte
goes, and every command whose stream contract contradicts another explicit
declaration (for example, declining JSON support while declaring a JSON stream
contract).

Existing behaviour is preserved: undeclared commands acquire no stream
contract, no claim is inferred from names, verbs, flags, renderers, or locales,
the documentation IR stays at version `2.0`, the agent-context schema stays at
version `1`, and the policy-report schema stays at version `1`.

This work closes the outstanding decision recorded in ADR-0023 ("Stream framing
and the `JsonModeContract`-style shape remain with roadmap 7.2.4") and realizes
the §6.2 "JSON mode must be stricter than mostly JSON" contract in
`docs/agent-native-cli-design.md`.

## Decisions requiring approval

The design review surfaced five choices that change what is built. Each has a
recommendation; approving the plan approves the recommendation unless the user
says otherwise.

- A1. Strict JSON-mode stderr. The design's vocabulary offers only
  `one_json_diagnostic` and `jsonl_diagnostics`, so in JSON mode stderr may
  carry only JSON diagnostics. `cargo-orthohelp` today passes Cargo's stderr
  straight through (`cargo-orthohelp/src/bridge_build_process.rs:62`,
  `Stdio::inherit()`) and prints a human summary on stderr, so it could not
  truthfully declare either token. Recommendation: keep the design's strict
  vocabulary, and add a roadmap 8.1.1 sub-bullet requiring `cargo-orthohelp` to
  capture Cargo and bridge child output in JSON mode and verify its real
  streams against its declared contract. Alternative: add a third stderr token
  such as `text_diagnostics` (amends design §6.2; more tokens, weaker promise).
- A2. An agreed artefact reference. The roadmap requires child output to
  reach a consumer only "via an agreed artefact path", and `artifact_path`
  needs a path an agent can find without application-specific knowledge.
  Recommendation: ADR-0026 fixes one minimal reference shape — a top-level
  `files` member holding an array of `{"path": …, "role": …}` objects, where
  `path` is a UTF-8 string (absolute, or relative to the invocation's working
  directory) and `role` is `result`, `log`, or another application-defined
  snake-case token. Metadata does not validate payloads; this is the agreed
  convention a declaration promises. Alternative: leave reference fields
  application-owned and drop the claim that the path is agreed.
- A3. Where a captured preview may appear. Recommendation: under
  `capture_preview_and_log`, a bounded preview of child output may appear only
  inside the JSON diagnostic document on stderr; the full output is reachable
  only through a `log` reference. This follows the roadmap's "never leaks to
  stdout … except via an agreed artefact path" literally. Alternative: also
  allow an escaped preview inside the stdout result document.
- A4. Member names. The design publishes `success_stdout` and
  `failure_stderr`, yet `success_stdout` also governs stdout on failure and
  `failure_stderr` also governs stderr on success. Recommendation: keep the
  published wire names (the roadmap bullets use the same words), publish an
  outcome-by-member table in ADR-0026, and name the Rust types after the concept
  (`StdoutFraming`, `StderrFraming`). Alternative: rename the wire members to
  `stdout_framing` and `stderr_framing` before anything ships and amend design
  §6.2.
- A5. Cross-layer consistency deferred. The draft compared the stream
  contract with the machine renderer's `subprocess_inheritance` and with
  `stdout = "none"`. Review showed both comparisons depend on unstated bindings
  (ADR-0016 says machine output "need not be JSON"; ADR-0023's `stdout` purpose
  is mode-independent), so they would infer. Recommendation: drop both
  conditions now and record, as an outstanding decision in ADR-0026, that a
  later item may bind `--json` to a renderer role and to `stdout` purpose
  before such checks are added.

## Constraints

- Implementation begins only after the user approves this plan. Until then
  only this ExecPlan may change.
- Preserve the no-inference rule (`docs/agent-native-cli-design.md` §8.1 and
  ADR-0023). Never derive a stream contract, JSON support, or a subprocess
  policy from a command name, verb, flag, output format token, renderer
  default, locale, or parent command. A child never inherits its parent's
  `json_mode` record. Consistency conditions compare only explicit declarations
  whose meanings already bind them to the same mode.
- Preserve schema ownership (ADR-0003): `ortho_config::docs` owns the strict
  IR; `ortho_config::agent_context` owns the tolerant wire;
  `cargo-orthohelp/src/schema` owns the IR mirror; the bridge mapping in
  `cargo-orthohelp/src/agent_context/` is the only code that crosses from
  mirror to wire. No schema module imports another.
- Keep `ORTHO_DOCS_IR_VERSION` at `"2.0"` (`ortho_config/src/docs/mod.rs:21`
  and `cargo-orthohelp/src/schema/mod.rs:36`),
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` at `"1"`, and
  `ORTHO_POLICY_REPORT_SCHEMA_VERSION` at `"1"`. The addition is optional and
  additive.
- Do not change the meaning, serialization, or defaults of any existing field,
  including `output.supports_json`, `output.stdout`,
  `output.stderr_diagnostics`, `output.exit_classes`, every `renderer` member,
  and `output_modes`.
- Do not change the `agent-native.output.json-missing` rule, its predicate,
  its messages, `check_output`'s signature, its rule tests, or its report
  snapshots.
- Do not wire the new rules into `cargo orthohelp --check-agent-native`. That
  path is bridge-free and has no command metadata. Composition into the future
  `cargo orthohelp check` belongs to roadmap 7.2.8.
- Do not implement `cargo-orthohelp --json` (roadmap 8.1.1), numeric exit-code
  mappings or diagnostic class/code consistency lints (roadmap 7.2.5), or a
  runtime checker that inspects real process output. This item models and lints
  declarations; a declaration is not a certificate.
- A token's meaning, once published in ADR-0026, never changes. A different
  meaning requires a new token.
- Protocol tokens are ASCII, snake case, and never localized. The wire token
  `artifact_path` keeps the design's American spelling; prose uses "artefact".
  In spellcheck-scanned Markdown write the token only as its own inline code
  span or inside a fenced block (`typos.local.toml` allow-lists exactly the
  stand-alone span).
- Public API change authorized: `OutputMetadata` (IR, both owners) and
  `OutputContract` (wire) each gain `json_mode: Option<…>`; the new public
  types listed under `Interfaces and dependencies`; the new public function
  `check_json_mode`. These are pre-1.0 APIs, so every exhaustive struct literal
  in the workspace, fixtures, examples, doctests, and live Markdown examples is
  updated in the same commit. No alias, facade, or deprecated entry point is
  added.
- Authorized dependencies: none. Existing dev-dependencies (`rstest`,
  `rstest-bdd`, `proptest`, `insta`, `trybuild`, and, in `ortho_config` and
  `cargo-orthohelp`, `googletest` and `pretty_assertions`) are sufficient.
  `ortho_config_macros` tests use plain assertions with messages.
- Repository limits (`AGENTS.md`, `clippy.toml`): no code file above 400
  lines; every module starts with a `//!` comment; every `assert!` family macro
  carries a diagnostic message; no `.expect()` outside tests; lint suppressions
  only as item-scoped `#[expect(…, reason = "…")]` as a last resort;
  `excessive-nesting-threshold = 4`, `too-many-arguments-threshold = 4`,
  `too-many-lines-threshold = 70`, and denied `integer_division` and
  `indexing_slicing`; en-GB-oxendict spelling in prose and comments.
- Tests inject the environment; no new process-wide environment mutation, no
  `serial_test`, no isolated `CARGO_HOME`, no new test binary that spawns
  nested Cargo (see `D16`).
- Historical documents stay untouched: completed ExecPlans (including
  `docs/execplans/7-2-3-structured-output-and-exit-metadata.md`), released
  migration guides (`docs/v0-11-0-migration-guide.md` and older), and the
  bodies of accepted ADRs, which change only through the amendment process in
  `docs/documentation-style-guide.md`.
- Run formatting, linting, and tests sequentially; never bypass compile
  admission (`RUSTC_WRAPPER`); never build in `/tmp`.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` (currently
`6ec295f4730003a74b131a40526ea47548d3f1db`). Count net lines (additions minus
deletions) per file with:

```sh
git diff --numstat "$(git merge-base origin/main HEAD)" -- . \
  ':(exclude)docs/execplans/7-2-4-json-mode-stream-contract.md'
```

Classify each file by purpose, whole-file:

- Code: `*.rs` (production and tests, including trybuild fixtures),
  `*.stderr`, `*.snap`, `*.feature`, test-fixture `*.json` and `*.txt`, and
  `proptest-regressions/*`. Production Rust is every `*.rs` file outside
  `tests/` directories, `*_tests.rs`, `tests_*.rs`, `*_proptests*.rs`, and
  files compiled only under `#[cfg(test)]`.
- Configuration: `Cargo.toml`, `Cargo.lock`, `typos.local.toml`, generated
  `typos.toml`, and anything under `.github/` or `.config/`.
- Documentation: every other `*.md` file. This ExecPlan is excluded from every
  count.

Budget survey. The closest precedent is 7.2.3, which added the same kind of
record across the same layers and squash-merged as `ef46b48d`. Measured with
the classification above, it changed 97 files (96 excluding its plan): about
1,006 net lines of production Rust, 3,236 of test Rust, 205 of snapshots, 28 of
fixtures, 15 of configuration, and 763 of documentation. Its plan set no
numeric budget, so it has no revision history. 7.2.1 is the only plan in this
family with a measured overrun: it set 30 files and about 1,500 net Rust lines,
then delivered 89 files and 2,838 net Rust lines, with tests at 73 percent of
the Rust; that was missed scope (tests and trybuild fixtures were not
estimated), not approved growth. 7.2.2 recorded no size. This plan therefore
budgets tests explicitly and sets limits a little above the 7.2.3 measurement,
because it adds a second rule, a pure verdict kernel with an exhaustive table,
and documentation-example inventory updates, while review removed one test
binary and two consistency conditions.

The estimate is about 800 net production lines (three enums and one record in
each of three schema owners, derive parsing and emission, bridge mapping, and
one rule module with its kernel) and about 3,400 net code lines of tests,
snapshots, and fixtures.

- Files: if more than 105 non-ExecPlan files change, stop and escalate.
- Code: if net code lines exceed 5,400 in total, or net production Rust
  exceeds 1,300, stop and escalate.
- Configuration: if net configuration lines exceed 60, stop and escalate.
- Documentation: if net documentation lines exceed 1,000, stop and escalate.
- Interface: if a public API change beyond those authorized in `Constraints`
  is needed (for example, changing `check_output`, adding a field to
  `AgentCommand` or `DocMetadata` directly, exposing the verdict kernel
  publicly, or bumping a schema version), stop and escalate.
- Dependencies: if any dependency change is needed, stop and escalate.
- Architecture: if the work requires a bridge build in policy-only execution,
  wiring rules into `--check-agent-native`, changing renderer semantics, a
  runtime stream checker, or a new nested-Cargo test binary, stop and escalate.
  If roadmap 7.2.8 has landed an orchestrator that composes `check_output`
  before this work is implemented, stop and agree with the user whether this
  item also composes `check_json_mode` there.
- Iterations: if the same gate still fails after three diagnosed repair
  attempts, stop and escalate with the captured log.
- Time: no wall-clock limit applies (`AGENTS.md`); record timestamps in
  `Progress` so slow milestones are visible.
- Ambiguity: if implementation exposes more than one defensible meaning for a
  token or condition beyond those settled in `D3`–`D9`, stop and present the
  options.

Each category is enforced independently; spare capacity in one cannot offset
another.

## Risks

- Risk: the subprocess policy duplicates 7.2.2's per-renderer
  `subprocess_inheritance` (`allowed | forbidden`). Severity: medium.
  Likelihood: medium. Mitigation: `D6` gives the fields distinct jobs. The
  renderer is authoritative for whether child output may pass through in each
  renderer role; `subprocess_output_policy` says only where child output goes
  instead of stdout in JSON mode, plus (for two tokens) a human-mode permission
  that must agree with `renderer.human`. Two named conditions report explicit
  human-mode disagreement; nothing compares JSON mode with the machine renderer
  (A5).
- Risk: `artifact_path` overlaps 7.2.3's `stdout = "path"`. Severity: low.
  Likelihood: medium. Mitigation: `D3` makes `artifact_path` stream framing and
  leaves `stdout` as semantic purpose; ADR-0026 publishes an informative
  purpose-by-framing table (no lint).
- Risk: adoption ratchet. `check_output`'s remedy text tells authors to add
  `supports_json = true`; once 7.2.8 composes the new rule, that edit produces
  a deny-mode `json_mode_contract_incomplete` finding. Severity: medium.
  Likelihood: high. Mitigation: the incomplete message includes a complete,
  paste-able `json_mode(...)` template; CHANGELOG, the users' guide, and the
  check-gen design note warn 7.2.3 adopters; the check-gen note recommends that
  7.2.8 roll the new rule out in warn mode first.
- Risk: struct-literal sweep misses a target that compiles only in some
  configurations (7.2.2 missed fixture literals; 7.2.3 missed a doctest).
  Severity: medium. Likelihood: medium. Mitigation: run
  `cargo test --workspace --doc` and the full `make test` in EP-M1, and search
  with the command below, excluding completed ExecPlans and released migration
  guides from its results.

  ```sh
  grep -rn "OutputMetadata {\|OutputContract {" --include=*.rs --include=*.md .
  ```

- Risk: near-cap files. `cargo-orthohelp/src/policy/rules/output_tests.rs`
  (385 lines), `ortho_config/tests/docs_ir_output.rs` (376),
  `ortho_config/src/agent_context/tests.rs` (371),
  `cargo-orthohelp/src/schema/tests/mod.rs` (352),
  `tests/fixtures/orthohelp_fixture/src/lib.rs` (348),
  `ortho_config/src/agent_context/mod.rs` (345), and
  `cargo-orthohelp/tests/rstest_bdd_support/behaviour/steps_agent_context.rs`
  (394, do not touch). Severity: low. Likelihood: high. Mitigation: new types
  and tests go in new sibling modules named in `Plan of work`; near-cap roots
  gain at most a `mod` line and a `pub use` line; check `wc -l` before each
  commit.
- Risk: documentation fences are executable contracts. Every tested fence in
  `docs/users-guide.md` and `docs/cargo-orthohelp-api-guide.md` carries a
  `<!-- tested-example: id -->` marker whose ID appears in four inventories
  (`ortho_config/tests/documentation_examples_tests.rs` lines 18 and 230,
  `STANDARD_RUST_EXAMPLES` in
  `ortho_config/tests/documentation_examples_rust_tests.rs`, and the expected
  stdout in `ortho_config/tests/documentation_examples/simple_runs.rs`).
  Severity: medium. Likelihood: high. Mitigation: extend the existing
  `guide-output-metadata` fence and its `SimpleRun` stdout instead of adding a
  users' guide fence; add at most one API-guide example and register it in
  every inventory in the same commit.
- Risk: nested-Cargo test targets must be listed in `.config/nextest.toml`
  test-group and warm-up filters, enforced only by
  `tests/workflow_contracts/child_cargo_tier_test.py`, which `make test` does
  not run; 7.2.3 failed CI this way. Severity: high. Likelihood: high if a new
  binary is added. Mitigation: `D16` extends the existing
  `agent_context_output_end_to_end` target instead. If a nested-Cargo test ever
  fails intermittently, check test-group membership before attributing the
  failure to load.
- Risk: BDD step-text collisions. Both feature files run in one
  `rstest_bdd` binary; 7.2.3 already binds
  `a command named "get" with no output declaration` and an
  `exactly one "…" finding has severity "…"` pattern in
  `cargo-orthohelp/tests/rstest_bdd_support/output_policy.rs`. Severity:
  medium. Likelihood: high. Mitigation: every new step starts with the unique
  prefix "a JSON-mode" or "the JSON-mode", patterns stay literal where a
  placeholder could match 7.2.3's text, and the module comment in
  `rstest_bdd_support/behaviour/scenarios.rs` (which counts feature files) is
  updated.
- Risk: the rolling Whitaker suite or compile-admission saturation moves a
  gate verdict without a source change. Severity: medium. Likelihood: medium.
  Mitigation: record the suite revision and `build-limits-status` beside any
  surprising verdict.

## Progress

- [x] (2026-11-23 10:20Z) Branch renamed to
  `7-2-4-json-mode-stream-contract`; Lody session titled.
- [x] (2026-11-23 10:50Z) Reconnaissance by a Wyvern team: 7.2.3 output
  block, policy rules and 7.2.8 status, renderer and prover infrastructure, and
  budget history.
- [x] (2026-11-23 11:00Z) Prior-art research: JSON Lines, RFC 7464 JSON text
  sequences, the Command Line Interface Guidelines, Cargo
  `--message-format json`, and rustc `--error-format=json`.
- [x] (2026-11-23 11:40Z) Initial draft written and committed.
- [x] (2026-11-23 13:10Z) Community-of-experts design review (contracts and
  structure; alternatives and viability; failure modes and delivery;
  verification rigour) completed; verdicts ranged from "Revise" to "Proceed
  with conditions".
- [x] (2026-11-23 14:00Z) Review findings applied (see `Decision log` and the
  revision note); approval decisions A1–A5 stated.
- [ ] Plan and decisions A1–A5 approved by the user.
- [ ] `EP-M1` JSON-mode stream contract carried from derive to agent context.
- [ ] `EP-M2` byte-accounting policy rules delivered as a library API.
- [ ] `EP-M3` documentation reconciled and roadmap 7.2.4 marked done.

## Surprises & discoveries

- Observation: `cargo-orthohelp` keeps child stdout off its own stdout, but
  not child stderr. Cargo's stdout is piped into the protocol reader
  (`cargo-orthohelp/src/bridge_build_process.rs:62`, `spawn_cargo`), Cargo's
  stderr is inherited and its retained diagnostics are forwarded to stderr
  (`bridge_build_diagnostics.rs`, `forward_cargo_diagnostics`), and the bridge
  executable's streams are captured with `Command::output()`
  (`bridge_execution.rs`, `run_bridge`). Evidence: design review, correcting
  this plan's first draft, which wrongly said 8.1.1 could declare a capture
  policy truthfully today. Impact: decision A1 and an 8.1.1 roadmap sub-bullet.
- Observation: the 7.2.3 end-to-end assertion in
  `cargo-orthohelp/tests/fixtures/output_assertions.rs`
  (`expected_greet_output`, `assert_greet_output`) compares greet's whole
  output block for equality. Evidence: lines 20–46 at `6ec295f4`. Impact: adding
  `json_mode` to the greet fixture requires updating that literal in EP-M1.
- Observation: `NestedGreetCommand` declares a machine renderer with
  `subprocess_inheritance = "allowed"` and a human renderer with
  `subprocess_inheritance = "forbidden"`, deliberately different so a swapped
  pair is visible (fixture comment, lines 157–162). Impact: after A5, greet can
  carry a complete, consistent `capture_preview_and_log` contract; its renderer
  values must not be changed.
- Observation: no Verus or Kani proof exists in the repository; `make verus`
  and `make kani` exit 2 with `FORMAL-EMPTY` when no proof files are named.
  Evidence: `Makefile` lines 153–156 and 266–300, and
  `docs/developers-guide.md` "Production proof inventory". Impact: see the
  formal verification decision.

## Decision log

- D1. Decision: extend 7.2.3's `output` block with one nested optional record,
  `json_mode`, rather than flat keys on `output` or new keys on `renderer`.
  Rationale: ADR-0023 reserved room for exactly this inside `output` and gave
  it a single presence rule. A nested record keeps "no contract declared"
  distinct from "contract partly declared", which flat keys cannot express, and
  keeps framing separate from the renderer's presentation vocabulary
  (ADR-0016). Rejected: renderer keys (the renderer's `stdout` would mean two
  things); flat `output` keys (no presence rule); defaulting a contract when
  `supports_json = true` (would attach claims never made, which ADR-0023
  forbids); a single combined enum (18 combinations). Date/Author: 2026-11-23,
  planning agent; confirmed by design review.
- D2. Decision: keep the design's wire member names and tokens:
  `success_stdout` ∈ {`one_json_document`, `jsonl_stream`, `artifact_path`},
  `failure_stderr` ∈ {`one_json_diagnostic`, `jsonl_diagnostics`},
  `subprocess_output_policy` ∈ {`capture_preview_and_log`,
  `inherit_only_in_human_mode`, `forbidden`}. Name the Rust enums
  `StdoutFraming`, `StderrFraming`, and `SubprocessOutputPolicy`. Rationale:
  the names are published in design §6.2 and used by the roadmap; an
  outcome-by-member table in ADR-0026 removes the scope ambiguity. Subject to
  approval decision A4. Date/Author: 2026-11-23, planning agent; revised after
  design review (type names).
- D3. Decision: `artifact_path` means the result is not inline: the command
  writes its result to one or more file artefacts, and success stdout is
  exactly one JSON text that references them through the agreed `files` shape
  (A2). On failure, stdout is empty unless an artefact was already delivered,
  in which case stdout holds at most one such JSON text. Rationale: keeps
  "JSON-mode stdout contains only complete JSON texts" uniform across tokens
  and makes design §6.2's exception ("unless an explicit artefact has already
  been delivered") precise. Review removed the draft's "at least one artefact"
  and failure-time accounting requirements as over-specified. Rejected: a bare
  path line on stdout (non-JSON in JSON mode). Date/Author: 2026-11-23,
  planning agent; revised after design review.
- D4. Decision: single-document tokens (`one_json_document`,
  `one_json_diagnostic`) require exactly one RFC 8259 JSON text, optionally
  surrounded by whitespace, so pretty-printed output conforms. Stream tokens
  (`jsonl_stream`, `jsonl_diagnostics`) follow JSON Lines: UTF-8 without a byte
  order mark, one JSON value per line with no raw line feed inside it, each
  record terminated by LF; consumers tolerate CRLF; records may be any JSON
  value. Under `jsonl_stream`, failure stdout may hold complete records already
  delivered but never a partial record; this is an explicit amendment of design
  §6.2's "failure writes no stdout" default, recorded in ADR-0026, and only the
  exit status distinguishes a complete stream from a truncated one. RFC 7464
  record-separator framing was considered and rejected: nothing in the estate
  emits it and line tools cannot split it. Date/Author: 2026-11-23, planning
  agent; revised after design review (whitespace).
- D5. Decision: under `one_json_diagnostic`, success writes nothing to stderr
  and failure writes exactly one JSON diagnostic text. Under
  `jsonl_diagnostics`, stderr is a JSON Lines stream on both outcomes; success
  may carry zero or more records, failure at least one. Diagnostic fields
  (class and code) belong to 7.2.5. Subject to approval decision A1.
  Date/Author: 2026-11-23, planning agent.
- D6. Decision: `subprocess_output_policy` governs only where child process
  output goes instead of the parent's stdout in JSON mode; every stderr byte is
  accounted for by `failure_stderr`, so child bytes on stderr are allowed only
  when they conform to that framing (in practice, only by being wrapped in the
  parent's own diagnostic). The tokens mean: `capture_preview_and_log` — the
  parent captures child output; the full output is reachable through a `log`
  entry in the `files` reference of the result or diagnostic document, and a
  bounded preview may appear only inside the diagnostic document (A3);
  `inherit_only_in_human_mode` — child output is never connected to stdout in
  JSON mode, and in human mode the child may inherit the parent's streams (a
  permission, which must agree with `renderer.human.subprocess_inheritance`);
  `forbidden` — child output is never connected to stdout in any mode, and this
  is the correct declaration for a command that spawns no subprocess. Retaining
  child output elsewhere (for example in an unreferenced cache log) is
  unconstrained. Rationale: the roadmap and design §6.2 limit the subprocess
  rule to stdout; scoping it so removes double accounting of stderr.
  Date/Author: 2026-11-23, planning agent; revised after design review.
- D7. Decision: deliver the checks as a separate public function,
  `check_json_mode(context: &AgentContext, mode: PolicyMode) -> PolicyReport`,
  in `cargo-orthohelp/src/policy/rules/json_mode.rs`, rather than adding them to
  `check_output`. Rationale: `check_output` is 7.2.3's stable consumer
  interface for 7.2.8; the incomplete rule would fire on every existing
  `supports_json = true` command and change that report, invalidating 7.2.3's
  tables and the 7.2.8 cross-over contract. Matches the
  `check_behaviour`/`check_output` precedent. Consequence: 7.2.8 composes one
  more call, and its coverage counts need an applicability signal; this plan
  records that as a 7.2.8 interface requirement in
  `docs/cargo-orthohelp-check-gen-design.md` rather than adding a speculative
  public predicate. Date/Author: 2026-11-23, planning agent; confirmed by
  design review.
- D8. Decision: rule `agent-native.output.json-mode-incomplete`, code
  `json_mode_contract_incomplete`. A command is applicable when `supports_json`
  is `true`, or when `supports_json` is undeclared and its `json_mode` record
  declares at least one member. An applicable command is incomplete when
  `supports_json` is undeclared, or when `json_mode` is absent, or when any
  member is undeclared or is the wire `unknown` fallback. One finding per
  command occurrence lists, in schema order, the missing members (including
  `supports_json` when undeclared) and, separately, the members carrying
  unrecognized tokens, whose remedy is to upgrade the checker rather than to
  declare the member. The message embeds a complete paste-able `json_mode(...)`
  template. Rationale: only an explicit or implied JSON claim makes byte
  accounting applicable; reporting an undeclared `supports_json` beside a
  declared stream contract reports a missing declaration without inferring one
  (the precedent is `check_output`'s treatment of undeclared support).
  Date/Author: 2026-11-23, planning agent; revised after design review (implied
  applicability, unrecognized tokens, template).
- D9. Decision: rule `agent-native.output.json-mode-inconsistent`, code
  `json_mode_contract_inconsistent`, fires once per command occurrence when any
  of these named conditions holds, listing every holding condition in this
  order: `json_declined` — `supports_json` is `false` and `json_mode` declares
  at least one known token; `diagnostics_declined` — `stderr_diagnostics` is
  `false` and `failure_stderr` is a known token; `human_inheritance_permitted` —
  `subprocess_output_policy` is `forbidden` and
  `renderer.human.subprocess_inheritance` is `allowed`;
  `human_inheritance_forbidden` — `subprocess_output_policy` is
  `inherit_only_in_human_mode` and `renderer.human.subprocess_inheritance` is
  `forbidden`. Undeclared values and `unknown` tokens never trigger a
  condition. Each condition pairs two explicit claims about the same mode that
  cannot both be true under `D3`–`D6`, ADR-0016, and ADR-0023; ADR-0026 gives a
  one-line argument for each. The draft's machine-renderer and
  `stdout = "none"` conditions are dropped (A5). Date/Author: 2026-11-23,
  planning agent; revised after design review.
- D10. Decision: the derive validates grammar only (unknown keys, unknown
  tokens, repeated leaves, wrong literal kinds); it performs no cross-field
  validation. Rationale: ADR-0023 assigns claim consistency to policy, and a
  derive error cannot be downgraded by the project's policy mode. Date/Author:
  2026-11-23, planning agent.
- D11. Decision: no schema version bumps. Rationale: the field is optional and
  additive under the §8.2 additive-field policy, as in 7.2.2 and 7.2.3.
  Date/Author: 2026-11-23, planning agent.
- D12. Decision: record the accepted surface in a new ADR,
  `docs/adrs/adr-0026-json-mode-stream-contract.md` (re-check the next free
  number at implementation time), drafted in EP-M1 and finalized at the end of
  EP-M2 once the rule semantics are exercised. Add an `Amended by ADR-0026`
  banner and an `## Amendments` section to ADR-0023 recording that its
  stream-framing outstanding decision is resolved. ADR-0026 states: the scope
  definitions (`D13`), the token table with outcome-by-member semantics, the
  `files` reference shape, the purpose-by-framing table, the condition
  arguments, the rule that a meaning change needs a new token, and the
  outstanding decisions (A5's mode binding; additional stderr tokens for tools
  that write human text in JSON mode). Date/Author: 2026-11-23, planning agent;
  revised after design review.
- D13. Decision: define the contract's scope. JSON mode is an invocation that
  requested the command's JSON output (the `--json` capability asserted by
  `supports_json`) and whose arguments parsed. Success is exit status 0;
  failure is any non-zero status. Usage errors raised before JSON mode is
  established (Clap status 2), help and version output, termination by a
  signal, and operator-enabled diagnostic logging (for example through
  `RUST_LOG`) are outside the contract; an application that writes log lines to
  stderr in JSON mode by default cannot declare `one_json_diagnostic`.
  Rationale: without these definitions two implementations would disagree on
  what the tokens promise. Date/Author: 2026-11-23, planning agent; added after
  design review.
- D14. Decision: no runtime stream conformance checker in this item.
  Rationale: the roadmap asks to model the contract; the first executable
  consumer is roadmap 8.1.1 (`cargo-orthohelp --json`), which gains a
  sub-bullet to capture child output and verify its real streams against its
  declared contract. A reusable checker needs that concrete consumer before it
  earns a public API (`AGENTS.md` abstraction policy). Date/Author: 2026-11-23,
  planning agent; confirmed by design review.
- D15. Decision: structure `check_json_mode` around a pure per-command kernel:
  a private `JsonModeView` projection of an `AgentCommand` (the four inputs the
  rules read), a total `verdict` function returning a structured
  `JsonModeVerdict` (ordered missing members, ordered unrecognized members,
  ordered condition labels), and a thin rendering layer that sorts commands,
  projects, judges, and builds findings. Rationale: the verification review
  showed that exhaustive enumeration is complete only for such a kernel; with
  the split, the kernel cannot read fields it must ignore, so the frame
  condition holds by construction and the enumeration claim is honest.
  Date/Author: 2026-11-23, planning agent; added after design review.
- D16. Decision: extend the existing
  `cargo-orthohelp/tests/agent_context_output_end_to_end.rs` target (180 lines)
  with a `#[path]` helper module
  `cargo-orthohelp/tests/fixtures/json_mode_assertions.rs` and one policy test,
  rather than adding a new nested-Cargo test binary. Rationale: avoids
  `.config/nextest.toml` test-group and warm-up filter edits, an extra bridge
  build per locale, and a race with BDD scenarios that clear
  `target/orthohelp`. Date/Author: 2026-11-23, planning agent; added after
  design review.
- D17. Decision: fixture witnesses use existing commands without changing
  their existing declarations. `NestedGreetCommand` gains a complete,
  consistent contract (`one_json_document`, `one_json_diagnostic`,
  `capture_preview_and_log`), so the 7.2.8 cross-over's supported-data command
  stays clean under the new rule. `NestedGrantAccessCommand` (a `submit`
  command that plausibly returns a one-document JSON summary) gains a JSON
  claim whose stream contract omits the subprocess policy, as the incomplete
  witness. Inconsistent witnesses are exercised at unit, table, and BDD level,
  which suffices because the rules are pure functions of the decoded context.
  Rationale: adding a new subcommand would ripple through nested fixture
  mirrors and snapshots; the draft's prune and `jsonl_stream` witnesses were
  unrealistic for their commands. Date/Author: 2026-11-23, planning agent;
  added after design review.
- D18. Decision: no Verus or Kani milestone. See
  `Formal verification decision`. Date/Author: 2026-11-23, planning agent;
  confirmed by verification review.

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
  defines `STDOUT_VALUES` (line 27), the private helpers `group_is_empty` (line
  81, accepts `output()`) and `set_once` (line 249, rejects repeated leaves
  across groups), and `unknown_output_key` (line 92), whose message lists the
  accepted keys and is pinned by
  `ortho_config_macros/src/derive/parse/tests/output_attrs.rs:165`. Leaf helpers
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
  exactly like the IR; `cargo-orthohelp/src/schema/tests/output_snapshots.rs`
  pins it.
- Wire: `OutputContract`, `StdoutPurpose` (`#[non_exhaustive]`, with
  `#[serde(other)] Unknown`), `ExitClass`, `deserialize_present`, and
  `deserialize_contract` in `ortho_config/src/agent_context/output.rs` (125
  lines), attached as `AgentCommand::output`
  (`ortho_config/src/agent_context/mod.rs:179`) and re-exported at line 44. A
  present `null` is a decode error in agent context. Wire tests live in
  `ortho_config/src/agent_context/tests_output_wire.rs`, registered with
  `#[path]` in `ortho_config/src/agent_context/tests.rs:28`. The canonical wire
  fixture is
  `ortho_config/src/agent_context/fixtures/agent_context_wire_contract.json`.
- Bridge: `map_output` and `map_stdout` (one explicit arm per variant) in
  `cargo-orthohelp/src/agent_context/output.rs` (48 lines), called from
  `cargo-orthohelp/src/agent_context/mod.rs:243`; property tests in
  `cargo-orthohelp/src/agent_context/output_proptests.rs` with generators in
  `output_proptests/generators.rs`.
- Policy: `check_output` in `cargo-orthohelp/src/policy/rules/output.rs`
  (155 lines), rule `agent-native.output.json-missing`, code
  `json_support_missing`, severity `warn` in warn mode and `deny` otherwise,
  stable `sort_by` on command path, `location: None`, report built with
  `PolicyReport::with_details(mode, results, Vec::new(), Vocabulary::canonical())`.
  It is a library API: no production path calls it. Shared rule-test helpers
  live in `cargo-orthohelp/src/policy/rules/tests.rs` (`context_with`,
  `command`, `ctx_for_command`).

Renderer metadata (7.2.2, ADR-0016) carries, per renderer role (`human` and
`machine`), `stdout` and `stderr` content categories
(`none | text | structured | mixed`) and `progress_inheritance` and
`subprocess_inheritance` (`allowed | forbidden`, wire `unknown` fallback). The
wire types are `RendererContract` and `RendererRecord` with `InheritancePolicy`
in `ortho_config/src/agent_context/renderer.rs` (`InheritancePolicy` at line
290). `AgentCommand::renderer` is not optional and carries role defaults, but
`subprocess_inheritance` stays `None` unless declared.

`cargo-orthohelp`'s `--check-agent-native` path (`main.rs:112`,
`policy/check/mod.rs:71`) runs configuration-sanity rules only and never calls
`policy::rules`. Neither `check`/`gen` (roadmap 7.2.8) nor `--json` (roadmap
8.1.1) exists yet; `docs/cargo-orthohelp-check-gen-design.md` §§4 and 6
describe how 7.2.8 will compose rule reports with per-rule-group coverage
counts and state that 7.2.8 does not implement stream framing or new
completeness rules.

Tests and tooling. Workspace dev-dependencies (`Cargo.toml` lines 29–38):
`rstest 0.28.0`, `rstest-bdd 0.6.0`, `rstest-bdd-macros 0.6.0`,
`proptest 1.11.0`, `googletest 0.14.3`, `pretty_assertions 1.4.1`.
`ortho_config` and `cargo-orthohelp` also use `insta` and `trybuild`.
`make test` runs `cargo test --all-targets --all-features` (not nextest) and
then pytest; it does not run doctests or `tests/workflow_contracts`.
`cargo-orthohelp` BDD scenarios live in `cargo-orthohelp/tests/features/`,
bound through `cargo-orthohelp/tests/rstest_bdd.rs` with step modules under
`cargo-orthohelp/tests/rstest_bdd_support/`; 7.2.3's
`structured_output.feature` is bound in
`rstest_bdd_support/behaviour/scenarios.rs` near line 56. rstest-bdd's
`scenarios!` input is not tracked by Cargo, so after editing a `.feature` file
touch the binding module. Compile-fail tests run
`ortho_config/tests/compile_fail.rs` over `ortho_config/tests/ui/*.rs` with
committed `.stderr` files. Golden agent-context snapshots live in
`cargo-orthohelp/tests/golden/` (only `agent_context__nested_fixture.json.snap`
reads the nested fixture's output metadata; roff and PowerShell goldens use
handwritten IR). The real-binary precedent is
`cargo-orthohelp/tests/agent_context_output_end_to_end.rs`, a member of the
`orthohelp_cache` nextest group. Prover targets `make kani`, `make kani-full`,
and `make verus VERUS_PROOF_FILES='…'` exist but have no production inputs.

Relevant documentation and skills. Read `docs/agent-native-cli-design.md`
§§6.2, 6.2.1, 6.2.2, 8.1, and 8.2; ADR-0003, ADR-0016, and ADR-0023;
`docs/cargo-orthohelp-check-gen-design.md` §§4–6; `docs/design.md`;
`docs/cargo-orthohelp-design.md`; `docs/users-guide.md` sections "Declare
structured output and exit classes" (line 2397) and "Lint declared structured
output" (line 2780); `docs/developers-guide.md` "Structured output metadata
ownership" (line 847) and "Kani and Verus verification tools" (line 55);
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`; and
`docs/documentation-style-guide.md` (ADR amendments, line 545). The
localization documents (`docs/cli-localization-design.md` and
`docs/localizable-rust-libraries-with-fluent.md`) matter only to confirm that
tokens stay non-localized; `docs/improved-error-message-design.md` informs the
finding messages; `docs/clap-dispatch-and-ortho-config-integration.md` and
`docs/scripting-standards.md` need no change. Load the `rust-router` skill
first, then `rust-types-and-apis` (public types), `rust-unit-testing` (rstest,
googletest, insta), `proptest`, `rust-verification` (method selection),
`arch-decision-records` (ADR-0026), `execplans`, `mapsplice` (for the roadmap
sub-bullet), and `en-gb-oxendict-style` for prose. Load `verus` only if the
formal verification decision is reopened.

## Conformance basis

Upstream artefacts, all at revision `6ec295f4`:

- `docs/roadmap.md` item 7.2.4 and its three bullets; item 8.1.1.
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

- `GAP-fields`: `OutputMetadata` and `OutputContract` have no stream contract
  members.
- `GAP-rule`: no rule checks byte accounting or stream-contract consistency.
- `GAP-renderer`: nothing reconciles the human renderer's
  `subprocess_inheritance` with a subprocess output policy.
- `GAP-docs`: §6.2, §8.1, §8.2, ADR-0023, the users' guide, and the
  developers' guide describe stream contracts as future work; §8.1 still lists
  the delivered renderer rows as "planned"; roadmap 8.1.1 does not require
  `cargo-orthohelp` to honour a declared stream contract.

Trace links:

```plaintext
roadmap 7.2.4 bullet 1 -> REQ-success-stdout -> GAP-fields -> EP-M1 -> agent_context_output_end_to_end::declared_stream_contract_survives_to_both_artefacts
roadmap 7.2.4 bullet 2 -> REQ-failure-stderr -> GAP-fields -> EP-M1 -> tests_json_mode_wire::known_tokens_decode
roadmap 7.2.4 bullet 3 -> REQ-subprocess -> GAP-fields, GAP-renderer -> EP-M1, EP-M2 -> json_mode_kernel_tests::verdict_matches_the_specification_exhaustively
design §6.2 lint sentence -> REQ-byte-accounting -> GAP-rule -> EP-M2 -> json_mode_stream_contract.feature
ADR-0023 no-inference -> REQ-no-inference -> EP-M1, EP-M2 -> json_mode_proptests::projection_ignores_unrelated_fields
ADR-0003 -> REQ-ownership -> EP-M1 -> schema json_mode mirror parity snapshots
design §8.2 -> REQ-additive -> EP-M1 -> tests_json_mode_wire::context_without_json_mode_decodes_unchanged
design §6.2 -> REQ-nonlocalized -> EP-M1 -> agent_context_output_end_to_end, second locale
ADR-0023 outstanding decision -> GAP-docs -> EP-M1 to EP-M3 -> ADR-0026 and amended ADR-0023
roadmap 8.1.1 -> A1 -> GAP-docs -> EP-M3 -> roadmap sub-bullet
```

## Verification plan

### Abstract model

A command's JSON-mode stream contract is a partial record `J = (σ, φ, π)`. Each
member is undeclared (written `⊥`), a known token, or (in agent context only)
`unknown`. Write `known(x)` for "x is a known token" and `declared(x)` for
`x ≠ ⊥`.

- `σ`, the stdout framing, ranges over `one_json_document`, `jsonl_stream`,
  `artifact_path`, `⊥`, and `unknown` (5 states).
- `φ`, the stderr framing, ranges over `one_json_diagnostic`,
  `jsonl_diagnostics`, `⊥`, and `unknown` (4 states).
- `π`, the subprocess output policy, ranges over `capture_preview_and_log`,
  `inherit_only_in_human_mode`, `forbidden`, `⊥`, and `unknown` (5 states).

`J` itself may be absent (written `J = absent`), so `J` has 1 + 5 × 4 × 5 = 101
states. The other inputs the rules read are `S ∈ {⊥, false, true}`
(`output.supports_json`), `D ∈ {⊥, false, true}` (`output.stderr_diagnostics`),
and `H ∈ {⊥, allowed, forbidden, unknown}`
(`renderer.human.subprocess_inheritance`). A command with `output` absent has
`S = D = ⊥` and `J = absent`. The per-command view is therefore the tuple
`(S, D, J, H)`, with 3 × 3 × 101 × 4 = 3,636 states.

Stream semantics (normative for ADR-0026; not executed by this item), within
the scope of `D13`. Let a JSON text be an RFC 8259 text, a record be a JSON
value encoded without a raw line feed followed by LF, and `ε` the empty byte
string.

```plaintext
stdout, success:
  one_json_document  -> exactly one JSON text, optional surrounding whitespace
  jsonl_stream       -> zero or more records
  artifact_path      -> exactly one JSON text referencing result files ("files")
stdout, failure:
  one_json_document  -> ε
  jsonl_stream       -> zero or more complete records already delivered
  artifact_path      -> ε, or at most one JSON text referencing delivered files
stderr, success:
  one_json_diagnostic -> ε
  jsonl_diagnostics   -> zero or more records
stderr, failure:
  one_json_diagnostic -> exactly one JSON text
  jsonl_diagnostics   -> one or more records
child output, JSON mode (never connected to stdout under any π):
  capture_preview_and_log    -> captured; full output via a "log" file reference
                                in the result or diagnostic document; bounded
                                preview only inside the diagnostic document
  inherit_only_in_human_mode -> not connected to stdout; may be inherited in
                                human mode only
  forbidden                  -> not connected to stdout in any mode
```

`complete(J)` holds when `J` is present and `known(σ) ∧ known(φ) ∧ known(π)`.
`declares(J)` holds when `J` is present and some member is declared.

Policy specification, written independently of the implementation:

```plaintext
applicable(v)  = S = true  or  (S = ⊥ and declares(J))
incomplete(v)  = applicable(v) and (S = ⊥ or not complete(J))
missing(v)     = [supports_json if S = ⊥] ++ [each member of (σ, φ, π), in that
                 order, that is ⊥ or J absent]          -- when incomplete(v)
unrecognized(v)= [each member of (σ, φ, π), in that order, that is unknown]
json_declined(v)               = S = false and J present and
                                 (known(σ) or known(φ) or known(π))
diagnostics_declined(v)        = D = false and known(φ)
human_inheritance_permitted(v) = π = forbidden and H = allowed
human_inheritance_forbidden(v) = π = inherit_only_in_human_mode and H = forbidden
conflicts(v) = the holding conditions, in the order listed above
verdict(v)   = (incomplete(v), missing(v), unrecognized(v), conflicts(v))

check_json_mode(ctx, off)  = report with no results
check_json_mode(ctx, m)    = for each command c of ctx in stable path order:
                               [incomplete finding  if incomplete(view(c))]
                               ++ [inconsistent finding if conflicts(view(c)) ≠ []]
                             each with severity warn if m = warn, deny if m = deny
```

Malformed authoring input (unknown key, unknown token, wrong literal kind,
repeated leaf) is a derive error with a span. Malformed IR JSON (unknown token)
is a decode error. Malformed agent-context JSON (present `null` for the record
or a member) is a decode error; an unknown token decodes to `unknown`.

Representation relation. `view : AgentCommand → (S, D, J, H)` reads exactly
`output.supports_json`, `output.stderr_diagnostics`, `output.json_mode`, and
`renderer.human.subprocess_inheritance`, mapping `output: None` to
`(⊥, ⊥, absent)` for the first three. Every other `AgentCommand` field,
`renderer.machine`, every other `renderer.human` field, and every
`AgentContext` field other than `commands` are outside the view.

### Invariants and lemmas

- `INV-preserve` (REQ-success-stdout, REQ-failure-stderr, REQ-subprocess,
  REQ-ownership): for every command independently, declared `json_mode` members
  survive derive → IR → mirror → wire unchanged, an absent record stays absent,
  an empty `json_mode()` stays a present empty record, and no command acquires
  a parent's record.
- `INV-presence` (REQ-additive): an agent context or IR document without
  `json_mode` decodes exactly as before; serialization omits absent members and
  the absent record; a present `null` is rejected in agent context and read as
  absence in the IR.
- `INV-unknown` (REQ-additive): each wire enum maps an unrecognized token to
  `unknown` and leaves the rest of the document readable; each IR enum and each
  derive token list rejects an unrecognized token.
- `INV-variants` (REQ-additive): the variant lists the verification enumerates
  equal the variant lists the crates define; adding a variant fails a test in
  the defining crate.
- `INV-nonlocalized` (REQ-nonlocalized): the stream contract is identical in
  agent context generated under two locales.
- `L-view` (REQ-no-inference): `view` agrees with the representation relation
  for every concrete combination of `output` (absent or present, with every
  member state) and `renderer.human.subprocess_inheritance`.
- `L-verdict` (REQ-byte-accounting, REQ-subprocess): for every one of the
  3,636 views, the production `verdict` equals the specification's `verdict`
  exactly (flags, ordered member lists, and ordered condition labels).
- `L-render` (REQ-byte-accounting): for a one-command context and every mode,
  `check_json_mode` emits exactly the findings the verdict determines, with the
  stable rule IDs and codes, severity from the mode, and `location: None`; off
  mode emits none.
- `L-decompose` (REQ-byte-accounting): for any context, the report equals the
  stable-path-ordered concatenation of per-command renderings; reports are
  additive over disjoint command sets, equal paths are not deduplicated,
  checking does not mutate the context, and empty and off reports advertise the
  canonical vocabulary.
- `INV-no-inference` (REQ-no-inference): each command's verdict is invariant
  under changes to its path, canonical verb, inputs, summary, flags,
  `output_modes`, `output.stdout`, `output.exit_classes`, every
  `renderer.machine` field, every `renderer.human` field other than
  `subprocess_inheritance`, and every context-level field (`policy`, `profiles`,
  `skill_manifests`); a parent command with a complete record never changes a
  child's verdict.
- `L-cond-sound` (REQ-byte-accounting): each of the four conditions pairs two
  explicit claims that cannot both hold under `D3`–`D6`, ADR-0016, and
  ADR-0023, and each "consistent by decision" pairing listed below is
  deliberately not a condition.
- `DEF-routing` (REQ-subprocess): ADR-0026's table assigns a destination to
  every (policy token, mode ∈ {human, JSON}, child stream ∈ {stdout, stderr}),
  and no JSON-mode destination is the parent's stdout. This is a definition
  reviewed for totality, not a theorem; nothing in this item executes it.

### Axioms (trusted, not verified here)

- `AXIOM-serde`: Serde's documented behaviour for `default`,
  `skip_serializing_if`, `deserialize_with`, `rename_all`, and
  `#[serde(other)]`. Exercised by the round-trip and decode tests.
- `AXIOM-syn`: `syn` parses nested attribute groups as documented; empty
  groups are handled by the existing `group_is_empty`. Exercised by parser
  tests and the trybuild fixture.
- `AXIOM-sort`: the standard library's `sort_by` is stable. Relied on by
  `L-decompose`; exercised by the equal-path property.
- `AXIOM-bridge`: the bridge binary transports the IR faithfully as JSON.
  Exercised by the real-binary end-to-end test.
- `AXIOM-jsonlines`: JSON Lines as specified at jsonlines.org, and RFC 8259
  JSON texts. Used only in ADR prose.
- `AXIOM-declaration`: a downstream application's declarations are claims.
  Nothing here verifies that a real process honours them; roadmap 8.1.1 is the
  first place real streams are checked.

### Methods per obligation

- Obligation: `INV-preserve`.
  Method: `rstest` tables for the grammar plus `proptest` round trips.
  Rationale: three optional enums inside an optional record, over nested
  command trees, is a broad but shallow space; 7.2.3 used the same approach.
  Domain: every token of every member; absent, empty, partial, and full
  records; bounded command trees of depth 0–4 with up to 32 commands. Artefacts:
  `ortho_config_macros/src/derive/parse/tests/json_mode_attrs.rs`,
  `ortho_config/tests/docs_ir_json_mode.rs`,
  `cargo-orthohelp/src/agent_context/json_mode_tests.rs`, and
  `cargo-orthohelp/src/agent_context/json_mode_proptests.rs`. Evidence: the
  focused EP-M1 commands fail before implementation on missing symbols and pass
  after. Non-vacuity: a deterministic coverage test runs the strategy through a
  `proptest::test_runner::TestRunner` with a fixed seed, samples a few hundred
  values, and asserts that every token, the absent record, the empty record,
  and a partial record occur. Seeded faults: map `forbidden` to
  `inherit_only_in_human_mode` in the bridge; copy a parent's record into a
  child. A named test must fail for each.
- Obligation: `INV-presence`, `INV-unknown`, and `INV-variants`.
  Method: named `rstest` cases and one exhaustive `match` per enum in the
  defining crate. Artefacts:
  `ortho_config/src/agent_context/tests_json_mode_wire.rs`,
  `ortho_config/tests/docs_ir_json_mode.rs`,
  `cargo-orthohelp/src/schema/tests/json_mode_snapshots.rs`, and the trybuild
  fixture `ortho_config/tests/ui/output_invalid_json_mode.rs` with its
  `.stderr`. Evidence: wire tests show `unknown` for a future token in each
  member, rejection of present `null` for the record and each member, and an
  unchanged decode of a small inline 7.2.3-era context (not a copy of the
  375-line golden); IR tests reject an unknown token; the trybuild `.stderr`
  pins the error naming the valid tokens; the variant tests list each enum's
  variants in an exhaustive `match` (allowed inside the defining crate despite
  `#[non_exhaustive]`) and their comments point to the kernel table test.
  Non-vacuity: every test asserts the decoded value, not merely `is_ok()`.
- Obligation: `L-verdict`.
  Method: exhaustive enumeration of the kernel. Rationale: the kernel's domain
  is finite (3,636 views); evaluating every view against an independent oracle
  and comparing complete structured verdicts is an exhaustive check of that
  domain, the strongest evidence available for a finite function, with no extra
  tooling. Artefact:
  `cargo-orthohelp/src/policy/rules/json_mode_kernel_tests.rs`, test
  `verdict_matches_the_specification_exhaustively`. Construction: a `Copy`
  test-local `Case` struct built by chaining `flat_map` over each dimension's
  variant slice (no nested `for` loops, no index arithmetic, no `#[values]`
  product, which would generate thousands of test functions); the oracle is
  four named condition functions and two list builders over the test's own
  plain enums, written from the specification above without calling production
  predicates; the test reports at most the first ten mismatches. Evidence:
  fails before EP-M2 (missing kernel) and passes after; asserts that the
  enumerated cases are pairwise distinct and number 3,636, and that every enum
  variant and every ADR token string occurs. Non-vacuity: each condition and
  `incomplete` is true for at least one view and false for at least one; at
  least one view holds two or more conditions, exercising ordering. Seeded
  faults, each applied alone on a clean committed tree and reverted: `unknown`
  treated as a known token in `json_declined`; the two human conditions' labels
  swapped; `S = ⊥` with an empty record treated as applicable; `supports_json`
  omitted from `missing`; members listed in the wrong order. Each must fail
  with a counterexample naming the view. Residual gap: production's `_`
  wildcard arms over the `#[non_exhaustive]` wire enums (required outside the
  defining crate) are unreachable from the enumeration; `INV-variants` bounds
  that gap.
- Obligation: `L-view` and `INV-no-inference`.
  Method: exhaustive enumeration of `view` over concrete states, plus property
  tests for invariance. Artefacts: `json_mode_kernel_tests.rs`
  (`projection_matches_the_representation_relation`, covering `output: None`,
  `Some(OutputContract::default())`, and every member state, crossed with every
  human inheritance state) and
  `cargo-orthohelp/src/policy/rules/json_mode_proptests.rs`
  (`projection_ignores_unrelated_fields`, varying every field listed in
  `INV-no-inference`). Seeded faults: map `output: None` to `S = false`; read
  `renderer.machine` instead of `renderer.human`; swap `allowed` and
  `forbidden` in the projection. Each must fail.
- Obligation: `L-render`.
  Method: exhaustive over 3,636 views × 3 modes through the public function on
  one-command contexts, comparing rule IDs, codes, severities, and
  `location: None` with the oracle's verdict; plus named `rstest` rows and
  `insta` snapshots of off, warn, and deny reports for a small context to pin
  message text, including the paste-able template and the unrecognized-token
  wording. Artefacts: `json_mode_kernel_tests.rs`
  (`rendered_findings_follow_the_verdict`) and
  `cargo-orthohelp/src/policy/rules/json_mode_tests.rs`. Seeded faults: emit
  the inconsistent finding before the incomplete one; emit findings in off
  mode. Each must fail. Snapshots are reviewed with `cargo insta review`, never
  accepted wholesale.
- Obligation: `L-decompose`.
  Method: `proptest` over generated command vectors of length 0–64, mirroring
  7.2.3's `output_proptests.rs` (permutation invariance for distinct paths,
  stable ties, additivity, non-mutation). Artefact: `json_mode_proptests.rs`.
  Rationale: the decomposition is over unbounded command lists, so it is
  sampled; proving it would need a verified specification of `sort_by`
  (`AXIOM-sort`), which `AGENTS.md` forbids trusting as a proof body.
  Non-vacuity: the fixed-seed coverage test asserts generated vectors contain
  offending and clean commands and equal paths. Seeded faults: replace
  `sort_by` with `sort_unstable_by`; deduplicate equal paths. Each must fail.
- Obligation: `L-cond-sound`.
  Method: argument in ADR-0026 plus named "consistent by decision" rows in
  `json_mode_tests.rs`: `S = ⊥` with an empty record; `S = false` with an empty
  record; `S = false` with only `unknown` members; `stdout = "none"` with
  `one_json_document`; `stdout = "path"` or `"summary"` with
  `one_json_document`; `stdout = "data"` with `artifact_path`;
  `renderer.machine.subprocess_inheritance = "allowed"` with every policy; and
  `capture_preview_and_log` with every human inheritance value. Each row
  asserts no inconsistent finding and cites the ADR sentence that justifies it.
- Obligation: `INV-nonlocalized` and the executable boundary.
  Method: real-binary end-to-end test in the existing target. Artefact:
  `cargo-orthohelp/tests/agent_context_output_end_to_end.rs` with the new helper
  `cargo-orthohelp/tests/fixtures/json_mode_assertions.rs`, reusing each
  locale's existing run. Tests:
  `declared_stream_contract_survives_to_both_artefacts` (greet's complete
  record and grant-access's partial record, in `agent-context.json` and stdout,
  under two locales) and `generated_context_drives_the_json_mode_rule` (decode
  the generated context and call `check_json_mode` in deny mode: exactly one
  `json_mode_contract_incomplete` finding, for grant-access, naming
  `subprocess_output_policy`; none for greet, audit, version, or any other
  command; `summary.deny` is 1). Non-vacuity: a bridge that dropped `json_mode`
  would turn greet into an incomplete finding and fail the second test; one
  that dropped grant-access's record would remove its finding.
- Obligation: `DEF-routing`.
  Method: design review of the ADR-0026 table for totality (every policy, mode,
  and child stream has a destination) and for the stdout exclusion. Residual
  gap: runtime conformance is unverified until roadmap 8.1.1 checks real
  streams.

Every seeded fault is applied alone to a clean, committed, green tree, its
failing output is recorded under `Artefacts and notes`, and it is reverted with
`git restore --source=HEAD -- <path>`. Acceptance for each probe is an empty
`git status --porcelain --untracked-files=all` afterwards, which also proves
that no `.snap.new`, pending-snapshot, or `proptest-regressions` residue
remains (the committed
`cargo-orthohelp/proptest-regressions/agent_context/output_proptests.txt` must
be unchanged).

### Formal verification decision

Verus and Kani are not selected, and no production proof is added. Every
executable obligation introduced here is either a total function over a finite
domain (the 3,636-view kernel and its 10,908 rendered cases, which this plan
enumerates completely against an independent oracle) or a decomposition over
unbounded command lists whose only non-trivial step is the standard library's
stable sort. Kani would re-establish the kernel's exhaustiveness with more
tooling. Verus is suited to unbounded properties, but the decomposition's proof
would depend on a trusted specification of `sort_by`, and `AGENTS.md` forbids
trusted proof bodies. The unbounded byte streams in the abstract model are
specified, not implemented, so a Verus proof about them would verify a detached
model rather than shipped code. This matches the 7.2.2 and 7.2.3 decisions;
7.2.2's stated reconsideration trigger (cross-field enforcement) is met here by
exhaustive enumeration because the cross-field domain is finite. Tests remain
the evidence for executable integration and externally observable behaviour in
all cases.

Reconsideration triggers. Revise this plan with a proof milestone before
implementing any of the following:

- A runtime stream conformance checker (for example
  `admits(contract, outcome, stdout, stderr)`), whose framing kernel has an
  unbounded domain. Candidate lemmas, to be refined when verification
  conditions appear: `L-frame-roundtrip` (splitting the concatenation of
  LF-free records each followed by LF returns exactly those records);
  `L-no-torn` (an accepted record stream is empty or ends with LF);
  `L-one-document` (single-document acceptance holds exactly when the trimmed
  stream parses as one JSON text).
- An in-house merge, deduplication, or precedence step over unbounded finding
  sequences (for example in 7.2.8's report composition), with the lemma that it
  preserves the multiset of per-command verdicts and stable path order.

The acceptance command for such a milestone is
`make verus VERUS_PROOF_FILES='<production proof path>'` beside the ordinary
gates, and the proof must reject a seeded fault. No `assume`, `admit`, or
trusted body may be introduced.

### Suggested BDD features

Feature file:
`cargo-orthohelp/tests/features/json_mode_stream_contract.feature`, bound in
`cargo-orthohelp/tests/rstest_bdd_support/behaviour/scenarios.rs` with step
definitions in a new
`cargo-orthohelp/tests/rstest_bdd_support/json_mode_policy.rs`. It covers
`REQ-byte-accounting`, `REQ-subprocess`, and `REQ-no-inference` in `EP-M2`.
Every step uses the "JSON-mode" prefix so no pattern collides with 7.2.3's
`structured_output.feature` steps in the same binary. Detailed condition
coverage lives in the kernel table and named rows, not here.

```gherkin
Feature: JSON mode stream contract
  Scenario: A JSON-mode command states where every byte goes
    Given a JSON-mode command with a complete stream contract
    When the JSON-mode policy is checked in "deny" mode
    Then the JSON-mode policy report has no findings

  Scenario: A JSON-mode command omits its subprocess policy
    Given a JSON-mode command whose stream contract omits the subprocess policy
    When the JSON-mode policy is checked in "deny" mode
    Then the JSON-mode policy report has one incomplete finding with severity "deny"
    And the JSON-mode finding names the subprocess output policy

  Scenario: A JSON-mode stream contract contradicts a JSON refusal
    Given a JSON-mode command that declines JSON support but declares a stream contract
    When the JSON-mode policy is checked in "warn" mode
    Then the JSON-mode policy report has one inconsistent finding with severity "warn"

  Scenario: A JSON-mode contract is not inferred from a verb
    Given a JSON-mode candidate named "get" with no output declaration
    When the JSON-mode policy is checked in "deny" mode
    Then the JSON-mode policy report has no findings
```

Runner: `cargo test -p cargo-orthohelp --test rstest_bdd json_mode`. The
scenarios fail before `check_json_mode` exists (missing function) and pass
after.

## Plan of work

Work proceeds in three milestones. Each starts red, ends green, passes the full
gate sequence, and is committed as a coherent unit.

### `EP-M1` carry the JSON-mode stream contract from derive to agent context

Requirements: `REQ-success-stdout`, `REQ-failure-stderr`, `REQ-subprocess`
(declaration only), `REQ-no-inference`, `REQ-ownership`, `REQ-additive`,
`REQ-nonlocalized`; gap `GAP-fields`; decisions `D1`–`D6`, `D10`–`D13`, `D16`,
`D17`.

Red. Add, before any production change:

1. `ortho_config_macros/src/derive/parse/tests/json_mode_attrs.rs`
   (registered in `ortho_config_macros/src/derive/parse/tests/mod.rs` beside
   `output_attrs`): a full declaration, an empty `json_mode()`, composition of
   members across two `output(...)` groups, every token of every member
   (`rstest` cases), and rejection of an unknown member key, an unknown token
   for each member, a non-string literal, a repeated member in one group and
   across groups, and `json_mode` outside `output(...)`. Update the pinned
   unknown-key message expectation in
   `ortho_config_macros/src/derive/parse/tests/output_attrs.rs:165` to list
   `json_mode`.
2. `ortho_config/tests/docs_ir_json_mode.rs`: IR decode and encode for absent,
   `null`, empty, partial, and full records; unknown-token rejection; derive
   emission for a declared, empty, and undeclared record.
3. `ortho_config/src/agent_context/tests_json_mode_wire.rs` (registered with
   `#[path]` in `ortho_config/src/agent_context/tests.rs` beside
   `tests_output_wire.rs`) and a variant-pinning test: omission, present-`null`
   rejection for the record and each member, empty record, each known token, a
   future token per member decoding to `unknown` and serializing as
   `"unknown"`, tolerance of a future member key, and an unchanged decode of a
   small inline 7.2.3-era context.
4. `cargo-orthohelp/src/agent_context/json_mode_tests.rs`: explicit mapping of
   every token, absent and empty records, per-command independence, and
   localizer independence.
5. `ortho_config/tests/ui/output_invalid_json_mode.rs` with an unknown
   `success_stdout` token. Generate its `.stderr` only after the parser exists,
   and review it.
6. In `cargo-orthohelp/tests/agent_context_output_end_to_end.rs`, the test
   `declared_stream_contract_survives_to_both_artefacts`, using the new helper
   `cargo-orthohelp/tests/fixtures/json_mode_assertions.rs`; extend
   `expected_greet_output()` in
   `cargo-orthohelp/tests/fixtures/output_assertions.rs` with greet's
   `json_mode` block.

Run the focused commands in `Concrete steps`; they must fail on missing
symbols, modules, or fixture declarations, not on tool errors.

Green. Implement:

1. Derive parsing in a new
   `ortho_config_macros/src/derive/parse/json_mode_attrs.rs`
   (`parse_json_mode_meta`), dispatched from `parse_output_meta` for the key
   `json_mode`. Make `group_is_empty` and `set_once` in `output_attrs.rs`
   `pub(super)` and reuse them with `parse_token`. Add `JsonModeAttrs` (three
   `Option<String>` members) to `doc_types.rs` and
   `json_mode: Option<JsonModeAttrs>` to `OutputAttrs`. Token lists are
   constants beside the parser, as `STDOUT_VALUES` is. Extend
   `unknown_output_key`'s message with `json_mode`.
2. Emission in `ortho_config_macros/src/derive/generate/docs/output.rs`, or a
   sibling `json_mode.rs` if that file would pass about 250 lines, emitting
   `json_mode: Some(JsonModeMetadata { … })` or `None` with
   `enum_option_tokens`.
3. IR types in a new `ortho_config/src/docs/ir/json_mode.rs`:
   `JsonModeMetadata`, `StdoutFraming`, `StderrFraming`,
   `SubprocessOutputPolicy` (strict enums, no `Unknown`), with
   `OutputMetadata::json_mode: Option<JsonModeMetadata>` using the same serde
   attributes as `DocMetadata::output`. Re-export from `ortho_config::docs`.
4. Mirror types in a new `cargo-orthohelp/src/schema/json_mode.rs`, wired into
   `cargo-orthohelp/src/schema/output.rs`, with snapshots in
   `cargo-orthohelp/src/schema/tests/json_mode_snapshots.rs` (empty, full, each
   token).
5. Wire types in a new `ortho_config/src/agent_context/json_mode.rs`:
   `JsonModeContract` with each member using `deserialize_present`, and the
   three enums marked `#[non_exhaustive]` with
   `#[default] #[serde(other)] Unknown`, each variant carrying a normative
   one-line Rustdoc summary of its ADR-0026 meaning. Add
   `OutputContract::json_mode` with a `deserialize_with` function that rejects
   a present `null`, mirroring `deserialize_contract`. Re-export the four types
   from `ortho_config::agent_context` (one `mod` line and one `pub use` line in
   the 345-line `mod.rs`).
6. Bridge mapping: `map_json_mode` plus one explicit-arm mapping function per
   enum, in `cargo-orthohelp/src/agent_context/output.rs` or a sibling
   `json_mode.rs`, called from `map_output`, with property tests in
   `cargo-orthohelp/src/agent_context/json_mode_proptests.rs`.
7. Update every exhaustive `OutputMetadata { … }` and `OutputContract { … }`
   literal in the workspace, fixtures, examples, doctests, and live Markdown
   examples with `json_mode: None`, excluding historical documents named in
   `Constraints`.
8. Fixture witnesses in `tests/fixtures/orthohelp_fixture/src/lib.rs`
   (`D17`), extending each command's doc comment to say why:

   ```rust,no_run
   // NestedGreetCommand: added inside the existing output(...) group.
   json_mode(
       success_stdout = "one_json_document",
       failure_stderr = "one_json_diagnostic",
       subprocess_output_policy = "capture_preview_and_log"
   )

   // NestedGrantAccessCommand: incomplete witness (no subprocess policy).
   output(
       supports_json = true,
       stdout = "summary",
       json_mode(
           success_stdout = "one_json_document",
           failure_stderr = "one_json_diagnostic"
       )
   )
   ```

   Review the regenerated
   `cargo-orthohelp/tests/golden/agent_context__nested_fixture.json.snap`;
   accept only the added `json_mode` and grant-access `output` blocks.
9. Extend the tested users' guide fence `guide-output-metadata`
   (`docs/users-guide.md:2404`) with a `json_mode(...)` declaration, and update
   its expected stdout in
   `ortho_config/tests/documentation_examples/simple_runs.rs`.

Refactor. Keep every touched file under 400 lines, share helpers rather than
duplicating 7.2.3 test setup, and expect `make lint` (Clippy with
`missing_const_for_fn` and `cognitive_complexity`) to find issues that
`cargo test` does not.

Documentation in the same milestone: draft ADR-0026 (status `Proposed`) with
the token table, scope definitions, `files` reference shape, and
purpose-by-framing table; amend ADR-0023 per `D12`; add ADR-0026 to
`docs/contents.md`; describe the derive syntax in `docs/users-guide.md`
"Declare structured output and exit classes", stating that the contract is a
declaration, not a runtime guarantee.

### `EP-M2` lint byte accounting as a library API

Requirements: `REQ-byte-accounting`, `REQ-subprocess`, `REQ-no-inference`; gaps
`GAP-rule`, `GAP-renderer`; decisions `D7`–`D9`, `D15`.

Red. Add `cargo-orthohelp/src/policy/rules/json_mode_kernel_tests.rs`
(exhaustive kernel, projection, and rendering tests), `json_mode_tests.rs`
(named rows, consistent-by-decision rows, and report snapshots),
`json_mode_proptests.rs` (decomposition and invariance properties, plus the
fixed-seed coverage test), the BDD feature and step module, and
`generated_context_drives_the_json_mode_rule` in the end-to-end target.
Register the unit test modules in `cargo-orthohelp/src/policy/rules/mod.rs`
under `#[cfg(test)]`. They must fail because `check_json_mode` and its kernel
do not exist.

Green. Add `cargo-orthohelp/src/policy/rules/json_mode.rs` with a `//!` module
comment and `pub mod json_mode;` in `rules/mod.rs`. Inside it, define the
private kernel (`JsonModeView`, `view`, `JsonModeVerdict`, `verdict`, visible
to the sibling test modules as `pub(super)`) and the public entry point:

```rust
#[must_use]
pub fn check_json_mode(context: &AgentContext, mode: PolicyMode) -> PolicyReport
```

following `check_output`'s structure: in off mode return
`PolicyReport::with_details(mode, Vec::new(), Vec::new(), Vocabulary::canonical())`;
otherwise collect commands, `sort_by` path (stable), project, judge, and push
an incomplete finding then an inconsistent finding when the verdict says so.
Keep each predicate a small private function below the cognitive-complexity
threshold. Messages name the command path; the incomplete message lists missing
and unrecognized members separately and embeds a complete paste-able
`json_mode(...)` template; the inconsistent message names each holding
condition and the remedy (correct whichever declaration is untrue). Add a
Rustdoc example decoding a small context with `serde_json::from_value` and
showing one incomplete finding in deny mode, as `check_output`'s example does.

Refactor. Share the severity projection with `check_output` only if both can
use one private helper without changing `check_output`'s behaviour; otherwise
leave the three-line function duplicated and record why. Run the seeded-fault
probes listed in `Verification plan`, recording each failure and restoring a
clean tree after each.

Documentation in the same milestone: finalize ADR-0026 (status `Accepted`, with
the condition arguments and the consistent-by-decision table); add "Lint the
JSON mode stream contract" to `docs/users-guide.md` after "Lint declared
structured output", including the warning for 7.2.3 adopters; add one tested
API-guide example `api-guide-json-mode-policy` to
`docs/cargo-orthohelp-api-guide.md` beside the structured-output rule and
register it in `documentation_examples_tests.rs` (both inventories),
`STANDARD_RUST_EXAMPLES` in `documentation_examples_rust_tests.rs`, and
`simple_runs.rs`; add a paragraph to `docs/cargo-orthohelp-check-gen-design.md`
§6 recording that the 7.2.8 orchestrator composes `check_json_mode` beside
`check_output`, needs an applicability count for coverage without duplicating
the predicate, and should roll the new rule out in warn mode first, without
changing 7.2.8's acceptance gate.

### `EP-M3` documentation reconciliation and delivery closure

Requirements: all, through `GAP-docs`.

Update `docs/agent-native-cli-design.md`: §6.2 replaces "remain roadmap 7.2.4
work" with the delivered shape and links ADR-0026, including the `jsonl_stream`
failure amendment; §8.1 adds rows for `output.json_mode` and its three members
(absent, `v1`, no inference) and corrects the stale "planned" renderer rows;
§8.2 records the additive `json_mode` history and the unknown-token fallback;
§9 removes JSON-mode stream contracts from the gap list. Update
`docs/design.md` (the deferral near line 1399) and
`docs/cargo-orthohelp-design.md` (deferral notes near lines 254, 815, and 948).
Update `docs/developers-guide.md` "Structured output metadata ownership" with
the new modules, the kernel-plus-exhaustive-table testing pattern, and the
reasons no production proof was added. Add a `CHANGELOG.md` `## Unreleased`
entry for the new metadata, the new rule API, the breaking struct-literal
change, and the 7.2.3-adopter warning. Add the 8.1.1 sub-bullet from A1 to
`docs/roadmap.md` (use the `mapsplice` skill for the structural edit). Search
for stale claims with:

```sh
grep -rn "7\.2\.4\|JsonModeContract\|stream framing\|stream contract" docs README.md CHANGELOG.md \
  | grep -v "docs/execplans/\|docs/v0-\|docs/adrs/"
```

and fix every live document it reports; accepted ADR bodies change only by
amendment. Mark roadmap 7.2.4 and its three bullets `[x]` only after all
evidence passes. Run all gates, update this plan's living sections, and set the
status to `COMPLETE`.

## Milestones and plateaus

- Identifier and outcome: `EP-M1`, authors can declare a JSON-mode stream
  contract and see it in IR, mirror, and agent context.
  - Requirements and gaps: `REQ-success-stdout`, `REQ-failure-stderr`,
    `REQ-subprocess` (declaration), `REQ-no-inference`, `REQ-ownership`,
    `REQ-additive`, `REQ-nonlocalized`; `GAP-fields`.
  - Acceptance evidence: focused EP-M1 tests, the end-to-end declaration
    test, reviewed snapshots, the documentation-example suites, and the full
    gate sequence.
  - Conformance check:
    - declared members round-trip unchanged per command;
    - ADR-0003 ownership and mirror parity hold;
    - no version constant changed;
    - only the authorized public API changed;
    - no trust boundary or persisted-format change beyond the additive field;
    - `check_output` rule tests and report snapshots unchanged;
    - trace links in `Conformance basis` still name real tests.
  - Recovery: revert the milestone commit; nothing else depends on it.
  - Remaining gaps: `GAP-rule`, `GAP-renderer`, most of `GAP-docs`.
  - Compatibility decision: none. Older IR and agent-context JSON must decode
    unchanged (a wire-format requirement, not a source shim).
- Identifier and outcome: `EP-M2`, `check_json_mode` reports incomplete and
  inconsistent stream contracts.
  - Requirements and gaps: `REQ-byte-accounting`, `REQ-subprocess`,
    `REQ-no-inference`; `GAP-rule`, `GAP-renderer`.
  - Acceptance evidence: exhaustive kernel, projection, and rendering tests;
    named and consistent-by-decision rows; snapshots; property tests; BDD
    scenarios; the end-to-end policy test; the doctest; recorded seeded-fault
    failures; and the full gate sequence.
  - Conformance check:
    - the specification in `Verification plan` matches the implementation;
    - `check_output` and its tests and snapshots unchanged;
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
    sequence, roadmap checkboxes, 8.1.1 sub-bullet.
  - Conformance check: every upstream artefact reconciled; ADR-0023 history
    preserved.
  - Recovery: revert the documentation commit.
  - Remaining gaps: runtime conformance (8.1.1), numeric exit codes and
    diagnostic class/code consistency (7.2.5), CLI composition (7.2.8), and
    ADR-0026's outstanding decisions (A5's mode binding; additional stderr
    tokens).
  - Compatibility decision: none.

## Concrete steps

Working directory: the repository root on branch
`7-2-4-json-mode-stream-contract`. Start each session with `git status --short`
and `git log --oneline -3`.

EP-M1 focused commands (red then green), narrowed to avoid compiling every
integration-test binary:

```sh
cargo test -p ortho_config_macros --lib json_mode
cargo test -p ortho_config --lib json_mode
cargo test -p ortho_config --test docs_ir_json_mode
cargo test -p ortho_config --test compile_fail
cargo test -p cargo-orthohelp --lib json_mode
cargo test -p cargo-orthohelp --test agent_context_output_end_to_end
cargo test -p cargo-orthohelp --test golden_tests
cargo test -p ortho_config --test documentation_examples_tests
cargo test --workspace --doc
```

Expected red: compile errors naming the missing `json_mode` member, module, or
type, or assertion failures on the absent `json_mode` block. Expected green:
all named tests pass; record the counts here. A filter that matches zero tests
is not acceptance.

EP-M2 focused commands:

```sh
cargo test -p cargo-orthohelp --lib json_mode
cargo test -p cargo-orthohelp --test rstest_bdd json_mode
cargo test -p cargo-orthohelp --test agent_context_output_end_to_end
cargo test -p ortho_config --test documentation_examples_tests
cargo test -p ortho_config --test documentation_examples_rust_tests
cargo test -p cargo-orthohelp --doc
```

Gate sequence after each milestone, run sequentially (preferably delegated to
`scrutineer`), each with `pipefail` and a log carrying a milestone suffix so a
later run does not overwrite failing evidence:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-ortho-config-7-2-4-json-mode-stream-contract-m1.out
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-7-2-4-json-mode-stream-contract-m1.out
make lint 2>&1 | tee /tmp/lint-ortho-config-7-2-4-json-mode-stream-contract-m1.out
make test 2>&1 | tee /tmp/test-ortho-config-7-2-4-json-mode-stream-contract-m1.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-7-2-4-json-mode-stream-contract-m1.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-7-2-4-json-mode-stream-contract-m1.out
```

Read a failing log before rerunning anything. Proceed only when every command
exits zero. If `make spelling` regenerates unrelated `typos.toml` content,
follow the repository's separate-base-PR policy.

For this planning change (documentation only), run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`; the Rust gates are not claimed for
unimplemented work.

## Validation and acceptance

After `EP-M1`: running `cargo orthohelp --format agent-context` against
`tests/fixtures/orthohelp_fixture` writes an `agent-context.json` whose `greet`
command's `output` block contains
`"json_mode":{"success_stdout":"one_json_document","failure_stderr":"one_json_diagnostic","subprocess_output_policy":"capture_preview_and_log"}`,
whose `grant-access` command carries the partial record, whose `version`
command has no `output` block, and whose stdout carries the same document in
both tested locales. The test
`agent_context_output_end_to_end::declared_stream_contract_survives_to_both_artefacts`
fails before and passes after.

After `EP-M2`: decoding that context and calling
`check_json_mode(&context, PolicyMode::Deny)` yields exactly one
`json_mode_contract_incomplete` finding, for grant-access, naming
`subprocess_output_policy`, and no other finding; `summary.deny` is 1.
`check_output` on the same context is unchanged from its pre-change result. The
kernel tests evaluate 3,636 views and 10,908 rendered cases with no mismatch.

Quality criteria:

- Tests: `make test` passes, plus `cargo test --workspace --doc`.
- Verification: `INV-*` and `L-*` obligations discharged as specified, with
  every seeded fault observed failing and the tree restored clean.
- Lint and format: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- Performance: the kernel tests complete in well under a second in a debug
  build; record the time.
- Security: no new input surface beyond derive attributes and JSON decoding;
  no secrets in snapshots or logs.

## Idempotence and recovery

All focused tests and gates can be rerun safely. Snapshot updates are reviewed
with `cargo insta review` rather than accepted wholesale. Seeded fault probes
run one at a time on a clean committed tree and are reverted with
`git restore --source=HEAD -- <path>`, never with a destructive reset;
acceptance is an empty `git status --porcelain --untracked-files=all`. Commit
each green milestone with `git commit -F` using an inspected message file; roll
back with a reviewed `git revert`. If a nested-Cargo test fails or times out,
first confirm its target is in the `orthohelp_cache` nextest group, then check
`build-limits-status`, and rerun the single test before treating it as a defect.

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

- JSON Lines (jsonlines.org): UTF-8, each line one JSON value, LF terminator
  (CRLF tolerated), no blank lines.
- RFC 7464 JSON text sequences: record-separator prefix framing,
  `application/json-seq`; considered for `jsonl_stream` and rejected (`D4`).
- Command Line Interface Guidelines (clig.dev): primary and machine-readable
  output on stdout; messages, logs, and errors on stderr; JSON when `--json` is
  passed.
- Cargo external tools, `--message-format json`: JSON objects one per line on
  stdout, with a caveat that other processes may print non-JSON lines to
  stdout; the subprocess policy exists to rule that out.
- rustc `--error-format=json`: diagnostics emitted one JSON object per line
  on stderr, the precedent for `jsonl_diagnostics`.

Design review (2026-11-23), four independent reviewers:

```plaintext
contracts and structure      Revise                    D6 stderr scope; C4 inference; agreed path
alternatives and viability   Proceed with conditions   drop C2/C4; cargo-orthohelp stderr; framing
failure modes and delivery   Approve after revision    output_assertions; nextest groups; doc harness
verification rigour          Approve after fixes       C1 unknown; kernel split; structured verdict
```

Seeded-fault evidence is recorded here during EP-M1 and EP-M2.

## Interfaces and dependencies

In `ortho_config/src/docs/ir/json_mode.rs` (re-exported from
`ortho_config::docs`):

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonModeMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_stdout: Option<StdoutFraming>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_stderr: Option<StderrFraming>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subprocess_output_policy: Option<SubprocessOutputPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StdoutFraming { OneJsonDocument, JsonlStream, ArtifactPath }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StderrFraming { OneJsonDiagnostic, JsonlDiagnostics }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubprocessOutputPolicy { CapturePreviewAndLog, InheritOnlyInHumanMode, Forbidden }
```

and `OutputMetadata` gains
`#[serde(default, skip_serializing_if = "Option::is_none")] pub json_mode: Option<JsonModeMetadata>`.

In `cargo-orthohelp/src/schema/json_mode.rs`: identical mirror types that
serialize exactly as the IR does; the mirror `OutputMetadata` in
`cargo-orthohelp/src/schema/output.rs` gains the same member.

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
const INCOMPLETE_RULE_ID: &str = "agent-native.output.json-mode-incomplete";
const INCOMPLETE_CODE: &str = "json_mode_contract_incomplete";
const INCONSISTENT_RULE_ID: &str = "agent-native.output.json-mode-inconsistent";
const INCONSISTENT_CODE: &str = "json_mode_contract_inconsistent";

/// Runs the JSON-mode stream-contract rules over a compiled agent context.
#[must_use]
pub fn check_json_mode(context: &AgentContext, mode: PolicyMode) -> PolicyReport;

// Private kernel, visible to sibling test modules only.
pub(super) struct JsonModeView { /* S, D, J members, H */ }
pub(super) fn view(command: &AgentCommand) -> JsonModeView;
pub(super) struct JsonModeVerdict {
    pub(super) incomplete: bool,
    pub(super) missing: Vec<ContractMember>,
    pub(super) unrecognized: Vec<ContractMember>,
    pub(super) conflicts: Vec<Condition>,
}
pub(super) fn verdict(view: &JsonModeView) -> JsonModeVerdict;
```

Reuse policy: the IR, mirror, and wire types are owned by their schema modules
and constructed only by the derive, decoding, and the bridge respectively.
`check_json_mode` is the only owner of the two new predicates; 7.2.8 and any
CLI must call it rather than reimplementing them, and any applicability signal
7.2.8 needs is added to this module, not copied. The kernel stays private until
a second consumer exists. No new helper abstraction is introduced beyond
private functions inside these modules; existing helpers (`group_is_empty`,
`set_once`, `parse_token`, `enum_option_tokens`, `deserialize_present`,
`PolicyReport::with_details`, `Vocabulary::canonical`) are reused.

Dependencies: none added.

## Revision note

- 2026-11-23: initial draft from Wyvern reconnaissance and prior-art
  research, before community-of-experts review.
- 2026-11-23: community-of-experts review applied. Changed: subprocess policy
  scoped to stdout (`D6`); `artifact_path` simplified and an agreed `files`
  reference proposed (`D3`, A2); single-document framing admits whitespace
  (`D4`); scope definitions added (`D13`); incomplete rule covers implied
  applicability and unrecognized tokens (`D8`); machine-renderer and
  `stdout = "none"` conditions dropped and the rest named (`D9`, A5); verdict
  kernel and honest exhaustiveness claim (`D15`); existing end-to-end target
  extended instead of a new binary (`D16`); realistic fixture witnesses
  (`D17`); documentation-example inventories, BDD step prefixes, probe hygiene,
  narrowed focused commands, historical-document exclusions, and a larger
  budget. The corrected `cargo-orthohelp` stderr discovery added decision A1
  and an 8.1.1 roadmap sub-bullet. Milestone order unchanged; EP-M2's
  verification is larger, and remaining work now waits on approval of A1–A5.
