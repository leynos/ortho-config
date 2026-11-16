# 7.1.2 Lint off-policy verbs and flags

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review and user approval; no implementation has
started)

Branch: `7-1-2-lint-off-policy-verbs-and-flags`

Roadmap item: `7.1.2` in `docs/roadmap.md`.

## Purpose / big picture

OrthoConfig helps Rust command-line interface (CLI) authors publish a
machine-readable description of their command tree, called *agent context*, so
that automated agents can drive the CLI safely. `cargo-orthohelp` is the Cargo
subcommand that generates that description and checks it against an opt-in
*agent-native policy*: a project-level rule set, configured in
`[package.metadata.ortho_config.policy]`, that holds a CLI to the canonical
command verbs and flags listed in `docs/agent-native-cli-design.md` §5. The
policy has three enforcement modes: `off` (check nothing), `warn` (report
findings without failing), and `deny` (report findings and fail).

Roadmap item 7.1.1 delivered the configuration, the report format, and the
canonical vocabulary, but no rule that looks at the commands themselves. Today
a project can write this and nothing complains:

```rust
#[derive(clap::Subcommand)]
enum Command {
    /// Show one widget.
    Info(InfoArgs),
    /// Show all widgets.
    Ls(LsArgs),
}

#[derive(clap::Args)]
struct ExportArgs {
    #[arg(long, value_enum)]
    format: Format, // accepts `json` and `yaml`
    #[arg(long)]
    skip_confirmations: bool,
}
```

After this change, the reusable vocabulary rule group,
`cargo_orthohelp::policy::rules::vocabulary::check_vocabulary`, reads the
command surface recorded in agent context and reports one finding per
off-policy spelling. Every finding names the canonical replacement both in its
message and in a new machine-readable `replacement` field:

```json
{
  "rule_id": "agent-native.vocabulary.off-policy-verb",
  "code": "off_policy_verb",
  "severity": "warn",
  "message": "command `widgets info` uses off-policy verb `info`; use `get`",
  "replacement": "get"
}
```

The five spellings the roadmap names are covered: the verbs `info` (replace with
`get`) and `ls` (replace with `list`), a `--format` option that accepts the
value `json` (replace with `--json`), an `--output` option that accepts the
value `json` (replace with `--json`), and `--skip-confirmations` (replace with
`--force`). In `warn` mode the findings are warnings; in `deny` mode they are
hard failures; in `off` mode nothing is evaluated. A configured exception, such
as a `verb` exception for `info` scoped to `widgets info`, suppresses exactly
the findings it covers and remains listed in the report.

The rule group is a library API, matching the existing behaviour rule group
from roadmap 7.2.1 (`check_behaviour`) and the output rule group from 7.2.3
(`check_output`). Roadmap item 7.2.8 owns the `cargo orthohelp check` command
that acquires command metadata once and composes every rule group into one
report; `docs/cargo-orthohelp-check-gen-design.md` §2.2 states that "later
vocabulary and other rules join when their own roadmap items land". The
existing configuration-only `--check-agent-native` operation keeps its
no-bridge-build guarantee and does not change.

Preserved behaviour: configuration-sanity findings, the report schema version
(`"1"`), existing golden snapshots, the `--check-agent-native` process
contract, and the agent-context wire format. Deliberately changed behaviour:
the shared off-policy table maps `--skip-confirmations` to `--force` instead of
`--no-input` (see Decision D4), which also changes the advice the
skill-manifest validator from 6.3.2 gives for that spelling.

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
  stay `"1"`. Report changes are additive and serde-defaulted.
- The canonical vocabulary (`cargo-orthohelp/src/policy/vocabulary/mod.rs`) and
  the off-policy table
  (`cargo-orthohelp/src/policy/vocabulary/global_options.rs`) each keep one
  owner. No rule restates either list.
- Glossary near-miss rules (`--output-format`, `--colour-policy`, and so on)
  belong to roadmap 7.1.3 and are out of scope.
- Per-rule severity levels (the reserved `rules` key) are out of scope; the
  strict configuration parser continues to reject `rules` (Decision D3).
- No Rust source file may exceed 400 lines (`AGENTS.md`). Every module starts
  with a `//!` comment. Every assertion carries a diagnostic message.
- Production code must not call `std::env::var`; tests that spawn
  `cargo-orthohelp` may configure only the child's environment.
- Authorized test dependencies are already present in
  `cargo-orthohelp/Cargo.toml`: `rstest` 0.28, `rstest-bdd` 0.6, `insta` 1,
  `proptest` 1, `googletest` 0.14, `pretty_assertions` 1, and `trybuild` 1. No
  new dependency is authorized.
- `PolicyResult` gains one public field. Pre-1.0 public API: update every
  in-repository constructor in the same change and document the struct-literal
  impact; add no compatibility shim.
