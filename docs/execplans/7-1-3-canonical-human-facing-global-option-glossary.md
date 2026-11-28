# 7.1.3 Add the canonical human-facing global option glossary

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (community-of-experts review applied; awaiting user approval; no
implementation has started)

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
    colour_policy: ColourPolicy,
    /// Suppress emoji.
    no_emoji: bool,
    /// Show progress.
    progress: bool,
    /// Use accessible output.
    accessible: bool,
}
```

After this change, the reusable rule group
`cargo_orthohelp::policy::rules::global_options::check_global_options(context, mode)`
reads the command surface recorded in agent context and reports near-miss
spellings and boolean forms of the glossary's policy options. Each finding
names the canonical usage both in the message and in the machine-readable
`replacement` field:

```json
{
  "rule_id": "agent-native.global-options.near-miss",
  "code": "global_option_near_miss",
  "severity": "warn",
  "message": "command `demo` declares `--no-emoji`, a near miss for the emoji option; use `--emoji auto|always|never`",
  "location": null,
  "replacement": "--emoji auto|always|never"
}
```

A project that offers no emoji, no pager, and no progress display hears nothing
about those concepts: an absent concept is never a finding. A project that must
keep a legacy spelling records a `flag` exception in its policy table, and the
finding disappears while the exception stays visible in agent context.

The lint checks names and the boolean form only. It does not compare declared
value sets with the glossary, does not check the shape of switches or free
values (`--quiet`, `--width`), and cannot see options that never reach agent
context (D7).

Preserved behaviour: the report schema version (`"1"`), the agent-context and
documentation schemas, `--check-agent-native` (which stays configuration-only
and bridge-free), the configuration grammar, the glossary's spellings, and
every existing golden snapshot. Deliberately changed behaviour: the usage text
for the width and locale concepts gains its placeholder (`--width <columns>`,
`--locale <locale>`), as §5 writes it, which changes the skill validator's
message for a global `--columns`, `--terminal-width`, `--lang`, or
`--language`; and, only if EP-M1 runs here, `PolicyResult` (released in
`v0.11.0`) becomes `#[non_exhaustive]` with constructors.

## Approval requested

Approving this plan also settles the following points. Each one touches an
accepted document, a sibling plan, or a reading of the roadmap, so each is
listed for an explicit decision. The plan's recommendation is given first.

