# 7.1.2 Lint off-policy verbs and flags

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (community-of-experts review applied; awaiting user approval; no
implementation has started)

Branch: `7-1-2-lint-off-policy-verbs-and-flags`

Roadmap item: `7.1.2` in `docs/roadmap.md`.

## Purpose / big picture

OrthoConfig helps Rust command-line interface (CLI) authors publish a
machine-readable description of their command tree, called *agent context*
(`agent-context.json`), so that automated agents can drive the CLI safely.
`cargo-orthohelp` is the Cargo subcommand that generates that description and
checks it against an opt-in *agent-native policy*: a project-level rule set,
configured in `[package.metadata.ortho_config.policy]`, that holds a CLI to the
canonical command verbs and flags listed in `docs/agent-native-cli-design.md`
§5. The policy has three enforcement modes: `off` (check nothing), `warn`
(report findings without failing), and `deny` (report findings and fail). The
roadmap's phrase "strict policy" means an active policy, that is `warn` or
`deny` (Decision D10).

Roadmap item 7.1.1 delivered the configuration, the report format, and the
canonical vocabulary, but no rule that looks at the commands themselves. Today
a project can declare this and nothing complains:

```rust
#[derive(clap::Subcommand)]
enum Command {
    /// Show one widget.
    Info(InfoArgs),
    /// Show all widgets.
    Ls(LsArgs),
    /// Export widgets.
    Export(ExportArgs),
}

#[derive(clap::Args, ortho_config::OrthoConfig /* , … */)]
struct ExportArgs {
    /// Output encoding.
    #[ortho_config(value(type = "enum(json, yaml)"))]
    format: Format,
    /// Proceed without asking.
    skip_confirmations: bool,
}
```

After this change, the reusable vocabulary rule group,
`cargo_orthohelp::policy::rules::vocabulary::check_vocabulary(context, mode)`,
reads the command surface recorded in agent context and reports one finding per
off-policy spelling. Every finding names the canonical replacement both in its
message and in a new machine-readable `replacement` field:

```json
{
  "rule_id": "agent-native.vocabulary.noncanonical-verb",
  "code": "noncanonical_verb",
  "severity": "warn",
  "message": "command `widgets info` uses off-policy verb `info`; use `get`",
  "replacement": "get"
}
```

The five spellings the roadmap names are covered, and only those: the verbs
`info` (replace with `get`) and `ls` (replace with `list`); a `--format` option
that declares the value `json`, matched without regard to ASCII case (replace
with `--json`); an `--output` option that declares the value `json` (replace
with `--json`); and `--skip-confirmations` (replace with `--force`). In `warn`
mode the findings are warnings; in `deny` mode they are hard failures; in `off`
mode nothing is reported. A project exception recorded in the agent context's
`policy.exceptions`, such as a `verb` exception for `info` scoped to
`widgets info`, suppresses exactly the findings it covers.

The rule group is a library API with the same shape as the 7.2.1 behaviour rule
group (`check_behaviour(context, mode)`, documented in the users' guide section
"Lint the declared behaviour") and the 7.2.3 output rule group
(`check_output(context, mode)`). Roadmap item 7.2.8 owns the
`cargo orthohelp check` command that acquires command metadata once and
composes every rule group into one report;
`docs/cargo-orthohelp-check-gen-design.md` §2.2 says "later vocabulary and
other rules join when their own roadmap items land". The configuration-only
`--check-agent-native` operation keeps its no-bridge-build guarantee and does
not change.

The rule can see only what the documentation intermediate representation (IR)
records. The derive does not read clap's `#[arg(value_enum)]` variants; values
reach agent context only through `#[ortho_config(value(type = "enum(...)"))]`
(`ortho_config_macros/src/derive/generate/docs/fields/value_types.rs:154-157`
and `:311`). An undeclared `--format` value therefore produces no finding. This
plan documents that limit and proposes a follow-up roadmap item rather than
inferring values (Decision D11).

Preserved behaviour: configuration-sanity findings, the report schema version
(`"1"`), existing golden snapshots, the `--check-agent-native` process
contract, and the agent-context wire format. Deliberately changed behaviour,
all in code merged after the `v0.11.0` tag: the shared off-policy table maps
`--skip-confirmations` to `--force` instead of `--no-input` (D4), compares
declared values without regard to ASCII case (D12), and no longer maps
`--format text` to `--plain`, which belongs to the 7.1.3 glossary (D13). The
skill-manifest validator from 6.3.2 shares that table, so its advice changes in
the same way. `PolicyResult` becomes `#[non_exhaustive]` with constructors
(D5), which changes how downstream code builds results.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- `docs/agent-native-cli-design.md` §8.1 forbids inferring metadata from names.
  The rules may *report* a declared spelling, but must never infer mutation,
  interaction, or output semantics from it, and must never flag a value the
  metadata does not declare.
- `--check-agent-native` must continue to evaluate configuration only, without
  a bridge build (ADR-0008 D11 and its 2026-11-14 amendment). This plan adds no
  CLI wiring; 7.2.8 owns `cargo orthohelp check`.
- `ORTHO_POLICY_REPORT_SCHEMA_VERSION` and `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`
  stay `"1"`. Report changes are additive and serde-defaulted. The
  agent-context schema and the documentation IR schema do not change.
- The canonical vocabulary (`cargo-orthohelp/src/policy/vocabulary/mod.rs`) and
  the off-policy table
  (`cargo-orthohelp/src/policy/vocabulary/global_options.rs`) each keep one
  owner. No rule restates either list.
- Glossary rules (near misses such as `--output-format`, and `--plain`) belong
  to roadmap 7.1.3 and are out of scope.
- Per-rule severity levels (the reserved `rules` key) are out of scope; the
  strict configuration parser continues to reject `rules` (D3).
- No Rust source file may exceed 400 lines (`AGENTS.md`). Every module starts
  with a `//!` comment. Every assertion carries a diagnostic message.
- Production code must not call `std::env::var`. The end-to-end test configures
  only its child process. No new process-wide lock (`Mutex`, `OnceLock`, or
  `serial_test`) is introduced.
- Authorized test dependencies are already present in
  `cargo-orthohelp/Cargo.toml`: `rstest` 0.28, `rstest-bdd` 0.6, `insta` 1,
  `proptest` 1, `googletest` 0.14, `pretty_assertions` 1, and `trybuild` 1. No
  new dependency is authorized.
- `PolicyResult` changes shape (D5). This is a pre-1.0 public API: update every
  in-repository constructor and the compiled documentation example in the same
  change, document the impact, and add no compatibility shim.
