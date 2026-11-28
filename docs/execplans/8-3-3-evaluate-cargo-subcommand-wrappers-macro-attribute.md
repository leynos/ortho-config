# 8.3.3 Evaluate an optional macro attribute for Cargo subcommand wrappers

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review and user approval; no implementation has
started)

Branch: `8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute`

Roadmap item: `8.3.3` in `docs/roadmap.md`.

## Purpose / big picture

Cargo runs an installed tool named `cargo-<name>` whenever a user types
`cargo <name> [OPTIONS]`. Cargo does not pass the options through unchanged: it
inserts the subcommand name as the first argument, so the tool receives
`["cargo-<name>", "<name>", OPTIONS...]`. A `clap` parser that models only
`cargo-<name> [OPTIONS]` rejects that injected `<name>` token before any
application code runs. This "injected token" rule is the *Cargo dispatch
contract*.

OrthoConfig already supports the contract in two ways. Roadmap item 8.3.1
shipped a helper, `ortho_config::cargo::external_subcommand`, which wraps a
hand-built `clap::Command` in a synthetic `cargo` parent. Roadmap item 8.3.2
documented a derive template: a `#[derive(clap::Parser)]` struct named `Cli`
with `#[command(name = "cargo", bin_name = "cargo-<name>")]` and a
single-variant `#[derive(clap::Subcommand)]` enum wrapping the existing options
struct. `docs/design.md` §4.17 also floats a third, unbuilt option: a derive
attribute

```rust
#[ortho_config(cargo_subcommand = "orthohelp", cargo_bin = "cargo-orthohelp")]
```

that would let the `OrthoConfig` derive macro write the wrapper.

This plan answers one question: should that attribute exist? It does so by
prototyping the attribute's candidate expansions against the real
OrthoConfig-powered Cargo tools, scoring them against criteria fixed before the
prototype runs, and recording the decision in an architectural decision record
(ADR). The roadmap's success criterion is that "the design records either a
narrow accepted macro surface or a clear reason to keep the helper and
documentation as the only supported abstraction".

After this work a maintainer reading `docs/design.md` §4.17 sees a decision,
not an open candidate. A downstream author who tries the floated syntax gets a
compiler error that names the supported alternatives. Today the attribute
compiles and silently does nothing; see `Surprises & discoveries`. No runtime
behaviour, configuration precedence, or existing public API changes on the
rejection path. If the evidence instead favours a macro, the plan stops and
returns a specific, narrow surface for approval before any public API is added.

The reconnaissance already points one way (prediction P1 in `Decision log`).
The plan still runs the prototype and scores every candidate at its strongest,
so the evidence, not the prediction, decides.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
  A design-review pass by a community-of-experts team precedes that approval;
  its findings are folded into this document.
- Do not move Cargo dispatch semantics into the `OrthoConfig` trait,
  `OrthoConfig::load`, or the merge pipeline (`docs/design.md` §4.17;
  ADR-0004). Configuration precedence stays defaults → files → selected profile
  (opt-in) → environment → explicit command-line arguments.