- A1 (D1). Deliver a separate rule group, not candidates inside
  `check_vocabulary`. The unapproved 7.1.2 plan (PR #705) says the opposite in
  its D13; approving A1 means that sentence changes when 7.1.2 is next revised.
- A2 (D4). Judge near misses on the root command, and on nested commands only
  when the declaration itself carries evidence. Design §3.4.1 says, for skill
  validation, that global-option policy applies only to options stated to be
  global, "never to an option whose name merely resembles one"; the developers'
  guide says classification "is supplied, never inferred". Both bind skill
  validation; this lint uses declared position and shape, never calls an option
  global, narrows §5's unqualified `--output-format`, and reports a root
  `--accessible` of any type although §5 says boolean. EP-M5 documents that the
  two consumers scope differently.
- A3 (D5). Report boolean `--progress` on every command, because §5 names it as
  a near miss, but boolean `--color`, `--emoji`, and `--accessibility` on the
  root command only, so that a domain command's own `--color` stays unjudged. A
  domain command's own boolean `--progress` is therefore flagged; the exception
  recipe applies.
- A4 (D2). If 7.1.3 lands before 7.1.2, it delivers the `replacement` field that
  ADR-0008 D14 assigns to 7.1.2, plus 7.1.2's proposed `#[non_exhaustive]` and
  constructors (a source break of a type released in `v0.11.0`) and
  `ExceptionScope`.
- A5 (D6). Do not flag `--no-color` or `--no-progress`. §5 does not list them
  and clig.dev recommends `--no-color`. Adding them later is a one-line table
  change that also affects skill validation.
- A6 (D7). Do not compare declared value sets in this item. The roadmap bullet
  standardizes names, and most value sets are invisible until the IR records
  clap `ValueEnum` variants (a follow-up stub that 7.1.2 proposes).
- A7 (D16). Use `googletest` and `pretty_assertions` in the new test modules,
  as this task authorizes, and amend the three developers' guide passages that
  conflict: "Scoped use of `googletest` and `pretty_assertions`", the D8
  assertion split in "Policy configuration test layout", and the sentence
  saying policy tests already use both.
- A8 (D10, D11). Define `replacement` as canonical usage for all rule groups,
  not 7.1.2's "canonical spelling", and do not require it to appear in the
  report's `vocabulary` block, which 7.1.2 D13 cites as a concern.
- A9. Do not adopt the `--format text` → `--plain` mapping, although the 7.1.2
  revision note says it "moves to 7.1.3". If 7.1.2 removes it, no rule owns it.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- `docs/agent-native-cli-design.md` §8.1 forbids inferring semantics from names
  or absent data. The rules may report a declared spelling and a declared
  shape. They must never infer that a concept is supported, never report a
  missing option, and never draw a conclusion from an undeclared value set.
- The glossary has one owner,
  `cargo-orthohelp/src/policy/vocabulary/global_options.rs`. No rule restates a
  spelling, a value list, or usage text. Lint scoping lives in the rule module,
  not in the shared table, so the skill validator's behaviour cannot change by
  accident.
- `--check-agent-native` stays configuration-only and builds no bridge
  (ADR-0008 D11 and its 2026-11-14 amendment). This plan adds no CLI wiring;
  roadmap 7.2.8 owns `cargo orthohelp check` and composes rule groups.
- `ORTHO_POLICY_REPORT_SCHEMA_VERSION` and `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`
  stay `"1"`. The `Vocabulary` wire struct is not extended (D10).
- The agent-context schema, the documentation intermediate representation (IR),
  the derive macros, and the policy configuration schema do not change.
- Off-policy verbs and the `--format json` / `--output json` /
  `--skip-confirmations` rules belong to roadmap 7.1.2 and are out of scope. So
  is the `--format text` → `--plain` mapping that the 7.1.2 plan removes: this
  plan neither adopts nor deletes it.
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
  added. `GlobalOptionConcept` is unreleased (6.3.2 merged after `v0.11.0`).
  `PolicyResult` is released, so if EP-M1 runs here the change needs a
  migration note and an ADR-0008 amendment.
- Completed ExecPlans are historical and must not be edited. The active 7.1.2
  (PR #705) and 7.2.4 (PR #732) ExecPlans must not be edited from this branch.
- Gates run sequentially: `make check-fmt`, `make lint`, `make test`, plus
  `make markdownlint` and `make nixie` when Markdown changes, and
  `make test-workflow-contracts` when `.config/nextest.toml` changes.
  `make test` skips Rust doctests, so each milestone that touches a doc example
  also runs `cargo test --doc -p cargo-orthohelp`.

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

- Files: more than 36 non-ExecPlan files changed → stop and escalate.
- Code: more than 3,400 net lines, or 2,900 if EP-M1 is already satisfied →
  stop and escalate.
- Configuration: more than 40 net lines → stop and escalate.
- Documentation: more than 650 net lines → stop and escalate.
- Interface: any public API change beyond those in `Interfaces and dependencies`
  → escalate.
- Dependencies: any new crate → escalate.
- Architecture: CLI wiring, a schema change, or a configuration change →
  escalate.
- Table: adding, removing, or respelling any glossary entry → escalate.
- Iterations: a focused test still failing after three fix attempts → escalate.
- Time: a milestone taking more than one working day → escalate.
- Ambiguity: a second plausible reading of placement, evidence, or exception
  scope → stop and present options.

Budget basis. The survey estimates about 2,450 net code lines: table change and
its tests (~220), conditional foundation (~300), rule module (~170), unit
tables and the exhaustive quotient test (~300), property tests with their
oracle (~350), committed mutant checkers (~250), behavioural feature and steps
(~450), end-to-end test and fixture (~310), and snapshots (~100). History
argues against optimism. 7.1.1 raised its limit from 1,200 to 1,400 lines;
7.2.1 landed at 2,838 against 1,500; 6.3.2 landed at 5,466 against 1,500; and
7.2.2 and 7.2.3 landed at about 5,900 and 4,500, although both also changed the
derive and the IR. The rule-group slices alone were about 1,230 lines (7.2.1)
and 1,015 lines (7.2.3), and the model BDD harness is about 605 lines. Tests
and oracles dominated each overrun, so they are itemized here. Design review
halved the table machinery to keep the estimate credible. The limit is the
estimate plus about 40 per cent; a breach means the estimate is wrong and must
be re-planned. Documentation is estimated at 430 lines with 50 per cent
contingency. Configuration is two nextest filter entries.

## Risks

- Risk: 7.1.2 (PR #705) is an unmerged draft that plans the same `PolicyResult`
  change and the same exception matcher. Severity: medium. Likelihood: high.
  Mitigation: EP-M1 is conditional and its contract is written out in
  `Interfaces and dependencies`; whichever item lands second skips it and runs
  its acceptance tests instead (D2).
- Risk: a clean report is read as proof of conformance. Netsuke declares
  `color`, `emoji`, `progress`, and `accessibility` with
  `#[ortho_config(skip_cli)]` and adds them to clap by hand, so those options
  never reach agent context. Severity: medium. Likelihood: high. Mitigation:
  the ADR and the users' guide say that a clean report means no *declared* near
  miss; 7.2.8 coverage counts expose how little was visited.
- Risk: a domain option that shares a glossary spelling
  (`translate --language fr`, `export --columns a,b`, `paint --colour red`) is
  flagged, so teams switch the policy off. Severity: medium. Likelihood: low
  after D4. Mitigation: nested commands need declared evidence; the users'
  guide gives the scoped-exception recipe.
- Risk: single-command CLIs, whose root command carries domain options too, see
  root spellings flagged (`cut --columns`, `say --text`, or an
  `--output-format` with undeclared values). Severity: low. Likelihood: medium.
  Mitigation: the exception recipe; the `--json` template never tells an author
  to delete other values (D14).
- Risk: a context rooted at a wrapper command (a Cargo-style `cargo demo` shape)
  gives every real option a two-segment path, so root-only checks see nothing.
  Severity: low. Likelihood: low, because the documented root type is the
  options struct rather than the wrapper. Mitigation: a pinned test row and a
  documented limit.
- Risk: following two pieces of advice produces a duplicate option
  (`--diag-json` and `--output-format` both map to `--json`). Severity: low.
  Likelihood: low. Mitigation: documented; convergence is claimed per option
  only (`L-converge`).
- Risk: short-only options, clap aliases, `hide_in_help` fields, and spellings
  outside the closed table (`--is-verbose`) go unreported. Severity: low.
  Likelihood: certain. Mitigation: documented. Hiding a legacy option for one
  release is the intended migration path.
- Risk: the new end-to-end binary adds one serialized bridge build to a nextest
  run whose recorded headroom against its 2,300 s budget is 47 s. Severity:
  medium. Likelihood: medium. Mitigation: one spawn only; EP-M4 measures the
  binary on a warm tree and records the figure; no contract measures run time,
  so if the figure exceeds 47 s, stop and escalate rather than raise the budget.
- Risk: the next free ADR number is contested (PRs #588 and #723 both add an
  ADR-0026, and the 7.2.4 plan reserves it). Severity: low. Likelihood: high.
  Mitigation: take the next free number on `main` at implementation time.

## Progress

- [x] (2026-11-24) Reconnaissance by a Wyvern team: option-metadata
  pipeline, design history, and test and proof infrastructure.
- [x] (2026-11-24) Prior-art check: clig.dev, the Cargo book, Rain's
  Rust CLI recommendations, `clap-verbosity-flag`, GitHub CLI accessibility,
  and the current Netsuke command surface.
- [x] (2026-11-24 17:41Z) Initial draft written.
- [x] (2026-11-24) Community-of-experts review (structure and
  contracts; verification and failure modes; alternatives, cost, and viability)
  applied; see `Revision note`.
- [x] (2026-11-24 18:09Z) Second review round applied; the model was scripted
  and executed to pin the exhaustive totals and check each scenario's verdicts.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Shared foundation (conditional).
- [ ] `EP-M2` Glossary usage text and table well-formedness.
- [ ] `EP-M3` `check_global_options` rule group with unit, snapshot, and
  property tests.
- [ ] `EP-M4` Behavioural feature and bridge-backed end-to-end test.
- [ ] `EP-M5` Documentation, ADR, and roadmap closure.

## Surprises & discoveries

- Observation: the glossary data already exists, added early by 6.3.2.
  Evidence: `cargo-orthohelp/src/policy/vocabulary/global_options.rs:34-101`;
  the 6.3.2 plan says what remains for 7.1.3 is "the bridge-IR lint wiring, not
  the glossary data". Impact: 7.1.3 adds a consumer, not a table.
- Observation: `accepted_values` is documented as "empty for a boolean option",
  yet `--width` and `--locale` are also empty and take values, so
  `replacement_hint` omits the placeholders §5 writes. Impact: D3.
- Observation: agent context has no global-option marker, and the derive never
  emits clap's `global = true` for user fields. Evidence:
  `ortho_config/src/agent_context/mod.rs:212-232`; the only `global = true` in
  the derive is the profile selector
  (`ortho_config_macros/src/derive/build/profile_flag.rs:84`). Impact: D4.
- Observation: the users' guide promises, for skill validation, that "a domain
  command with its own `--color` is not judged by the glossary rule". Impact:
  canonical names other than boolean `--progress` are judged on the root only.
- Observation: a `bool` or `Option<bool>` field always reaches agent context as
  `value_type: "bool"` with no enumerated values, even though clap accepts
  `--flag=false`. Evidence: `cargo-orthohelp/src/agent_context/input.rs:73-91`.
  Impact: boolean forms are detectable from declared metadata alone.
- Observation (review): Netsuke's clap surface conforms to the glossary, but its
  four policy options are `skip_cli`, so its agent context does not contain
  them. Impact: the evidence is for the glossary's *names*, not for this lint's
  input; see `Risks`.
- Observation (review): no surface among Netsuke, Weaver, ddlint, `hello_world`,
  and `cargo-orthohelp` would produce a finding today. Impact: nothing
  calibrates per-spelling precision settings, so D4 uses declared evidence
  instead.
- Observation: clig.dev lists `--no-color` as an expected switch, while Cargo,
  Netsuke, and Rain's recommendations use `--color auto|always|never`. Impact:
  D6.
- Observation: `googletest` is declared in `cargo-orthohelp` but unused in its
  Rust sources, and the developers' guide scopes both assertion crates to
  profile tests. Impact: D16.
- Observation: no gate runs Rust doctests, and `make test` does not run
  `tests/workflow_contracts`. Impact: explicit commands.
- Observation: `make verus` and `make kani` exist with empty proof registries.
  Impact: see `Formal verification decision`.

## Decision log

- D1. Decision: deliver the lint as its own rule group,
  `check_global_options(context: &AgentContext, mode: PolicyMode) -> PolicyReport`
  in `cargo-orthohelp/src/policy/rules/global_options.rs`. Rationale: the
  signature matches `check_behaviour`, `check_output`, and 7.1.2's planned
  `check_vocabulary`; 7.2.8 composes per-group reports and records per-group
  coverage, so "glossary evaluated" must be distinguishable from "verbs
  evaluated"; and a separate group can land in either order with 7.1.2.
  Alternative rejected: candidates inside `check_vocabulary` (7.1.2 draft D13),
  which serializes two items the roadmap does not serialize and merges two
  coverage groups. See A1.
- D2. Decision: make the shared foundation conditional. If `ExceptionScope` and
  `PolicyResult::replacement` are on `main` when work starts, EP-M1 is
  satisfied; otherwise EP-M1 delivers the contract written out in
  `Interfaces and dependencies`, which agrees with 7.1.2 D5 and D6 except for
  the meaning of `replacement` (D11). Rationale: the roadmap dependency is
  7.1.1 alone, and exactly one matcher and one result shape must exist
  whichever item lands first. See A4.
- D3. Decision: change the table type minimally: add
  `placeholder: Option<&'static str>` to `GlobalOptionConcept`, mark it
  `#[non_exhaustive]`, correct the `accepted_values` comment, and make
  `replacement_hint` render the placeholder. Spellings do not change.
  Rationale: the roadmap's "boolean `--progress`" is already derivable from a
  non-empty `accepted_values` and a declared boolean. The first draft's
  `OptionShape`, `NearMiss`, reach, and value predicates were rejected in
  review: they encoded precision that no known surface exercises and gave one
  table two meanings.
- D4. Decision: placement and evidence decide where a near-miss spelling is
  reported. On the *root* command (a command whose path has exactly one
  segment), a near-miss spelling is reported whatever its declared form. On a
  *nested* command it is reported only when its declaration carries evidence.
  Either it declares enumerated values that include every *identifying value*
  of the concept, ignoring ASCII case; or it is declared boolean, the concept
  takes no free value, and the spelling is compound (contains `-`) or is
  `accessible`, whose boolean form §5 names. The identifying values are the
  concept's `accepted_values`, or `json` for the structured-output concept;
  other concepts have none. In both places, a structured-output spelling whose
  declared values exclude `json` is silent, because the declaration rules the
  concept out. Rationale: position and declared shape are facts recorded in
  agent context. `paint --colour red`, boolean `print --colour` and
  `search --text`, `export --columns a,b`, and
  `convert --output-format png|webp` stay silent, while nested `--no-emoji`,
  boolean `--accessible`, and `--output-format json|yaml` are caught. Single
  words are not judged as nested booleans, so renaming a nested boolean
  `--colour` to `--color` cannot change its verdict. The rule never claims an
  option is global. Alternatives rejected: judge every name everywhere (noisy);
  judge the root only (misses nested `--no-emoji`); per-spelling reach data
  (speculative); a caller-supplied global list (7.2.8 has no such input); a
  `global` marker in the IR (schema change). Cost accepted: nested, non-boolean
  near misses without declared values, and nested single-word booleans other
  than `--accessible`, are silent. On the root, `--accessible` of any type is
  reported, although §5 says boolean: the name itself is the near miss. See A2.
- D5. Decision: report a canonical policy option (one with accepted values:
  colour, emoji, progress, accessibility) declared as a boolean. `--progress`
  is reported on every command, because §5 lists boolean `--progress` as a near
  miss; the other three are reported on the root only. Rationale: a boolean
  cannot carry a three-valued policy, and an agent that passes the canonical
  usage is rejected by the parser; but a domain command's own `--color` keeps
  the users' guide's promise. Rules and codes:
  `agent-native.global-options.near-miss` / `global_option_near_miss` for
  spellings, and `agent-native.global-options.value` / `global_option_value`
  for boolean policy options, mirroring the skill validator's
  `skill.policy.global_option_near_miss` and
  `skill.policy.global_option_value`. See A3.
- D6. Decision: do not extend the near-miss list. Rationale: the switch
  `--no-color` is recommended by clig.dev and its absence already means `auto`;
  `--no-emoji` has no such standard, so the asymmetry is principled; the change
  would alter skill validation; and no consumer evidence asks for it. See A5.
- D7. Decision: no value-set comparison, no shape check for switches or free
  values. Rationale: the roadmap standardizes names; §5 says accepted values
  match exactly while the derive records `enum(Auto, …)` with the author's
  casing, so a comparison would either disagree with the skill validator or
  contradict §5. The `AgentInput` projection also reports a list of enums and a
  scalar enum identically. Test rows pin the silence. See A6.
- D8. Decision: the table is closed. No edit distance, stemming, or prefix
  match. Rationale: deterministic findings, and §8.1.
- D9. Decision: exceptions are the existing `flag` kind, read from
  `context.policy.exceptions` and matched by `ExceptionScope`: no
  `command_path` means everywhere; otherwise it must equal the space-joined
  path. Leading dashes are ignored. Findings carry no `location`. Reports list
  no exceptions, like the sibling groups, because wire exceptions carry no
  `reason` and `evaluate` owns their advertisement; EP-M5 corrects the field
  comment on `PolicyReport::exceptions`. Findings carry the path and spelling
  only in `message`, as sibling groups do; structured fields need a
  report-schema decision that 7.2.8 owns, and the users' guide tells consumers
  not to parse `message`. Rationale: what suppresses a finding is what agents
  are told.
- D10. Decision: do not extend the `Vocabulary` block, and do not require a
  `replacement` to appear in it. Rationale: extending it would change every
  golden report; each finding already names its canonical usage; and consumers
  can read the public `GLOBAL_OPTION_CONCEPTS` and `replacement_hint`.
- D11. Decision: define `replacement` once, for all rule groups, as the
  canonical usage: its first whitespace-delimited token is the canonical
  spelling, and any remainder describes the value. `--json` and
  `--emoji auto|always|never` both satisfy it. Rationale: 7.1.2's values
  already conform; for a boolean policy finding the bare name would equal the
  offending name and say nothing. If 7.1.2 landed first, EP-M3 updates the
  field comment.
- D12. Decision: renderer metadata (7.2.2) is not consulted. Rationale:
  declaring `renderer(human(colour = "auto"))` does not oblige a CLI to offer
  `--color`; requiring it would break the omission criterion, and ADR-0016 says
  renderer keys are not flags.
- D13. Decision: findings are ordered by command path (stable sort, so equal
  paths keep context order), then by input order in the context (the bridge
  sorts inputs by name). At most one finding per input. Rationale:
  deterministic reports, as `check_output` does.
- D14. Decision: messages follow three fixed templates and are functions of the
  path, the normalized name, and the violation (its concept, its rule, and
  whether a structured-output option is declared boolean):

  ```plaintext
  command `<path>` declares `--<name>`, a near miss for the <concept> option; use `<usage>`
  command `<path>` declares `--<name>`, a near miss for the structured-output option;
    expose `--json` (other `--<name>` values may remain if declared)

  command `<path>` declares `--<name>` as a boolean; use `<usage>`
  ```

  Each template is one line in the message; the second wraps here for width
  only. The second template applies to a structured-output near miss that is
  not declared boolean. Rationale: §5 forbids echoing values, and an author
  must not be told to delete a domain format.
- D15. Decision: no Verus or Kani milestone; see `Formal verification decision`.
- D16. Decision: new test modules use `googletest` matchers for finding lists
  and `pretty_assertions` for scalar equality; existing modules are not
  migrated. Rationale: the task authorizes both; three developers' guide
  passages conflict with that or with each other and are amended in EP-M5. See
  A7.

## Outcomes & retrospective

Not started.

## Conformance basis

No Terms of Reference exists for this work. Upstream artefacts, all inspected at
`cbba0d03`:

- `docs/roadmap.md` 7.1.3: `RM-7.1.3-a` (standardize names when concepts are
  present), `RM-7.1.3-b` (lint near misses), `RM-7.1.3-c` (permit omission).
- `docs/agent-native-cli-design.md` §3.4.1 (`DES-3.4.1-supplied`: for skill
  validation, global classification is supplied), §5 (`DES-5-glossary`,
  `DES-5-near-miss`, `DES-5-exceptions`, `DES-5-one-table`, `DES-5-no-echo`),
  §6.2.1 (`DES-6.2.1-renderer`), and §8.1 (`DES-8.1-no-infer`).
- `docs/users-guide.md` "Validate skill manifests against commands"
  (`UG-domain-silent`: a domain command's own `--color` is not judged).
- `docs/adrs/adr-0008-agent-native-policy-configuration.md`: D3, D11, D12, D14,
  and the 2026-11-14 amendment.
- `docs/adrs/adr-0016-dual-renderer-metadata-surface.md`: renderer keys are not
  flags.
- `docs/cargo-orthohelp-check-gen-design.md` §2.2 and §4: rule groups join
  `check` when their items land; per-group coverage.
- The 7.1.2 ExecPlan draft at `4e6c2a5f` (PR #705), D5, D6, and D13: a
  coordination input, not an authority.

Baseline gaps: `GAP-consumer` (nothing lints the CLI's own options),
`GAP-usage` (usage text omits placeholders), `GAP-placement` (no accepted rule
says where a glossary spelling is judged in agent context).

Trace links:

```plaintext
RM-7.1.3-a -> DES-5-glossary -> GAP-usage -> D3 -> EP-M2 -> vocabulary::global_options::table_tests::usage_text_matches_design
RM-7.1.3-a -> DES-5-near-miss -> GAP-consumer -> D4, D5 -> EP-M3 -> rules::global_options_tests::every_roadmap_concept_is_covered
RM-7.1.3-b -> DES-5-near-miss -> GAP-placement -> D4, D5, D14 -> EP-M3, EP-M4 -> rules::global_options_tests::roadmap_spellings
RM-7.1.3-c -> DES-8.1-no-infer -> D4, D12 -> EP-M3, EP-M4 -> global_options_proptests::omission_never_adds_a_finding
DES-5-exceptions -> D9 -> EP-M1, EP-M3, EP-M4 -> tests/features/global_option_policy.feature
DES-5-one-table -> D3, D8 -> EP-M2 -> vocabulary::global_options::table_tests::table_is_well_formed
UG-domain-silent -> D4, D5 -> EP-M3, EP-M4 -> rules::global_options_tests::nested_domain_options_are_silent
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
  redundant when it names a `CANONICAL_FLAGS` entry. No concept with accepted
  values is in that list, so an exception for `--progress` is not reported as
  redundant.
- `tests/fixtures/orthohelp_fixture/src/lib.rs` (348 lines) is the real fixture
  crate; its root types are selected with `--root-type`.
- `cargo-orthohelp/tests/agent_context_output_end_to_end.rs` shows how to spawn
  the real binary. `cargo-orthohelp/tests/skill_validation_bdd.rs` and
  `skill_validation_bdd/steps.rs` show an in-process `rstest-bdd` harness.
- `.config/nextest.toml` groups bridge-building binaries in `orthohelp_cache`
  and names binaries that need the `nested-cargo-builds` warm-up in a
  `[[profile.default.scripts]]` filter;
  `tests/workflow_contracts/child_cargo_tier_test.py` fails if a binary that
  spawns a child Cargo is missing from the raised allowances. It classifies a
  binary by its sources, so an in-process test must not reach
  `cargo_orthohelp_exe`.
- `clippy.toml` leaves `max-struct-bools` at its default of three, and the
  fixture crate inherits the pedantic workspace lints.
- `docs/users-guide.md` holds compiled examples; each has a
  `<!-- tested-example: id -->` marker and must be registered in
  `ortho_config/tests/documentation_examples_tests.rs` and, for Rust,
  `documentation_examples_rust_tests.rs` and
  `documentation_examples/guide_flows.rs`.

Non-test callers of the glossary API number one file (`policy_check.rs`). The
survey counted about 32 affected files: 6 production, 13 test, 2 fixture, 1
configuration, and 10 documentation. No existing test pins the four
skill-validation messages that change.

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

The obligations below were sketched before the API was fixed, and they shaped
it: `L-pointwise` is why the classifier takes no context, `L-quotient` is why
its declared form is a three-way type, and `INV-no-echo` is why the finding
builder never receives declared values.

### Abstract model

The projection from `AgentInput` and its command is total:

- `name(i)` is `i.long` with every leading `-` removed. An absent or empty
  result has no verdict. Names compare byte for byte and case-sensitively.
- `declared(i)` is `Boolean` when `value_type` is exactly `"bool"`, whatever
  `enum_values` holds. Otherwise it is `Enumerated(S)` when `enum_values` is
  not empty, whatever `value_type` says, where `S` is the set of values folded
  to ASCII lowercase (no trimming, no Unicode folding). Otherwise it is `Other`.
- `place(c)` is `Root` when `c.path` has exactly one segment and `Nested`
  otherwise, including an empty path.

A *table* `T` is a finite sequence of concepts, each with a canonical name, a
set of accepted values, a set of near-miss names, and a set of identifying
values `ident(c)`: the accepted values, or `{json}` for structured output. A
concept is a *policy* when its accepted set is not empty.

`evident(c, n, d)` holds when `d = Boolean`, `c` has no placeholder, and `n`
contains `-` or `n = accessible`; or when `ident(c) ≠ ∅`, `d = Enumerated(S)`,
and `ident(c) ⊆ S`. `d` is *counter-evidence* for `c` when `c` is structured
output, `d = Enumerated(S)`, and `json ∉ S`; nothing else is.

`verdict(n, p, d)` is:

- `NearMiss(c, keeps)` when `n` is a near miss of `c`, `d` is not
  counter-evidence for `c`, and (`p = Root` or `evident(c, n, d)`); `keeps`
  holds when `c` is structured output and `d ≠ Boolean`, and selects the second
  D14 template;
- `BooleanPolicy(c)` when `c` is a policy, `n` is its canonical name,
  `d = Boolean`, and (`p = Root` or `c` is progress);
- `None` otherwise.

With `E` the exceptions in `context.policy`, the report is empty in `off`, and
otherwise is:

```plaintext
check = map(severity(mode)) ∘ filter(¬ covered(E)) ∘ flat_map(verdict) ∘ stable_sort_by(path)
```

`covered(E)` holds for a finding with name `n` and path `p` when some exception
in `E` has kind `flag`, the same name after stripping leading dashes, and no
`command_path` or one equal to `p` joined with single spaces. An unknown kind
covers nothing. The join is not injective (`["a b"]` and `["a", "b"]` collide);
this is the existing matcher's behaviour and is preserved.

### Contracts

- `name`, `declared`, `place`. Requires: nothing. Ensures: the definitions
  above, for every deserializable input.
- `classify(name, place, &declared) -> Option<Violation>`. Requires:
  `L-table`. Ensures: equals `verdict`; reads nothing else.
- `ExceptionScope::covers`. Requires: nothing. Ensures: the definition above,
  and equality with the matcher it replaces.
- `replacement_hint(concept)`. Ensures: the §5 usage text; its first token is
  `concept.canonical`.
- `finding(path, name, violation, severity)`. Ensures: the D14 template and
  `replacement = replacement_hint(concept)`. It does not receive `declared`; the
  `Violation` carries `keeps`.
- `check_global_options(context, mode)`. Requires: nothing. Ensures: total; the
  context is unchanged; the report equals `check`; `summary` matches `results`;
  `exceptions` is empty; `vocabulary` is canonical. The policy mode recorded in
  the context is never consulted.

### Invariants and lemmas

- `INV-exact`. The report equals the model's, finding for finding.
  (`RM-7.1.3-a`, `RM-7.1.3-b`)
- `INV-mode`. `off` gives no results; `warn` and `deny` give the same findings
  with severity `warn` and `deny`, whatever the context's own policy mode.
- `INV-omission`. Deleting any input or command never adds a finding, and a
  context that declares no glossary-related name has none. (`RM-7.1.3-c`)
- `INV-exception`. A finding is suppressed if and only if `covered(E)`; adding
  an exception never adds a finding.
- `INV-locality`. For a fixed `E` and mode, the report for `A ++ B` is a
  permutation of the union of the reports for `A` and `B`. The shared `E` is
  necessary: a global exception in `B` would otherwise suppress a finding in
  `A`.
- `INV-order`. Permuting commands with distinct paths changes nothing; commands
  with equal paths keep context order.
- `INV-no-echo`. Two inputs that differ only in declared values and share a
  verdict and a D14 template give equal findings. (`DES-5-no-echo`)
- `INV-skill`. Skill-manifest findings are unchanged except for the message text
  of a global `--columns`, `--terminal-width`, `--lang`, or `--language`. No
  finding appears or disappears.
- `L-table`. Modulo dash stripping: canonical names are distinct; near-miss
  names are distinct across the table; no name is both; none is in the 7.1.2
  off-policy flag table; no near miss and no policy canonical is in
  `CANONICAL_FLAGS` (so an exception for one is never reported as redundant).
  Every name is non-empty ASCII-lowercase kebab case; accepted values are
  non-empty, ASCII lowercase, and distinct; placeholders are non-empty and
  appear only on concepts without accepted values.
- `L-functional`. At most one concept yields a verdict for an input. Sketch:
  each disjunct fixes `n` in a set; `L-table` makes those sets disjoint.
- `L-pointwise`. `verdict` depends only on `(name, place, declared)`, and
  `covered` only on `(name, path, E)`. Sketch: by the types of `classify` and
  `covers`. `INV-omission`, `INV-locality`, and the monotonic half of
  `INV-exception` follow from `check` being a map of a filter of a flat-map.
- `L-quotient`. `verdict` depends on `S` only through
  `S ∩ {auto, always, never, on, off, json}` and whether `S` has another
  member. Sketch: `S` occurs only in `ident(c) ⊆ S` and `json ∉ S`, and every
  `ident(c)` is inside that set.
- `L-converge`. For each concept, the canonical name with its recommended
  declaration (`Enumerated(accepted)` for a policy, `Boolean` for a switch,
  `Other` for a placeholder option) has no verdict in either place. After
  `--json` is exposed and `json` is removed from a structured-output near
  miss's declared values, that option has no verdict in either place; an
  undeclared one on the root stays reported until its remaining values are
  declared or an exception is recorded. The claim is per option: two near
  misses of one concept converge on one name.
- `L-disjoint`. No input is flagged both here and by 7.1.2's flag rule. Sketch:
  `L-table`, given that both strip every leading dash.

### Axioms (trusted, not verified here)

- `AXIOM-derive`: the derive records long names, booleans, and `enum(...)`
  values in the IR. Exercised for the fixture by the EP-M4 end-to-end test.
- `AXIOM-root`: the bridge gives only the root command a one-segment path.
  Exercised by the same test.
- `AXIOM-policy`: `apply_policy_to_context` copies configured exceptions into
  agent context. Not exercised here, because the end-to-end test injects the
  wire exception; the evidence is the existing `agent_context` policy tests.
- `AXIOM-serde`: `serde` honours `default` and `skip_serializing_if`.

### Methods per obligation

- `L-table`, `L-functional`, `L-converge`, `L-disjoint`. Method: exhaustive
  iteration over the compile-time table, in
  `cargo-orthohelp/src/policy/vocabulary/global_options_table_tests.rs`,
  declared inside `global_options.rs` as
  `#[cfg(test)] #[path = "global_options_table_tests.rs"] mod table_tests;`. It
  is a child module, so it can read the private off-policy table, and the
  `#[path]` avoids the denied `self_named_module_files` lint. Probing that
  table through `off_policy_flag_replacement(name, None)` would miss `--format`
  and `--output`. Non-vacuity: the well-formedness check is a pure function
  over slices; controls pass broken tables (duplicate name, canonical that is
  also a near miss, an overlap on `--format` specifically, an uppercase
  accepted value, an empty placeholder) and assert the named rejection. Command:
  `cargo test -p cargo-orthohelp --lib policy::vocabulary`.
- `INV-exact` for the classifier. Method: exhaustive enumeration in
  `rules/global_options_tests.rs`, run through `check_global_options` so that
  it also covers the projection. Names are the union of the table's names and
  the oracle's literal names, plus four neutral names; places are both;
  declared forms are `Boolean`, `Other`, and all 127 non-empty subsets of
  `{auto, always, never, on, off, json, foreign}`: 35 × 2 × 129 = 9,030 cases,
  compared with a literal oracle that repeats the spellings and never reads the
  table. The test pins the totals, which were computed from the model at
  planning time: 2,724 near misses (2,391 root, 333 nested) and 5 boolean
  policies. By `L-quotient`, and because `verdict` reads a name only by
  equality with a table entry, the cases exhaust `verdict`. That `classify`
  factors the same way is not assumed: a property shows that an arbitrary name
  outside the table has no verdict and that replacing the foreign member of `S`
  with arbitrary foreign values changes nothing.
- `INV-exact`, named cases. Method: `rstest` tables with `googletest` matchers:
  `roadmap_spellings` (each spelling in the roadmap bullet, on the root and
  nested, with its exact message), `every_roadmap_concept_is_covered` (one row
  per noun in the first bullet; the `verbose` row asserts that the concept
  exists, its canonical name is `--verbose`, and it has no near miss),
  `nested_domain_options_are_silent`, `undeclared_shapes_are_silent` (the D7
  rows, including boolean `--width`, valued `--json`, a superset of accepted
  values, and a root `--output-format png|webp`), `projection_edges` (dashed,
  empty, and absent names; `Bool`; `bool` with values; `enum` without values;
  `list` with values; empty path; a Unicode-cased value; near neighbours
  `no-emojis`, `colour_policy`, `Emoji`), and `wrapper_rooted_context`.
- Context-level invariants. Method: `proptest` in
  `rules/global_options_proptests.rs`. Each property is a function of
  `checker: fn(&AgentContext, PolicyMode) -> PolicyReport`, run on the real
  checker and compared with an oracle in `rules/global_options_oracle.rs`. The
  generator builds interesting cases by construction: exception names are
  selected from names present (dashed or not), scopes from none, own path,
  another present path, and a decoy; the context's own `agent_native` mode from
  all three; kinds from `flag`, `verb`, and an unknown string; values from the
  model's classes; paths of zero to three segments with repeats. A fixed-seed
  sample asserts a non-zero count for every reachable combination of verdict,
  place, and suppression reason, so reachability is measured and not assumed.
- Non-vacuity of the properties. Method: committed mutant checkers in
  `rules/global_options_controls.rs`, one per property, each first shown to
  agree with the real checker on a neighbouring input and then shown to be
  rejected: reports an absent concept, or flags `--no-emoji` only when
  `--emoji` is absent (`INV-omission`); deduplicates by name across commands
  (`INV-locality`); skips the sort, or reverses runs of equal paths
  (`INV-order`); appends declared values to the message (`INV-no-echo`); reads
  `context.policy.agent_native` instead of `mode` (`INV-mode`); ignores scope,
  ignores kind, or matches names by prefix (`INV-exception`); treats an empty
  identifying set as a subset of everything, which flags a nested `--columns`
  that declares values; treats any boolean as evidence, which flags a nested
  boolean `--columns`; ignores place; or drops the boolean rule (`INV-exact`).
  Comparator controls drop, duplicate, and reorder one finding.
- `INV-skill`. Method: the existing skill-validation suites pass unchanged;
  four new rows pin the changed messages; a set-equality test pins the
  spellings of the table against a literal list.
- Report shape. Method: `insta` snapshots `global_options_report__warn` and
  `global_options_report__deny`, paired with the semantic assertions above.
- Documentation agreement. Method: `table_tests::users_guide_matches_table`,
  added in EP-M5 and red before the table is written. It reads
  `concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/users-guide.md")`, takes the
  table rows under the heading "Lint global option names" up to the next
  heading, unescapes `\|`, and compares concept, usage, and near misses with
  `GLOBAL_OPTION_CONCEPTS` and `replacement_hint`, ignoring prose such as
  "boolean `--progress`".
- Real pipeline. Method: the EP-M4 end-to-end test and behavioural feature.

Commands: `cargo test -p cargo-orthohelp --lib policy::rules::global_options`
runs the tables, properties, and controls.

### Formal verification decision

Verus is warranted when all three conditions hold: the domain is unbounded; the
property does not factor through a finite quotient that enumeration can
exhaust; and a proof would establish something about the shipped code beyond a
library law or a definition.

| Obligation                                                | Unbounded     | No finite quotient | Proof adds evidence about shipped code  | Choice                                                       |
| --------------------------------------------------------- | ------------- | ------------------ | --------------------------------------- | ------------------------------------------------------------ |
| `L-table`, `L-functional`, `L-converge`, `L-disjoint`     | no            | no                 | no: constant table                      | exhaustive test                                              |
| `verdict` versus `classify` (`INV-exact`)                 | names, values | no                 | no                                      | exhaustive test, plus a property outside the representatives |
| `L-quotient`                                              | values        | yes                | no: set identity on the model           | sketch, plus that property                                   |
| `L-pointwise`, `INV-omission`, `INV-locality`, `INV-mode` | yes           | yes                | no: filter and flat-map laws on a model | property and mutants                                         |
| `INV-order`                                               | yes           | yes                | no: the contract of `slice::sort_by`    | property and mutants                                         |
| `INV-exception`                                           | yes           | yes                | no: `covers` is its own definition      | property and mutants                                         |
| `INV-no-echo`                                             | yes           | yes                | no: the signature of `finding`          | property and mutants                                         |
| `INV-skill`                                               | no            | no                 | no                                      | exhaustive test                                              |

Table 1: Where formal proof is warranted.

No row meets all three conditions, so no obligation is selected for Verus. For
the unbounded rows, a Verus proof would have to be written over a hand-made
sequence model, because Verus does not verify the serde types and iterator
adapters in the executable. It would prove that filter and flat-map distribute
over append, and would say nothing about the realistic defects: a driver that
deduplicates, skips, or miscomputes place. The signature of `classify` rules
out context-peeking inside `classify` only; the mutant checkers address the
driver. Kani would re-explore what enumeration already exhausts. No `assume`,
`admit`, or trusted body is introduced, and no unproved theorem is left in
source.

Triggers that reopen this decision before any code is written: a rule that
relates two inputs or two commands (`L-pointwise` fails, for example "flag
`--no-color` only when `--color` is absent"), name matching beyond equality, or
numeric or ordering logic in the verdict.

### Suggested BDD features

Proposed path: `cargo-orthohelp/tests/features/global_option_policy.feature`,
run by `cargo test -p cargo-orthohelp --test global_option_policy_bdd`:

```gherkin
Feature: Human-facing global option glossary lint

  Scenario: Warn mode reports near misses with their canonical usage
    Given the root command declares the boolean option "no-emoji"
    And the root command declares the boolean option "accessible"
    And the root command declares the option "colour-policy" without values
    When the global option rules check the command surface in "warn" mode
    Then every finding has severity "warn"
    And the replacements are "--emoji auto|always|never", "--accessibility auto|on|off", and "--color auto|always|never"

  Scenario: Deny mode fails a boolean progress option on any command
    Given the command "demo build" declares the boolean option "progress"
    When the global option rules check the command surface in "deny" mode
    Then the report has one "deny" finding "global_option_value" replaced by "--progress auto|always|never"

  Scenario: Structured-output advice keeps domain values
    Given the command "demo report" declares the option "output-format" with values "json,yaml"
    When the global option rules check the command surface in "deny" mode
    Then the report has one "deny" finding "global_option_near_miss" replaced by "--json"
    And the finding says other values may remain

  Scenario: A project may omit every glossary concept
    Given the root command declares the option "target" without values
    When the global option rules check the command surface in "deny" mode
    Then the report has no findings

  Scenario: Canonical options pass
    Given the root command declares the option "color" with values "Auto,Always,Never"
    And the root command declares the option "width" without values
    And the root command declares the boolean option "quiet"
    When the global option rules check the command surface in "deny" mode
    Then the report has no findings

  Scenario: Domain options on subcommands are left alone
    Given the command "demo paint" declares the option "colour" with values "red,green"
        And the command "demo print" declares the boolean option "color"
    And the command "demo plot" declares the boolean option "colour"
    And the command "demo search" declares the boolean option "text"

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
sequentially, and is committed. Unless a milestone says otherwise, its
compatibility decision is "none" and its remaining gaps are the later
milestones. `clippy.toml` limits functions to four parameters and 70 lines.

### `EP-M0` entry check

Rebase onto `origin/main`. Run
`git grep -n "pub struct ExceptionScope" cargo-orthohelp/src` and
`git grep -n "replacement: Option<String>" cargo-orthohelp/src/policy/mod.rs`.
If both are present, mark EP-M1 satisfied and record the commit; if one is
present, do only the missing half. Post a coordination comment on PR #705 (or
its successor) naming A1, A8, A9, and which item owns the foundation. If the
glossary table has changed since `cbba0d03`, stop and reconcile.

### `EP-M1` shared foundation (conditional)

Requirements: `DES-5-exceptions`, D2, D9, D11. Add
`cargo-orthohelp/src/policy/config/scope.rs` with `ExceptionScope`, and extend
`PolicyResult` as written in `Interfaces and dependencies`. Red: `rstest` cases
in `policy/config/tests.rs` (global, scoped match, scoped mismatch, kind
mismatch, dash normalization, unknown wire kind), serialization cases in
`policy/tests.rs` (`None` omits the key; a version-1 report without the key
deserializes), a property in `skill_validation/proptests.rs` (or a sibling file
if it would exceed 400 lines) that `covers` equals a restatement of the old
matcher, and an extension of `cargo-orthohelp/tests/ui/policy_public_api.rs`.
Green: move `names_match` and `scope_matches` out of
`skill_validation/policy_check.rs`, delegate from `allows`, and replace every
`PolicyResult` struct literal, including the compiled example in
`docs/cargo-orthohelp-api-guide.md`. Refactor: delete the moved helpers.
Acceptance: new cases pass, every skill-validation test passes unchanged,
`git diff --stat -- '*.snap'` is empty, and
`cargo test --doc -p cargo-orthohelp policy` passes. Conformance check: no
schema version changes; the migration note and ADR-0008 amendment are queued
for EP-M5. Recovery: revert the commit. Compatibility decision: none (pre-1.0;
no deployed reader rejects an additional optional key). Remaining gaps: no
glossary change.

### `EP-M2` glossary usage text and well-formedness

Requirements: `RM-7.1.3-a`, `GAP-usage`, `DES-5-one-table`, D3. Red: write
`vocabulary/global_options_table_tests.rs` with `table_is_well_formed`, its
broken-table controls, `advice_converges`, `usage_text_matches_design` (one row
per concept, with the text in §5), and `spellings_match_the_plan`; the width
and locale rows fail on the missing placeholder. Green: add `placeholder`, mark
the struct `#[non_exhaustive]`, correct the comment, and render the
placeholder. Add the four skill-validation message rows. Acceptance: the new
tests pass; existing skill-validation tests pass unchanged;
`cargo test --doc -p cargo-orthohelp policy::vocabulary` passes. Recovery:
revert the commit. Remaining gaps: no lint yet.

### `EP-M3` the rule group

Requirements: all roadmap bullets, D1, D4, D5, D7 to D14. Start with a stub
`check_global_options` that returns an empty report, so that each red test
fails by assertion rather than by a compile error. Red: write
`rules/global_options_tests.rs` (exhaustive quotient test and named tables) and
the snapshot tests. Green: implement `rules/global_options.rs` as small private
functions: `normalized_name(&AgentInput)`, `declared_form(&AgentInput)`,
`place(&AgentCommand)`, `identifying_values(&GlobalOptionConcept)`,
`classify(name, place, &declared) -> Option<Violation>`,
`exception_scopes(context)`, and `finding(path, name, violation, severity)`.
Look concepts up with the existing `concept_for_long_name` and
`near_miss_concept`. Share the severity mapping: move the private
`severity_for` from `rules/output.rs` to `rules/mod.rs`, where child modules
can already see it, rather than writing a third copy. Then add the oracle,
properties, and mutant checkers. Keep identifying values, the compound-spelling
rule, and the progress placement rule in this module, with a comment naming D4
and D5. Acceptance:
`cargo test -p cargo-orthohelp --lib policy::rules::global_options` passes;
every control is rejected for its named reason; snapshots are reviewed;
`cargo test --doc -p cargo-orthohelp policy::rules` passes. Conformance check:
no CLI path calls the group; no schema changed; skill validation is untouched.
Recovery: revert; the module is additive.

### `EP-M4` behavioural and end-to-end evidence

Requirements: `RM-7.1.3-a` to `-c`, `AXIOM-derive`, `AXIOM-root`. Add the
feature above, `cargo-orthohelp/tests/global_option_policy_bdd.rs`, and its
`global_option_policy_bdd/steps.rs`, modelled on the skill-validation harness;
neither may reach `cargo_orthohelp_exe`. Add root type
`GlobalOptionsFixtureConfig` (`prefix = "GLOBAL_OPTIONS_FIXTURE"`) in
`tests/fixtures/orthohelp_fixture/src/global_options.rs`, re-exported from the
crate root. Its root declares `colour_policy` (`enum(Auto, Always, Never)`),
`progress` and `accessible` (`bool`), `no_emoji` and `diag_json`
(`Option<bool>`), `output_format` (a custom enum with no override), and
conforming `locale` (`String`) and `quiet` (`bool`). The two `Option<bool>`
fields keep the struct within Clippy's limit of three bools and cover that
mapping. Its `render` subcommand declares `no_emoji` and `accessible` (`bool`),
`columns` (`Vec<String>`), `color` (`enum(red, green)`), and `output_format`
(`enum(png, webp)`). Add `cargo-orthohelp/tests/global_option_policy_e2e.rs`
with one `#[test]` that spawns the built binary once, parses
`agent-context.json`, and checks `warn`, `deny`, `off`, and one scoped wire
exception. Expect exactly eight findings, in this order: on
`global_options_fixture`, `accessible`, `colour-policy`, `diag-json`,
`no-emoji`, `output-format`, `progress`; on `global_options_fixture render`,
`accessible`, `no-emoji`. The invocation is:

```bash
cargo-orthohelp orthohelp --package orthohelp_fixture \
  --root-type orthohelp_fixture::GlobalOptionsFixtureConfig \
  --locale en-US --format agent-context --out-dir "$TEMPDIR"
```

In `.config/nextest.toml`, add the end-to-end binary to the `orthohelp_cache`
override and to the `[[profile.default.scripts]]` filter, beside the existing
agent-context end-to-end binaries. As with those binaries, the warm-up script's
command is not extended, so the new root type's bridge builds inside the test's
960 s allowance. Measure it on a warm tree and record the figure. Acceptance:
both binaries pass; `make test-workflow-contracts` passes; golden snapshots for
other root types are unchanged. Recovery: revert; the fixture type is additive.

### `EP-M5` documentation and closure

Update:

- A new ADR (next free number), "Global option glossary lint scope": D1, D4, D5,
  D6, D7, the formal-verification criterion and its reopening triggers, and the
  residual limits. Index it in `docs/contents.md` and reference it from design
  §5.
- `docs/agent-native-cli-design.md` §5: rule identifiers, placement and
  evidence, the boolean rule, what is not checked; replace "The roadmap 7.1.3
  policy pass will read the same table". §3.4.1: one sentence that the context
  lint scopes by placement and declared evidence, while skill validation keeps
  supplied classification.
- `docs/adrs/adr-0008-agent-native-policy-configuration.md`: an amendment
  recording D11's definition of `replacement`, and, if EP-M1 ran here, the
  `PolicyResult` change.
- `docs/cargo-orthohelp-check-gen-design.md` §6: name
  `check_global_options(context, mode)` as a group 7.2.8 composes, state that
  `apply_policy_to_context` must run first or exceptions silently vanish, and
  propose "commands that declare at least one input with a long name" as the
  group's applicable count for 7.2.8 to accept or replace.
- `docs/users-guide.md`: a section "Lint global option names" after "Lint the
  declared behaviour" (or after 7.1.2's section), with a compiled example
  `guide-check-global-options` registered in the documentation-example files,
  the glossary table, both rules, the omission rule, the exception recipe (copy
  `path` from `agent-context.json`), the hidden-option migration path, and the
  limits. Note the changed width and locale usage text under "Validate skill
  manifests against commands".
- `docs/developers-guide.md`: the glossary paragraph (two consumers; how to add
  a spelling; lint scoping stays in the rule module), convention 3 of the
  skill-validation internals (this lint differs, and why), "Policy rule API
  testing" (new files), the three assertion-crate passages (A7), the nextest
  group list and count, the sentence that a nested-crate binary belongs in both
  the warm-up command and the filter, the stale sentence that the 7.1.2 rules
  do not import the vocabulary, and the `ExceptionScope` re-use policy if EP-M1
  ran.
- `docs/cargo-orthohelp-api-guide.md`: a subsection beside the structured-output
  rule, and the `PolicyReport::exceptions` field comment in code (D9).
- `README.md` and `ortho_config_macros/README.md`: only if their "Next" wording
  becomes stale.
- `CHANGELOG.md` under `Unreleased`; a migration note if EP-M1 ran, in an
  unreleased migration guide if one exists, otherwise in a new
  `docs/v0-12-0-migration-guide.md` indexed in `docs/contents.md`.
- `docs/roadmap.md`: mark 7.1.3 and its three sub-items done.

Acceptance: `make fmt` leaves a clean tree, all gates pass, and every example,
path, and identifier is checked against the code.

## Concrete steps

Run from the repository root, sequentially. After Markdown edits, run
`make fmt` first.

```bash
set -o pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-ortho-config-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-ortho-config-${BRANCH}.out"
cargo test --doc -p cargo-orthohelp 2>&1 | tee "/tmp/doctest-ortho-config-${BRANCH}.out"
make test-workflow-contracts 2>&1 | tee "/tmp/contracts-ortho-config-${BRANCH}.out"
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

Expected red output for EP-M3, with the stub in place:

```plaintext
test policy::rules::global_options_tests::roadmap_spellings::case_… ... FAILED
```

Expected green output:

```plaintext
test policy::rules::global_options_tests::roadmap_spellings::case_… ... ok
...
test result: ok.
```

No Verus command is an acceptance criterion because no proof is selected. If
the decision is reopened, the milestone that adds a proof must register it in
`VERUS_PROOF_FILES` and add `make verus` to its acceptance; today `make verus`
exits 2 because its registry is empty.

## Validation and acceptance

The work is accepted when all of the following are observable:

- `check_global_options` over the fixture's real agent context reports exactly
  the eight findings in EP-M4, in order, in `warn` and `deny`; none in `off`;
  and seven with a `flag` exception for `no-emoji` scoped to
  `global_options_fixture render`.
- Each finding's `replacement` is the §5 usage text and its message follows D14.
- A context that declares no glossary concept has no findings in `deny`.
- The `render` subcommand's `--color red|green`, `--columns`, and
  `--output-format png|webp` produce no finding.
- Skill validation reports the same findings as before, with four changed
  messages.
- Every mutant checker is rejected for its named reason.
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

At the end of EP-M1 (or on `main` already) these items exist:

```rust
// cargo-orthohelp/src/policy/config/scope.rs
/// Borrowed view of one exception, shared by every rule group and validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionScope<'a> {
    pub kind: ExceptionKind,
    pub name: &'a str,
    pub command_path: Option<&'a str>,
}
impl<'a> ExceptionScope<'a> {
    /// Flag names compare after stripping every leading `-`; verb names compare
    /// exactly. No `command_path` covers every path; otherwise it must equal
    /// `path` joined with single spaces.
    #[must_use]
    pub fn covers(&self, kind: ExceptionKind, name: &str, path: &[String]) -> bool;
    /// `None` for a wire kind other than `verb` or `flag`.
    #[must_use]
    pub fn from_wire(
        exception: &'a ortho_config::agent_context::PolicyException,
    ) -> Option<Self>;
}
impl<'a> From<&'a PolicyException> for ExceptionScope<'a> { /* … */ }

// cargo-orthohelp/src/policy/mod.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct PolicyResult {
    pub rule_id: String,
    pub code: String,
    pub severity: PolicySeverity,
    pub message: String,
    #[serde(default)]
    pub location: Option<SourceLocation>,
    /// Canonical usage: the first whitespace-delimited token is the canonical
    /// spelling; any remainder describes its value.
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
```

`policy/config/mod.rs` declares `pub mod scope;`, and `policy/mod.rs` re-exports
`pub use config::scope::ExceptionScope;`. The signatures omit documentation
for brevity: `missing_docs` is denied, so every public item, field, and method
gets a doc comment, and each method a doc example. `scope.rs` joins the path
itself and must not import `skill_validation`.

At the end of EP-M3 these items exist as well:

```rust
// cargo-orthohelp/src/policy/vocabulary/global_options.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GlobalOptionConcept {
    pub concept: &'static str,
    pub canonical: &'static str,
    /// Closed policy values; empty when the option is a switch or takes a free
    /// value.
    pub accepted_values: &'static [&'static str],
    /// Placeholder for a free value, without angle brackets; `None` otherwise.
    pub placeholder: Option<&'static str>,
    pub near_misses: &'static [&'static str],
}

// cargo-orthohelp/src/policy/rules/global_options.rs
#[must_use]
pub fn check_global_options(
    context: &ortho_config::AgentContext,
    mode: PolicyMode,
) -> PolicyReport;
```

The table's spellings are unchanged from `cbba0d03`. For reference:

| Concept           | Canonical usage                  | Near misses                                       |
| ----------------- | -------------------------------- | ------------------------------------------------- |
| structured-output | `--json`                         | `--output-format`, `--diag-json`, `--json-output` |
| plain-output      | `--plain`                        | `--text`, `--plain-output`                        |
| colour            | `--color auto\|always\|never`    | `--colour`, `--colour-policy`, `--color-policy`   |
| emoji             | `--emoji auto\|always\|never`    | `--no-emoji`, `--emoji-policy`                    |
| progress          | `--progress auto\|always\|never` | `--progress-policy`; boolean `--progress`         |
| accessibility     | `--accessibility auto\|on\|off`  | `--accessible`, `--accessibility-mode`            |
| pager             | `--no-pager`                     | `--no-paging`, `--disable-pager`                  |
| width             | `--width <columns>`              | `--columns`, `--terminal-width`                   |
| locale            | `--locale <locale>`              | `--lang`, `--language`                            |
| quiet             | `--quiet`                        | `--silent`                                        |
| verbose           | `--verbose`                      | none                                              |

Table 2: The glossary after EP-M2.

Reuse policy: the table is the only owner of spellings, accepted values, and
usage text; the skill validator and the rule group look concepts up and never
restate them. `ExceptionScope` is the only exception matcher. Placement,
evidence, and identifying values belong to the rule group and are private; a
second consumer that needs them must first move them to a shared module under
`policy/rules/`. No dependency is added.

## Revision note

- 2026-11-24: initial draft.
- 2026-11-24: applied the community-of-experts review. The per-spelling shape
  and reach types, the `--output-format` value predicate, value-set comparison,
  and the three added spellings were removed; placement plus declared evidence
  replaces them (D3 to D7). The abstract projection is now total, and
  `INV-locality`, the no-echo clause, and convergence are restated correctly
  after counterexamples. The classifier is checked exhaustively through
  `L-quotient`; properties are tested against committed mutant checkers; the
  Verus decision follows an explicit criterion. `replacement` has one
  definition (D11). EP-M1's interfaces are written out instead of cited. The
  Netsuke evidence, the fixture's boolean count, nextest entries,
  workflow-contract gate, assertion-crate scope (D16), and budgets were
  corrected. `Approval requested` lists the decisions that touch accepted
  documents or PR #705.
- 2026-11-24: applied the second review round. Nested single-word booleans are
  no longer evidence, declared values without `json` rule structured output
  out, the advice template can no longer contradict the rule, the Verus table's
  third criterion was corrected, the test-module path avoids a denied lint, the
  exhaustive totals are pinned from a scripted model, and A8 and A9 were added.
  Remaining work: user approval, then EP-M0.