- Completed ExecPlans under `docs/execplans/` are historical and must not be
  edited. The active 7.2.3 ExecPlan (PR #661) must not be edited either.
- Formatting, lint, and test gates run sequentially: `make check-fmt`,
  `make lint`, `make test`, plus `make markdownlint` and `make nixie` when
  Markdown changes.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` at the time implementation
starts (`git merge-base HEAD origin/main`). This ExecPlan file is excluded from
every count. Counting command, run per category:

```bash
git diff --numstat "$(git merge-base HEAD origin/main)" -- <paths> \
  ':(exclude)docs/execplans/7-1-2-lint-off-policy-verbs-and-flags.md'
```

Categories are assigned by whole file: code is `*.rs` and `*.feature`;
configuration is `*.toml` and `.config/**`; documentation is `docs/**/*.md`
other than ExecPlans. Insta `.snap` files count as code. Limits are net lines
(additions minus deletions) and are enforced independently.

- Files: more than 28 non-ExecPlan files changed → stop and escalate.
- Code: more than 2,000 net lines → stop and escalate.
- Configuration: more than 40 net lines → stop and escalate.
- Documentation: more than 500 net lines → stop and escalate.
- Interface: any public API change other than `check_vocabulary`, the
  `PolicyResult::replacement` field, and `PolicyException::covers` → escalate.
- Dependencies: any new crate → escalate.
- Architecture: wiring vocabulary rules into a CLI path, changing the
  configuration schema, or changing the agent-context schema → escalate.
- Iterations: a focused test still failing after three fix attempts →
  escalate.
- Time: a milestone taking more than one working day → escalate.
- Ambiguity: a second plausible reading of a diagnostic contract, exception
  scope, or replacement mapping → stop and present options.

Budget basis. The survey (see `Context and orientation`) estimates about 1,450
net code lines: the rule module (~190), unit tests (~280), property tests
(~300), behavioural feature and steps (~330), end-to-end test (~230), fixture
types (~80), exception predicate and refactor (~20), and the `replacement`
field (~20). History argues for contingency: the 7.1.1 plan raised its budget
from 1,200 to 1,400 lines, the 7.2.1 plan landed at 2,838 lines against 1,500
("low by roughly 2x"), and the 6.3.2 plan landed at 5,466 lines against 1,500
with maintainer approval. The 2,000-line code limit is the estimate plus about
40 per cent; a breach means the estimate is wrong and must be re-planned, not
silently absorbed. Documentation is estimated at 330 lines (ADR amendment,
design §5 and §8.1, users' and developers' guides, gap analysis, migration
note) with 50 per cent contingency. Configuration is a single nextest override
line and a fixture-type registration, so 40 lines is generous.

## Risks

- Risk: PR #661 (7.2.3) constructs `PolicyResult` with a struct literal, so
  whichever of the two lands second fails to compile until the other's literal
  gains `replacement: None`. Severity: low. Likelihood: high. Mitigation: add a
  `PolicyResult::new(rule_id, code, severity, message)` constructor in
  Milestone 2, mention it in the PR, and rebase onto `main` before final gates.
- Risk: a clap `global = true` option might be copied into every subcommand's
  `inputs`, so one `--format` would produce a finding per command. Severity:
  medium. Likelihood: low (`walk` reads only `meta.fields` per node).
  Mitigation: the end-to-end fixture declares the off-policy flags on one
  command and asserts the exact finding count.
- Risk: clap subcommand aliases (`visible_alias = "ls"`) are not represented in
  the documentation IR, so an `ls` alias of `list` goes unreported. Severity:
  low. Likelihood: certain. Mitigation: documented residual gap; representing
  aliases would change the IR schema, which is out of scope.
- Risk: an option declared as a free-form string (no enumerated values) may
  still accept `json` at runtime, and the rule cannot see it. Severity: low.
  Likelihood: medium. Mitigation: documented as a consequence of the
  no-inference rule; the users' guide tells authors to declare `value_enum` or
  `possible_values`.
- Risk: the bridge-backed end-to-end test is a cold-build cost under host
  saturation. Severity: medium. Likelihood: medium. Mitigation: reuse the
  existing `orthohelp_fixture` crate (no new workspace member) and add the new
  test binary to the existing `orthohelp_cache` nextest override.
- Risk: changing the shared `--skip-confirmations` replacement changes 6.3.2
  advice. Severity: low. Likelihood: certain. Mitigation: 6.3.2 is unreleased
  (it merged after the `v0.11.0` tag), the design text already names `--force`,
  and the change is a single table entry with a regression test.

## Progress

- [x] (2026-11-16 15:00Z) Reconnaissance by a Wyvern team: policy module,
  test infrastructure, design history.
- [x] (2026-11-16 15:40Z) Prior-art check: clig.dev, Heroku CLI style guide,
  Spectral ruleset severities.
- [x] (2026-11-16 16:10Z) Initial draft written.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Shared exception predicate extracted; skill validator delegates.
- [ ] `EP-M2` `PolicyResult::replacement` added; `--skip-confirmations`
  remapped.
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
  preferred destructive bypass flag is `--force`"). Impact: Decision D4.
- Observation: `check_behaviour` (7.2.1) has no production caller; it is a
  library rule group awaiting 7.2.8. Evidence: every call site is under
  `cargo-orthohelp/src/policy/rules/` tests. Impact: Decision D1 follows the
  same pattern.
- Observation: ADR-0008 D14 says 7.1.2 "passes the bridge IR through
  `PolicyInputs`". The later 7.2.8 design composes one report from separate
  per-group evaluators with per-group coverage. Impact: Decision D1 records an
  ADR amendment.
- Observation: the agent-context verb mapper sets `canonical_verb` only for
  already-canonical segments, so `info` yields `None`. Evidence:
  `cargo-orthohelp/src/agent_context/mod.rs:281`. Impact: the verb rule reads
  the final path segment, not `canonical_verb`.

## Decision log

- D1. Decision: deliver vocabulary linting as a separate rule group,
  `check_vocabulary(context: &AgentContext, config: &PolicyConfig) ->
  PolicyReport`,
  in `cargo-orthohelp/src/policy/rules/vocabulary.rs`, and leave
  `PolicyInputs` empty. Rationale: it matches `check_behaviour` (7.2.1) and
  `check_output` (7.2.3), which 7.2.8 composes group by group with per-group
  coverage (`docs/cargo-orthohelp-check-gen-design.md` §4). Folding command
  rules into `evaluate` would make "configuration only" and "configuration plus
  vocabulary" indistinguishable in coverage. The rule takes the whole
  `PolicyConfig`, not only the mode, because "existing exception semantics
  remain owned by the corresponding evaluators" (same design, §2.3). Strongest
  alternative: populate `PolicyInputs` with the command list, as ADR-0008 D14
  literally says; rejected for the coverage reason. Upstream impact: an
  amendment to ADR-0008 D14, recorded in Milestone 5. This is an architecture
  deviation from an accepted ADR and is presented for explicit approval with
  this plan.
- D2. Decision: two rules. `agent-native.vocabulary.off-policy-verb` (code
  `off_policy_verb`) fires when a non-root command's final path segment is an
  off-policy verb. `agent-native.vocabulary.off-policy-flag` (code
  `off_policy_flag`) fires when an input's long name is `--skip-confirmations`,
  or when `--format` or `--output` declares an enumerated value that the shared
  table maps (`json` → `--json`; `text` → `--plain`). One finding is emitted
  per mapped value. Rationale: the design's spellings are `--format=json` and
  `--output json`, not bare `--format`; `--output` is the conventional
  output-*file* flag (clig.dev: "`-o`, `--output`: Output file"), and a
  domain-specific `--format compact|rich` is the domain policy the gap analysis
  leaves open. The `text` mapping already exists in the shared table and keeps
  6.3.2 and 7.1.2 consistent. The root command is skipped because its segment
  is the binary name, not a verb.
- D3. Decision: do not implement per-rule levels now; the strict parser keeps
  rejecting `rules`. Rationale: the roadmap acceptance criteria name only mode
  and exceptions; per-rule levels add a configuration schema and a severity
  lattice with no current consumer. The reservation stays in force for a later
  item; ADR-0008 and `cargo-orthohelp/src/policy/config/mod.rs` doc comments
  change "reserved for 7.1.2" to "reserved for a later per-rule level item".
- D4. Decision: correct the shared table so `--skip-confirmations` maps to
  `--force`. Rationale: the design names `--force` twice; `--no-input` means
  "never prompt, and fail if input is needed", so a script that relied on
  auto-confirmation would start failing if an author followed the old advice.
  6.3.2 is unreleased, so no released behaviour changes.
- D5. Decision: add `replacement: Option<String>` to `PolicyResult` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`, plus a
  `PolicyResult::new` constructor and a `with_replacement` builder. Rationale:
  ADR-0008 D14 prescribes the field; skipping `None` keeps every existing
  golden snapshot byte-identical, and readers of older reports default to
  `None`.
- D6. Decision: extract exception matching into a
  `PolicyException::covers(kind, name, path)` method (signature under
  `Interfaces and dependencies`) in `cargo-orthohelp/src/policy/config/mod.rs`,
  and make `skill_validation::policy_check::ResolvedAgentPolicy::allows`
  delegate to it. Rationale: the abstraction policy in `AGENTS.md` requires
  reuse of the existing behaviour (`allows`, `names_match`, `scope_matches`);
  two call sites justify one shared predicate. Scope semantics are unchanged: no
  `command_path` means global; otherwise the space-joined path must match
  exactly; flag names compare after stripping leading `-`; verb names compare
  exactly.
- D7. Decision: the vocabulary report advertises the configured exceptions and
  the canonical vocabulary, like `evaluate`. Rationale: a standalone consumer
  must see the exceptions that silenced findings. 7.2.8 deduplicates when it
  composes groups.
- D8. Decision: findings are ordered by command path (stable sort, so equal
  paths keep input order), then verb before flags, then input declaration
  order, then enumerated-value declaration order. Rationale: deterministic
  reports and snapshot stability; stable sorting preserves distinct occurrences
  with equal paths, as the 7.2.8 design requires.
- D9. Decision: no Verus or Kani milestone. Rationale: see `Verification plan`.

## Outcomes & retrospective

Not started.

## Conformance basis

- Roadmap: `docs/roadmap.md` 7.1.2 at commit `eea7273f` (`RM-7.1.2-a` flag the
  five spellings under strict policy; `RM-7.1.2-b` report the canonical
  replacement in every diagnostic; `RM-7.1.2-c` tests for warning mode, deny
  mode, and configured exceptions).
- Design: `docs/agent-native-cli-design.md` §5 (`DES-5-strict`: flag
  off-convention aliases when strict mode is enabled; exceptions explicit and
  visible), §6.1 (`DES-6.1-bypass`: `--force` is the destructive bypass), and
  §8.1 (`DES-8.1-no-infer`: no inference from names; findings follow the
  selected mode).
- ADR: `docs/adrs/adr-0008-agent-native-policy-configuration.md`, including D3
  (structured exceptions), D9 (opt-in `off`), D11 (no bridge build for the
  configuration check), D14 (evaluator seam and `replacement` field), and the
  2026-11-14 amendment.
- Design amendment: `docs/cargo-orthohelp-check-gen-design.md` §2.2, §4, and
  §5 (`CG-groups`: rule groups join `check` when their items land;
  `CG-coverage`: per-group coverage).
- Gap analysis: `docs/ddlint-gap-analysis.md` "Next steps" item 1
  (`GAP-1`: treatment of domain-specific `--format` flags).
- No Terms of Reference document exists for this work.

Trace links:

```plaintext
RM-7.1.2-a -> DES-5-strict -> D2 -> EP-M3 -> policy::rules::vocabulary_tests::flags_each_off_policy_spelling
RM-7.1.2-a -> DES-6.1-bypass -> D4 -> EP-M2 -> policy::vocabulary::tests::skip_confirmations_maps_to_force
RM-7.1.2-b -> ADR-0008-D14 -> D5 -> EP-M2, EP-M3 -> vocabulary_proptests::every_finding_names_a_canonical_replacement
RM-7.1.2-c -> DES-5-strict -> D6 -> EP-M1, EP-M3, EP-M4 -> tests/features/vocabulary_policy.feature
DES-8.1-no-infer -> D2 -> EP-M3 -> vocabulary_tests::undeclared_values_are_not_flagged
GAP-1 -> D2 -> EP-M3, EP-M5 -> vocabulary_tests::domain_format_values_are_left_alone
CG-groups, CG-coverage -> D1, D7 -> EP-M3, EP-M5 -> ADR-0008 amendment
```

## Context and orientation

The repository is a Cargo workspace. The two crates involved are `ortho_config`
(the library that owns reusable agent-context types) and `cargo-orthohelp` (the
generator and policy checker). Key files:

- `ortho_config/src/agent_context/mod.rs` defines `AgentContext`,
  `AgentCommand` (with `path: Vec<String>` and `inputs: Vec<AgentInput>`), and
  `AgentInput` (with `long: Option<String>` and `enum_values: Vec<String>`).
  These are the rule group's only input besides configuration.
- `cargo-orthohelp/src/agent_context/mod.rs` builds an `AgentContext` from the
  bridge's documentation IR. `walk` emits one `AgentCommand` per command node,
  root first; the root path segment is the binary name.
  `cargo-orthohelp/src/agent_context/input.rs` fills `enum_values` from a
  `ValueEnum` or `possible_values`, and leaves booleans empty.
- `cargo-orthohelp/src/policy/mod.rs` (262 lines) defines `PolicyReport`,
  `PolicyResult`, `PolicySeverity`, and `PolicyMode`.
- `cargo-orthohelp/src/policy/config/mod.rs` (160 lines) defines
  `PolicyConfig`, `PolicyException`, `ExceptionKind`, and the strict parser.
- `cargo-orthohelp/src/policy/evaluate/mod.rs` (169 lines) holds the
  configuration-sanity rules (`agent-native.config.*`).
- `cargo-orthohelp/src/policy/rules/behaviour.rs` (212 lines) is the 7.2.1 rule
  group; its shape (total function, `Rule` constants, `RuleSeverity`) is the
  model for the new module. `rules/proptests.rs` is the model for oracle-based
  property tests.
- `cargo-orthohelp/src/policy/vocabulary/mod.rs` owns `CANONICAL_VERBS` and
  `CANONICAL_FLAGS`; `vocabulary/global_options.rs` (258 lines) owns the
  off-policy table and `off_policy_flag_replacement` /
  `off_policy_verb_replacement`.
- `cargo-orthohelp/src/skill_validation/policy_check.rs` (293 lines) is the
  existing consumer and holds the exception matcher to extract.
- `tests/fixtures/orthohelp_fixture/src/lib.rs` (330 lines) is the real
  workspace fixture crate; it already serves two root types selected with
  `--root-type`.
- `cargo-orthohelp/tests/nested_subcommand_end_to_end.rs` and
  `cargo-orthohelp/tests/agent_context_renderer_end_to_end.rs` show how an
  end-to-end test spawns the real binary against that fixture.
- `cargo-orthohelp/tests/skill_validation_bdd.rs` and
  `cargo-orthohelp/tests/skill_validation_bdd/steps.rs` show a library-level
  `rstest-bdd` harness; `cargo-orthohelp/tests/features/` holds feature files.
- `.config/nextest.toml` groups bridge-building test binaries in the
  `orthohelp_cache` override; `tests/workflow_contracts` asserts that every
  binary that spawns a child Cargo is covered.
- `Makefile` provides `make verus` and `make kani`, both with empty proof
  registries (`VERUS_PROOF_FILES`, `KANI_*_HARNESSES`). No proof exists yet.

Survey for budgets: the change touches about 22 files (4 production, 9 test, 1
fixture, 1 nextest, 7 documentation). In-repository `PolicyResult` struct
literals number six, across `policy/tests.rs` (two), `policy/evaluate/mod.rs`
(two), `policy/config/tests.rs`, and `policy/rules/behaviour.rs`.

Skills to load while implementing: `rust-router` (then `rust-unit-testing`,
`proptest`, and `rust-types-and-apis` as needed), `execplans`, `nextest`,
`en-gb-oxendict-style`, and `arch-decision-records` for the ADR amendment.
Documentation to read first: `docs/agent-native-cli-design.md` §5, §6.1, §8.1;
`docs/adrs/adr-0008-agent-native-policy-configuration.md`;
`docs/cargo-orthohelp-check-gen-design.md`;
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

- C1 (mode gate). `off` returns zero results without evaluating any rule;
  `warn` makes every vocabulary finding `warn`; `deny` makes every finding
  `deny`; the findings, ignoring severity, are identical in `warn` and `deny`.
- C2 (detection, relative to declared metadata). For every non-root command
  whose final segment is `info` or `ls`, exactly one verb finding is emitted.
  For every input whose long name is `skip-confirmations`, exactly one flag
  finding is emitted. For every input whose long name is `format` or `output`,
  exactly one flag finding is emitted per declared enumerated value that the
  table maps. Nothing else is emitted.
- C3 (replacement totality). Every vocabulary finding has
  `replacement = Some(r)` where `r` is a canonical verb, a canonical flag, or a
  glossary canonical name (`--plain`), and the message contains `r`.
- C4 (exception soundness and completeness). A candidate finding for
  (kind, name, path) is suppressed if and only if some configured exception
  `covers` it (D6 semantics).
- C5 (determinism). Equal inputs give equal reports; reordering commands with
  distinct paths does not change the report.
- C6 (no inference). Undeclared values, free-form options, booleans, and
  domain-specific values (`compact`, `yaml`) never produce findings.
- C7 (wire compatibility). A result without a replacement serializes exactly
  as before; a report written before this change deserializes with
  `replacement = None`.
- C8 (refactor preservation). Skill-manifest validation findings are
  unchanged except for the deliberate D4 remapping.

Axioms (external assumptions, not verified here): clap and the OrthoConfig
derive faithfully record subcommand names, long names, and `ValueEnum` variants
in the documentation IR; serde honours `skip_serializing_if` and `default`;
`AgentContext` deserializes the generator's own output. The end-to-end test
exercises these against the real pipeline rather than assuming them for the
fixture.

Lemmas (intermediate claims connecting the rules to the contracts):

- L1. `PolicyException::covers` equals the old `allows` predicate for every
  input. Supports C4 and C8.
- L2. For each table entry, the mapped replacement is canonical. Supports C3.
  The domain is finite (two verbs, three flags, two values).
- L3. Candidate generation is independent of mode and exceptions; mode and
  exceptions act only as a filter and a severity map afterwards. Supports C1
  and C4, and is enforced structurally: `check_vocabulary` computes candidates,
  filters with `covers`, then maps severity.

Methods, artefacts, and discharge:

- C1, C2, C4, C5: oracle-based property tests in
  `cargo-orthohelp/src/policy/rules/vocabulary_proptests.rs`, modelled on
  `rules/proptests.rs`. The generator draws command paths from a pool mixing
  canonical verbs, off-policy verbs, and neutral words; inputs from a pool of
  long names including `format`, `output`, `skip-confirmations`, canonical
  flags, and neutral names, with enumerated values drawn from `json`, `text`,
  `yaml`, `compact`, and empty; exceptions from the same pools with and without
  scopes. The oracle is an independent, list-comprehension restatement of C2
  and C4 that does not call `covers` or the table functions; it enumerates the
  five spellings literally. Command:
  `cargo test -p cargo-orthohelp vocabulary_proptests`. Expected evidence: all
  properties pass at the default 256 cases. Discharge: pass plus the
  non-vacuity checks below. Non-vacuity: a deterministic witness test asserts
  that a fixed seed produces at least one case in each class (verb finding,
  flag finding per value, exception-suppressed finding, scoped exception on
  another path, domain value without a finding). Negative control: during
  Milestone 3, temporarily make `covers` ignore `command_path` and record that
  the C4 property fails with a shrunk counterexample; revert, and record the
  transcript in `Surprises & discoveries`.
- C2, C6 named cases: an `rstest` table in
  `cargo-orthohelp/src/policy/rules/vocabulary_tests.rs` covering each
  spelling, each value class, the root command named `ls`, a free-form
  `--format`, a path-typed `--output`, and a boolean. Each row asserts the exact
  `(rule_id, code, replacement)` list with `googletest` matchers.
- C3, L2: an `rstest` case per table row in
  `cargo-orthohelp/src/policy/vocabulary/tests.rs` asserting the replacement is
  canonical; because the table is finite, this enumeration is exhaustive, not a
  sample. A property in `vocabulary_proptests.rs` asserts the message contains
  the replacement for every generated finding.
- C7: unit tests in `cargo-orthohelp/src/policy/tests.rs` deserializing a
  version-1 report without `replacement`, and asserting a `None` result
  serializes with no `replacement` key; the existing golden snapshots must stay
  unchanged, which `cargo insta test` would otherwise report.
- C8, L1: a property in `cargo-orthohelp/src/skill_validation/proptests.rs`
  (or a sibling file if that exceeds 400 lines) comparing `covers` with a
  restatement of the old `names_match`/`scope_matches` logic kept in the test;
  plus the existing skill-validation suites, which must pass unchanged apart
  from the D4 assertion.
- Report shape: an `insta` JSON snapshot
  `vocabulary_report__every_rule` in `rules/snapshots/` covering one finding
  per rule and value class in `warn` mode, plus a second snapshot for `deny`.
  Snapshots are paired with the semantic assertions above, so a snapshot can
  never be the only evidence for a rule.
- Real-pipeline evidence: the end-to-end test in Milestone 4.

Why no Verus or Kani (D9). Verus is warranted for a lemma whose guarantee must
hold over an unbounded domain and whose proof would not merely restate the
implementation. Here, L2 and the table mappings are finite and are enumerated
exhaustively by `rstest`; L1 and C4 are definitional (the exception predicate
*is* the specification of "covered"), so a Verus model would restate the code
it claims to verify; and C1, C3, and L3 are structural properties of a
straight-line filter-and-map that oracle-based property tests reject when
broken, as the negative control demonstrates. Verus also cannot check the
production `String`-based code directly; it would verify a mirrored model,
leaving the correspondence unproven. Kani would add bounded exploration of the
same finite tables that `rstest` already enumerates. This matches the precedent
of 7.1.1, 7.2.1, 6.3.2, and 7.2.2, all of which declined formal tools for
table-driven classifiers. If review identifies an unbounded invariant not
covered above, revise this section before implementation.

## Plan of work

Each milestone follows red, green, refactor. Each ends with the full gates run
sequentially and a commit.

### Milestone EP-M1 — shared exception predicate (refactor)

Requirements: `RM-7.1.2-c` (exceptions), D6. In
`cargo-orthohelp/src/policy/config/mod.rs`, add
`PolicyException::covers(&self, kind, name, path) -> bool` with a doc example.
Red: add `rstest` cases in `cargo-orthohelp/src/policy/config/tests.rs` for
global, scoped-match, scoped-mismatch, kind-mismatch, and `--` normalization;
they fail to compile until the method exists. Green: implement by moving the
logic from `skill_validation/policy_check.rs` (`names_match`, `scope_matches`)
and make `ResolvedAgentPolicy::allows` call it. Refactor: delete the moved
helpers. Add the L1 equivalence property. End state: identical behaviour, one
predicate. Acceptance: the new cases pass; every existing skill-validation test
passes unchanged. Recovery: revert the single commit. Conformance check: no
public behaviour change; `covers` is the only new public item.

### Milestone EP-M2 — replacement field and table correction

Requirements: `RM-7.1.2-b`, `RM-7.1.2-a` (for `--skip-confirmations`), D4, D5.
Red: in `cargo-orthohelp/src/policy/tests.rs`, add a test that a result built
with `with_replacement("--json")` serializes a `replacement` key, a test that a
`None` result has no such key, and a test that version-1 JSON without the key
deserializes; in `cargo-orthohelp/src/policy/vocabulary/tests.rs`, add
`skip_confirmations_maps_to_force` and the L2 rows. Green: add the field,
`PolicyResult::new`, and `with_replacement`; update the six struct literals to
use `new`; change the table entry and its doc example to `--force`. Extend
`cargo-orthohelp/tests/ui/policy_public_api.rs` to name the new items.
Refactor: none expected. Acceptance: new tests pass; existing golden snapshots
are byte-identical (`git diff --stat -- '*.snap'` is empty). No existing test
pins the `--skip-confirmations` replacement (only the doc example in
`global_options.rs` does), so the new `skip_confirmations_maps_to_force` case
and a new skill-validation case asserting `--force` advice are the regression
guards. Recovery: revert the commit. Conformance check: schema version
unchanged.

### Milestone EP-M3 — vocabulary rule group

Requirements: `RM-7.1.2-a`, `RM-7.1.2-b`, `RM-7.1.2-c`, D1, D2, D7, D8. Create
`cargo-orthohelp/src/policy/rules/vocabulary.rs` and register it in
`rules/mod.rs`. Red: write `vocabulary_tests.rs` (named `rstest` table,
warn/deny/off cases, exception cases) and the snapshot test first; they fail
because `check_vocabulary` does not exist. Green: implement as three small
functions — `candidates(context) -> Vec<Candidate>` (verb and flag candidates
from the table, independent of configuration), `is_excepted(config, candidate)`
using `covers`, and `to_result(candidate, severity)` building a `PolicyResult`
with `replacement` and a message of the form
``command `<path>` uses off-policy verb `<verb>`; use `<replacement>` `` or
``command `<path>` declares `--<flag> <value>`; use `<replacement>` ``.
`check_vocabulary` returns early in `off` mode with an empty report that still
advertises exceptions and vocabulary. Then add `vocabulary_proptests.rs` with
the C1–C5 properties, the witness test, and the negative control. Refactor:
keep each file under 400 lines; split tests if needed. Acceptance:
`cargo test -p cargo-orthohelp policy::rules::vocabulary` passes; snapshots
reviewed and accepted with `cargo insta review`; the negative-control
transcript is recorded. Recovery: the module is additive; revert the commit.
Conformance check: no CLI path calls `check_vocabulary`; no inference from
names beyond reporting the declared spelling.

### Milestone EP-M4 — behavioural and end-to-end evidence

Requirements: `RM-7.1.2-c`, D2, D6, C6. Behavioural tests: add
`cargo-orthohelp/tests/features/vocabulary_policy.feature`, a harness
`cargo-orthohelp/tests/vocabulary_policy_bdd.rs`, and steps in
`cargo-orthohelp/tests/vocabulary_policy_bdd/steps.rs`, modelled on the
skill-validation harness. Steps build an in-memory `AgentContext` and
`PolicyConfig` through `rstest` fixtures and call the public API. The feature
text is:

```gherkin
Feature: Off-policy vocabulary lint

  Scenario: Warn mode reports off-policy verbs with their replacements
    Given a command surface with commands "widgets info" and "widgets ls"
    And the policy mode is "warn"
    When the vocabulary rules check the command surface
    Then the report has a "warn" finding "off_policy_verb" for "widgets info" replaced by "get"
    And the report has a "warn" finding "off_policy_verb" for "widgets ls" replaced by "list"

  Scenario: Warn mode reports off-policy flags with their replacements
    Given a command "widgets export" declaring "--format" with values "json,yaml"
    And the command "widgets export" declaring "--output" with values "json"
    And the command "widgets export" declaring flag "--skip-confirmations"
    And the policy mode is "warn"
    When the vocabulary rules check the command surface
    Then the report has 3 findings, each naming its replacement
    And the replacements are "--json", "--json", and "--force"

  Scenario: Deny mode fails every off-policy spelling
    Given a command surface with commands "widgets info" and "widgets ls"
    And the policy mode is "deny"
    When the vocabulary rules check the command surface
    Then every finding has severity "deny"
    And the report summary counts 2 deny findings

  Scenario: A configured exception suppresses its finding and stays visible
    Given a command surface with commands "widgets info" and "widgets ls"
    And the policy mode is "deny"
    And a "verb" exception for "info" because "published v1 surface"
    When the vocabulary rules check the command surface
    Then the report has no finding for "widgets info"
    And the report lists the exception for "info" with its reason

  Scenario: A scoped exception covers only its command
    Given a command surface with commands "widgets info" and "gadgets info"
    And the policy mode is "warn"
    And a "verb" exception for "info" scoped to "widgets info"
    When the vocabulary rules check the command surface
    Then the report has a "warn" finding "off_policy_verb" for "gadgets info" replaced by "get"
    And the report has no finding for "widgets info"

  Scenario: Off mode evaluates nothing
    Given a command surface with commands "widgets info" and "widgets ls"
    And the policy mode is "off"
    When the vocabulary rules check the command surface
    Then the report has no findings

  Scenario: Domain-specific values and output paths are left to the project
    Given a command "widgets export" declaring "--format" with values "compact,rich"
    And the command "widgets export" declaring option "--output" without values
    And the policy mode is "deny"
    When the vocabulary rules check the command surface
    Then the report has no findings
```

End-to-end test: add a third root type, `VocabularyFixtureConfig`, to
`tests/fixtures/orthohelp_fixture` in a new module file
`tests/fixtures/orthohelp_fixture/src/vocabulary.rs` (keeping `lib.rs` under
400 lines). It declares subcommands `info` and `ls`, and an `export` subcommand
with `--format` (`ValueEnum`: `json`, `yaml`), `--output` (a path), and
`--skip-confirmations` (a boolean). Add
`cargo-orthohelp/tests/vocabulary_policy_e2e.rs`, which spawns the built
`cargo-orthohelp` binary with the arguments below, using only `Command::env`
for child configuration, parses `agent-context.json` into `AgentContext`, and
calls `check_vocabulary` in `warn` and `deny` modes and with a scoped
exception. It asserts exactly four findings (`info` → `get`, `ls` → `list`,
`--format json` → `--json`, `--skip-confirmations` → `--force`) and none for
the path-typed `--output`. Add the binary to the `orthohelp_cache` override in
`.config/nextest.toml`. Acceptance: the feature and the end-to-end test pass
under `make test`; the `tests/workflow_contracts` suite passes. Recovery:
revert the commit; the fixture type is additive and does not change other root
types' snapshots. Conformance check: the test proves the real derive pipeline
exposes the metadata the rules need (axioms above).

The end-to-end invocation is:

```bash
cargo-orthohelp orthohelp --package orthohelp_fixture \
  --root-type orthohelp_fixture::VocabularyFixtureConfig \
  --locale en-US --format agent-context --out-dir "$TEMPDIR"
```

### Milestone EP-M5 — documentation and closure

Requirements: all; D1, D3, D4, D5. Update:

- `docs/adrs/adr-0008-agent-native-policy-configuration.md`: append an
  amendment (dated at implementation) recording D1 (rule group instead of
  `PolicyInputs`), D3 (reservation deferred), and D5 (field shape).
- `docs/agent-native-cli-design.md` §5: the rule identifiers, the
  value-sensitive flag semantics, the `--force` replacement, the no-inference
  residual gaps (aliases, free-form values); §8.1 lines 811-816: replace
  "planned for roadmap 7.1.2" with the delivered behaviour.
- `docs/ddlint-gap-analysis.md` "Next steps" item 1: record the decision that
  domain-specific `--format` values are not flagged; only `json` and `text` are.
- `docs/cargo-orthohelp-check-gen-design.md` §6: one sentence naming
  `check_vocabulary(context, config)` as the vocabulary group 7.2.8 composes.
- `docs/users-guide.md` "Agent-native policy checking": replace the "planned
  for a later release" wording with the two rules, the `replacement` field, an
  exception example, the advice to declare enumerated values, and a note that
  the CLI runs the group once `cargo orthohelp check` lands (7.2.8).
- `docs/developers-guide.md` "Policy configuration test layout" and
  "Skill-manifest validation internals": the shared `covers` predicate, its
  re-use policy (the only exception matcher; rule groups and validators call
  it, never restate it), and the new test files.
- A migration note for the next release covering `PolicyResult::replacement`,
  `PolicyResult::new`, and the `--skip-confirmations` advice change. Add it to
  the unreleased migration guide if one exists at implementation time;
  otherwise create `docs/v0-12-0-migration-guide.md` following the v0.11.0
  guide's structure and index it in `docs/contents.md`.
- `docs/roadmap.md`: mark 7.1.2 and its three sub-items done.

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
make markdownlint 2>&1 | tee "/tmp/markdownlint-ortho-config-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-ortho-config-${BRANCH}.out"
```

Focused loops during red and green:

```bash
cargo test -p cargo-orthohelp --lib policy::config 2>&1 | tail -20
cargo test -p cargo-orthohelp --lib policy::rules::vocabulary 2>&1 | tail -20
cargo test -p cargo-orthohelp --test vocabulary_policy_bdd 2>&1 | tail -20
cargo test -p cargo-orthohelp --test vocabulary_policy_e2e 2>&1 | tail -20
```

Expected red output for Milestone 3 before implementation:

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
  `list`, `--json`, or `--force`, and the process-level gates pass.
- In `deny` mode the same four findings are `deny` and `summary.deny` is 4.
- With a `verb` exception for `info` scoped to the `info` command's path, the
  `info` finding disappears, the exception appears in `report.exceptions`, and
  the other three findings remain.
- Existing golden snapshots and `--check-agent-native` behaviour are unchanged.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` succeed.

## Idempotence and recovery

Every step is a file edit plus a test run; re-running is safe. Each milestone
is one or two commits, so `git revert` restores the previous plateau. The
end-to-end test writes only into a `tempfile::TempDir` and the existing
content-addressed bridge cache. If a gate is interrupted, re-run it; check
`target/` for a stale bridge-build marker before blaming the change.

## Interfaces and dependencies

At the end of Milestone 3 these items exist:

```rust
// cargo-orthohelp/src/policy/config/mod.rs
impl PolicyException {
    #[must_use]
    pub fn covers(&self, kind: ExceptionKind, name: &str, path: &[String]) -> bool;
}

// cargo-orthohelp/src/policy/mod.rs
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
    pub fn new(rule_id: &str, code: &str, severity: PolicySeverity, message: String) -> Self;
    #[must_use]
    pub fn with_replacement(self, replacement: &str) -> Self;
}

// cargo-orthohelp/src/policy/rules/vocabulary.rs
#[must_use]
pub fn check_vocabulary(context: &ortho_config::AgentContext, config: &PolicyConfig) -> PolicyReport;
```

No dependency is added.

## Revision note

- 2026-11-16: initial draft. Awaiting the community-of-experts review and user
  approval.
