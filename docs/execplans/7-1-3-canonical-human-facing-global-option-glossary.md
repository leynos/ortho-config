# 7.1.3 Add the canonical human-facing global option glossary

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (initial draft; design review pending; no implementation has
started)

Branch: `7-1-3-canonical-human-facing-global-option-glossary`

Roadmap item: `7.1.3` in `docs/roadmap.md`.

## Purpose / big picture

OrthoConfig helps Rust command-line interface (CLI) authors publish a
machine-readable description of their command tree, called *agent context*
(`agent-context.json`), so that automated agents can drive the CLI safely.
`cargo-orthohelp` is the Cargo subcommand that generates that description and
owns an opt-in *agent-native policy*: rules, configured in
`[package.metadata.ortho_config.policy]`, that hold a CLI to the conventions in
`docs/agent-native-cli-design.md` §5. The policy has three modes: `off` (check
nothing), `warn` (report findings without failing), and `deny` (report findings
as hard failures).

Section 5 also defines a *human-facing global option glossary*: one canonical
long option for each presentation concept a CLI may offer, such as
`--color auto|always|never` for colour policy. A *near miss* is a legacy or
improvised spelling of one of those concepts, such as `--colour-policy`. The
glossary exists as data today, but only the skill-manifest validator reads it.
Nothing checks the CLI itself, so a project can ship this and hear nothing:

```rust
#[derive(clap::Parser, ortho_config::OrthoConfig /* , … */)]
struct Cli {
    /// Colour policy.
    #[ortho_config(value(type = "enum(auto, always, never)"))]
    colour_policy: ColourPolicy,
    /// Suppress emoji.
    no_emoji: bool,
    /// Show progress.
    progress: bool,
    /// Use accessible output.
    accessible: bool,
    /// Emit diagnostics as JSON.
    diag_json: bool,
}
```

After this change, the reusable rule group
`cargo_orthohelp::policy::rules::global_options::check_global_options(context, mode)`
reads the command surface recorded in agent context and reports each departure
from the glossary, naming the canonical usage both in the message and in the
machine-readable `replacement` field:

```json
{
  "rule_id": "agent-native.global-options.near-miss",
  "code": "global_option_near_miss",
  "severity": "warn",
  "message": "command `demo` declares `--no-emoji`, a near miss for the emoji global option; use `--emoji auto|always|never`",
  "location": null,
  "replacement": "--emoji auto|always|never"
}
```

A project that offers no emoji, no pager, and no progress display hears nothing
about those concepts: an absent concept is never a finding. A project that must
keep a legacy spelling records a `flag` exception in its policy table, and the
finding disappears while the exception stays visible in agent context.

Preserved behaviour: the report schema version (`"1"`), the agent-context and
documentation schemas, `--check-agent-native` (which stays configuration-only
and bridge-free), the configuration grammar, every existing golden snapshot,
and the skill validator's gating (it judges glossary spellings only where the
consumer states that the option is global). Deliberately changed behaviour: the
glossary types gain structure (D3), and the skill validator's advice for the
width and locale concepts names the placeholder (`--width <columns>`,
`--locale <locale>`), as design §5 writes it.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- `docs/agent-native-cli-design.md` §8.1 forbids inferring semantics from names
  or absent data. The rules may report a declared spelling and a declared value
  shape. They must never infer that a concept is supported, never report a
  missing option, and never treat an undeclared value set as a wrong one.
- The glossary has one owner,
  `cargo-orthohelp/src/policy/vocabulary/global_options.rs`. No rule restates a
  spelling, a value list, or a hint.
- `--check-agent-native` stays configuration-only and builds no bridge
  (ADR-0008 D11 and its 2026-11-14 amendment). This plan adds no CLI wiring;
  roadmap 7.2.8 owns `cargo orthohelp check` and composes rule groups.
- `ORTHO_POLICY_REPORT_SCHEMA_VERSION` and `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`
  stay `"1"`. The `Vocabulary` wire struct is not extended (D10).
- The agent-context schema, the documentation intermediate representation (IR),
  the derive macros, and the policy configuration schema do not change.
- Off-policy verbs and the `--format json` / `--output json` /
  `--skip-confirmations` rules belong to roadmap 7.1.2 and are out of scope.
- No Rust source file may exceed 400 lines. Every module starts with a `//!`
  comment. Every assertion carries a diagnostic message. Clippy and Whitaker
  findings are fixed, not silenced (the cognitive-complexity gate is 9).
- Production code must not read the process environment. The end-to-end test
  configures only its child process. No new process-wide lock is introduced.
- Authorized test dependencies are already declared in
  `cargo-orthohelp/Cargo.toml`: `rstest` 0.28, `rstest-bdd` 0.6, `insta` 1,
  `proptest` 1, `googletest` 0.14, `pretty_assertions` 1, and `trybuild` 1. No
  new dependency is authorized.
- Public API: pre-1.0, so callers change together and no compatibility shim is
  added. `GlobalOptionConcept` is unreleased (6.3.2 merged after `v0.11.0`), so
  reshaping it has no published consumer.