- Completed ExecPlans under `docs/execplans/` are historical and must not be
  edited. The active 7.2.3 ExecPlan (PR #661) must not be edited either.
- Formatting, lint, and test gates run sequentially: `make check-fmt`,
  `make lint`, `make test`, plus `make markdownlint` and `make nixie` when
  Markdown changes. Because `make test` skips doctests, every milestone that
  touches a doc example also runs `cargo test --doc -p cargo-orthohelp`.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` at the time implementation
starts (`git merge-base HEAD origin/main`). This ExecPlan file is excluded from
every count. Counting command, run per category:

```bash
git diff --numstat "$(git merge-base HEAD origin/main)" -- <paths> \
  ':(exclude)docs/execplans/7-1-2-lint-off-policy-verbs-and-flags.md'
```

Categories are assigned by whole file: code is `*.rs`, `*.feature`, and
`*.snap`; configuration is `*.toml` and `.config/**`; documentation is `*.md`
other than ExecPlans, including `CHANGELOG.md`. Limits are net lines (additions
minus deletions) and are enforced independently.

- Files: more than 32 non-ExecPlan files changed → stop and escalate.
- Code: more than 2,300 net lines → stop and escalate.
- Configuration: more than 40 net lines → stop and escalate.
- Documentation: more than 650 net lines → stop and escalate.
- Interface: any public API change other than `check_vocabulary`,
  `ExceptionScope` and its `covers`, and the `PolicyResult` changes in D5 →
  escalate.
- Dependencies: any new crate → escalate.
- Architecture: wiring vocabulary rules into a CLI path, changing the
  configuration schema, the agent-context schema, or the documentation IR →
  escalate.
- Iterations: a focused test still failing after three fix attempts →
  escalate.
- Time: a milestone taking more than one working day → escalate.
- Ambiguity: a second plausible reading of a diagnostic contract, exception
  scope, or replacement mapping → stop and present options.

Budget basis. The survey (see `Context and orientation`) estimates about 1,650
net code lines: the rule module (~200), the exception scope type and refactor
(~60), the `PolicyResult` constructors and literal updates (~40), unit tests
(~300), property tests and committed controls (~380), behavioural feature and
steps (~280), end-to-end test (~200), fixture types (~90), and snapshots
(~100). History argues for contingency: the 7.1.1 plan raised its budget from
1,200 to 1,400 lines, the 7.2.1 plan landed at 2,838 lines against 1,500 ("low
by roughly 2x"), and the 6.3.2 plan landed at 5,466 lines against 1,500 with
maintainer approval. In both overruns, tests and property-test oracles
dominated, so this estimate itemizes them and the behavioural feature is
trimmed to acceptance scenarios that the `rstest` table does not already pin.
The 2,300-line code limit is the estimate plus about 40 per cent; a breach
means the estimate is wrong and must be re-planned, not silently absorbed.
Documentation is estimated at 430 lines (ADR amendment, design §5 and §8.1,
users' guide with a compiled example, developers' guide, API guide, gap
analysis, migration note, changelog, roadmap stubs) with 50 per cent
contingency. Configuration is one nextest override entry, so 40 lines is
generous.

## Risks

- Risk: PR #661 (7.2.3) constructs `PolicyResult` with a struct literal and
  adds `pub output: Option<OutputContract>` to `AgentCommand`. Whichever lands
  second must adapt. It also edits `policy/rules/mod.rs`,
  `policy/rules/proptests.rs`, `policy/rules/tests.rs`, `.config/nextest.toml`,
  and `tests/fixtures/orthohelp_fixture/src/lib.rs`. Severity: medium.
  Likelihood: high. Mitigation: new tests build `AgentCommand` only through the
  existing `command(...)` helper in `policy/rules/tests.rs` (or one shared
  builder), never by literal; new modules go in new files; rebase onto `main`
  before the final gates and re-run them.
- Risk: authors who use clap `value_enum` without the `enum(...)` override get
  no `--format json` finding, so the rule is quieter than users expect.
  Severity: medium. Likelihood: high. Mitigation: the users' guide states the
  requirement with an example; the design records the gap; a follow-up roadmap
  item is proposed (D11). A test row proves the unannotated case is silent, so
  the limit is pinned rather than accidental.
- Risk: a scoped exception whose `command_path` does not exactly equal a
  command's space-joined path (which starts with the root `bin_name` or
  `app_name`, for example `nested_fixture`) never matches, so deny-mode CI
  stays red and the team falls back to a global exception. Severity: medium.
  Likelihood: medium. Mitigation: the users' guide tells authors to copy `path`
  from `agent-context.json`; an "unmatched exception" finding is recorded as a
  7.2.8 composition candidate (D9).
- Risk: a domain-specific export option such as
  `config export --format json|yaml|toml` is flagged in deny mode, and the only
  escape is a flag exception that silences every value. Severity: low.
  Likelihood: medium. Mitigation: the message says other `--format` values may
  remain; the users' guide documents the scoped `format` exception recipe; the
  gap-analysis note records the decision.
- Risk: clap subcommand aliases, flag aliases, short-only flags, and
  `hide_in_help` fields are not represented in agent context, so off-policy
  spellings there go unreported. Severity: low. Likelihood: certain.
  Mitigation: documented residual gaps; representing them would change the IR
  schema, which is out of scope.
- Risk: adding a root type changes the `orthohelp_fixture` package fingerprint,
  so every bridge-backed test binary that uses it builds cold once on a
  saturated host. Severity: medium. Likelihood: certain. Mitigation: one new
  test binary with one `#[test]` that spawns the bridge once; add it to the
  existing `orthohelp_cache` nextest override; expect the first gate to be slow
  and do not treat a queued build as a failure.

## Progress

- [x] (2026-11-16 15:00Z) Reconnaissance by a Wyvern team: policy module,
  test infrastructure, design history.
- [x] (2026-11-16 15:40Z) Prior-art check: clig.dev, Heroku CLI style guide,
  Spectral ruleset severities.
- [x] (2026-11-16 16:10Z) Initial draft written and committed.
- [x] (2026-11-16 17:30Z) Community-of-experts review (structure and
  contracts; failure modes and verification; alternatives, cost, and viability)
  applied; see `Decision log` D1, D7, and D10 to D16.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Shared exception scope extracted; skill validator delegates.
- [ ] `EP-M2` `PolicyResult` constructors and `replacement`; shared table
  corrected.
- [ ] `EP-M3` `check_vocabulary` rule group with unit, snapshot, and property
  tests.
- [ ] `EP-M4` Behavioural feature and bridge-backed end-to-end test.
- [ ] `EP-M5` Documentation, ADR amendment, roadmap entry marked done.

## Surprises & discoveries

- Observation: the off-policy table already exists and already maps all five
  spellings. Evidence: `OFF_POLICY_FLAGS` and `OFF_POLICY_VERBS` in
  `cargo-orthohelp/src/policy/vocabulary/global_options.rs:107-114`, consumed
  only by `cargo-orthohelp/src/skill_validation/policy_check.rs`. Impact: 7.1.2
  adds a second consumer rather than a second table.
- Observation: the table maps `--skip-confirmations` to `--no-input`, but the
  design names `--force`. Evidence: `docs/agent-native-cli-design.md:14-15`
  ("accepts `--skip-confirmations` instead of `--force`") and §6.1 ("The
  preferred destructive bypass flag is `--force`"). Impact: D4.
- Observation: `check_behaviour` (7.2.1) has no production caller; it is a
  library rule group awaiting 7.2.8. Evidence: every call site is under
  `cargo-orthohelp/src/policy/rules/` tests, and the users' guide documents it
  as a library call over `agent-context.json`. Impact: D1 follows the same
  pattern.
- Observation: ADR-0008 D14 says 7.1.2 "passes the bridge IR through
  `PolicyInputs`". The later 7.2.8 design composes one report from separate
  per-group evaluators with per-group coverage. Impact: D1 records an ADR
  amendment.
- Observation: the agent-context verb mapper sets `canonical_verb` only for
  already-canonical segments, so `info` yields `None`. Evidence:
  `cargo-orthohelp/src/agent_context/mod.rs:281`. Impact: the verb rule reads
  the final path segment, not `canonical_verb`.
- Observation (review): clap `ValueEnum` variants never reach agent context;
  only the `value(type = "enum(...)")` override does, verbatim, and the
  repository's own fixture writes capitalized variants
  (`tests/fixtures/orthohelp_fixture/src/lib.rs:70`). Impact: D11 and D12; the
  first draft's fixture and users'-guide advice would have failed.
- Observation (review): `docs/cargo-orthohelp-api-guide.md:44` tells users to
  build `PolicyResult` as a struct literal, and its example at line 65 is a
  compiled `tested-example`. Impact: EP-M2 updates it with D5.
- Observation (review): `make test` runs `cargo test --all-targets`, which
  skips doctests, so the only pin of the `--skip-confirmations` replacement (a
  doc example at `global_options.rs:197-200`) is ungated. Impact: explicit
  doctest steps in EP-M1 to EP-M3.

## Decision log

- D1. Decision: deliver vocabulary linting as a separate rule group,
  `check_vocabulary(context: &AgentContext, mode: PolicyMode) -> PolicyReport`,
  in `cargo-orthohelp/src/policy/rules/vocabulary.rs`, and leave `PolicyInputs`
  empty. Exceptions come from `context.policy.exceptions`, which
  `apply_policy_to_context` (`cargo-orthohelp/src/agent_context/policy.rs:41`)
  fills from the configured policy. Rationale: the signature matches
  `check_behaviour` and `check_output`, which 7.2.8 composes group by group
  with per-group coverage (`docs/cargo-orthohelp-check-gen-design.md` §4); the
  run-time `--policy-mode` override stays a plain argument, as §2.3 requires;
  design §5 requires exceptions to be "explicit and visible in generated
  context", so what the rule suppresses is, by construction, what agents are
  told; and users run it on `agent-context.json` exactly as the users' guide
  already shows for `check_behaviour`. Alternatives considered: (a) populate
  `PolicyInputs` with the command list, as ADR-0008 D14 literally says —
  rejected because "configuration only" and "configuration plus vocabulary"
  would become indistinguishable in coverage, and `evaluate` would need bridge
  output; (b) `check_vocabulary(context, &PolicyConfig)` — the first draft's
  choice, rejected in review because users would have to rebuild a
  `PolicyConfig` from Cargo metadata and the report would duplicate the
  exceptions `evaluate` already advertises. Cost accepted: a standalone
  vocabulary report cannot show exception reasons; `evaluate` remains their
  owner. Upstream impact: an amendment to ADR-0008 D14, recorded in EP-M5. This
  is a deviation from an accepted ADR and is presented for explicit approval
  with this plan.
- D2. Decision: two rules, named to match the skill validator's existing codes
  (`skill.policy.noncanonical_verb`, `skill.policy.noncanonical_flag`).
  `agent-native.vocabulary.noncanonical-verb` (code `noncanonical_verb`) fires
  when a *leaf* command (no other command's path extends it) with at least two
  path segments has an off-policy final segment.
  `agent-native.vocabulary.noncanonical-flag` (code `noncanonical_flag`) fires
  when an input's long name, after stripping leading `-`, is
  `skip-confirmations`, or is `format` or `output` and declares an enumerated
  value equal to `json` ignoring ASCII case. One flag finding is emitted per
  input, not per matching value. Inputs without a long name (positionals) are
  ignored; root-command inputs are checked. Rationale: the design's spellings
  are `--format=json` and `--output json`, not bare `--format`; `--output` is
  the conventional output-*file* flag (clig.dev: "`-o`, `--output`: Output
  file"); a domain-specific `--format compact|rich` is the domain policy the
  gap analysis leaves open; the root segment is the binary name, not a verb;
  and in the design's noun-then-verb grammar a group such as
  `system info status` uses `info` as a noun.
- D3. Decision: do not implement per-rule levels now; the strict parser keeps
  rejecting `rules`. Rationale: the roadmap acceptance criteria name only mode
  and exceptions; per-rule levels add a configuration schema and a severity
  lattice with no current consumer, and must change every rule group's
  signature at once. EP-M5 adds a roadmap stub so the reservation names a real
  item, and replaces every "reserved for 7.1.2" text (ADR-0008 lines 87, 150,
  197-198; `policy/config/mod.rs` lines 7-10, 42-43, 146-149;
  `policy/evaluate/mod.rs` lines 10-11; `skill_validation/policy_check.rs` line
  24). The `#[non_exhaustive]` `PolicyInputs` seam and its compile test are
  retained for configuration-scoped inputs (for example per-rule levels), and
  the amendment says so.
- D4. Decision: correct the shared table so `--skip-confirmations` maps to
  `--force`. Rationale: the design names `--force` twice; `--no-input` means
  "never prompt, and fail if input is needed", so a script that relied on
  auto-confirmation would start failing if an author followed the old advice.
  The change is unreleased (6.3.2 merged after `v0.11.0`).
- D5. Decision: make `PolicyResult` `#[non_exhaustive]`, add
  `replacement: Option<String>` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`, and add
  `PolicyResult::new(rule_id, code, severity, message)` (each text argument
  `impl Into<String>`) with `with_replacement` and `with_location` builders.
  Rationale: ADR-0008 D14 prescribes the field; skipping `None` keeps every
  existing golden snapshot byte-identical, and readers of older reports default
  to `None`; since the struct breaks anyway, making it non-exhaustive now means
  later fields (per-rule level, structured path) stop being breaking changes.
- D6. Decision: extract exception matching into a borrowed view,
  `ExceptionScope<'a>` with fields `kind: ExceptionKind`, `name: &'a str`, and
  `command_path: Option<&'a str>`, providing `covers(&self, kind, name, path)`,
  `From<&PolicyException>`, and `from_wire(&ortho_config::PolicyException)`
  returning `Option<Self>` (an unknown wire `kind` yields `None` and covers
  nothing). It lives in `cargo-orthohelp/src/policy/config/scope.rs`. The skill
  validator's `ResolvedAgentPolicy::allows` delegates to it. Rationale: the
  abstraction policy in `AGENTS.md` requires reuse of the existing behaviour
  (`allows`, `names_match`, `scope_matches` in `policy_check.rs:64-71` and
  `278-293`); the vocabulary rule reads wire exceptions while the skill
  validator reads configured ones, so a borrowed view serves both without
  cloning or a second matcher. Semantics are unchanged: no `command_path` means
  global; otherwise the space-joined path must equal it exactly; flag names
  compare after stripping every leading `-`; verb names compare exactly; flag
  exceptions ignore option values.
- D7. Decision: the vocabulary report's `exceptions` list is empty in every
  mode, matching `check_behaviour` and `check_output`; it still advertises the
  canonical vocabulary. Rationale: `evaluate` is the single owner of exception
  advertisement (with reasons); composing groups must not duplicate entries that
  `duplicate_exception` deliberately distinguishes. Supersedes the first
  draft's D7.
- D8. Decision: findings are ordered by command path (stable sort, so equal
  paths keep input order), then the verb finding before flag findings, then
  input declaration order. Rationale: deterministic reports and snapshot
  stability; stable sorting preserves distinct occurrences with equal paths, as
  the 7.2.8 design requires.
- D9. Decision: no "unmatched exception" finding in 7.1.2. Rationale: it is a
  new rule beyond the roadmap item, and it needs the composed view of every
  rule group's exception use, which 7.2.8 owns. The users' guide gives the
  copy-the-path recipe instead.
- D10. Decision: "strict policy" in the roadmap and design §5 means an active
  policy, `warn` or `deny`. Rationale: design §8.1 says "Projects should be
  able to run the check in warning mode before enforcing it in CI", and the
  roadmap pairs strict policy with warning-mode and deny-mode tests.
- D11. Decision: do not infer enumerated values from clap types; document that
  the `--format` and `--output` rules need the
  `#[ortho_config(value(type = "enum(...)"))]` declaration, and propose (in
  EP-M5, as a roadmap stub for the user to accept or strike) a follow-up item
  for recording clap `ValueEnum` and `possible_values` in the documentation IR.
  Rationale: §8.1 forbids inference, and changing the derive or the IR is out
  of scope. No "declared without values" note is emitted: it would flag every
  free-form `--format`, which §8.1 and the noise budget both argue against.
- D12. Decision: `off_policy_flag_replacement` compares the value without
  regard to ASCII case, inside the shared table function. Rationale: the
  override records the author's spelling verbatim, and `enum(Json, Yaml)`
  describes the same values as `enum(json, yaml)` (clap kebab-cases `ValueEnum`
  names by default); normalizing in the owner keeps 6.3.2 and 7.1.2 consistent.
  Verbs stay exact, because subcommand names are kebab-cased by the derive
  (`ortho_config_macros/src/subcommand_docs.rs:78-86`).
- D13. Decision: remove the `text` → `--plain` mapping from the shared table.
  Rationale: the roadmap and design §5 name only `json`; `--format text` is
  usually the default human output while `--plain` is a plain-text fallback, so
  the advice misleads; `--plain` is a 7.1.3 glossary concept and is not in
  `CANONICAL_FLAGS`, so a replacement could fall outside the report's own
  `vocabulary` block. 7.1.3 will add glossary candidates inside
  `check_vocabulary` under the same `ExceptionScope` filter.
- D14. Decision: messages follow three fixed templates (verb, value-sensitive
  flag, plain flag), each naming the replacement in backticks:

  ```plaintext
  command `<path>` uses off-policy verb `<verb>`; use `<replacement>`
  command `<path>` declares `--<flag> json`, which selects structured output;
    expose `<replacement>` (other `--<flag>` values may remain)
  command `<path>` declares off-policy flag `--<flag>`; use `<replacement>`
  ```

  The second template is one line in the message; it wraps here only for width.
  Rationale: every diagnostic names its replacement; the `--format` wording
  stops authors deleting a domain format option; templates make the C3 property
  exact rather than a substring test.
- D15. Decision: the end-to-end test is one new binary,
  `cargo-orthohelp/tests/vocabulary_policy_e2e.rs`, with a single `#[test]`
  that spawns the bridge once and checks every mode on the parsed context; it
  takes no process-wide lock. Rationale: a single spawn needs no serialization
  inside the binary, and nextest's `orthohelp_cache` group serializes it
  against other bridge users.
- D16. Decision: no Verus or Kani milestone. Rationale: see `Verification plan`.

## Outcomes & retrospective

Not started.

## Conformance basis

- Roadmap: `docs/roadmap.md` 7.1.2 at commit `eea7273f` (`RM-7.1.2-a` flag the
  five spellings under strict policy; `RM-7.1.2-b` report the canonical
  replacement in every diagnostic; `RM-7.1.2-c` tests for warning mode, deny
  mode, and configured exceptions).
- Design: `docs/agent-native-cli-design.md` §5 (`DES-5-strict`: flag
  off-convention aliases when strict mode is enabled; exceptions explicit and
  visible in generated context), §6.1 (`DES-6.1-bypass`: `--force` is the
  destructive bypass), and §8.1 (`DES-8.1-no-infer`: no inference from names;
  findings follow the selected mode).
- ADR: `docs/adrs/adr-0008-agent-native-policy-configuration.md`, including D3
  (structured exceptions), D9 (opt-in `off`; the override never reaches agent
  context), D11 (no bridge build for the configuration check), D12 (exception
  visibility without reasons), D14 (evaluator seam and `replacement` field),
  and the 2026-11-14 amendment.
- Design amendment: `docs/cargo-orthohelp-check-gen-design.md` §2.2, §2.3, §4,
  and §6 (`CG-groups`: rule groups join `check` when their items land;
  `CG-coverage`: per-group coverage; `CG-exceptions`: evaluators own exception
  semantics).
- Gap analysis: `docs/ddlint-gap-analysis.md` "Next steps" item 1 (`GAP-1`:
  treatment of domain-specific `--format` flags).
- No Terms of Reference document exists for this work.

Trace links:

```plaintext
RM-7.1.2-a -> DES-5-strict -> D2, D10, D12 -> EP-M3 -> vocabulary_tests::flags_each_off_policy_spelling
RM-7.1.2-a -> DES-6.1-bypass -> D4 -> EP-M2 -> vocabulary::tests::skip_confirmations_maps_to_force
RM-7.1.2-b -> ADR-0008-D14 -> D5, D14 -> EP-M2, EP-M3 -> vocabulary_proptests::every_finding_names_a_canonical_replacement
RM-7.1.2-c -> DES-5-strict, CG-exceptions -> D1, D6 -> EP-M1, EP-M3, EP-M4 -> tests/features/vocabulary_policy.feature
DES-8.1-no-infer -> D2, D11 -> EP-M3 -> vocabulary_tests::undeclared_values_are_not_flagged
GAP-1 -> D2, D13, D14 -> EP-M3, EP-M5 -> vocabulary_tests::domain_format_values_are_left_alone
CG-groups, CG-coverage -> D1, D7 -> EP-M3, EP-M5 -> ADR-0008 amendment
```

## Context and orientation

The repository is a Cargo workspace. The two crates involved are `ortho_config`
(the library that owns reusable agent-context types) and `cargo-orthohelp` (the
generator and policy checker). Key files:

- `ortho_config/src/agent_context/mod.rs` defines `AgentContext` (with
  `commands` and `policy: AgentPolicy`), `AgentCommand` (with
  `path: Vec<String>` and `inputs: Vec<AgentInput>`), `AgentInput` (with
  `long: Option<String>`, written without dashes by the generator, and
  `enum_values: Vec<String>`), and the wire `PolicyException` (string `kind`,
  `name`, optional `command_path`, no reason). These are the rule group's only
  inputs besides the mode.
- `cargo-orthohelp/src/agent_context/mod.rs` builds an `AgentContext` from the
  bridge's documentation IR. `walk` emits one `AgentCommand` per command node,
  groups included, using only each node's own fields; the root path segment is
  the root's `bin_name` or `app_name`.
  `cargo-orthohelp/src/agent_context/input.rs` fills `enum_values` from the IR's
  `ValueType::Enum` or `possible_values`, both of which come only from the
  `value(type = "enum(...)")` override, and leaves booleans empty; it drops
  `hide_in_help` fields.
- `cargo-orthohelp/src/agent_context/policy.rs` holds `apply_policy_to_context`
  and the tool-to-wire `From` conversions (ADR-0008 D12).
- `cargo-orthohelp/src/policy/mod.rs` (262 lines) defines `PolicyReport`,
  `PolicyResult`, `PolicySeverity`, and `PolicyMode`.
- `cargo-orthohelp/src/policy/config/mod.rs` (160 lines) defines
  `PolicyConfig`, `PolicyException`, `ExceptionKind`, and the strict parser.
- `cargo-orthohelp/src/policy/evaluate/mod.rs` (169 lines) holds the
  configuration-sanity rules (`agent-native.config.*`).
- `cargo-orthohelp/src/policy/rules/behaviour.rs` (212 lines) is the 7.2.1 rule
  group; its shape (total function, `Rule` constants, `RuleSeverity`) is the
  model for the new module. `rules/proptests.rs` (384 lines) is the model for
  oracle-based property tests, and
  `cargo-orthohelp/src/skill_validation/proptests_controls.rs` is the model for
  committed mutation controls.
- `cargo-orthohelp/src/policy/vocabulary/mod.rs` owns `CANONICAL_VERBS` and
  `CANONICAL_FLAGS`; `vocabulary/global_options.rs` (258 lines) owns the
  off-policy table and `off_policy_flag_replacement` /
  `off_policy_verb_replacement`.
- `cargo-orthohelp/src/skill_validation/policy_check.rs` (293 lines) is the
  existing consumer and holds the exception matcher to extract.
- `tests/fixtures/orthohelp_fixture/src/lib.rs` (330 lines) is the real
  workspace fixture crate; it already serves four root types selected with
  `--root-type`.
- `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs` and
  `cargo-orthohelp/tests/agent_context_renderer_end_to_end.rs` show how an
  end-to-end test spawns the real binary against that fixture.
- `cargo-orthohelp/tests/skill_validation_bdd.rs` and
  `cargo-orthohelp/tests/skill_validation_bdd/steps.rs` show a library-level
  `rstest-bdd` harness; `cargo-orthohelp/tests/features/` holds feature files.
- `.config/nextest.toml` groups bridge-building test binaries in the
  `orthohelp_cache` override (`max-threads = 1`); `tests/workflow_contracts`
  asserts that every binary that spawns a child Cargo is covered.
- `docs/users-guide.md` "Lint the declared behaviour" holds the compiled
  example `guide-check-command`, registered in
  `ortho_config/tests/documentation_examples_tests.rs`,
  `ortho_config/tests/documentation_examples_rust_tests.rs`, and
  `ortho_config/tests/documentation_examples/guide_flows.rs`; a new example is
  registered the same way.
- `Makefile` provides `make verus` and `make kani`, both with empty proof
  registries (`VERUS_PROOF_FILES`, `KANI_*_HARNESSES`). No proof exists yet.

Survey for budgets: the change touches about 28 files (6 production, 10 test, 1
fixture, 1 nextest, 10 documentation). In-repository `PolicyResult` struct
literals number five: `policy/tests.rs` (two), `policy/evaluate/mod.rs` (one,
in a helper), `policy/config/tests.rs` (one), and `policy/rules/behaviour.rs`
(one), plus the compiled example in `docs/cargo-orthohelp-api-guide.md`.

Skills to load while implementing: `rust-router` (then `rust-unit-testing`,
`proptest`, and `rust-types-and-apis` as needed), `execplans`, `nextest`,
`en-gb-oxendict-style`, and `arch-decision-records` for the ADR amendment.
Documentation to read first: `docs/agent-native-cli-design.md` §5, §6.1, §8.1;
`docs/adrs/adr-0008-agent-native-policy-configuration.md`;
`docs/cargo-orthohelp-check-gen-design.md`; `docs/design.md`;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/documentation-style-guide.md`; and `docs/developers-guide.md` "Policy
configuration test layout". `docs/cli-localization-design.md`,
`docs/localizable-rust-libraries-with-fluent.md`, and
`docs/improved-error-message-design.md` were reviewed: policy messages are
English diagnostics in the machine report and are not localized, matching the
existing `agent-native.*` rules.
`docs/clap-dispatch-and-ortho-config-integration.md` and
`docs/cargo-orthohelp-design.md` explain how clap subcommands become
documentation IR. `docs/scripting-standards.md` does not apply: no script is
added.

## Verification plan

Behavioural contracts introduced or affected:

- C1 (mode gate, observable). With `off`, the report has no results. With
  `warn`, every vocabulary result has severity `warn`. With `deny`, every
  result has severity `deny`. The results, ignoring severity, are identical in
  `warn` and `deny`.
- C2 (detection, relative to declared metadata). For every leaf command with
  at least two path segments whose final segment is `info` or `ls`, exactly one
  verb finding is emitted. For every input, on any command, whose long name
  (leading `-` stripped) is `skip-confirmations`, exactly one flag finding is
  emitted; whose long name is `format` or `output` and which declares a value
  equal to `json` ignoring ASCII case, exactly one flag finding is emitted.
  Nothing else is emitted.
- C3 (replacement totality). Every vocabulary finding has
  `replacement = Some(r)` where `r` is in `CANONICAL_VERBS` or
  `CANONICAL_FLAGS`, and the message equals its D14 template instantiated with
  the finding's path, spelling, and `r`.
- C4 (exception soundness and completeness). A candidate finding for
  (kind, name, path) is suppressed if and only if some wire exception's
  `ExceptionScope` `covers` it. Flag exceptions ignore values; wire exceptions
  with an unknown `kind` cover nothing.
- C5 (determinism). Equal inputs give equal reports; reordering commands with
  distinct paths does not change the report.
- C6 (no inference). Undeclared values, free-form options, positionals,
  booleans other than `skip-confirmations`, unannotated `ValueEnum` types, and
  domain values (`compact`, `yaml`, `text`) never produce findings.
- C7 (wire compatibility). A result without a replacement serializes exactly
  as before; a report written before this change deserializes with
  `replacement = None`; existing golden snapshots are byte-identical.
- C8 (refactor preservation). Skill-manifest validation findings are
  unchanged except for the deliberate D4, D12, and D13 table changes.
- C9 (exception monotonicity). Adding an exception never adds a finding.
- C10 (command locality). The findings for two contexts with disjoint command
  paths, joined, equal the multiset union of their separate findings. 7.2.8's
  composition relies on this.

Axioms (external assumptions, not verified here): clap and the OrthoConfig
derive faithfully record subcommand names, long names, and declared `enum(...)`
values in the documentation IR; `apply_policy_to_context` copies configured
exceptions into agent context; serde honours `skip_serializing_if` and
`default`; `AgentContext` deserializes the generator's own output. The
end-to-end test exercises these against the real pipeline rather than assuming
them for the fixture.

Lemmas (intermediate claims connecting the rules to the contracts):

- L1. `ExceptionScope::covers` equals the old `allows` predicate for every
  input. Supports C4 and C8.
- L2. For each table entry, the mapped replacement is canonical. Supports C3.
  The domain is finite (two verbs, two value-sensitive flags, one plain flag).
- L3. Candidate generation is independent of mode and exceptions; mode and
  exceptions act only as a filter and a severity map afterwards. Supports C1,
  C4, C9, and C10, and is enforced structurally: `check_vocabulary` computes
  candidates from `context.commands`, filters with `covers`, then maps severity.

Methods, artefacts, and discharge:

- C1, C2, C4, C5, C9, C10: oracle-based property tests in
  `cargo-orthohelp/src/policy/rules/vocabulary_proptests.rs`, modelled on
  `rules/proptests.rs`. The generator draws command paths from a pool mixing
  canonical verbs, off-policy verbs, and neutral words (with leaves and
  groups); inputs from a pool of long names including `format`, `output`,
  `skip-confirmations`, `--format`, canonical flags, and neutral names, plus
  positionals; enumerated values from `json`, `Json`, `JSON`, `text`, `yaml`,
  `compact`, and empty; and exceptions with kinds `verb`, `flag`, and an
  unknown string. Exception scopes are drawn with `prop::sample::select` from
  the generated joined paths plus a decoy, so matching scopes are reachable by
  construction. The oracle is an independent restatement of C2 and C4 written
  over the literal five spellings; it calls neither `covers` nor the table
  functions. Command:
  `cargo test -p cargo-orthohelp --lib vocabulary_proptests`. Expected
  evidence: all properties pass at the default 256 cases. Discharge: pass plus
  the non-vacuity evidence below. Non-vacuity: committed deterministic controls
  in `cargo-orthohelp/src/policy/rules/vocabulary_controls.rs`, modelled on
  `skill_validation/proptests_controls.rs`, each holding one fixed input and
  showing that (a) an oracle that ignores scope disagrees with the rule on a
  scoped exception for another path, (b) an oracle missing the `output`
  spelling disagrees on an `--output json` input, (c) an oracle with
  case-sensitive values disagrees on `enum(Json)`, and (d) dropping one finding
  from the report breaks the comparison. A witness test asserts that fixed
  hand-built inputs exercise each class the generator must reach: verb finding,
  each flag finding, global-exception suppression, scoped-exception suppression
  on its own path, scoped exception on another path, unknown-kind exception,
  domain value without a finding, and a group named `info`.
- C2, C6 named cases: an `rstest` table in
  `cargo-orthohelp/src/policy/rules/vocabulary_tests.rs` covering each spelling;
  `Json` and `JSON`; `--format` declared with a leading `--`; `-format` and
  `---format` exception names; a root command named `ls`; a root input
  `--format json`; a group named `info`; a free-form `--format`; a path-typed
  `--output`; a positional named `format`; an unannotated custom type; and a
  boolean. Each row asserts the exact `(rule_id, code, path, replacement)` list
  with `googletest` matchers.
- C3, L2: an `rstest` case per table row in
  `cargo-orthohelp/src/policy/vocabulary/tests.rs` asserting the replacement is
  canonical; because the table is finite, this enumeration is exhaustive, not a
  sample. A property asserts the message equals its D14 template for every
  generated finding.
- C7: unit tests in `cargo-orthohelp/src/policy/tests.rs` deserializing a
  version-1 report without `replacement`, and asserting a `None` result
  serializes with no `replacement` key; existing golden snapshots must stay
  unchanged, which `cargo insta test` would otherwise report.
- C8, L1: a property in `cargo-orthohelp/src/skill_validation/proptests.rs`
  (283 lines; move to a sibling file if it would exceed 400) comparing
  `ExceptionScope::covers` with a restatement of the old
  `names_match`/`scope_matches` logic kept in the test, plus the existing
  skill-validation suites, which must pass unchanged apart from new D4, D12,
  and D13 assertions.
- Report shape: `insta` JSON snapshots `vocabulary_report__warn` and
  `vocabulary_report__deny` in `policy/rules/snapshots/`, each covering one
  finding per rule and value class. They are paired with the semantic
  assertions above, so a snapshot is never the only evidence for a rule.
- Real-pipeline evidence: the end-to-end test in EP-M4.

Why no Verus or Kani (D16). Verus is warranted for a lemma whose guarantee must
hold over an unbounded domain *and* that has a specification independent of the
code it verifies. Neither condition holds here. L2 and the table mappings are
finite and `rstest` enumerates them exhaustively. L1 and C4 are definitional:
the exception predicate *is* the specification of "covered", so a Verus model
would restate the implementation it claims to verify (Verus's `vstd` can model
strings, so representation is not the obstacle; the missing independent
specification is). C1, C3, C9, C10, and L3 are structural properties of a
straight-line generate-filter-map, and oracle-based property tests with
committed controls reject the plausible faults. Kani would add bounded
exploration of tables that `rstest` already enumerates. This matches the
precedent of 7.1.1, 7.2.1, 6.3.2, and 7.2.2, all of which declined formal tools
for table-driven classifiers. If implementation exposes an unbounded invariant
not covered above, stop and revise this section.

## Plan of work

Each milestone follows red, green, refactor. Each ends with the full gates run
sequentially and a commit.

### Milestone EP-M1 — shared exception scope (refactor)

Requirements: `RM-7.1.2-c` (exceptions), D6. Add
`cargo-orthohelp/src/policy/config/scope.rs` with `ExceptionScope`, `covers`,
`From<&PolicyException>`, and `from_wire`, each with a doc example. Red: add
`rstest` cases in `cargo-orthohelp/src/policy/config/tests.rs` for global,
scoped-match, scoped-mismatch, kind-mismatch, `--`/`-`/`---` normalization, and
unknown wire kind; they fail to compile until the type exists. Green: implement
by moving the logic from `skill_validation/policy_check.rs` (`names_match`,
`scope_matches`) and make `ResolvedAgentPolicy::allows` use it. Refactor:
delete the moved helpers. Add the L1 equivalence property. Acceptance: the new
cases pass; every existing skill-validation test passes unchanged;
`cargo test --doc -p cargo-orthohelp policy::config` passes. Recovery: revert
the single commit. Conformance check: no behaviour change; `ExceptionScope` is
the only new public item.

### Milestone EP-M2 — `PolicyResult` constructors and table corrections

Requirements: `RM-7.1.2-b`, `RM-7.1.2-a` (for `--skip-confirmations`), D4, D5,
D12, D13. Red: in `cargo-orthohelp/src/policy/tests.rs`, add tests that a
result built with `with_replacement("--json")` serializes a `replacement` key,
that a `None` result has no such key, and that version-1 JSON without the key
deserializes. In `cargo-orthohelp/src/policy/vocabulary/tests.rs`, add
`skip_confirmations_maps_to_force`, `json_value_matches_without_case`,
`text_value_has_no_replacement`, and the L2 rows. Add a skill-validation case
asserting `--force` advice for `--skip-confirmations`. Green: add the field,
`#[non_exhaustive]`, `new`, `with_replacement`, and `with_location`; replace
the five struct literals with `new`; rewrite the compiled example and the
"Build each `PolicyResult` as a struct literal" paragraph in
`docs/cargo-orthohelp-api-guide.md`; change the table entry, the value
comparison, and the doc examples. Extend
`cargo-orthohelp/tests/ui/policy_public_api.rs` to name the new items.
Refactor: none expected. Acceptance: new tests pass;
`git diff --stat -- '*.snap'` is empty;
`cargo test --doc -p cargo-orthohelp policy` passes; the documentation-examples
tests pass under `make test`. Recovery: revert the commit. Conformance check:
schema version unchanged.

### Milestone EP-M3 — vocabulary rule group

Requirements: `RM-7.1.2-a`, `RM-7.1.2-b`, `RM-7.1.2-c`, D1, D2, D7, D8, D14.
Create `cargo-orthohelp/src/policy/rules/vocabulary.rs` and register it in
`rules/mod.rs`. Red: write `vocabulary_tests.rs` (the named `rstest` table,
`warn`/`deny`/`off` cases, exception cases) and the snapshot tests first; they
fail because `check_vocabulary` does not exist. Green: implement as small
functions — `candidates(context) -> Vec<Candidate>` (leaf detection, verb and
flag candidates from the table, independent of mode and exceptions),
`exception_scopes(context) -> Vec<ExceptionScope<'_>>` via `from_wire`,
`is_covered(scopes, candidate)`, and `to_result(candidate, severity)` building a
`PolicyResult` with `replacement` and the D14 message. `check_vocabulary`
returns early in `off` mode with an empty report that still advertises the
canonical vocabulary. Then add `vocabulary_proptests.rs`,
`vocabulary_controls.rs`, and the witness test. Refactor: keep each file under
400 lines. Acceptance: `cargo test -p cargo-orthohelp --lib policy::rules`
passes; snapshots reviewed and accepted with `cargo insta review`; the four
controls pass; `cargo test --doc -p cargo-orthohelp policy::rules` passes.
Recovery: the module is additive; revert the commit. Conformance check: no CLI
path calls `check_vocabulary`; no inference beyond reporting a declared
spelling.

### Milestone EP-M4 — behavioural and end-to-end evidence

Requirements: `RM-7.1.2-c`, D2, D6, D15, C6. Behavioural tests: add
`cargo-orthohelp/tests/features/vocabulary_policy.feature`, a harness
`cargo-orthohelp/tests/vocabulary_policy_bdd.rs`, and steps in
`cargo-orthohelp/tests/vocabulary_policy_bdd/steps.rs`, modelled on the
skill-validation harness. Steps build an in-memory `AgentContext` through an
`rstest` fixture and one command builder, set `policy.exceptions` for exception
scenarios, and call the public API. The scenarios are the acceptance outcomes;
per-spelling detail stays in the `rstest` table. The feature text is:

```gherkin
Feature: Off-policy vocabulary lint

  Scenario: Warn mode reports off-policy spellings with their replacements
    Given a command surface with leaf commands "widgets info" and "widgets ls"
    And the command "widgets export" declaring "--format" with values "json,yaml"
    And the command "widgets export" declaring flag "--skip-confirmations"
    When the vocabulary rules check the command surface in "warn" mode
    Then every finding has severity "warn"
    And the replacements are "get", "list", "--json", and "--force"

  Scenario: Deny mode fails every off-policy spelling
    Given a command surface with leaf commands "widgets info" and "widgets ls"
    When the vocabulary rules check the command surface in "deny" mode
    Then every finding has severity "deny"
    And the report summary counts 2 deny findings

  Scenario: A scoped exception covers only its command
    Given a command surface with leaf commands "widgets info" and "gadgets info"
    And the agent context declares a "verb" exception for "info" scoped to "widgets info"
    When the vocabulary rules check the command surface in "deny" mode
    Then the report has a "deny" finding "noncanonical_verb" for "gadgets info" replaced by "get"
    And the report has no finding for "widgets info"

  Scenario: Off mode reports nothing
    Given a command surface with leaf commands "widgets info" and "widgets ls"
    When the vocabulary rules check the command surface in "off" mode
    Then the report has no findings

  Scenario: Domain-specific values and output paths are left to the project
    Given the command "widgets export" declaring "--format" with values "compact,rich"
    And the command "widgets export" declaring option "--output" without values
    When the vocabulary rules check the command surface in "deny" mode
    Then the report has no findings
```

End-to-end test: add a fifth root type, `VocabularyFixtureConfig`, to
`tests/fixtures/orthohelp_fixture` in a new module file
`tests/fixtures/orthohelp_fixture/src/vocabulary.rs` (keeping `lib.rs` under
400 lines). It declares leaf subcommands `info` and `ls`, and an `export`
subcommand with `--format` declared through
`#[ortho_config(value(type = "enum(Json, Yaml)"))]` (capitalized on purpose, to
exercise D12 through the real pipeline), `--output` (a path), and
`--skip-confirmations` (a boolean). Add
`cargo-orthohelp/tests/vocabulary_policy_e2e.rs` with one `#[test]` that spawns
the built `cargo-orthohelp` binary once (invocation below), configuring only
the child's environment, parses `agent-context.json` into `AgentContext`, and
then, on that parsed value, calls `check_vocabulary` in `warn`, `deny`, and
`off` modes and once more after inserting a scoped wire exception for the
`info` command's path. It asserts exactly four findings (`info` → `get`, `ls` →
`list`, `--format json` → `--json`, `--skip-confirmations` → `--force`), none
for the path-typed `--output`, `deny` severities and `summary.deny == 4` in
deny mode, no results in off mode, and three findings with the exception. Add
the binary to the `orthohelp_cache` override in `.config/nextest.toml`.
Acceptance: the feature and the end-to-end test pass under `make test`; the
`tests/workflow_contracts` suite passes; existing golden snapshots for the
other root types are unchanged. Recovery: revert the commit; the fixture type
is additive. Conformance check: the test proves the real derive pipeline
exposes the metadata the rules need (axioms above).

The end-to-end invocation is:

```bash
cargo-orthohelp orthohelp --package orthohelp_fixture \
  --root-type orthohelp_fixture::VocabularyFixtureConfig \
  --locale en-US --format agent-context --out-dir "$TEMPDIR"
```

### Milestone EP-M5 — documentation and closure

Requirements: all; D1, D3, D4, D5, D11, D12, D13. Update:

- `docs/adrs/adr-0008-agent-native-policy-configuration.md`: append a dated
  amendment recording D1 (rule group over agent context instead of
  `PolicyInputs`; `PolicyInputs` retained for configuration-scoped inputs), D3
  (reservation re-pointed to the new roadmap stub), and D5 (field shape and
  `#[non_exhaustive]`); fix the stale "reserved for 7.1.2" lines.
- `docs/agent-native-cli-design.md` §5: the rule identifiers, the
  value-sensitive and case-insensitive flag semantics, leaf-only verbs, the
  `--force` replacement, the removed `text` mapping, and the residual gaps
  (subcommand and flag aliases, short-only flags, hidden fields, undeclared
  values); §8.1 lines 811-816: replace "planned for roadmap 7.1.2" with the
  delivered behaviour.
- `docs/ddlint-gap-analysis.md` "Next steps" item 1: domain-specific `--format`
  values are not flagged; a `--format` that also accepts `json` is, and the
  advice keeps the domain values.
- `docs/cargo-orthohelp-check-gen-design.md` §6: one sentence naming
  `check_vocabulary(context, mode)` as the vocabulary group 7.2.8 composes, and
  noting that it reads exceptions from the acquired context, so 7.2.8 must
  apply the configured policy to that context before calling it.
- `docs/users-guide.md`: replace the "planned for a later release" wording in
  "Agent-native policy checking"; add a section "Lint off-policy verbs and
  flags" after "Lint the declared behaviour", with a compiled example
  (`guide-check-vocabulary`, registered in the three documentation-example
  files), the two rules, the `replacement` field, the `enum(...)` requirement
  for `--format`/`--output`, the copy-the-path recipe for scoped exceptions,
  and the note that the CLI runs the group once `cargo orthohelp check` lands
  (7.2.8). Update "Validate skill manifests against commands" for the D4, D12,
  and D13 advice changes.
- `docs/developers-guide.md` "Policy configuration test layout" and
  "Skill-manifest validation internals": `ExceptionScope` and its re-use policy
  (the only exception matcher; rule groups and validators convert to it and
  never restate matching), the new test and control files, and the fixture root
  type.
- `docs/cargo-orthohelp-api-guide.md`: constructors replace struct literals
  (EP-M2 makes the code change; this step checks the prose).
- A migration note for the next release covering `#[non_exhaustive]`
  `PolicyResult`, its constructors, the `replacement` field, and the advice
  changes. Add it to an unreleased migration guide if one exists at
  implementation time; otherwise create `docs/v0-12-0-migration-guide.md`
  following the v0.11.0 guide's structure and index it in `docs/contents.md`.
  Add matching `CHANGELOG.md` entries under an `Unreleased` heading.
- `docs/roadmap.md`: mark 7.1.2 and its three sub-items done; add two stubs
  for the user to accept or strike at review — per-rule severity levels (the
  reserved `rules` key) and recording clap `ValueEnum`/`possible_values` in the
  documentation IR.

Acceptance: `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
and `make test` pass; every changed example, path, and identifier is checked
against the code.

## Concrete steps

Run from the repository root. Capture every gate in a log, sequentially:

```bash
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-ortho-config-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-ortho-config-${BRANCH}.out"
cargo test --doc -p cargo-orthohelp 2>&1 | tee "/tmp/doctest-ortho-config-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-ortho-config-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-ortho-config-${BRANCH}.out"
```

Focused loops during red and green:

```bash
cargo test -p cargo-orthohelp --lib policy::config 2>&1 | tail -20
cargo test -p cargo-orthohelp --lib policy::rules 2>&1 | tail -20
cargo test -p cargo-orthohelp --test vocabulary_policy_bdd 2>&1 | tail -20
cargo test -p cargo-orthohelp --test vocabulary_policy_e2e 2>&1 | tail -20
```

Expected red output for EP-M3 before implementation:

```plaintext
error[E0432]: unresolved import `super::vocabulary::check_vocabulary`
```

Expected green output after implementation:

```plaintext
test policy::rules::vocabulary_tests::flags_each_off_policy_spelling::case_1 ... ok
...
test result: ok.
```

## Validation and acceptance

The work is accepted when all of the following are observable:

- In `warn` mode, `check_vocabulary` over the fixture's real agent context
  reports exactly four `warn` findings, each with a `replacement` naming `get`,
  `list`, `--json`, or `--force`, and each message following its D14 template.
- In `deny` mode the same four findings are `deny` and `summary.deny` is 4.
- With a wire `verb` exception for `info` scoped to the `info` command's path,
  the `info` finding disappears and the other three remain.
- In `off` mode the report has no results.
- Existing golden snapshots and `--check-agent-native` behaviour are unchanged.
- `make check-fmt`, `make lint`, `make test`,
  `cargo test --doc -p cargo-orthohelp`, `make markdownlint`, and `make nixie`
  succeed.

## Idempotence and recovery

Every step is a file edit plus a test run; re-running is safe. Each milestone
is one or two commits, so `git revert` restores the previous plateau. The
end-to-end test writes only into a `tempfile::TempDir` and the existing
content-addressed bridge cache. If a gate is interrupted, re-run it; check
`target/` for a stale bridge-build marker before blaming the change.

## Interfaces and dependencies

At the end of EP-M3 these items exist:

```rust
// cargo-orthohelp/src/policy/config/scope.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionScope<'a> {
    pub kind: ExceptionKind,
    pub name: &'a str,
    pub command_path: Option<&'a str>,
}
impl<'a> ExceptionScope<'a> {
    #[must_use]
    pub fn covers(&self, kind: ExceptionKind, name: &str, path: &[String]) -> bool;
    #[must_use]
    pub fn from_wire(exception: &'a ortho_config::agent_context::PolicyException) -> Option<Self>;
}
impl<'a> From<&'a PolicyException> for ExceptionScope<'a> { /* … */ }