- Preserve the public signature and behaviour of
  `ortho_config::cargo::external_subcommand`
  (`ortho_config/src/cargo/mod.rs:105`). Preserve the documented derive
  template and its tested examples (`docs/users-guide.md` §"Derive-based
  template", `README.md` §"Cargo subcommands").
- Preserve accepted ADR decisions. ADR-0004 is not rewritten; any change to
  its decision would go through the amendment process in
  `docs/documentation-style-guide.md` and requires escalation.
- Do not add a production dependency. Do not add a dev-dependency to
  `ortho_config_macros`. `ortho_config` already has `rstest`, `rstest-bdd`,
  `googletest`, `pretty_assertions`, `insta`, `trybuild`, and `proptest` as
  dev-dependencies (`ortho_config/Cargo.toml:56-73`).
- The rejection path may add exactly one behavioural change: a targeted
  compile error for the struct-level keys `cargo_subcommand` and `cargo_bin`
  (decision D6). Every other unknown struct-level key stays silently discarded,
  as `parse_struct_attrs` documents
  (`ortho_config_macros/src/derive/parse/struct_attrs.rs:309-316`).
- No Rust source file may exceed 400 lines (`AGENTS.md`).
  `ortho_config_macros/src/derive/parse/struct_attrs.rs` is already at 397
  lines, so the diagnostic goes in a new module, and the edit to
  `struct_attrs.rs` must be line-neutral.
- Follow `AGENTS.md` test rules: every assertion carries a diagnostic
  message; environment is injected, never mutated in process; child processes
  alone may set environment variables.
- The prototype is evidence, not product. It lives in one integration-test
  target that is deleted, not promoted, on the rejection path. It must pass
  every gate while it exists.
- Completed ExecPlans (`8-3-1-…`, `8-3-2-…`) are historical and must not be
  edited.
- Run gates sequentially through a `scrutineer` agent with `tee` logs under
  `/tmp`. Respect compile admission: never bypass `RUSTC_WRAPPER`, never use an
  isolated Cargo cache, and never build under `/tmp`.
- `typos.toml` is generated; put any spelling exception in
  `typos.local.toml` only.

## Tolerances (exception triggers)

ExecPlan files are excluded from every count below, including this file.

Survey basis. The 8.3.1 plan set one numeric budget: about 150 net production
lines and at most 10 files. Its Outcomes record about 120 production lines, and
no increase was needed. The 8.3.2 plan set only qualitative triggers. Its scope
grew through milestones M3 to M5 (recipes, a merge example, a build test)
without a recorded tolerance breach, and recipes pinned to 0.10.0 broke on the
0.11.0 merge commit. Those histories inform two choices here. Each category
gets a numeric limit, so unrecorded growth trips an escalation. No
documentation example hard-codes a release version.

The comparison base is `git merge-base HEAD origin/main`. Counting command:

```bash
base=$(git merge-base HEAD origin/main)
git diff --numstat "$base" -- . \
  ':!docs/execplans/8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute.md'
```

Classification is by whole file. Code means `*.rs` files and trybuild `.stderr`
files. Configuration means `Cargo.toml`, `Cargo.lock`, `Makefile`, `.config/**`,
`.github/**`, and `typos.local.toml`. Documentation means every other `*.md`
file. `typos.toml` is generated and excluded; if it changes, commit it
separately under `AGENTS.md` §Spelling.

- Files: stop and escalate if the final diff touches more than 14
  non-ExecPlan files.
- Code (net lines at the end of the work): stop at more than 150. Expected
  use is about 60: the diagnostic module, its unit tests, and one trybuild pair.
- Prototype (transient code, measured at its own commit): stop at more than
  600 lines across the prototype target, or any single file over 400. The
  prototype is deleted before delivery, so it does not count towards the final
  code budget.
- Configuration: stop at more than 0 net lines. No manifest or workflow
  change is planned.
- Documentation (net lines): stop at more than 450. Expected use is about
  320: the ADR (~190), design §4.17 (~25), the user's guide (~25), the
  developers' guide (~25), the CHANGELOG (~10), `docs/contents.md` (~6), and
  the roadmap (~10).
- Interface: stop if any public item outside the diagnostic text must
  change.
- Architecture: stop if the evidence favours candidate D (making
  `OrthoConfig::load` accept the injected token). Stop if it favours any
  candidate that passes the acceptance rubric. Either outcome sets `BLOCKED`,
  because it changes the protected boundary in ADR-0004 or adds a public macro
  surface.
- Iterations: stop after three unsuccessful repairs of the same gate.
- Infrastructure: stop on disk exhaustion or a `build-limits:` refusal.
  Waiting for an admitted compile slot is normal and is not a retry trigger.
- Ambiguity: stop if two consumers' shapes support conflicting readings of a
  rubric gate. Present both readings.

## Risks

- Risk: confirmation bias. Reconnaissance already predicts rejection (P1),
  so a weak prototype could "confirm" the prediction. Severity: high.
  Likelihood: medium. Mitigation: the rubric (D2) is fixed before EP-M1. Each
  candidate is prototyped in its strongest reasonable form (D1). The line
  census is recounted independently by a read-only `wyvern` agent that is not
  shown the prediction.
- Risk: estate drift. The consumer census reads other repositories, which
  keep moving. Severity: medium. Likelihood: high. Mitigation: pin each
  repository to a commit printed by `gh api` (`Context and orientation`), and
  cite only pinned line ranges.
- Risk: incomplete census. Only repositories under the `leynos` account were
  searched. Severity: low. Likelihood: medium. Mitigation: state the search
  command and its scope in the ADR. The rubric's repetition gate counts known
  consumers only and says so.
- Risk: the new diagnostic breaks a downstream crate that already wrote the
  floated syntax. Today such code compiles and silently does nothing. Severity:
  low. Likelihood: low. Mitigation: run
  `gh search code 'cargo_subcommand' --owner leynos` before EP-M3 and record
  the result. The crates are pre-1.0, so no compatibility shim is warranted;
  the CHANGELOG entry names the change.
- Risk: trybuild `.stderr` files go stale on toolchain bumps (repository
  memory: rustc 1.98 reworded E0277). Severity: low. Likelihood: medium.
  Mitigation: the diagnostic is a `syn::Error` with fixed text, so the
  `.stderr` holds the project's own message and span, not rustc wording.
- Risk: `struct_attrs.rs` sits at 397 lines. Severity: low. Likelihood: high.
  Mitigation: route the `_` arm's existing `discard_unknown(meta)` call through
  the new module, so the edit adds no net lines.
- Risk: the prototype target adds a cold compile to `make test` while it
  exists. Severity: low. Likelihood: high. Mitigation: one target with
  `#[path]` submodules, and deletion in EP-M2.

## Progress

- [x] (2026-11-28) Branch renamed; Lody session titled.
- [x] (2026-11-28) Wyvern reconnaissance: helper and consumers, derive
  attribute machinery, documentation and prior plans, test tooling.
- [x] (2026-11-28) Prior-art research (Cargo book, clap cookbook,
  `clap-cargo`) and estate census of OrthoConfig-powered `cargo-*` tools.
- [x] (2026-11-28) Initial draft written.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Prototype and census evidence committed (prototype target
  green under all gates).
- [ ] `EP-M2` Decision gate scored; ADR-0028 drafted as `Proposed`;
  prototype retired (rejection path) or plan set `BLOCKED` (acceptance path).
- [ ] `EP-M3` Targeted diagnostic for the rejected keys (rejection path).
- [ ] `EP-M4` Design, guides, CHANGELOG, index, and ADR-0028 `Accepted`;
  roadmap 8.3.3 marked done.
- [ ] `EP-M5` Delivery: PR ready for review, CI green.

## Surprises & discoveries

- Observation: the floated syntax already compiles and silently does
  nothing. Evidence: `parse_struct_attrs` sends unrecognized struct-level keys
  to `discard_unknown`
  (`ortho_config_macros/src/derive/parse/struct_attrs.rs:365-373`, `43-52`).
  The unit test `parse/tests/ortho_attrs_unknown_keys.rs:9-19` asserts that
  `unknown = "ignored"` is accepted. Impact: a reader of design §4.17 can write
  `#[ortho_config(cargo_subcommand = "x")]` today, believe a wrapper was
  generated, and find out only when Cargo dispatch fails at run time. This
  motivates D6.
- Observation: none of the three real Cargo tools would be a legal host for
  the attribute. The attribute belongs to `#[derive(OrthoConfig)]`, but the
  type each tool wraps in its Cargo entry shape does not derive `OrthoConfig`.
  Evidence: `cargo-orthohelp` `Args` derives only `ClapArgs, Clone`
  (`cargo-orthohelp/src/cli/mod.rs:73-74`). `cargo-bdd` wraps `Cli`, which
  derives only `Parser`. `cargo-theorem` wraps `TheoremArgs`, which derives only
  `Args` and implements `OrthoConfigDocs` by hand (pinned lines in
  `Context and orientation`). Impact: rubric gate G1 is likely to fail, which
  makes P1 concrete.
- Observation: the consumers disagree on the shape the macro would
  generate. `cargo-orthohelp` and `cargo-theorem` use `bin_name = "cargo"`, so
  usage reads `Usage: cargo <name> …`. The documented derive template uses
  `bin_name = "cargo-<name>"`. `cargo-bdd` parses both the Cargo-injected form
  and a flat direct form, and records which one it saw. `cargo-theorem` puts
  `version` on both levels, derives `OrthoConfigSubcommandDocs` on the wrapper
  enum, and parses through `parse_localized_command`. Impact: a generated
  wrapper needs a knob for each of these variance points (rubric gate G3).
- Observation: the only consumer-side repetition that is not already one
  line of `clap` is documentation metadata. `cargo-theorem`'s
  `command_metadata()` patches a `DocMetadata` by hand to describe the
  `cargo-theorem` → `theorem` → `context` tree for agent-context output (11
  lines). Impact: candidate C (metadata only) has a real motivating case, but a
  plain function would serve it equally well (D9).
- Observation: clap's own cookbook (`cargo_example_derive`) expresses the
  whole wrapper as one `#[derive(Parser)]` enum with
  `#[command(name = "cargo", bin_name = "cargo")]`, unpacked by an irrefutable
  `let CargoCli::Example(args) = CargoCli::parse();`. That is five lines plus
  the variant. Impact: the floor for "boilerplate a macro can remove" is lower
  than the 8.3.2 template's struct-plus-enum spelling.

## Decision log

- P1. Prediction (pre-registered, not a decision): every candidate fails at
  least one rubric gate, and the outcome is to keep the helper and the
  documented templates as the only supported abstraction. The plan treats this
  as a hypothesis that EP-M1 can falsify. If any candidate passes G1 to G5, the
  prediction is wrong and the acceptance path runs.
- D1. Decision: prototype by handwritten expansion, not by implementing
  the macro first. Each candidate's expansion, meaning the code the macro would
  emit, is written by hand in one disposable integration-test target
  (`ortho_config/tests/cargo_wrapper_prototype.rs`, with submodules under
  `ortho_config/tests/cargo_wrapper_prototype/`). It is applied to faithful
  reductions of the three consumers' entry shapes. Rationale: the open
  questions are about the expansion and its fit to consumers, not about
  attribute parsing. The repository already parses comparable keys, with
  precedents in `struct_attrs.rs` and `doc_attrs.rs`. Writing a throwaway
  proc-macro would spend compile slots and review effort on code the evidence
  may discard. The attribute syntax is still prototyped at the call site: each
  port is written with the attribute in the position the candidate requires,
  inside a doc comment, so legality of the host is visible. Each port also
  carries a compile probe proving the attribute currently has no effect. If the
  decision is to accept, the real macro is implemented under a revised plan.
- D2. Decision: score candidates against a rubric fixed in advance (see
  `Verification plan`, obligations E1 to E5). Accept a candidate only if it
  passes all five gates: G1 legal host, G2 net savings, G3 variance coverage,
  G4 visible dispatch contract, and G5 ADR-0004 conformance. Rationale: the
  roadmap's rejection rule is "unless it removes real repeated boilerplate
  across multiple OrthoConfig-powered Cargo tools without hiding the Cargo
  dispatch contract". G1 to G3 operationalize "real repeated boilerplate across
  multiple tools". G4 and G5 operationalize "without hiding the contract".
- D3. Decision: the evidence base is the estate census, meaning every Cargo
  tool under the `leynos` account that depends on `ortho_config`, pinned to
  printed commits. Three qualify: `cargo-orthohelp` (this repository),
  `cargo-bdd` (`leynos/rstest-bdd`), and `cargo-theorem` (`leynos/theoremc`).
  The documentation-harness `cargo-demo` binaries are templates, not
  independent tools; they are reported separately and do not count towards
  "multiple tools". Rationale: the roadmap requires repetition across tools,
  and templates repeat by construction.
- D4. Decision: candidate D is out of scope. Candidate D would let the
  generated `load()` accept the injected token by wrapping the hidden
  `__<Name>Cli` parser. It is recorded and scored for completeness only.
  Rationale: design §4.17 and ADR-0004 place dispatch at the command boundary,
  outside `OrthoConfig::load`. If D scores as the only viable candidate, that
  is an architecture deviation: set `BLOCKED` and escalate.
- D5. Decision: record the outcome in a new ADR,
  `docs/adrs/adr-0028-cargo-subcommand-wrapper-attribute.md`, referenced from
  design §4.17. Do not amend ADR-0004. Rationale: rejecting (or narrowly
  accepting) a new macro surface is a decision in its own right. It does not
  correct ADR-0004, whose decision stands either way. The ADR is `Proposed` at
  EP-M2 and becomes `Accepted` at EP-M4, under the style guide's status rules.
- D6. Decision (rejection path): make `cargo_subcommand` and `cargo_bin` at
  struct level a targeted compile error that names
  `ortho_config::cargo::external_subcommand` and the derive template. Keep
  every other unknown key silently discarded. Rationale: the design document
  publicized this exact syntax, and today it fails silently, the worst outcome
  for a Cargo tool, because the failure surfaces only under real Cargo
  dispatch. The repository already rejects one specific known-wrong key, the
  misspelt `behavior` (`doc_attrs.rs:35`, `47-55`), while keeping forward
  compatibility for others. D6 follows that precedent exactly. Alternative
  considered: no diagnostic, documentation only. Rejected, because
  documentation cannot reach a user who copied the syntax from design §4.17 or
  an older guide.
- D7. Decision: no Verus or Kani obligation. See `Verification plan`.
  Rationale: the rejection path introduces no invariant over an unbounded
  domain, no state machine, and no arithmetic or `unsafe` code. Its only
  behavioural change is a finite, two-key compile-time diagnostic, which an
  `rstest` table and a trybuild case cover exhaustively.
- D8. Decision: the prototype is committed (EP-M1), gated, scored, and then
  deleted in a separate commit (EP-M2). Its measured results and the key
  expansions are reproduced in ADR-0028 §"Evidence", so the decision survives a
  squash merge. Rationale: the git history gives a reviewable rollback point,
  and the ADR gives durable evidence. No prototype code ships.
- D9. Decision: if the rejection path runs and candidate C's motivating case
  (`cargo-theorem`'s hand-built `DocMetadata`) stands, ADR-0028 lists a
  follow-up option under "Outstanding decisions": a plain library function that
  wraps a `DocMetadata` tree in its Cargo entry shape. The follow-up is not
  implemented here. Rationale: 8.3.3 decides about a macro attribute; a docs
  helper is a separate public API that needs its own roadmap entry and
  approval. Whether to add that roadmap entry is approval question Q2.

## Outcomes & retrospective

Not started. To be completed at EP-M5 with the decision, the rubric scores,
final tolerance usage per category, and lessons learned.

## Context and orientation

The repository is a Cargo workspace (root `Cargo.toml:2-14`). The parts that
matter here are:

- `ortho_config/src/cargo/mod.rs` (135 lines). The helper
  `external_subcommand(installed_bin_name, subcommand_name, command)` returns
  `Command::new("cargo").bin_name("cargo").subcommand_required(true)` with the
  renamed inner command as its only subcommand. The inner command's `bin_name`
  is reset and its `display_name` is set to the installed name. Debug
  assertions require a non-empty name, a name other than `help`, and
  `installed_bin_name == "cargo-<name>"`. The helper has no tracing and reads
  no environment.
- Helper tests: `ortho_config/src/cargo/tests.rs` (unit tests),
  `ortho_config/src/cargo/proptests.rs:125` (property
  `wrapper_preserves_generated_leaf_command_matches`),
  `ortho_config/tests/features/cargo_entry_point.feature` (two rstest-bdd
  scenarios, wired at
  `ortho_config/tests/rstest_bdd_support/behaviour/scenarios.rs:17`), and
  `ortho_config/tests/cargo_entry_point.rs` (three insta snapshots).
- Documented templates: `docs/users-guide.md` §"Cargo external-subcommand
  entry points" (line 780 onwards), with tested examples
  `guide-cargo-external-subcommand`, `guide-cargo-derive-subcommand`,
  `guide-cargo-derive-merge`, and `guide-cargo-error-hints`. `README.md`
  §"Cargo subcommands" (lines 158-244). The harness tests are
  `ortho_config/tests/documentation_examples/cargo_derive.rs` and
  `cargo_flows.rs`.
- Derive attribute parsing: `ortho_config_macros/src/derive/parse/`.
  `StructAttrs` is in `mod.rs:82-103`. `parse_struct_attrs` is in
  `struct_attrs.rs:320-387` (397 lines total). Struct-level documentation keys,
  and the `behavior` precedent, are in `doc_attrs.rs` (321 lines). Parser unit
  tests are registered in `parse/tests/mod.rs`. Compile-fail cases are
  `.rs`/`.stderr` pairs in `ortho_config/tests/ui/`, run by
  `ortho_config/tests/compile_fail.rs`. Regenerate a `.stderr` with
  `TRYBUILD=overwrite cargo test -p ortho_config --test compile_fail`, then
  rerun without `TRYBUILD` (`docs/developers-guide.md:3729-3734`).
- Companion code the derive already emits: a hidden
  `#[derive(clap::Parser)]` struct `__<Name>Cli`
  (`ortho_config_macros/src/derive/generate/structs.rs:50-69`). No
  `clap::Subcommand` type is generated anywhere today.
- Documentation IR: `ortho_config/src/docs/ir/mod.rs:36-80` (`DocMetadata`,
  with `app_name`, `bin_name: Option<String>`, and no Cargo field).
  `cargo-orthohelp` consumes `bin_name` in `src/generation.rs:217-228` and
  `src/agent_context/mod.rs:316-321`.

The estate census is pinned to the commits that `gh api` printed on 2026-11-28.
Re-print them with:

```bash
gh api repos/leynos/theoremc/commits/HEAD --jq .sha
gh api repos/leynos/rstest-bdd/commits/HEAD --jq .sha
git rev-parse origin/main
```

At drafting time these were `cd2cffa13614555bb2aaf369a0714782b3e9d256`
(theoremc), `470c20ae27ef6ed9e97d361c738666a8cad22175` (rstest-bdd), and
`de2e7ae1d0ac42dfe8adf02cd9dad944283970d3` (this repository's main). The census
found:

- `cargo-orthohelp`, `cargo-orthohelp/src/cli/mod.rs:53-70`. A derive
  parent with `bin_name = "cargo"` and `version`, plus a single-variant enum
  `Orthohelp(Args)`. `src/main.rs:76-83` parses through `try_get_matches` and
  `from_arg_matches` so it can add invocation hints (`main.rs:85-111`).
  `main.rs:138-141` destructures the variant and emits the dispatch-boundary
  debug event.
- `cargo-bdd`, `crates/cargo-bdd/src/cli/invocation.rs:43-78` at the
  rstest-bdd commit above. It uses
  `external_subcommand("cargo-bdd", "bdd", Cli::command())` only when
  `argv[1] == "bdd"`, and otherwise parses the flat direct form. It records an
  `InvocationForm` that later error paths reuse
  (`missing_required_skipped_error`). The wrapper-specific glue, meaning
  `subcommand_matches` plus a "lost `bdd`" error, is about 12 lines.
- `cargo-theorem`, `crates/cargo-theorem/src/cli.rs:98-127` and `186` at the
  theoremc commit above. A derive parent `CargoEntry` (`bin_name = "cargo"`,
  `version`, `subcommand_required = true`) and an enum `CargoCommand` deriving
  `OrthoConfigSubcommandDocs`, with `#[command(name = "theorem", version)]` on
  the variant. It parses through `parse_localized_command::<CargoEntry, _, _>`.
  `command_metadata()` (lines 117-127) hand-patches `DocMetadata` for agent
  context.

Prior art (Firecrawl, 2026-11-28):

- The Cargo book, "External tools", §"Custom subcommands", states the
  contract: the first argument is the binary's file name, followed by the
  subcommand name, then the forwarded arguments. `cargo help <name>` runs
  `cargo-<name> <name> --help`. Cargo sets the `CARGO` environment variable.
- clap 4.6's cookbook `_derive::_cookbook::cargo_example_derive` uses a
  single `#[derive(Parser)]` enum as the whole wrapper.
- `clap-cargo` 0.19 provides reusable `Manifest`, `Workspace`, and
  `Features` flag structs and a `style` module. It generates no wrapper.
- No crate was found that generates a Cargo external-subcommand wrapper
  from an attribute. The search terms are recorded in the ADR's evidence.

Relevant documentation, which an implementer should read before starting:
`docs/design.md` §4.17;
`docs/adrs/adr-0004-cargo-external-subcommand-entry-point.md`;
`docs/users-guide.md` §"Cargo external-subcommand entry points";
`docs/developers-guide.md` §"Subcommand dispatch changes" (lines 503-517);
`docs/documentation-style-guide.md` §ADRs (line 520 onwards);
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`;
`docs/rust-doctest-dry-guide.md`;
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/clap-dispatch-and-ortho-config-integration.md`;
`docs/cargo-orthohelp-design.md` §6; `docs/agent-native-cli-design.md`
(agent-context consumers of `DocMetadata`);
`docs/improved-error-message-design.md` (diagnostic wording);
`docs/cli-localization-design.md` and
`docs/localizable-rust-libraries-with-fluent.md` (the localized parse path that
`cargo-theorem` uses, variance point v5); `docs/scripting-standards.md` (only
if a counting script is added; none is planned).

Skills: `rust-router`, routing to `rust-types-and-apis` for the surface
decision and `rust-unit-testing` for the `rstest` and trybuild tables;
`arch-decision-records` for ADR-0028; `codegraph-mcp` for call-site analysis;
`execplans`; `en-gb-oxendict-style` for prose.

## Plan of work

### EP-M0: approval gate (no code)

Apply the community-of-experts review to this draft, then present it for
approval. Answer approval questions Q1 and Q2, listed at the end of this
section, before EP-M1. No repository code changes in EP-M0.

### EP-M1: prototype and census (prototyping milestone)

Create one integration-test target,
`ortho_config/tests/cargo_wrapper_prototype.rs`. Its module comment states that
it is a disposable 8.3.3 prototype, and names this plan and the commit that
will delete it. Submodules live under
`ortho_config/tests/cargo_wrapper_prototype/` and are attached with `#[path]`,
following the repository convention for long integration tests.

The target contains three consumer reductions and four candidate expansions.

The consumer reductions are minimal but faithful copies of each tool's entry
shape, keeping every variance point (v1 to v7 below) and dropping business
logic. They are `orthohelp.rs`, `bdd.rs`, and `theorem.rs`. Each keeps its
baseline shape exactly as the consumer writes it today.

The candidate expansions are handwritten as the macro would emit them for the
port's options type:

- Candidate A, companion wrapper parser. A visible, documented
  `#[derive(clap::Parser)]` enum named `<Name>Cargo`, with
  `#[command(name = "cargo", bin_name = …)]` and a single variant wrapping the
  options type. It also generates
  `impl <Name>Cargo { fn into_inner(self) -> <Name> }`.
- Candidate B, helper function. A generated associated function
  `<Name>::cargo_command() -> clap::Command` that returns
  `external_subcommand(cargo_bin, cargo_subcommand, <Name>::command())`,
  together with
  `<Name>::cargo_matches(&ArgMatches) -> Result<&ArgMatches, clap::Error>` for
  the inner-matches extraction that `cargo-bdd` and the merge example repeat.
- Candidate C, metadata only. A generated
  `const CARGO_ENTRY: ortho_config::cargo::CargoEntry` (a prototype-local type)
  and the `DocMetadata` wrapping it would let documentation tooling emit. This
  is measured against `cargo-theorem::command_metadata()`.
- Candidate D, generated `load()` accepting the injected token. This is
  scored on paper only (D4), and not compiled.

For each port, one `#[rstest]` table runs the baseline and each applicable
candidate over the same argv cases: Cargo-injected, direct-with-token, help,
and version. It asserts equal outcomes and equal rendered usage strings, using
`pretty_assertions::assert_eq!` and `googletest` matchers. A negative control
parses the Cargo-injected argv with the flat options parser and asserts that it
fails with `ErrorKind::UnknownArgument`, which proves the table detects a
missing wrapper. A compile probe, `cargo_attribute_is_silently_ignored_today`,
derives `OrthoConfig` on a struct carrying
`#[ortho_config(cargo_subcommand = "demo", cargo_bin = "cargo-demo")]`. It then
asserts that the struct loads through `load_from_iter(["demo"])` exactly as the
same struct without the attribute does. That shows the attribute is accepted
and has no effect. The probe is the red evidence for EP-M3, and EP-M3's
trybuild case replaces it when the prototype is retired.

Count the wrapper lines with the rule in `Verification plan` E2, and record a
table in `Artefacts and notes`. A `wyvern` agent, given only the counting rule
and the pinned paths, recounts independently. Discrepancies above one line per
cell are reconciled before EP-M2.

End state: one green, gated commit containing the prototype target and this
plan's updated evidence. Recovery: revert the commit; nothing else depends on
it.

### EP-M2: decision gate

Score each candidate against G1 to G5 in a table in `Decision log`, citing
prototype test names and census rows. Draft
`docs/adrs/adr-0028-cargo-subcommand-wrapper-attribute.md` with status
`Proposed`. Use the style guide's sections: Status, Date, Context and problem
statement, Decision drivers, Options considered (A to D and "helper plus
templates"), Decision outcome, Evidence, Consequences, and Outstanding
decisions.

- Rejection path (all candidates fail at least one gate): delete the
  prototype target in its own commit, titled "Retire the 8.3.3 Cargo wrapper
  prototype". The message cites the prototype commit and ADR-0028. Continue to
  EP-M3.
- Acceptance path (some candidate passes all gates): set this plan to
  `BLOCKED`. Write the narrow surface into `Decision log`: exact attribute
  keys, the generated item names and visibility, the knobs, and the
  verification additions sketched under "Acceptance-path obligations" in
  `Verification plan`. Wait for approval. The prototype stays in place as the
  red baseline for the real macro.
- Candidate D only: set `BLOCKED` and escalate under D4.

### EP-M3: diagnostic for the rejected keys (rejection path)

Red first:

1. Add `rstest` cases in a new file,
   `ortho_config_macros/src/derive/parse/tests/rejected_cargo_keys.rs`,
   registered in `parse/tests/mod.rs`. The cases are
   `cargo_subcommand = "demo"`, `cargo_bin = "cargo-demo"`, both together, and
   the list form `cargo_subcommand("demo")`. Each asserts that
   `parse_struct_attrs` returns an error whose text names
   `ortho_config::cargo::external_subcommand`. A control case, `cargo = "x"`,
   asserts that a near-miss key is still discarded (that is, `Ok`). Run the
   focused test, which must fail because parsing succeeds:

   ```bash
   cargo test -p ortho_config_macros rejected_cargo_keys 2>&1 \
     | tee /tmp/red-rejected-keys-ortho-config-8-3-3.out
   ```

2. Add `ortho_config/tests/ui/cargo_subcommand_attribute_rejected.rs`,
   deriving `OrthoConfig` with the floated attribute, without a `.stderr` yet.
   Run the trybuild target and record that it fails because the case compiles.

Green: add `ortho_config_macros/src/derive/parse/rejected_keys.rs`, with a
module comment explaining why these keys are rejected (ADR-0028). It exposes
`pub(crate) fn reject_or_discard(meta: &ParseNestedMeta) -> syn::Result<()>`.
That function returns the targeted error for the two keys and otherwise
delegates to `discard_unknown`. In `struct_attrs.rs`, replace the final
`discard_unknown(meta)` in the `_` arm with
`rejected_keys::reject_or_discard(meta)`, keeping the line count at 397 or
below. Generate the `.stderr` with `TRYBUILD=overwrite`, inspect it, then rerun
without the variable. The proposed message is (to be finalized with
`docs/improved-error-message-design.md`):

```plaintext
unsupported attribute `cargo_subcommand`; OrthoConfig does not generate
Cargo subcommand wrappers. Wrap the parser with
`ortho_config::cargo::external_subcommand` or use the derive template in the
user's guide ("Cargo external-subcommand entry points")
```

Refactor: none expected beyond doc comments. Update the `parse_struct_attrs`
doc comment so it names the rejected keys as the deliberate exception to silent
discard.

### EP-M4: documentation and decision record

- `docs/design.md` §4.17: replace the "A macro extension may be useful
  later…" paragraph and the candidate-syntax block with the decision, a
  one-sentence summary of why, and a link to ADR-0028. Keep the
  regression-coverage paragraph.
- ADR-0028: set the status to `Accepted` with the date and a one-line
  summary, as the style guide requires.
- `docs/users-guide.md` §"Cargo external-subcommand entry points": add a
  short paragraph saying no attribute generates the wrapper, that
  `#[ortho_config(cargo_subcommand)]` and `cargo_bin` are compile errors, and
  that the helper and templates are the supported routes.
- `docs/developers-guide.md`: in "Subcommand dispatch changes" or the
  derive-attribute conventions, record the convention that a struct-level key
  the project has publicly rejected gets a targeted error in
  `parse/rejected_keys.rs`, while other unknown keys stay discarded for forward
  compatibility. Name the owning module and its permitted callers, as the
  `AGENTS.md` abstraction policy requires.
- `CHANGELOG.md` §Unreleased: a "Changed" entry for the new compile error.
- `docs/contents.md`: index ADR-0028 after the ADR-0027 entry, and this
  ExecPlan after the 8.3.2 entry.
- `docs/roadmap.md`: tick the four 8.3.3 sub-items and mark 8.3.3 `[x]`.
  Replace "Prototype the candidate…" wording only if the roadmap convention
  requires it; otherwise tick in place.
- Run `make fmt`, then the Markdown gates.

### EP-M5: delivery

Mark the PR ready for review, then run the review loop (CodeRabbit via comenq)
to equilibrium. Address findings under the plan's tolerances, and complete
`Outcomes & retrospective`.

### Approval questions

- Q1. Approve D6, the targeted compile error for `cargo_subcommand` and
  `cargo_bin`? The alternative is documentation only, which leaves the floated
  syntax silently accepted.
- Q2. On the rejection path, should a new roadmap item be added for the
  `DocMetadata` Cargo-entry helper (D9), or should it stay as an outstanding
  decision in ADR-0028 only?

## Concrete steps

Run every command from the worktree root. Gate logs follow
`/tmp/$ACTION-ortho-config-8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute.out`.

```bash
# EP-M1 focused run
cargo test -p ortho_config --test cargo_wrapper_prototype 2>&1 \
  | tee /tmp/test-prototype-ortho-config-8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute.out

# EP-M3 focused runs
cargo test -p ortho_config_macros rejected_cargo_keys
cargo test -p ortho_config --test compile_fail

# Milestone gates (delegated to scrutineer, sequential)
make check-fmt
make lint
make test
make markdownlint   # also runs the spelling gate
make nixie          # only if a Mermaid diagram is added; none is planned
```

Expected EP-M3 green transcript excerpt:

```plaintext
test derive::parse::tests::rejected_cargo_keys::rejects::case_1_subcommand ... ok
test derive::parse::tests::rejected_cargo_keys::near_miss_key_is_still_discarded ... ok
test tests/ui/cargo_subcommand_attribute_rejected.rs ... ok
```

## Validation and acceptance

Acceptance is behavioural:

- EP-M1: `cargo test -p ortho_config --test cargo_wrapper_prototype`
  passes. Every baseline-versus-candidate row agrees. The negative control
  fails the flat parser as expected. `make check-fmt`, `make lint`, and
  `make test` pass at the prototype commit.
- EP-M2: ADR-0028 exists with status `Proposed` and a scored G1 to G5
  table. On the rejection path, `ortho_config/tests/cargo_wrapper_prototype*`
  no longer exists, and all three gates pass.
- EP-M3: before the change, the new unit cases and the trybuild case fail
  because the attribute compiles. After it, both pass, and
  `ortho_attrs_unknown_keys` still passes unchanged. The three gates pass.
- EP-M4: `make markdownlint` and `make check-fmt` pass. Design §4.17 no
  longer presents the attribute as an open candidate. Roadmap 8.3.3 is `[x]`.
- Overall: `make check-fmt`, `make lint`, and `make test` succeed after
  every milestone.

No rstest-bdd feature is added on the rejection path, because its only
behavioural change is a compile-time diagnostic, which runtime BDD scenarios
cannot observe. The existing
`ortho_config/tests/features/cargo_entry_point.feature` continues to specify
the supported helper. The acceptance path adds a feature; its proposed text is
in `Verification plan`.

## Verification plan

### Evaluation obligations (EP-M1 and EP-M2)

- E1 (G1, legal host): for each consumer, the attribute's required host
  type derives `#[derive(OrthoConfig)]` already, or would derive it without
  adding configuration behaviour the tool does not want. Method: census facts
  with pinned line citations, plus a port written with the attribute in place.
  Pass: at least two of the three consumers. Non-vacuity: the
  `cargo_attribute_is_silently_ignored_today` probe shows that a legal host
  compiles. Failure is therefore about the consumers, not the syntax.
- E2 (G2, net savings): wrapper lines removed, minus lines added (the
  attribute, the knobs, and adoption glue), is at least 8 per adopting
  consumer, in at least two consumers. The counting rule counts non-blank,
  non-comment lines in the items that exist only for the Cargo entry shape: the
  parent struct or enum, the single-variant enum, variant destructuring,
  inner-matches extraction and its "lost token" error, and wrapper
  `DocMetadata` patching. It excludes tracing initialization, error-hint
  rendering, and business logic, which are application policy that no candidate
  removes. Method: manual count, plus an independent `wyvern` recount.
  Non-vacuity: every consumer row must have a non-zero baseline count, or the
  gate is void for that consumer and reported as such.
- E3 (G3, variance coverage): the candidate reproduces each consumer's
  observed behaviour with at most two knobs in total across all consumers. The
  variance points are: v1, parent usage spelling (`cargo` or `cargo-<name>`);
  v2, `version` placement; v3, derives on the wrapper enum
  (`OrthoConfigSubcommandDocs`); v4, dual-form parsing with `InvocationForm`
  (`cargo-bdd`); v5, localized parsing (`parse_localized_command`); v6,
  invocation-hint error handling; and v7, `subcommand_required`. Method: the
  prototype's `rstest` equivalence tables, where an unsupported point shows as
  a row that cannot be written. Non-vacuity: the negative control proves the
  tables fail when the wrapper is wrong.
- E4 (G4, visible contract): a reader of the adopting source can tell that
  `argv[1]` is Cargo's injected token without reading macro output; the
  generated items are public, documented, and non-`__` named; and help output
  is byte-identical to the baseline's. Method: the equivalence tables'
  usage-string rows, plus review of the port source.
- E5 (G5, ADR-0004 conformance): the expansion touches neither
  `OrthoConfig::load` nor the merge pipeline. Method: inspection, where
  candidate D fails by construction.

### Behavioural obligations (rejection path)

- O1: the two rejected keys, in name-value and list forms and alone or
  together, produce a compile error naming the supported alternative. Method: an
  `rstest` table in `parse/tests/rejected_cargo_keys.rs`, plus the trybuild
  case `cargo_subcommand_attribute_rejected` with a committed `.stderr`. This
  finite domain is enumerated exhaustively, so no property test is needed.
  Non-vacuity: the red run shows both tests failing before the change, and the
  EP-M1 probe shows the attribute compiles today. Plausible wrong
  implementation rejected: one that rejects *all* unknown keys fails O2.
- O2: forward compatibility is preserved. Every other unknown struct-level
  key is still discarded. Method: the existing
  `parse/tests/ortho_attrs_unknown_keys.rs` table, unchanged, plus the
  `cargo = "x"` near-miss control in O1's file. Plausible wrong implementation
  rejected: a prefix match on `cargo` fails the control.
- O3: no runtime behaviour changes. Method: the existing suites stay green,
  including the helper's unit, property, BDD, and snapshot tests and the
  documentation-harness Cargo dispatch tests
  (`ortho_config/tests/documentation_examples/cargo_derive.rs`,
  `cargo_flows.rs`), which run real `cargo <name>` through `PATH`.

### Axioms (trusted, not verified here)

- Cargo's external-subcommand argument contract, as documented in the
  Cargo book.
- clap 4's derive semantics for `name`, `bin_name`, `version`,
  `subcommand_required`, and single-variant enums.
- `syn`'s `ParseNestedMeta` behaviour for name-value and list items, which
  is already relied on by `discard_unknown`.

### Formal verification decision

Verus is not warranted. The work introduces no lemma over an unbounded domain
and no contractual business logic: the decision is evidential, and the only
executable change is a two-key diagnostic. Kani is not warranted either: there
is no `unsafe` code, arithmetic, or bounded state machine. The helper's
existing property, `wrapper_preserves_generated_leaf_command_matches`, remains
the evidence for argv equivalence of the supported abstraction, and the work
leaves it unchanged.

### Acceptance-path obligations (only if EP-M2 accepts; requires re-approval)

These are sketched now so that the acceptance path is costed. They are not
authorized by approval of this plan.

- I1, argv equivalence: for every argv tail `T`, the generated wrapper
  accepts `[bin, name, T…]` exactly when the inner parser accepts `[name, T…]`,
  with equal matches. Method: reuse the strategy in
  `ortho_config/src/cargo/proptests.rs` against the generated item.
  Non-vacuity: the generator must reach flag, option, and positional shapes,
  and record its classification counts.
- I2, name consistency: a `cargo_bin` other than `cargo-<cargo_subcommand>`
  is a compile error. Method: a trybuild compile-fail case.
- I3, help stability: insta snapshots of top-level and inner help for both
  invocation forms.
- I4, end-to-end: a fixture binary run as `cargo <name> --help` through
  `PATH`, reusing `cargo_runner::cargo_dispatch_path`.
- Proposed feature,
  `ortho_config/tests/features/cargo_wrapper_attribute.feature`:

  ```gherkin
  Feature: Generated Cargo subcommand wrapper
    Scenario: Cargo dispatch reaches the options struct
      Given a configuration struct declaring Cargo subcommand "demo"
      When the generated wrapper parses "cargo-demo demo --verbose"
      Then the options struct sees verbose enabled

    Scenario: Flat invocation is rejected
      Given a configuration struct declaring Cargo subcommand "demo"
      When the generated wrapper parses "cargo-demo --verbose"
      Then parsing fails because the subcommand token is missing
  ```

## Conformance basis

- Roadmap `docs/roadmap.md` 8.3.3 (at origin/main
  `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3`), with sub-items RM-a (prototype
  the syntax), RM-b (decide the form), RM-c (reject unless real repetition
  without hiding the contract), and RM-d (success: the design records the
  outcome).
- `docs/design.md` §4.17 (DES-4.17), candidate-syntax paragraph.
- ADR-0004 (Accepted 2026-05-21; amended 2026-08-09; clarified
  2026-11-11).
- `AGENTS.md` (file-size cap, testing, documentation, abstraction policy).
- No Terms of Reference document exists for this phase.

Trace:

- RM-a → D1 → EP-M1 → `cargo_wrapper_prototype` tables and probe.
- RM-b → D2 and E1 to E5 → EP-M2 → ADR-0028 §"Decision outcome".
- RM-c → G1 to G4 → EP-M2 scoring table.
- RM-d → D5 → EP-M4 → design §4.17 and ADR-0028 `Accepted`.
- DES-4.17 and ADR-0004 → G5 and D4 → Constraint 2.
- The discovery of silent discard → D6 → EP-M3 → O1 and O2 tests.

## Idempotence and recovery

Every milestone is one or more commits on the branch, and each commit is gated.
Re-running a focused test is harmless. To abandon the prototype, revert its
commit. If `TRYBUILD=overwrite` writes an unexpected `.stderr`, discard it with
`git restore --source=HEAD -- <path>`. Back up first (repository memory:
`git checkout --` reverts to the index and destroys unstaged work). The ADR and
documentation edits are plain text and can be reverted per commit.

## Artefacts and notes

The census table, the independent recount, and the rubric scores are recorded
here during EP-M1 and EP-M2.

## Interfaces and dependencies

The rejection path adds no public interface. The new crate-private item is:

```rust
// ortho_config_macros/src/derive/parse/rejected_keys.rs
pub(crate) fn reject_or_discard(meta: &syn::meta::ParseNestedMeta) -> syn::Result<()>;
```

Its only permitted caller is the `_` arm of `parse_struct_attrs`. No dependency
is added. The prototype uses only existing `ortho_config` dev-dependencies:
`rstest`, `googletest`, `pretty_assertions`, and `clap`.

## Revision note

- 2026-11-28: initial draft from Wyvern reconnaissance, estate census, and
  prior-art research. Awaiting community-of-experts review.