- Completed ExecPlans are historical and must not be edited. The active 7.1.2
  (PR #705) and 7.2.4 (PR #732) ExecPlans must not be edited from this branch.
- Gates run sequentially: `make check-fmt`, `make lint`, `make test`, plus
  `make markdownlint` and `make nixie` when Markdown changes. `make test` skips
  Rust doctests, so each milestone that touches a doc example also runs
  `cargo test --doc -p cargo-orthohelp`.

## Tolerances (exception triggers)

Comparison base: the merge base with `origin/main` when implementation starts.
This ExecPlan is excluded from every count. Counting command, per category:

```bash
git diff --numstat "$(git merge-base HEAD origin/main)" -- <paths> \
  ':(exclude)docs/execplans/7-1-3-canonical-human-facing-global-option-glossary.md'
```

Categories are assigned by whole file: code is `*.rs`, `*.feature`, and
`*.snap`; configuration is `*.toml` and `.config/**`; documentation is `*.md`
other than ExecPlans. Limits are net lines (additions minus deletions) and are
enforced independently.

- Files: more than 34 non-ExecPlan files changed → stop and escalate.
- Code: more than 2,700 net lines → stop and escalate.
- Configuration: more than 40 net lines → stop and escalate.
- Documentation: more than 650 net lines → stop and escalate.
- Interface: any public API change beyond those in
  `Interfaces and dependencies` → escalate.
- Dependencies: any new crate → escalate.
- Architecture: CLI wiring, a schema change, or a configuration change →
  escalate.
- Table: adding or removing a near-miss spelling beyond the table in
  `Interfaces and dependencies` → escalate.
- Iterations: a focused test still failing after three fix attempts → escalate.
- Time: a milestone taking more than one working day → escalate.
- Ambiguity: a second plausible reading of reach, exception scope, or value
  comparison → stop and present options.

Budget basis. The survey estimates about 1,900 net code lines: table types and
hint (~110), shared foundation when 7.1.2 has not landed it (~220), rule module
(~190), unit tables (~320), property tests and committed controls (~400),
behavioural feature and steps (~260), end-to-end test and fixture (~300), and
snapshots (~100). Three comparable plans overran: 7.1.1 went from 1,200 to
1,400 lines, 7.2.1 landed at 2,838 against 1,500, and 6.3.2 landed at 5,466
against 1,500. In each, tests and oracles dominated and were under-itemized, so
they are itemized here, with about 40 per cent contingency. Documentation is
estimated at 430 lines with 50 per cent contingency. Configuration is one
nextest filter entry.

## Risks

- Risk: 7.1.2 (PR #705) is an unmerged draft that plans the same `PolicyResult`
  changes and the same exception matcher, and edits `global_options.rs`.
  Severity: medium. Likelihood: high. Mitigation: EP-M1 is conditional. It is
  specified to the same contract as 7.1.2 D5 and D6, so whichever item lands
  second skips it and runs its acceptance tests instead (D2).
- Risk: a domain option on a subcommand that shares a glossary spelling
  (`translate --language fr`, `export --columns a,b`, `paint --colour red`) is
  flagged, so teams disable the policy. Severity: medium. Likelihood: medium.
  Mitigation: reach (D4): single dictionary words are judged only on the root
  command; only spellings with no plausible domain reading are judged
  everywhere.
- Risk: single-command CLIs, whose root command carries both global and domain
  options, see ambiguous spellings flagged (`cut --columns`). Severity: low.
  Likelihood: medium. Mitigation: the flag exception recipe in the users'
  guide; the limit is recorded in the ADR.
- Risk: authors who use clap `value_enum` without
  `#[ortho_config(value(type = "enum(...)"))]` get no value-set finding.
  Severity: low. Likelihood: high. Mitigation: documented; a test row pins the
  silence so it is a decision, not an accident.
- Risk: short-only options, clap aliases, and `hide_in_help` fields never reach
  agent context, so near misses there go unreported. Severity: low. Likelihood:
  certain. Mitigation: documented. It also gives projects a migration path:
  keep the legacy spelling hidden for a release.
- Risk: adding a root type to `orthohelp_fixture` changes its fingerprint, so
  every bridge-backed test binary builds cold once. Severity: medium.
  Likelihood: certain. Mitigation: one new binary, one spawn, inside the
  existing `orthohelp_cache` nextest group.
- Risk: the next free ADR number is contested (PRs #588 and #723 both add an
  ADR-0026, and the 7.2.4 plan reserves it). Severity: low. Likelihood: high.
  Mitigation: take the next free number on `main` at implementation time.

## Progress

- [x] (2026-11-24 12:30Z) Reconnaissance by a Wyvern team: option-metadata
  pipeline, design history, and test and proof infrastructure.
- [x] (2026-11-24 12:45Z) Prior-art check: clig.dev, the Cargo book, Rain's
  Rust CLI recommendations, `clap-verbosity-flag`, GitHub CLI accessibility,
  and the current Netsuke command surface.
- [x] (2026-11-24 13:10Z) Initial draft written.
- [ ] Community-of-experts review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Shared foundation (conditional).
- [ ] `EP-M2` Glossary table gains shape and reach.
- [ ] `EP-M3` `check_global_options` rule group with unit, snapshot, and
  property tests.
- [ ] `EP-M4` Behavioural feature and bridge-backed end-to-end test.
- [ ] `EP-M5` Documentation, ADR, and roadmap closure.

## Surprises & discoveries

- Observation: the glossary data already exists, added early by 6.3.2.
  Evidence: `cargo-orthohelp/src/policy/vocabulary/global_options.rs:34-101`;
  the 6.3.2 plan says what remains for 7.1.3 is "the bridge-IR lint wiring, not
  the glossary data". Impact: 7.1.3 adds a consumer and structure, not a table.
- Observation: the table cannot express three things the roadmap asks for.
  Evidence: `accepted_values` is documented as "empty for a boolean option", yet
  `--width` and `--locale` are also empty and take values; `replacement_hint`
  therefore omits the `<columns>` and `<locale>` placeholders that §5 writes;
  and no entry represents boolean `--progress`. Impact: D3.
- Observation: agent context has no global-option marker, and the derive never
  emits clap's `global = true` for user fields. Evidence:
  `ortho_config/src/agent_context/mod.rs:212-232`; the only `global = true` in
  the derive is the profile selector
  (`ortho_config_macros/src/derive/build/profile_flag.rs:84`). Impact: D4 must
  define "global" structurally.
- Observation: the users' guide promises, for skill validation, that "a domain
  command with its own `--color` is not judged by the glossary rule". Impact:
  the lint must keep that promise for canonical names (D4, D5).
- Observation: a `bool` field always reaches agent context as
  `value_type: "bool"` with no enumerated values, even though clap accepts
  `--flag=false`. Evidence: `cargo-orthohelp/src/agent_context/input.rs:73-91`.
  Impact: boolean `--progress` is detectable from declared metadata alone.
- Observation: Netsuke, the first planned consumer, already conforms:
  `--color auto|always|never`, `--emoji`, `--progress`, and
  `--accessibility auto|on|off` on its root command, matched without regard to
  case. Its older `diag_json`, `colour_policy`, and `no_emoji` spellings
  survive only in historical plans. Impact: the glossary is proven against a
  real consumer, and the near-miss list is grounded in real history.
- Observation: clig.dev lists `--no-color` as an expected colour switch, while
  Cargo, Netsuke, and Rain's recommendations use `--color auto|always|never`.
  Impact: D6.
- Observation: `googletest` is declared in `cargo-orthohelp` but unused in its
  Rust sources. Impact: this plan's matcher assertions are its first use there;
  model them on `ortho_config/tests/missing_required_display.rs`.
- Observation: no gate runs Rust doctests. Impact: explicit doctest commands.
- Observation: `make verus` and `make kani` exist but have empty proof
  registries. Impact: see `Formal verification decision`.

## Decision log

- D1. Decision: deliver the lint as its own rule group,
  `check_global_options(context: &AgentContext, mode: PolicyMode) -> PolicyReport`
  in `cargo-orthohelp/src/policy/rules/global_options.rs`. Rationale: the
  signature matches `check_behaviour`, `check_output`, and 7.1.2's planned
  `check_vocabulary`; 7.2.8 composes per-group reports and records per-group
  coverage, so "glossary evaluated" must be distinguishable from "verbs
  evaluated"; and a separate group can land in either order with 7.1.2.
  Alternative rejected: add candidates inside `check_vocabulary`, as the 7.1.2
  draft suggests in its D13. That would serialize the two items, although the
  roadmap says 7.1.3 requires only 7.1.1, and would merge two coverage groups.
- D2. Decision: make the shared foundation conditional. If `ExceptionScope` and
  `PolicyResult::replacement` are on `main` when work starts, EP-M1 is
  satisfied; otherwise EP-M1 delivers exactly the contract 7.1.2 specifies for
  them. Rationale: the roadmap dependency is 7.1.1 alone, and one matcher and
  one result shape must exist whichever lands first.
- D3. Decision: replace `accepted_values` with
  `OptionShape::{Switch, Policy { accepted, boolean_reach }, Value(placeholder)}`
  and replace `&[&str]` near misses with
  `NearMiss { spelling, reach, enumerates }`. Rationale: the roadmap's "boolean
  `--progress`" cannot be expressed as a spelling, and the documented meaning
  of an empty list is wrong for `--width` and `--locale`.
- D4. Decision: an option is *global* when it is declared by the root command,
  that is, when its command's path has exactly one segment. Each near miss
  carries a *reach*: `GlobalOnly` spellings are single dictionary words that
  also have domain meanings (`--text`, `--colour`, `--columns`, `--lang`,
  `--language`, `--silent`, `--accessible`) and are judged only on the root
  command; `AnyCommand` spellings join the concept to a presentation qualifier
  (`--colour-policy`, `--no-emoji`, `--terminal-width`) and are judged wherever
  they are declared. Rationale: root position is a structural fact recorded in
  agent context, not an inference from a name; "a domain command with its own
  `--color` is not judged" stays true; and a subcommand's `--no-emoji` is still
  caught, which matters because derived CLIs repeat options per subcommand
  rather than using clap's `global = true`. Alternatives rejected: judge every
  name everywhere (noisy); judge the root only (misses
  `export --output-format`); require a caller-supplied global list (7.2.8 has
  no such input); add a `global` marker to the IR (schema change, out of scope).
- D5. Decision: two rules. `agent-native.global-options.near-miss` (code
  `global_option_near_miss`) reports a near-miss spelling.
  `agent-native.global-options.value` (code `global_option_value`) reports a
  canonical `Policy` option (colour, emoji, progress, accessibility) that is
  declared as a boolean, within that concept's `boolean_reach`, or, on the root
  command, declares an enumerated set that differs from the accepted set
  ignoring order, duplicates, and ASCII case. Progress has
  `boolean_reach: AnyCommand`; the other three are `GlobalOnly`. Rationale: the
  codes mirror the skill validator's `skill.policy.global_option_near_miss` and
  `skill.policy.global_option_value`; a boolean is the declared form of "used
  without a value". Undeclared sets, custom types, and other declared types are
  silent.
- D6. Decision: extend the table with `--no-color`, `--no-colour`, and
  `--no-progress` (all `AnyCommand`). Rationale: the roadmap list is introduced
  by "such as" and names `--no-emoji`; flagging one negated switch and not its
  siblings would make the standard arbitrary. A boolean negation cannot express
  `auto`. Cost accepted: clig.dev recommends `--no-color`; a project that
  offers it for compatibility records an exception or hides the field.
- D7. Decision: `--output-format` is not reported when it declares enumerated
  values and none of them is `json` ignoring ASCII case (`NearMiss::enumerates`
  is `Some("json")`). Rationale: `convert --output-format png|webp` is a domain
  option, and advising `--json` there is wrong; this is the same value test
  7.1.2 applies to `--format` and `--output`. With no declared values it is
  reported, as design §5 asks.
- D8. Decision: the table is closed. No edit distance, stemming, or prefix
  match. Rationale: deterministic findings, and §8.1.
- D9. Decision: exceptions are the existing `flag` kind, read from
  `context.policy.exceptions`, matched by `ExceptionScope`: no `command_path`
  means everywhere, otherwise the space-joined path must be equal; leading
  dashes are ignored. Findings carry no `location`. Reports list no exceptions,
  like the sibling groups. Rationale: design §5 requires exceptions to be
  visible in generated context, so what suppresses a finding is what agents see;
  `evaluate` remains the owner of exception advertisement.
- D10. Decision: do not extend the `Vocabulary` block. Rationale: it would
  change every golden report; each finding already names its canonical usage.
- D11. Decision: `replacement` holds the glossary usage text
  (`--emoji auto|always|never`, `--width <columns>`, `--json`) for both rules.
  Rationale: for a value finding the bare option name would equal the offending
  name and say nothing; one field with one meaning is easier to consume.
- D12. Decision: renderer metadata (7.2.2) is not consulted. Rationale:
  declaring `renderer(human(colour = "auto"))` does not oblige a CLI to offer
  `--color`; requiring it would break the omission criterion, and ADR-0016 says
  renderer keys are not flags.
- D13. Decision: findings are ordered by command path (stable sort, so equal
  paths keep input order), then by input declaration order. At most one finding
  per input. Rationale: deterministic reports, as `check_output` does.
- D14. Decision: messages follow three fixed templates and never echo declared
  values (design §5):

  ```plaintext
  command `<path>` declares `--<spelling>`, a near miss for the <concept> global option; use `<usage>`
  command `<path>` declares `<canonical>` as a boolean; use `<usage>`
  command `<path>` declares values for `<canonical>` that differ from the glossary; use `<usage>`
  ```

- D15. Decision: no Verus or Kani milestone; see `Formal verification decision`.

## Outcomes & retrospective

Not started.

## Conformance basis

No Terms of Reference exists for this work. Upstream artefacts, all inspected at
`cbba0d03`:

- `docs/roadmap.md` 7.1.3: `RM-7.1.3-a` (standardize names when concepts are
  present), `RM-7.1.3-b` (lint near misses), `RM-7.1.3-c` (permit omission).
- `docs/agent-native-cli-design.md` §5 (`DES-5-glossary`, `DES-5-near-miss`,
  `DES-5-exceptions`, `DES-5-one-table`), §6.2.1 (`DES-6.2.1-renderer`:
  renderer metadata is a separate axis), and §8.1 (`DES-8.1-no-infer`).
- `docs/adrs/adr-0008-agent-native-policy-configuration.md`: D3 (structured
  exceptions), D11 (bridge-free check), D12 (exception visibility), D14
  (`replacement`), and the 2026-11-14 amendment.
- `docs/adrs/adr-0016-dual-renderer-metadata-surface.md`: renderer keys are not
  flags.
- `docs/cargo-orthohelp-check-gen-design.md` §2.2 and §4: rule groups join
  `check` when their items land; per-group coverage.
- The 7.1.2 ExecPlan draft at `4e6c2a5f` (PR #705), D5, D6, and D13. It is not
  approved, so it is a coordination input, not an authority.

Baseline gaps: `GAP-consumer` (nothing lints the CLI's own options),
`GAP-shape` (the table cannot express boolean forms or placeholders),
`GAP-scope` ("global" is undefined for agent context).

Trace links:

```plaintext
RM-7.1.3-a -> DES-5-glossary -> GAP-shape -> D3, D5 -> EP-M2, EP-M3 -> global_options_tests::value_shape_table
RM-7.1.3-b -> DES-5-near-miss -> GAP-consumer, GAP-scope -> D4, D6, D7 -> EP-M2, EP-M3, EP-M4 -> global_options_tests::near_miss_table
RM-7.1.3-c -> DES-8.1-no-infer -> D4, D12 -> EP-M3, EP-M4 -> global_options_proptests::omission_never_adds_a_finding
DES-5-exceptions -> D9 -> EP-M1, EP-M3, EP-M4 -> tests/features/global_option_policy.feature
DES-5-one-table -> D3, D8 -> EP-M2 -> vocabulary::global_options_tests::table_is_well_formed
```

## Context and orientation

The repository is a Cargo workspace. `ortho_config` owns the reusable
agent-context types. `cargo-orthohelp` is the generator and policy checker. Key
files at `cbba0d03`:

- `ortho_config/src/agent_context/mod.rs` defines `AgentContext` (`commands`,
  `policy`), `AgentCommand` (`path`, `inputs`, …), `AgentInput` (`name`,
  `long: Option<String>` without dashes, `value_type: Option<String>`,
  `enum_values: Vec<String>`, …), and the wire `PolicyException` (`kind`,
  `name`, optional `command_path`).
- `cargo-orthohelp/src/agent_context/mod.rs` builds an `AgentContext` from the
  bridge's documentation IR. `walk` emits one `AgentCommand` per node using
  only that node's own fields. The root is the only one-segment path.
- `cargo-orthohelp/src/agent_context/input.rs` maps fields to inputs. Booleans
  become `value_type: "bool"`. Enumerated values come only from
  `#[ortho_config(value(type = "enum(...)"))]` and keep the author's casing. A
  custom enum without that override becomes its type name with no values.
  `hide_in_help` fields are dropped.
- `cargo-orthohelp/src/policy/vocabulary/global_options.rs` (258 lines) owns
  `GLOBAL_OPTION_CONCEPTS`, its lookups, and the off-policy tables 7.1.2 uses.
- `cargo-orthohelp/src/policy/vocabulary/tests.rs` (59 lines) tests the
  canonical lists. The glossary itself has no dedicated test module.
- `cargo-orthohelp/src/skill_validation/policy_check.rs` (293 lines) is the
  existing consumer and holds the exception matcher (`allows`, `names_match`,
  `scope_matches`).
- `cargo-orthohelp/src/policy/rules/output.rs` (155 lines) and `behaviour.rs`
  (212 lines) are the rule-group models. `rules/output_proptests.rs` and
  `skill_validation/proptests_controls.rs` are the property-test and
  committed-control models. `rules/tests.rs` holds the `command(...)` and
  `input(...)` builders.
- `cargo-orthohelp/src/policy/evaluate/mod.rs` reports an exception as
  redundant when it names a `CANONICAL_FLAGS` entry. No `Policy` concept is in
  that list, so an exception for `--progress` is not reported as redundant.
- `tests/fixtures/orthohelp_fixture/src/lib.rs` (348 lines) is the real fixture
  crate; its root types are selected with `--root-type`.
- `cargo-orthohelp/tests/agent_context_output_end_to_end.rs` shows how to spawn
  the real binary. `cargo-orthohelp/tests/skill_validation_bdd.rs` and
  `skill_validation_bdd/steps.rs` show an in-process `rstest-bdd` harness.
- `.config/nextest.toml` groups bridge-building binaries in `orthohelp_cache`;
  `tests/workflow_contracts/child_cargo_tier_test.py` fails if a binary that
  spawns a child Cargo is missing from the raised allowances.
- `docs/users-guide.md` holds compiled examples; each has a
  `<!-- tested-example: id -->` marker and must be registered in
  `ortho_config/tests/documentation_examples_tests.rs` and, for Rust,
  `documentation_examples_rust_tests.rs` and
  `documentation_examples/guide_flows.rs`.

Non-test callers of the glossary API number one file (`policy_check.rs`). The
survey counted about 30 affected files: 6 production, 11 test, 2 fixture, 1
configuration, and 10 documentation.

Prior art (checked 2026-11-24). clig.dev: "Use standard names for flags, if
there is a standard", and names `--no-color`, `--plain`, `--json`, and
`--quiet`. The Cargo book uses `auto|always|never` for both `--color` and
`term.progress.when`. Rain's Rust CLI recommendations put `--color` and
`--verbose` in a root-level `GlobalOpts`. `clap-verbosity-flag` uses
`--quiet`/`--verbose`, repeatable. GitHub CLI exposes accessibility through
configuration and environment variables and has no flag, which supports the
omission rule. No surveyed tool ships a glossary lint to reuse.

Skills to load: `execplans`, `rust-router` (then `rust-types-and-apis` for the
table types, `rust-unit-testing`, and `proptest`), `nextest`,
`arch-decision-records`, and `en-gb-oxendict-style`. Use `codegraph-mcp` for
callers and impact. Documentation to read first:
`docs/agent-native-cli-design.md` §3.4.1, §5, §6.2.1, §8.1; `docs/design.md`;
`docs/cargo-orthohelp-design.md`; `docs/cargo-orthohelp-check-gen-design.md`;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`
(the current name of the rstest-bdd guide); `docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/documentation-style-guide.md`; and `docs/developers-guide.md` "Policy
configuration test layout" and "Policy rule API testing". Reviewed without a
change needed: `docs/cli-localization-design.md`,
`docs/localizable-rust-libraries-with-fluent.md`, and
`docs/improved-error-message-design.md` (policy messages are English machine
diagnostics), `docs/clap-dispatch-and-ortho-config-integration.md` (subcommand
structure), and `docs/scripting-standards.md` (no script is added).

## Verification plan

### Abstract model

An *option* is a triple `(name, scope, declared)`. `name` is a long name
without leading dashes. `scope` is `Global` when the command path has one
segment, else `Local`. `declared` is `Boolean`, `Enumerated(S)` for a non-empty
set `S` of case-folded strings, or `Other` (everything else, including
undeclared). A *table* `T` is a finite set of concepts, each with a canonical
name, a shape, and a set of near misses `(spelling, reach, enumerates)`.
`reaches(AnyCommand, _)` holds; `reaches(GlobalOnly, s)` holds when
`s = Global`.

`verdict(o)` is:

- `NearMiss(c)` when some near miss `n` of `c` has `n.spelling = o.name`,
  `reaches(n.reach, o.scope)`, and not (`n.enumerates = Some(v)`,
  `o.declared = Enumerated(S)`, and `v ∉ S`);
- `Value(c)` when `c` is a policy concept, `o.name = c.canonical`, and either
  `o.declared = Boolean` and `reaches(c.boolean_reach, o.scope)`, or
  `o.declared = Enumerated(S)`, `o.scope = Global`, and `S ≠ c.accepted`;
- `None` otherwise.

`report(commands, exceptions, mode)` is empty in `off`. Otherwise, for each
command in stable path order and each input in declaration order, it contains
one finding when `verdict` is not `None` and no exception covers
`(Flag, name, path)`, with severity given by `mode`. An input with no long name
has no verdict. Malformed or unknown wire exception kinds cover nothing.

### Invariants and lemmas

- `INV-mode`. `off` gives no results; `warn` and `deny` give the same findings
  with severity `warn` and `deny`.
- `INV-exact`. The findings are exactly those of the model: none missing, none
  extra. (`RM-7.1.3-a`, `RM-7.1.3-b`)
- `INV-omission`. Deleting any input or any command from a context never adds a
  finding, and a context that declares no glossary-related option has none.
  (`RM-7.1.3-c`)
- `INV-exception`. A finding is suppressed if and only if a wire exception
  covers it; adding an exception never adds a finding.
- `INV-locality`. The findings for two contexts with disjoint command paths,
  joined, are the multiset union of their findings. 7.2.8 relies on this.
- `INV-order`. Equal inputs give equal reports; permuting commands with
  distinct paths changes nothing.
- `INV-message`. Each message equals its D14 template, `replacement` equals the
  concept's usage text, and no declared enumerated value appears in either
  unless it is part of the usage text.
- `INV-skill`. Skill-manifest findings are unchanged except for the width and
  locale usage text and the three D6 spellings.
- `L-table`. Canonical names are distinct; near-miss spellings are distinct
  across the whole table; no spelling is both canonical and a near miss; no
  glossary spelling is in the 7.1.2 off-policy flag table; no near miss is in
  `CANONICAL_FLAGS`; each accepted set is non-empty, lowercase, and free of
  duplicates.
- `L-functional`. `verdict` is a partial function: at most one concept applies.
  Follows from `L-table`.
- `L-converge`. Following the advice converges in one step: for every concept,
  the canonical name with its recommended declaration (`Boolean` for `Switch`,
  `Enumerated(accepted)` for `Policy`, `Other` for `Value`) has no verdict in
  either scope.
- `L-pointwise`. `verdict` reads only `(name, scope, declared)`, and
  suppression reads only `(name, path, exceptions)`. `INV-omission` and
  `INV-locality` follow. The implementation enforces this structurally: the
  classifier takes exactly those arguments and no context.
- `L-disjoint`. No input is flagged by both this group and 7.1.2's flag rule.
  Follows from `L-table`.

### Axioms (trusted, not verified here)

- `AXIOM-derive`: the derive records long names, booleans, and `enum(...)`
  values in the IR. Exercised for the fixture by the EP-M4 end-to-end test.
- `AXIOM-root`: only the root command has a one-segment path
  (`agent_context::command_path`). Exercised by the same test.
- `AXIOM-policy`: `apply_policy_to_context` copies configured exceptions into
  agent context.
- `AXIOM-serde`: `serde` honours `default` and `skip_serializing_if`.

### Methods per obligation

- `L-table`, `L-functional`, `L-converge`, `L-disjoint`. Method: exhaustive
  `rstest`/iteration over the finite table in
  `cargo-orthohelp/src/policy/vocabulary/global_options_tests.rs`. Rationale:
  the domain is a compile-time constant, so enumeration is a complete proof.
  Command: `cargo test -p cargo-orthohelp --lib policy::vocabulary`.
  Non-vacuity: the well-formedness checks are pure functions over
  `&[GlobalOptionConcept]`, and committed controls pass deliberately broken
  tables (a duplicate spelling, a spelling that is also canonical, an
  off-policy overlap, an empty accepted set) and assert that each is rejected
  for the named reason.
- `INV-exact` named cases. Method: `rstest` tables in
  `cargo-orthohelp/src/policy/rules/global_options_tests.rs`: `near_miss_table`
  (every spelling × both scopes × boolean / declared values / undeclared) and
  `value_shape_table` (each policy concept × scope × boolean, exact set,
  permuted set, case-varied set, subset, superset, disjoint set, custom type,
  undeclared). The expected column is written from the model and does not read
  the production table. Assertions use `googletest` matchers and
  `pretty_assertions`.
- `INV-mode`, `INV-exact`, `INV-omission`, `INV-exception`, `INV-locality`,
  `INV-order`, `INV-message`. Method: `proptest` in
  `cargo-orthohelp/src/policy/rules/global_options_proptests.rs` against an
  independent oracle written over literal spellings. The generator draws paths
  of one to three segments from a small pool (so root and nested commands and
  equal paths occur), names from near misses, canonical names, off-policy
  flags, dashed spellings, and neutral names, declared forms from all model
  classes, and exceptions whose scopes are selected from the generated paths
  plus a decoy. Command:
  `cargo test -p cargo-orthohelp --lib global_options_proptests`. Non-vacuity:
  committed controls in `global_options_controls.rs` show that (a) an oracle
  that ignores reach disagrees on a nested `--columns`, (b) an oracle without
  the boolean rule disagrees on boolean `--progress`, (c) a case-sensitive
  oracle disagrees on `enum(Auto, Always, Never)`, (d) an oracle that ignores
  exception scope disagrees, (e) an oracle that reports an absent concept
  disagrees on an empty context, and (f) dropping one finding breaks the
  comparison. A witness test shows that the generator's pools contain each
  class.
- `INV-skill`. Method: the existing skill-validation suites, unchanged, plus
  new cases for the width and locale usage text and the D6 spellings.
- Report shape. Method: `insta` snapshots `global_options_report__warn` and
  `global_options_report__deny`, paired with the semantic assertions above.
- Real pipeline. Method: the EP-M4 end-to-end test and behavioural feature.

### Formal verification decision

The obligations were sketched first, above, so that the API could be tested
against them: `L-pointwise` is why the classifier takes no context, and
`L-converge` is why the shape belongs in the table. None is selected for Verus.
`L-table`, `L-functional`, `L-converge`, and `L-disjoint` quantify over a
finite constant, and enumeration discharges them completely. `INV-omission` and
`INV-locality` quantify over unbounded sequences, but they are the standard
homomorphism property of filter and flat-map. A Verus proof would be written
over a hand-made model of `AgentContext` (Verus cannot verify the serde types
and iterator adapters in the executable), so it would prove a library fact
about sequences and leave the only realistic defect, a classifier that peeks at
its context, unexamined. The type signature excludes that defect and the
property tests look for it. Kani would re-explore the finite tables that
`rstest` enumerates. This matches 7.1.1, 7.2.1, 7.2.2, 7.2.3, and 6.3.2. No
`assume`, `admit`, or trusted body is introduced. Trigger for revision: if
implementation needs a rule that relates two inputs or two commands (for
example "flag `--no-emoji` only when `--emoji` is absent"), `L-pointwise` fails
and this decision must be reopened before coding it.

### Suggested BDD features

Proposed path: `cargo-orthohelp/tests/features/global_option_policy.feature`,
run by `cargo test -p cargo-orthohelp --test global_option_policy_bdd`:

```gherkin
Feature: Human-facing global option glossary lint

  Scenario: Warn mode reports near misses with their canonical usage
    Given the root command declares the boolean option "no-emoji"
    And the root command declares the boolean option "accessible"
    And the root command declares the option "colour-policy" with values "auto,always,never"
    When the global option rules check the command surface in "warn" mode
    Then every finding has severity "warn"
    And the replacements are "--emoji auto|always|never", "--accessibility auto|on|off", and "--color auto|always|never"

  Scenario: Deny mode fails a boolean progress option
    Given the root command declares the boolean option "progress"
    When the global option rules check the command surface in "deny" mode
    Then the report has one "deny" finding "global_option_value" replaced by "--progress auto|always|never"

  Scenario: A project may omit every glossary concept
    Given the root command declares the option "target" without values
    When the global option rules check the command surface in "deny" mode
    Then the report has no findings

  Scenario: A domain option on a subcommand is left alone
    Given the command "demo paint" declares the option "color" with values "red,green"
    And the command "demo export" declares the option "columns" without values
    And the command "demo convert" declares the option "output-format" with values "png,webp"
    When the global option rules check the command surface in "deny" mode
    Then the report has no findings

  Scenario: A scoped exception covers only its command
    Given the command "demo build" declares the boolean option "no-emoji"
    And the command "demo render" declares the boolean option "no-emoji"
    And the agent context declares a "flag" exception for "no-emoji" scoped to "demo build"
    When the global option rules check the command surface in "deny" mode
    Then the report has one finding, for "demo render"

  Scenario: Off mode reports nothing
    Given the root command declares the boolean option "no-emoji"
    When the global option rules check the command surface in "off" mode
    Then the report has no findings
```

## Plan of work

Each milestone follows red, green, refactor, ends with the full gates run
sequentially, and is committed.

### `EP-M0` entry check

Rebase onto `origin/main`. Run
`git grep -n "pub struct ExceptionScope" cargo-orthohelp/src` and
`git grep -n "replacement: Option<String>" cargo-orthohelp/src/policy/mod.rs`.
If both are present, mark EP-M1 satisfied and record the commit. If only one is
present, do only the missing half. Re-check that no other change has altered
the glossary table; if it has, stop and reconcile D3 and D6.

### `EP-M1` shared foundation (conditional)

Requirements: `DES-5-exceptions`, D2, D9. Add
`cargo-orthohelp/src/policy/config/scope.rs` with `ExceptionScope`, and make
`PolicyResult` `#[non_exhaustive]` with `replacement`, `new`,
`with_replacement`, and `with_location`. Red: `rstest` cases in
`policy/config/tests.rs` (global, scoped match, scoped mismatch, kind mismatch,
dash normalization, unknown wire kind) and serialization cases in
`policy/tests.rs` (`None` omits the key; a version-1 report without the key
deserializes). Green: move `names_match` and `scope_matches` out of
`skill_validation/policy_check.rs`, delegate from `allows`, and replace every
`PolicyResult` struct literal, including the compiled example in
`docs/cargo-orthohelp-api-guide.md`. Refactor: delete the moved helpers.
Acceptance: new cases pass, every skill-validation test passes unchanged, and
`git diff --stat -- '*.snap'` is empty. Recovery: revert the commit.
Compatibility decision: none (pre-1.0). Remaining gaps: no glossary change.

### `EP-M2` glossary table gains shape and reach

Requirements: `RM-7.1.3-a`, `GAP-shape`, D3, D4, D6, D7. Red: write
`vocabulary/global_options_tests.rs` with `table_is_well_formed`, its broken
table controls, `advice_converges`, `usage_text_matches_design` (one row per
concept, with the text in §5), and `near_miss_rows_match_the_plan`; the tests
fail to compile because the types do not exist. Green: add `OptionShape`,
`NearMiss`, `NearMissReach`, rewrite the table, derive `replacement_hint` from
the shape, and update `policy_check.rs` through `accepted_values()`. Add
skill-validation cases for the width and locale usage text. If
`global_options.rs` would exceed 400 lines, move the table to
`vocabulary/global_options/table.rs`. Acceptance: the new tests pass; existing
skill-validation tests pass unchanged;
`cargo test --doc -p cargo-orthohelp policy::vocabulary` passes. Recovery:
revert the commit. Remaining gaps: no lint yet.

### `EP-M3` the rule group

Requirements: all roadmap bullets, D1, D4, D5, D7, D9 to D14. Red: write
`rules/global_options_tests.rs` and the snapshot tests; they fail because
`check_global_options` does not exist. Green: create `rules/global_options.rs`
as small functions — `declared_form(&AgentInput)`,
`classify(name, scope, declared) -> Option<Violation>` (pure, no context),
`exception_scopes(context)`, and `finding(path, violation, severity)`. Share
the severity mapping: move `severity_for` from `rules/output.rs` to
`rules/mod.rs` as `pub(super)` rather than writing a third copy. Then add
`global_options_proptests.rs` and `global_options_controls.rs`. Acceptance:
`cargo test -p cargo-orthohelp --lib policy::rules` passes; all controls pass;
snapshots are reviewed; `cargo test --doc -p cargo-orthohelp policy::rules`
passes. Conformance check: no CLI path calls the group; no schema changed.
Recovery: revert; the module is additive.

### `EP-M4` behavioural and end-to-end evidence

Requirements: `RM-7.1.3-a` to `-c`, `AXIOM-derive`, `AXIOM-root`. Add the
feature above, `cargo-orthohelp/tests/global_option_policy_bdd.rs`, and its
`global_option_policy_bdd/steps.rs`, modelled on the skill-validation harness.
Add root type `GlobalOptionsFixtureConfig` in
`tests/fixtures/orthohelp_fixture/src/global_options.rs` declaring, on the root,
`colour_policy` (`enum(Auto, Always, Never)`), `no_emoji`, `progress`,
`accessible`, and `diag_json` (booleans), `output_format` (a custom enum with
no override), and conforming `locale` (string) and `quiet` (boolean); and, on a
`render` subcommand, `columns` (a list), `color` (`enum(red, green)`), and
`no_emoji` (boolean). Add `cargo-orthohelp/tests/global_option_policy_e2e.rs`
with one `#[test]` that spawns the built binary once, parses
`agent-context.json`, and checks `warn`, `deny`, `off`, and one scoped wire
exception. Expect exactly seven findings: six on the root (`colour-policy`,
`no-emoji`, `progress`, `accessible`, `diag-json`, `output-format`) and one on
`render` (`no-emoji`). Add the binary to the `orthohelp_cache` filter in
`.config/nextest.toml`. Acceptance: both binaries pass; the workflow-contract
suite passes; golden snapshots for other root types are unchanged. Recovery:
revert; the fixture type is additive.

### `EP-M5` documentation and closure

Update:

- A new ADR (next free number), "Global option glossary lint scope": D3, D4,
  D5, D7, and the residual limits. Index it in `docs/contents.md` and reference
  it from design §5.
- `docs/agent-native-cli-design.md` §5: rule identifiers, the definition of
  global, reach, the added spellings, and the value comparison; replace "The
  roadmap 7.1.3 policy pass will read the same table".
- `docs/cargo-orthohelp-check-gen-design.md` §6: one sentence naming
  `check_global_options(context, mode)` as a group 7.2.8 composes, which reads
  exceptions from the acquired context.
- `docs/users-guide.md`: a section "Lint global option names" after "Lint the
  declared behaviour" (or after 7.1.2's section), with a compiled example
  `guide-check-global-options`, the glossary table, both rules, the omission
  rule, the exception recipe, and the `enum(...)` note. Update "Validate skill
  manifests against commands" for the usage text.
- `docs/developers-guide.md`: the glossary paragraph (two consumers, shape and
  reach, how to add a spelling, the rule for choosing reach), "Policy rule API
  testing" (new files), the misstatement that 7.1.2 rules are the only planned
  importer, and the re-use policy for `ExceptionScope` if EP-M1 ran.
- `docs/cargo-orthohelp-api-guide.md`: a subsection beside the structured
  output rule.
- `README.md` and `ortho_config_macros/README.md`: only if their "Next" wording
  becomes stale.
- `CHANGELOG.md` under `Unreleased`; a migration note only if EP-M1 changed
  `PolicyResult` here.
- `docs/roadmap.md`: mark 7.1.3 and its three sub-items done.

Acceptance: `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
and `make test` pass; every example, path, and identifier is checked against
the code.

## Concrete steps

Run from the repository root, sequentially, with `set -o pipefail`:

```bash
set -o pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-ortho-config-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-ortho-config-${BRANCH}.out"
cargo test --doc -p cargo-orthohelp 2>&1 | tee "/tmp/doctest-ortho-config-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-ortho-config-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-ortho-config-${BRANCH}.out"
```

Focused loops:

```bash
cargo test -p cargo-orthohelp --lib policy::vocabulary
cargo test -p cargo-orthohelp --lib policy::rules::global_options
cargo test -p cargo-orthohelp --test global_option_policy_bdd
cargo test -p cargo-orthohelp --test global_option_policy_e2e
```

Expected red output for EP-M3:

```plaintext
error[E0432]: unresolved import `super::global_options::check_global_options`
```

Expected green output:

```plaintext
test policy::rules::global_options_tests::near_miss_table::case_01 ... ok
...
test result: ok.
```

No Verus command is an acceptance criterion because no proof is selected. If
the decision is reopened, the milestone that adds a proof must register it in
`VERUS_PROOF_FILES` and add `make verus` to its acceptance.

## Validation and acceptance

The work is accepted when all of the following are observable:

- `check_global_options` over the fixture's real agent context reports exactly
  the seven findings in EP-M4 in `warn` and `deny`, none in `off`, and six with
  a scoped exception for `render`'s `--no-emoji`.
- Each finding's `replacement` is the §5 usage text and its message follows
  D14.
- A context that declares no glossary concept has no findings in `deny`.
- The `render` subcommand's `--color red|green` and `--columns` produce no
  finding.
- Existing golden snapshots and `--check-agent-native` behaviour are unchanged.
- All gates in `Concrete steps` succeed.

Red-Green-Refactor evidence is recorded in `Artefacts and notes` for each
milestone.

## Idempotence and recovery

Every step is a file edit plus a test run and may be repeated. Each milestone
is one or two commits, so `git revert` restores the previous plateau. The
end-to-end test writes only to a `tempfile::TempDir` and the content-addressed
bridge cache. If a gate is interrupted, re-run it, checking `target/` for a
stale bridge-build marker first.

## Artefacts and notes

None yet. Planning-time evidence is cited in `Surprises & discoveries`.

## Interfaces and dependencies

At the end of EP-M3 these items exist (EP-M1's `ExceptionScope` and
`PolicyResult` contract is identical to 7.1.2 D5 and D6):

```rust
// cargo-orthohelp/src/policy/vocabulary/global_options.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OptionShape {
    /// A switch that takes no value, such as `--plain`.
    Switch,
    /// A closed policy value, such as `--color auto|always|never`.
    Policy {
        accepted: &'static [&'static str],
        boolean_reach: NearMissReach,
    },
    /// A free value named by a placeholder, such as `--width <columns>`.
    Value(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearMissReach { AnyCommand, GlobalOnly }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearMiss {
    pub spelling: &'static str,
    pub reach: NearMissReach,
    /// When values are declared, the spelling counts only if they include this.
    pub enumerates: Option<&'static str>,
}

pub struct GlobalOptionConcept {
    pub concept: &'static str,
    pub canonical: &'static str,
    pub shape: OptionShape,
    pub near_misses: &'static [NearMiss],
}

impl GlobalOptionConcept {
    #[must_use]
    pub const fn accepted_values(&self) -> &'static [&'static str];
}

// cargo-orthohelp/src/policy/rules/global_options.rs
#[must_use]
pub fn check_global_options(
    context: &ortho_config::AgentContext,
    mode: PolicyMode,
) -> PolicyReport;
```

The table at the end of EP-M2 (bold spellings are new in this plan):

| Concept           | Canonical usage                  | `AnyCommand` near misses                                                 | `GlobalOnly` near misses |
| ----------------- | -------------------------------- | ------------------------------------------------------------------------ | ------------------------ |
| structured-output | `--json`                         | `--output-format` (enumerates `json`), `--diag-json`, `--json-output`    | none                     |
| plain-output      | `--plain`                        | `--plain-output`                                                         | `--text`                 |
| colour            | `--color auto\|always\|never`    | `--colour-policy`, `--color-policy`, **`--no-color`**, **`--no-colour`** | `--colour`               |
| emoji             | `--emoji auto\|always\|never`    | `--no-emoji`, `--emoji-policy`                                           | none                     |
| progress          | `--progress auto\|always\|never` | `--progress-policy`, **`--no-progress`**; boolean `--progress`           | none                     |
| accessibility     | `--accessibility auto\|on\|off`  | `--accessibility-mode`                                                   | `--accessible`           |
| pager             | `--no-pager`                     | `--no-paging`, `--disable-pager`                                         | none                     |
| width             | `--width <columns>`              | `--terminal-width`                                                       | `--columns`              |
| locale            | `--locale <locale>`              | none                                                                     | `--lang`, `--language`   |
| quiet             | `--quiet`                        | none                                                                     | `--silent`               |
| verbose           | `--verbose`                      | none                                                                     | none                     |

Table 1: The glossary and the reach of each near miss.

Reuse policy: the table is the only owner of spellings, shapes, usage text, and
reach. The skill validator and the rule group look concepts up and never
restate them. `ExceptionScope` is the only exception matcher. `classify` is
private to the rule group; a future consumer that needs it must first move it
beside the table. No dependency is added.

## Revision note

- 2026-11-24: initial draft.