// cargo-orthohelp/src/policy/mod.rs
#[non_exhaustive]
pub struct PolicyResult {
    pub rule_id: String,
    pub code: String,
    pub severity: PolicySeverity,
    pub message: String,
    pub location: Option<SourceLocation>,
    /// Canonical spelling that should replace the offending one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}
impl PolicyResult {
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        code: impl Into<String>,
        severity: PolicySeverity,
        message: impl Into<String>,
    ) -> Self;
    #[must_use]
    pub fn with_replacement(self, replacement: impl Into<String>) -> Self;
    #[must_use]
    pub fn with_location(self, location: SourceLocation) -> Self;
}

// cargo-orthohelp/src/policy/rules/vocabulary.rs
#[must_use]
pub fn check_vocabulary(context: &ortho_config::AgentContext, mode: PolicyMode) -> PolicyReport;
```

No dependency is added.

## Revision note

- 2026-11-16: initial draft.
- 2026-11-16: applied the community-of-experts review. The rule group now
  takes `(context, mode)` and reads exceptions from agent context (D1, D7);
  enumerated values match without case and require the `enum(...)` override
  (D11, D12); the `text` mapping moves to 7.1.3 (D13); verbs are leaf-only and
  rule names follow the skill validator (D2); `PolicyResult` becomes
  non-exhaustive with constructors (D5); messages have fixed templates (D14);
  verification adds committed controls, monotonicity, and locality; the
  end-to-end test is a single spawn without a lock (D15); the API guide,
  doctests, changelog, and roadmap stubs join the plan; factual counts were
  corrected. Budgets were re-estimated upward. Remaining work: user approval,
  then EP-M1.
