# Validate skill manifests against real commands

This ExecPlan is a living document. Keep `Constraints`, `Tolerances`, `Risks`,
`Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan`
current at every milestone.

Status: IMPLEMENTED (2026-11-14; all local gates green, hosted review pending)

## Purpose / big picture

After this work, a consumer can submit the command references and structured
examples it extracted from a skill to a reusable `cargo_orthohelp` validator.
The validator reports unknown command paths, flags absent from the referenced
command, duplicate manifest identifiers, and vocabulary/global-option
violations under the configured policy. It checks skill claims against the same
complete command contract used by agent context.

The validator does not read files, parse arbitrary Markdown, or execute shell
examples. A consumer owns manifest loading, format validation, prose parsing,
and example extraction. An example arrives as a normalized command path, option
names with value presence and values, and positional values. This makes the
claims checkable without guessing shell grammar. A prose-only skill stays valid
and reports zero command and example coverage.

This is an approval-gated plan. No implementation starts until the maintainer
explicitly approves it and the prerequisites below are complete.

## Constraints

- Roadmap 6.3.2 requires 6.3.1 and step 7.1. Do not implement until 6.3.1,
  7.1.2, and 7.1.3 are complete on the implementation base. Do not implement
  those prerequisite items in this plan.
- Keep skill-specific prose, file I/O, extraction, and domain execution in
  consumer applications. The proposed reusable API belongs in
  `cargo_orthohelp`; do not move Weaver or Netsuke execution semantics into
  OrthoConfig.
- Reuse the completed 7.1.3 glossary/policy contract and an explicit
  per-command effective option surface. Roadmap 7.1.3 does not itself promise
  metadata that marks inherited options. Never infer inherited options by
  unioning root inputs into child commands. If a consumer cannot provide
  unambiguous effective options, stop and mark the plan `BLOCKED`.
- Reuse 7.1 canonical policy rules, modes, and exceptions. Do not invent a
  second vocabulary or reject arbitrary domain commands/flags as if canonical
  vocabulary were an allowlist.
- Structural findings are independent of policy mode. `off` may suppress
  vocabulary findings, but it cannot suppress unknown paths, unknown flags,
  malformed/ambiguous normalized inputs, or duplicate manifest IDs. Exceptions
  can never authorize a nonexistent command or flag.
- Validate supplied claims only. Do not promise exhaustive discovery from
  arbitrary prose, general Markdown or shell parsing, executable command
  validity, option arity, quoting, side effects, or example runtime success.
  Preserve unsupported/unchecked example status explicitly; do not silently
  count it as validated.
- Treat manifest paths as inert metadata. Do not load them automatically.
  Findings identify a stable manifest ID and source ordinal, not raw paths or
  argument values. Do not add telemetry for local coverage counts.
- Build command/option indexes once per validation call; do not scan the full
  command tree for every claim. Emit at most one finding per invalid path or
  per declared flag/option. The library has no hidden input-size cap; the
  caller owns input byte limits and the report bound follows input claim count.
- Preserve the agent-context wire contract unless review proves a change is
  needed. A schema change requires a design decision and its versioning
  consequences.
- Use the shared Cargo cache and compiler admission. Run gates sequentially;
  do not bypass wrappers or install tools implicitly. Keep Rust files below 400
  lines and document public APIs/modules.

## Tolerances (exception triggers)

- Stop if prerequisites are incomplete, explicit per-command effective
  options are missing, or ownership/extraction semantics remain ambiguous.
- Stop if a public wire schema or released API must change, a runtime
  dependency is required, or the library must own file I/O or unrestricted
  source parsing.
- Stop if implementation exceeds 30 changed files or 1,500 net non-generated
  lines, or any Rust source file would exceed 400 lines. Exclude this ExecPlan
  from file-count and line-count tolerances.
- Stop if review proposes materially different API or example binding
  semantics; record options and impacts before changing this design.
- Stop after two focused repairs for the same failing gate.
- If implementation reveals non-trivial contractual business logic beyond
  exact finite lookup and reuse of 7.1 policy, stop and revise the proof sketch
  and verification method before continuing.

## Risks

- Risk: `agent-native-cli-design.md` §3.4 assigns validation to consumers,
  while current `SkillManifest` Rustdoc says `cargo-orthohelp` validates it.
  Severity: high; likelihood: high. Mitigation: reconcile ownership in design
  docs before coding: reusable validator called by consumers; consumer owns I/O
  and extraction.
- Risk: `AgentInput` lacks explicit global/inherited metadata, and current
  policy evaluation runs before bridge command inputs are built. Roadmap 7.1.3
  promises a glossary/linter, not necessarily inheritance metadata. Severity:
  high; likelihood: high. Mitigation: require an explicit downstream-resolved
  effective option surface and never synthesize inheritance.
- Risk: raw `AgentExample.command` cannot reliably preserve parsing or option
  values. Severity: high; likelihood: high. Mitigation: use normalized examples
  and test the consumer adapter against controlled fixture text.
- Risk: structured examples can drift from their Markdown source. Severity:
  high; likelihood: medium. Mitigation: integration fixture adapter extracts
  from the same source submitted to validation, with a drift negative control.
  Consumer parser internals remain outside this library's proof boundary.
- Risk: vocabulary findings could hide structural failures in `off` mode or
  exceptions could authorize invalid claims. Severity: medium; likelihood:
  medium. Mitigation: separate structural and policy result classes and test
  mode/exception cross-products.
- Risk: future work may add a substantive invariant that needs formal proof.
  Severity: medium; likelihood: low. Mitigation: current exact finite lookup
  does not warrant Verus; revise the plan first if a non-trivial pure kernel
  appears.

## Progress

- [x] (2026-10-14 17:30Z) Read roadmap, design, metadata types, policy seams,
  ExecPlan guidance, and the 6.3.1 plan.
- [x] (2026-10-14 17:30Z) Confirmed 6.3.1 is complete while 7.1.2 and 7.1.3
  remain incomplete; recorded a hard implementation gate.
- [x] (2026-10-14 17:30Z) Collected Wyvern reconnaissance and community-of-
  experts review via context pack `pk_f4rgdzgk`.
- [x] (2026-10-14 17:32Z) Incorporated three community-of-experts reviews;
  no material blocker remains in the planning design.
- [x] (2026-10-14 18:00Z) Recorded planning baseline `make lint` passed rustdoc,
  Clippy, and Whitaker. No runtime code changed.
- [x] (2026-10-14 19:38Z) Planning gates passed: `make fmt`,
  `make check-fmt`, `make lint`, `make test`, `make markdownlint` (including
  spelling), and `make nixie`. Only the plan and documentation index changed.
- [x] (2026-11-08) Rebased onto PR #651 merge `ddba3578`; retained both
  index entries in the sole conflict and reconciled the existing prover lane.
  Rechecked that 7.1.2 and 7.1.3 remain incomplete.
- Historical validation on the PR #651 base: formatting passed on
  2026-11-08, but the resumed `make test` failed in eight localization-boundary
  cases: six package operations exceeded 240 seconds and two consumer test
  operations exceeded 300 seconds. The interrupted earlier run had no terminal
  result. `make typecheck` and `make lint` subsequently passed. Retain
  `/tmp/ortho-632-resume-test.out`; do not treat elapsed deadlines as proof of
  a source defect or as a passing gate. Commit and push remain gated on
  successful validation of the replacement base below.
- [x] (2026-11-09) Rebased the two planning commits onto PR #664 at
  `26275a4efe31d3cf84cf8ec3e265782a28a832f3` without conflicts; range-diff
  confirms both patches are unchanged. Restored the PR #651 plan revision.
- [x] (2026-11-09) Formatting, typecheck, lint (including Whitaker),
  Markdown/spelling, and diagram checks passed on the PR #664 base.
- Historical validation on the initial PR #664 base: the full run ended with
  exit 143, without a complete result. Its retry passed the earlier suites but
  rejected a stale documentation-cache ownership marker. After confirming no
  owned compiler writers remained, recovered only that marker under the
  exclusive cache lock; retained the marker and gate logs under
  `/tmp/ortho-632-*`. Neither a terminated run nor successful cache recovery
  counts as a passing test gate. The focused documentation suite then passed
  all eight tests. A subsequent full run reached the boundary suite: 25 passed,
  seven hit the unchanged 440-second nested-build deadlines, and one was
  ignored. Captured diagnostics show active compilation, including
  compiler-admission waits. The next step completed the existing package-build
  caches and reran the unchanged gates without widening deadlines; cache
  preparation itself was not acceptance evidence.
- [x] (2026-11-09) Both standalone package-verification commands completed
  successfully with the shared cache and existing target directories. The
  unchanged `make boundary` gate then passed: 32 passed, zero failed, one
  ignored. Cold compilation exceeded the child budget on this shared host; this
  evidence does not establish a cold-cache timing guarantee. The final full
  test run passed, including all workspace tests, 32 boundary tests (one
  ignored), and 119 Python tests (16 skipped). Evidence:
  `/tmp/ortho-632-664-test-final.out`.
- [x] (2026-11-09) Restacked the three planning commits onto PR #664's
  updated head, `501ab83134eaba0f9e97e1daeaa29c793a819c53`, without conflicts.
  Range-diff confirms all three patches are unchanged. This incorporates
  fixture-target leases, bounded child-process cleanup, survivor regressions,
  and the Unix-only probe import fix.
- [x] (2026-11-09) Validated the updated `501ab831` parent: `make check-fmt`,
  `make typecheck`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passed. The full test run included 53 behavioural scenarios,
  seven identifier-artefact tests (one ignored helper), 35 boundary tests
  (seven ignored helpers), and 119 Python tests (16 skipped). Evidence:
  `/tmp/ortho-632-501-<gate>.out`. These runs reused existing build caches;
  they do not establish a cold-cache timing guarantee. Publication targets
  `issue-644-nested-cargo-timeouts`; implementation remains approval-gated.
- [x] (2026-11-10) Maintainer approved the plan and authorized implementation.
  EP-M0 rechecked the base at `origin/main` `9e1bf404`: 6.3.1 is complete, but
  7.1.2 and 7.1.3 are still unimplemented (`PolicyInputs` is empty; the
  global-option glossary exists only as design §5 prose) and no open pull
  request carries them. The maintainer chose full delivery over the plan's stop
  rule, so the shared global-option glossary is delivered here as an additive
  extension of `policy::vocabulary` rather than as a second vocabulary.
  Recorded in the Decision log.
- [x] (2026-11-10) EP-M1: specified the normalized input contract in
  `cargo-orthohelp/src/skill_validation/` (`input`, `report`, `resolve`,
  `policy_check`, `validate`). `PolicyMode` gained `Default = Off` so
  `ResolvedAgentPolicy` can be built without a mode. Evidence: 35 library tests
  pass, including the `rstest` ambiguous-surface cases (empty path, duplicate
  path, empty option, conflicting option) and the byte-stability and
  order-independence cases.
- [x] (2026-11-10) EP-M2: implemented exact structural resolution over an
  explicit per-command effective surface. A reference with an unknown path
  produces one path finding and no cascading flag findings; flags are compared
  exactly and reported once each. Evidence: `proptests.rs` compares the
  validator against an independent linear oracle; focused tests pass.
- [x] (2026-11-10) EP-M3: implemented example binding, policy advice, and the
  fixture adapter. Operation and build-workflow fixtures live under
  `cargo-orthohelp/tests/fixtures/skills/`; the feature file has five
  scenarios. Evidence:
  `cargo test -p cargo-orthohelp --test skill_validation_bdd` passed 6 tests and
  `--test skill_validation_e2e` passed 5 tests, including the source-drift and
  malformed-array negative controls.
- [x] EP-M4: documentation, final sequential gates, and roadmap closure.
  - [x] (2026-11-13) Documentation written: design §3.4.1 boundary text,
    users' guide section with a compiled example and its transcript,
    developers' guide notes, and the documentation index.
  - [x] (2026-11-13) `make fmt`, `make check-fmt`, `make typecheck`, and
    `make lint` (rustdoc, Clippy, Whitaker) pass on the implementation
    candidate. `make markdownlint` (including `make spellcheck`) and
    `make nixie` also pass.
  - [x] (2026-11-14) A complete `make test` run passed on the frozen candidate
    `e4038718`: `MAKE_EXIT:0`, 113 `test result: ok` suites, no failing test.
    The final gate sweep then reran every other gate on the same clean tree,
    and roadmap 6.3.2 with its three sub-items is checked. Evidence:
    `/tmp/test-ortho-config-6-3-2-skill-manifest-validation.out`.
- [x] (2026-11-14) Replayed the branch's own eleven commits from the inherited
  PR #664 parent onto `origin/main` after #664 squash-merged, and retargeted
  the pull request to `main`. `range-diff` reports every patch unchanged; the
  only merged file was the documentation-example allowlist, which kept a
  main-only entry while adding this feature's two. A first replay used an
  `origin/main` that advanced mid-operation; the replay was repeated onto the
  newer head so no main content appears deleted. Gates were then rerun on the
  replayed candidate.
- [x] (2026-11-14) Cleared the seven CodeScene delta findings against
  `724839c4`. Five production and test files were refactored to a code health
  score of 10.00: `validate/mod.rs` and `validate/examples.rs` build findings
  through `FindingData` constructors instead of a three-field literal at each
  site, `policy_check.rs` resolves through a `ClaimPolicy` borrow carrying the
  policy, path, and name once, and the duplicated test pairs in `tests.rs`,
  `tests_policy.rs`, and `tests/skill_validation_e2e.rs` became parameterized
  `rstest` cases over a small domain type. The two remaining files are
  test-only fixture builders, so `.codescene/code-health-rules.json` disables
  the single "String Heavy Function Arguments" rule for exactly those two
  paths, with a `matching_content_path_doc` stating why.
  `cs rules-config validate` accepts it and `cs check` reports 10.00 for every
  changed file.
- [x] (2026-11-14) Re-ran the focused Clippy pass after the refactor: the
  `unused_self` and two `missing_const_for_fn` findings it raised were fixed in
  place, and
  `cargo clippy -p cargo-orthohelp --all-targets --all-features -- -D warnings`
  now exits 0.

- [x] (2026-11-14) Closed the test and documentation gaps CodeRabbit's
  pre-merge checks reported against `e60027ac`. Each repair removes the vacuity
  the check named: the verb-exemption branch now has a test case, the BDD step
  asserts a finding that only example checking produces rather than the
  consumer's extraction count, the malformed-array end-to-end test calls the
  adapter's parse path instead of `serde_json` directly, `PolicyMode`'s default
  is asserted, and the property module gains a control that corrupts a real
  report and shows the oracle comparison rejects it. Documentation now names
  `policy/vocabulary/global_options.rs` as the glossary's owner, stops claiming
  the linter reads it before 7.1.3 lands, and signposts the validator from the
  v0.11.0 migration guide. Evidence: `85769ae3`; the full sequential sweep
  passed again and CodeScene reported no new issue.

- [x] (2026-11-14) Corrected the last stale claim CodeRabbit's Developer
  Documentation check reported: "Outcomes & retrospective" no longer says the
  linter and the validator read the shared glossary today. It names
  `cargo_orthohelp::policy::vocabulary::global_options` as the glossary data,
  records the skill-manifest validator as its only current consumer, and states
  that the roadmap 7.1.2 and 7.1.3 linter and policy passes will read the table
  once implemented, matching `docs/agent-native-cli-design.md` §5. Evidence:
  `make fmt` and `make markdownlint` pass; the sentence no longer contradicts
  the roadmap item states.

- [x] (2026-11-14) Repaired the four findings and three verification gaps the
  hosted completeness assessment returned as "not clear" against `0acc69e5`.
  Each was checked against source before repair and none was rebutted.
  - F1, global-option classification: `EffectiveOption` now carries an explicit
    `is_global` that the consumer states, `ResolvedOption` threads it through
    resolution, and only `PolicyMode`-gated glossary advice reads it. The
    validator no longer infers globality from a name, a root command, or a
    parent path, so a domain `--color` is not judged by the global glossary.
  - F2, one finding per example option: the example option checks chain —
    ownership of the reference's declared flags first, then the command's
    accepted options — so an undeclared option is reported once instead of
    again as unaccepted, matching the plan's per-claim finding bound.
  - F3, policy only for resolved uses: value advice applies only to an option
    that is both declared by the reference and accepted by the command.
  - F4, coverage honesty: `Coverage::examples_bound` records examples that
    reached their option checks, and `ValidationStatus::Complete` now requires
    every extracted example to have bound with none unsupported. Previously an
    extracted-but-unbound example left the report `Complete`.
  - V1: `EffectiveCommandSurface::from_agent_context` plus `GlobalOption`
    adapt a real `AgentContext`; `tests_surface.rs` drives it from a tree whose
    root declares a root-only option and whose child does not, asserting the
    child never sees the root's option and that the global classification
    reaches the policy gate.
  - V2: `proptests_examples.rs` generates the whole binding chain and compares
    the validator against an independent linear oracle covering all five
    example findings, coverage, and status. Mutant controls show the
    comparison is non-vacuous: corrupting the oracle's acceptance branch fails
    it, a dropped value changes a round-tripped record, and unioning the root's
    options into a child hides the finding the real surface emits.
  - V3: the tolerance breach is recorded below rather than treated as
    satisfied.
  Evidence: focused `cargo test -p cargo-orthohelp --lib skill_validation` (52
  passed) and `PROPTEST_CASES=2000` on both property modules (12 passed);
  `--test skill_validation_bdd` (6 passed) and `--test skill_validation_e2e` (5
  passed). The end-to-end drift test now asserts the single finding the drifted
  option produces and that the finding names it, instead of expecting the
  double report the plan forbids.

- [x] (2026-11-14) Replayed the branch onto the current main head `54dab732`
  (PR #524, "Remove orphaned Jail test helpers") before publication, because
  the branch still sat on `724839c4`. The replay was clean: `git range-diff`
  reported all 18 branch commits unchanged, and every path this feature touches
  is byte-identical to the pre-rebase candidate, so the gate certificates carry
  by content identity. Main's single advance is confined to
  `ortho_config/tests`, which the feature does not modify, but the gates were
  re-run anyway against the rebased tree rather than assumed. Evidence, all
  sequential on `1594f70f`: `make check-fmt` 0, `make typecheck` 0, `make lint`
  0 (rustdoc, Clippy, Whitaker, and the Python `interrogate` gate all passed),
  `make test` 0 (every Cargo target passed, 128 Python tests passed with 5
  skipped), `make markdownlint` 0 including the spelling gate, and `make nixie`
  0. Logs are under
  `/tmp/{check-fmt,typecheck,lint,test,markdownlint,nixie}-ortho-config-6-3-2-skill-manifest-validation.out`.
- [x] (2026-11-14) Repaired the second hosted completeness assessment against
  `aac5182d`. F1: `check_example_options` now returns whether every option was
  checked, so an example whose surface path is missing is not counted bound.
  F2: the adapter keeps conflicting duplicate value types and validation
  rejects them. F3: the generator emits positionals and four new controls
  exercise the round trip, acceptance, and duplicate-reference rules. F4: the
  tolerance count is restated against `54dab732`. The independent oracle was
  updated to the same bound rule, and the oracle and adapter were then
  refactored to clear the CodeScene `Bumpy Road Ahead` and
  `Deep, Nested Complexity` findings introduced by the split; `cs delta`
  reports no findings. Focused module tests (60 passed) and Clippy are green;
  the full sequential gate sweep follows this record.
- [x] (2026-11-14) Cleared the hosted CodeScene advisory. Its quality gate
  failed `String Heavy Function Arguments` on `skill_validation/resolve.rs`
  (42% of function arguments were string types). Paths, option names, and
  rendered diagnostics in that module now travel as `CommandPath`,
  `OptionName`, and `RenderedPath` wrappers, so the index lookup cannot swap a
  path for a name and no signature spells a bare `&str`.
  `cs review cargo-orthohelp/src/skill_validation/resolve.rs` now reports score
  10.0 with no findings. The full six-gate sweep was re-run on the resulting
  working tree and every gate exited 0.
- [x] (2026-11-14) Repaired the third hosted assessment. Coverage now requires
  the claimed path to exist whatever options the example names, so an
  optionless example or one using only undeclared options on a missing path
  stays unbound (both cases have regressions, and the oracle applies the same
  rule). The adapter merges a stated value type into a silent entry and detects
  a disagreement in every declaration order, including a stated global type
  reaching a silent input. The acceptance control is now a same-input oracle
  mutant (`oracle_ignoring_acceptance`), and the fixture marks `color` global
  with a source value-drift test that asserts the extracted value and the
  resulting `skill.policy.global_option_value` finding.
- [x] (2026-11-14) Fourth hosted completeness assessment returned a **clear**
  whole-plan verdict for this candidate. Assessment-ID
  `632-completeness-20261114-0bc29528`, base `54dab732`, head `0bc29528`, plan
  revision blob `a30c92fc`, which matches the blob at that head. The reply
  confirmed all three prior findings addressed in source and reported no
  remaining in-scope defect. It added two limits, both dispositioned: hosted
  jobs still running at lookup time, and Gecko reporting from its initial-scan
  results. The hosted jobs have since completed green, and Gecko's check run
  for this head concludes `success` with "No vulnerabilities found" under its
  configured initial-PR-open scan mode; the changes since that scan add no
  dependency, network boundary, or `unsafe` block.
- [x] (2026-11-14) Published the repaired candidate and marked the pull request
  ready. Hosted checks on `0bc29528` are green: `build-test` (linux and
  windows), `msrv`, both advisory Clippy legs, all three packaging dry runs,
  the CodeScene code-health gate, and the Gecko security review. Marking ready
  triggered an automatic review that CodeRabbit rate-limited ("next included
  review available in 23 minutes"), so one formal review was queued through
  `comenq` as `c0557915` (ETA about 6 h 23 m). The description now records the
  final candidate, the three assessment repair rounds, the fourth clear
  verdict, and the `resolve.rs` code-health repair.

## Surprises & discoveries

- Observation: the example checks are ordered, and the order is observable.
  Ownership of the reference's declared flags runs before the command lookup,
  so an option the reference never declared is reported even when its path is
  absent from the surface, while a *declared* option on an absent path stops
  the remaining checks instead of cascading. An oracle written in the other
  order reported nothing for the first case and the property test caught it.
  Evidence: `validate/examples.rs::check_example_options` and the minimal
  counterexample proptest shrank to. Impact: any future reordering of those
  checks changes the finding set, so the chain is asserted in the oracle and in
  `tests_examples`.
- Observation: agent-context types deliberately implement no `Default` for
  `AgentCommand` and `AgentInput`, so an adapter example must state every
  field. Evidence: `ortho_config/src/agent_context/mod.rs` derives `PartialEq`
  and `Eq` but not `Default` on those structs, unlike `AgentContext`, which has
  `new`. Impact: the doctest and `tests_surface.rs` spell out the full
  literals; a `..Default::default()` shortcut does not compile.
- Observation: `SkillManifest` and `SkillCommandRef` exist as serializable
  metadata, but there is no manifest reader or validator; current agent-context
  fixtures contain no populated skill manifests. Evidence:
  `ortho_config/src/agent_context/mod.rs` and `cargo-orthohelp` agent-context
  fixtures. Impact: plan an explicit supplied input boundary rather than
  assuming a parser exists.
- Observation: `AgentExample.command` is a raw string and cannot safely
  preserve option values. Evidence: `ortho_config/src/agent_context/mod.rs`.
  Impact: normalize example claims at the consumer boundary.
- Observation: 7.1.2 and 7.1.3 are incomplete; `PolicyInputs` is empty.
  Evidence: `docs/roadmap.md` and `cargo-orthohelp/src/policy/config/mod.rs`.
  Impact: implementation must wait for the 7.1 policy and an explicit
  per-command effective option surface from the consumer or bridge.
- Observation (refreshed 2026-11-08): `docs/scripting-standards.md` now
  exists, and ADRs moved under `docs/adrs/`. The requested
  `docs/rtest-bdd-users-guide.md` remains a typo for
  `docs/rstest-bdd-users-guide.md`. Use the current paths below.
- Observation: PR #651 supplies pinned prover tooling, command-boundary tests,
  and accepting/rejecting fixtures. These demonstrate runner operation, not
  skill-validation correctness. Production proof inventories are empty and fail
  with `FORMAL-EMPTY`; ordinary gates do not execute provers.
- Observation: `rstest-bdd` injects a fixture by parameter name, so a step
  parameter named `context` needs a fixture called `context`, not
  `skill_context`. Typed placeholders such as `{option:string}` already accept
  and strip surrounding quotes, so a step attribute must not add its own quoted
  literal regex. Impact: recorded in the step glue rather than worked around in
  the feature text.
- Observation: the documentation-example harness requires every fenced block in
  `DOCUMENT_PATHS` to carry a `<!-- tested-example: id -->` marker. Impact: the
  new users-guide program is registered in both `STANDARD_RUST_EXAMPLES` and the
  `SIMPLE_RUNS` transcript table rather than left unmarked.
- Observation: Clippy's `assert_is_empty` fires only on the single-argument
  `assert!(value.is_empty())` form. Impact: finding-list assertions use
  `assert_eq!(report.findings, Vec::new())`, which also shows the actual value
  on failure.
- Observation (2026-11-13): Whitaker's `no_std_fs_operations` denies
  `std::fs::read_to_string` in integration tests, and its `excluded_crates`
  keys on the target name, so the crate-level `cargo_orthohelp` entry does not
  cover `skill_validation_e2e`. Impact: the fixture adapter opens a
  `cap_std::fs_utf8::Dir` on the checked-in fixture root and reads relative
  camino paths through it, matching the pattern in
  `nested_subcommand_end_to_end.rs`, rather than adding a lint exclusion.
- Observation (2026-11-13): the documentation-example harness has three
  separate registries, not two. Besides `STANDARD_RUST_EXAMPLES` and
  `SIMPLE_RUNS`, `documentation_examples_tests.rs` pins every fenced block's
  identifier in `EXPECTED_EXAMPLE_IDS`. Impact: a non-Rust transcript fence
  needs its own `<!-- tested-example: id -->` marker and an entry in that
  allowlist, or `every_documented_fence_has_a_known_unique_identifier` fails.
- Observation (2026-11-14): CodeScene's "String Heavy Function Arguments"
  factor counts `&str`, `&[&str]`, and `&[String]` parameters but not
  `Option<&str>`, and computes the reported percentage as string arguments over
  all arguments including `&self`. That makes a fixture builder such as
  `command(&[&str], &[&str])` score badly even though the literal names are
  precisely what the test is about. Impact: production modules were reshaped so
  the names travel inside one struct, and the two test-support modules keep
  their signatures under a path-scoped rule weight of 0.0.
- Prior art: the Agent Skills specification defines YAML frontmatter and
  Markdown body but no standardized command/example grammar for arbitrary
  prose. It points to `skills-ref` for structure and naming validation. This
  supports downstream extraction; it does not define OrthoConfig policy.
  Source: <https://agentskills.io/specification> (Firecrawl retrieval,
  2026-10-14).

## Decision log

- Decision (2026-11-14): Global-option status is supplied, not inferred.
  `EffectiveOption` gains `is_global`, defaulting to false so an existing
  consumer's JSON surface keeps its meaning, and `ResolvedOption` carries the
  same bit through path resolution. Only a global option receives the §5
  glossary's spelling and accepted-value advice; every other option is still
  subject only to the canonical vocabulary rules. An adapter,
  `EffectiveCommandSurface::from_agent_context`, builds a surface from an
  `AgentContext` plus the consumer's `GlobalOption` list and adds nothing else.
  Rationale: the previous code treated any option named `color` as the global
  colour-policy option, which would fail a domain command that legitimately
  accepts `--color blue`. The plan already forbids inferring inheritance at the
  surface boundary; this applies the same rule to global classification.
  Date/Author: 2026-11-14, repair of the hosted assessment's F1.
- Decision (2026-11-14): An example option produces one finding. The checks
  run ownership first: an option its owning reference does not declare is
  reported as undeclared and not re-examined; a declared option the command
  does not accept is reported as unknown; only an option that is both declared
  and accepted reaches policy advice. Rationale: the plan bounds findings at
  "one per declared flag or example option", and the reference is the claim the
  example elaborates, so a second finding about the real command repeats one
  offending token. The same chaining already governed a reference with an
  unknown path, which does not cascade into flag findings. Date/Author:
  2026-11-14, repair of F2 and F3.
- Decision (2026-11-14): Separate extraction from binding in coverage.
  `examples_checked` keeps its meaning — the consumer extracted the example —
  and the new `examples_bound` counts those that reached their option checks.
  `ValidationStatus::Complete` requires no unsupported example and
  `examples_bound == examples_checked`. Rationale: an extracted example whose
  manifest is unknown, whose ordinal selects no reference, or whose path
  mismatches was previously counted as validated, so a run with unchecked
  claims could report `Complete`. Consumers that read only the counters see a
  strictly more informative report; the field is `serde(default)` so older
  report JSON still deserializes. Date/Author: 2026-11-14, repair of F4.
- Decision (2026-11-14): Accept the file and line tolerance breach for this
  feature and record it rather than splitting the milestone. Disposition:
  **accepted**, not pending and not waived. The maintainer approved delivering
  6.3.2 in full on 2026-11-10 (Progress), which is the authorization for the
  breach; this entry records the measured excess and its rationale so the
  acceptance is auditable. Measured against the approved base, excluding this
  ExecPlan, the candidate changes 40 files and net +5,466 lines, against the
  stated 30-file and 1,500-line limits. The pre-repair candidate `0acc69e5`
  measured 36 files and net +4,152, which matches the hosted assessment's
  independent count; the review repairs added the adapter, property and
  regression modules. This entry was measured against the current base
  `54dab732` at `ce0cbc51` with `git diff --numstat` and the plan path
  excluded. The excess is the feature itself: a validator, its policy seam, a
  consumer adapter, five unit/property test modules, a behavioural feature
  file, two end-to-end fixtures, and the documentation that records the
  boundary. Splitting it would leave the plan's acceptance unverifiable in any
  single candidate. Date/Author: 2026-11-14, repair of the assessment's V3 and
  second-round F4.
- Decision (2026-11-14): Adapter tests live in `tests_surface.rs`, not
  `tests.rs` as the verification plan names. `tests.rs` already holds 319
  lines; the adapter's four cases would push it past the repository's 400-line
  cap. The artefact is renamed here so the plan and the tree agree.
  Date/Author: 2026-11-14, implementation detail of INV-EFFECTIVE-INPUTS.
- Decision (2026-11-14): An example whose surface path is missing is checked
  but not bound. The manifest check already reports the undeclared path once,
  so the example adds no second finding for it; the example instead stops at
  the first *declared* option because no option can be accepted without the
  path, and `examples_bound` does not count it. Rationale: the second hosted
  assessment found that the previous code returned `true` from that branch, so
  an example whose options were never examined still counted as validated,
  which overstates coverage and can report `Complete` for a run that checked
  nothing. Undeclared options before the stop are still reported, because
  ownership needs only the reference. Date/Author: 2026-11-14, repair of the
  second assessment's F1.
- Decision (2026-11-14): The agent-context adapter preserves conflicting
  duplicate declarations. A repeated input that states the same value type is
  collapsed to one option, as before. A repeat that states a *different* value
  type is kept, so the surface is ambiguous and validation returns
  `CommandSurfaceError::ConflictingOption` instead of letting list order decide
  which type a claim is judged against. Rationale: the second assessment found
  that first-wins collapsing silently discarded the disagreement, so an
  inconsistent source tree produced a plausible but arbitrary surface. An
  absent value type states nothing and therefore never conflicts. Date/Author:
  2026-11-14, repair of the second assessment's F2.
- Decision (2026-11-14): Coverage requires the claimed path to exist,
  independent of which options the example names. An optionless example, or one
  whose only options its reference does not declare, previously returned bound
  because nothing consulted the surface; a run of such examples could report
  `Complete` while every claimed path was undeclared. Ownership findings are
  still emitted first, in source order, before the example is left unbound.
  Date/Author: 2026-11-14, repair of the third assessment's F1.
- Decision (2026-11-14): A silent duplicate declaration is upgraded, not
  discarded. `EffectiveOption.value_type` is adopted from the first declaration
  that states one, and only two *stated* types can conflict. The previous
  first-wins comparison let `None, string, integer` collapse to `None`, so the
  disagreement never reached validation. Date/Author: 2026-11-14, repair of the
  third assessment's F2.
- Decision (2026-11-14): Customize code health with
  `.codescene/code-health-rules.json` rather than by reshaping test-support
  signatures. The file holds one rule set per path and nothing else, so every
  other rule keeps its default weight on the files it names, and each set
  carries a `matching_content_path_doc` recording the reason. Disabling the
  factor for two test-only fixture modules is CodeScene's own documented
  test-code leniency; the alternative was wrapping roughly three hundred call
  sites in newtypes or widening parameters to `impl AsRef<str>`, both of which
  make the tests harder to read for a metric rather than for a reader.
  Date/Author: 2026-11-14, execution of the approved plan.
- Decision (2026-11-14): Repair the `String Heavy Function Arguments` finding
  in `skill_validation/resolve.rs` with domain wrappers, not a rule exception.
  `CommandPath`, `OptionName`, and `RenderedPath` state which string is a path,
  a name, or a rendered diagnostic, so the declared types also stop a lookup
  from swapping a path for an option name. The path-scoped rule weight stays
  confined to the two test-support modules, where the literal names are the
  test's subject and a wrapper would only obscure it. Date/Author: 2026-11-14,
  execution of the approved plan.
- Decision (2026-11-14): Execute the return-to-main step that the 2026-11-09
  decision deferred. PR #664 merged on 2026-11-11 as `eaf4c028`; the branch's
  eleven own commits were replayed with `--onto origin/main 501ab831`. The
  bundled restack planner refused the boundary because the parent's final head
  (`91750238`) is later than the inherited parent, so the boundary was
  established from preserved history instead: `501ab831` is an ancestor of
  `91750238`, the landing commit and `91750238` differ only in reconciled
  `Cargo.lock` and `scripts/requirements-test.txt`, and the replay range holds
  no merges. `range-diff` then showed all eleven patches unchanged.
  Date/Author: 2026-11-14, execution of the approved plan.
- Decision (2026-11-10): No new ADR for this feature. Design §3.4.1 now records
  the ownership boundary, the normalized example shape, and the structural
  versus policy result classes; ADR-0003 already governs agent-context schema
  ownership and ADR-0008 governs policy modes and exceptions. This work adds no
  wire-schema change, no runtime dependency, and no new persisted format, so a
  further ADR would restate existing decisions. Revisit if a released consumer
  contract later depends on these result classes. Date/Author: 2026-11-10,
  approved deviation scope.
- Decision (2026-11-10): `PolicyMode` derives `Default` with `#[default] Off`.
  The documented enforcement default is off, so `ResolvedAgentPolicy` can be
  built by callers that supply only exceptions. This changes no serialized
  representation: the enum keeps its `snake_case` names. Date/Author:
  2026-11-10, implementation detail of the approved scope.
- Decision (2026-11-10): Deliver the full 6.3.2 feature on the maintainer's
  explicit approval, notwithstanding the incomplete 7.1.2 and 7.1.3
  prerequisites. The canonical human-facing global-option glossary from
  `docs/agent-native-cli-design.md` §5 is added to `cargo_orthohelp::policy`
  `::vocabulary` as a shared lookup (concept, canonical name, accepted values,
  and near-miss names). The `Vocabulary` wire struct is deliberately not
  extended, so the policy-report and agent-context schemas keep version 1.
  `skill_validation` consumes that one glossary and `is_canonical_flag`; it
  does not carry a private copy. Roadmap 7.1.2 and 7.1.3 remain separate items
  whose remaining work is the bridge-IR lint wiring, not the glossary data.
  Date/Author: 2026-11-10, approved deviation.
- Decision (2026-11-09): Temporarily target
  `issue-644-nested-cargo-timeouts` (PR #664). Its serialized nested Cargo
  operations and bounded diagnostic capture address the timeout blocker in the
  validation infrastructure. This does not complete the vocabulary
  prerequisites or approve feature implementation. After #664 merges, the
  implementing agent must fetch `main`, verify the merge is reachable there,
  and rebase this branch's own commits onto `origin/main`, then retarget its PR
  to `main`. Use the rebase skill to establish the replay boundary: the
  inherited parent is `501ab83134eaba0f9e97e1daeaa29c793a819c53`, not the
  eventual squash-merge commit. Recheck this boundary if the parent advances;
  preserve recovery refs and avoid replaying the parent's changes. Repeat
  required gates and publish with an explicit force-with-lease.

- Decision: Propose `cargo_orthohelp::skill_validation` as a reusable API
  called by consumers. Consumers retain manifest I/O, format checks, prose
  parsing, and example extraction. Rationale: this reconciles design §3.4 with
  current Rustdoc and preserves the consumer boundary. Date/Author: 2026-10-14,
  draft.
- Decision: Represent examples as normalized claims bound to manifest ID and
  command path. Preserve each option name, value-presence, values, positionals,
  and source ordinal. Rationale: validation cannot be sound if option values
  are discarded or shell syntax is guessed. Unsupported extraction remains
  explicitly unchecked. Date/Author: 2026-10-14, draft.
- Decision: Keep structural and policy findings separate. Structural failures
  are always returned; policy mode and exceptions affect vocabulary and
  global-option findings only. Rationale: `off` cannot make a false command
  claim valid. Date/Author: 2026-10-14, draft.
- Decision: Use only explicit effective per-command option sets. Do not infer
  child globals from root inputs. Rationale: current `AgentInput` does not
  encode inheritance. Date/Author: 2026-10-14, draft.
- Decision: Coverage counts are descriptive, not completeness thresholds.
  Empty refs/examples remain valid for prose-only skills. Rationale: the body
  format has no standard command-section grammar. Date/Author: 2026-10-14,
  draft.
- Decision: Do not add a Verus milestone for exact finite path/flag lookup.
  State soundness/completeness as acceptance obligations and compare the
  implementation against an independent linear oracle with generated cases and
  negative controls. Rationale: proof would formalize list membership but would
  not cover the uncertain extraction/global-option bridge; property and
  integration tests give more relevant evidence here. If implementation adds
  non-trivial contractual logic, pause and revise the proof sketch before
  proceeding. Date/Author: 2026-10-14, draft after expert review.

- Decision: Reuse PR #651 and ADR-0017 for any later proof milestone; do not
  add another installer, tool pin, or fixture-only proof claim. Tool
  availability does not change the proportionate verification decision for this
  feature. Date/Author: 2026-11-08, draft revision requested by maintainer.

## Outcomes & retrospective

Planning history: Wyvern reconnaissance and all six expert perspectives were
reconciled. The original October repository checks passed for that planning
snapshot; they do not constitute feature acceptance.

Implementation (2026-11-10): EP-M1 to EP-M3 are complete. The reusable
validator lives in `cargo_orthohelp::skill_validation` and consumes supplied
manifest claims, normalized examples, an explicit per-command effective option
surface, and a resolved policy. The shared global-option glossary is data in
`cargo_orthohelp::policy::vocabulary::global_options`. The skill-manifest
validator is its only consumer today. The roadmap 7.1.2 and 7.1.3 linter and
policy passes will read the table once they are implemented. Structural
findings are independent of policy mode, policy advice is emitted only for
resolved claims, and report order is stable by manifest input order, source
ordinal, then code.

Lessons: the two largest implementation files exceeded the 400-line cap and
were split (`validate` into `validate/mod.rs`, `validate/examples.rs`,
`validate/manifest_index.rs`; the tests into `tests.rs`, `tests_examples.rs`,
`tests_policy.rs`). `rstest-bdd` fixture binding is by parameter name, and
typed placeholders already handle quoted values. The independent property
oracle caught two ordering expectations that the unit tests had asserted
backwards, which is the evidence that the oracle is not a restatement of the
implementation.

Closure (2026-11-14): EP-M4 completed. The full sequential gate sweep passed on
the frozen candidate `e4038718` with a clean working tree, and roadmap 6.3.2
was checked only after that sweep. Remaining work is hosted review and CI,
which belong to the PR lifecycle rather than to this plan.

## Context and orientation

The reusable agent-context schema lives in
`ortho_config/src/agent_context/mod.rs`. `AgentCommand` holds a command path,
inputs, a canonical verb, and examples. `SkillManifest` holds a stable ID,
relative path, opaque schema-version string, and command references.
`SkillCommandRef.path` is exact; flags are exact long names without `--`. An
empty command-reference list is valid for a prose-only skill.

`cargo-orthohelp` owns the bridge and policy machinery. Current `PolicyInputs`
is empty, and policy checking occurs before bridge command metadata is built.
The 7.1.2/7.1.3 work is a real contract dependency. The proposed API receives
already extracted skill claims plus a complete command surface; it does not add
automatic file loading or a new CLI command.

Fixtures should resemble an operation skill and a build-workflow skill, as
requested, using generic command shapes and opaque values. The validator tests
structure, vocabulary, and bindings; it does not encode Weaver or Netsuke
domain semantics.

## Conformance basis

- Current base: `origin/main` at `54dab7321445204c12fa5f6a756cb099556c53fe`.
  The branch's own eleven commits were replayed off the inherited PR #664 parent
  `501ab83134eaba0f9e97e1daeaa29c793a819c53` on 2026-11-14, after #664
  squash-merged as `eaf4c028`. The replay boundary is evidence-based: the
  parent's final head `91750238` has the same tree as the landing commit apart
  from reconciled dependency files, `501ab831` is its ancestor, and the replay
  range contained no merges and no inherited commits. Main then advanced by one
  test-file-only commit, PR #524 (`54dab732`), and the branch was replayed
  again onto that head; the replay is an ancestor of the candidate head.
  Recovery refs are `refs/recovery/6-3-2-old-head` and
  `refs/recovery/6-3-2-old-base`.
- Previous planning base: `ddba3578fae04227e426ea3a61aa2b24aa043232`
  (PR #651, merged 2026-11-06), rebased from original planning base
  `dc979878b5728bbb23866df9c05c2570bdb5629e` on branch
  `6-3-2-skill-manifest-validation`. Recheck the base and prerequisite state
  before implementation; this planning snapshot does not certify future heads.
- `docs/roadmap.md` §6.3.2 requires real command-path and flag validation,
  examples checked against canonical vocabulary/global options, and generic
  operation/build-workflow fixtures. Prerequisite 6.3.1 is complete;
  prerequisites 7.1.2 and 7.1.3 are not. See `EP-M0`.
- `docs/agent-native-cli-design.md` §3.4 assigns skill prose/workflows to
  downstream apps and says manifests are checked against command metadata. §5
  defines canonical verbs, flags, policy, and global-option glossary. Reconcile
  its ownership with current Rustdoc before implementation.
- `docs/adrs/adr-0003-define-schema-ownership-for-agent-native-contracts.md`
  governs agent-context schema ownership. [ADR-008][adr-008] governs opt-in
  policy modes and exceptions.
- `docs/execplans/6-3-1-skill-manifest-metadata.md` is the preceding schema
  decision; validation was explicitly deferred to 6.3.2.
- [ADR-0017][adr-0017], `docs/design.md` prover-tooling section,
  `docs/developers-guide.md` verification tools and production inventory, and
  `docs/execplans/import-prover-tools.md` govern tool ownership and evidence.
  Their fixture acceptance is not feature proof acceptance.
- Trace links: `ROADMAP-6.3.2-PATH-FLAG` -> `EP-M2` -> structural tests,
  property oracle, and negative controls; `ROADMAP-6.3.2-EXAMPLES-VOCAB-GLOBAL`
  -> `EP-M3` -> normalized example and policy tests plus fixture integration;
  `ROADMAP-6.3.2-FIXTURES` -> `EP-M3` -> operation and build-workflow fixtures
  and BDD scenarios.
- No separate terms of reference exists. The roadmap and
  `agent-native-cli-design.md` are the governing requirement and design.

## Verification plan

Abstract model: let `C` be a finite set of command contracts with unique exact
paths and explicitly effective long options. Let `M` map unique manifest IDs to
declared command references `R`, and let `E` be normalized examples. A
reference resolves iff its path occurs in `C` and each flag listed on that
reference occurs in the resolved command's effective options. An example is
owned only when its `command_reference_ordinal` selects a reference in the
owning manifest, its path equals that reference's path, and each option name
occurs in that reference's declared flags. It is real only when that path
resolves in `C` and each option name belongs to the resolved command. Option
values are preserved, not tested for membership in `C`. Policy is a separate
function over resolved uses and the approved 7.1 policy.

Input-integrity predicate: a command surface is valid only when every path is
unique and each path has one unambiguous effective option set; duplicate paths
or conflicting duplicate option metadata return a typed surface error before
claims are evaluated. Manifest IDs must be unique; duplicate IDs produce a
structural finding keyed by ID and ordinal. Each example binds to the owning
manifest and a specific command-reference source ordinal, so duplicate paths in
a manifest do not cause an arbitrary reference to win. For a reference with an
unknown path, emit the path finding and do not cascade into unknown-flag
findings for that reference.

Postconditions: each structural finding corresponds to a failed integrity or
membership predicate and every failed predicate yields its defined finding;
policy mode/exceptions cannot change structural findings. Each offending claim
produces at most one finding for its path and one per declared flag or example
option. No postcondition claims exhaustive prose scanning, shell executability,
option arity, or domain-value correctness. These are contracts for tests, not a
decision to introduce formal verification.

- `INV-REF-SOUND-COMPLETE`: compare validator findings with an independent,
  deliberately simple linear reference oracle for generated command/reference
  lists. Artefact: `cargo-orthohelp/src/skill_validation/proptests.rs`.
  Evidence: `cargo test -p cargo-orthohelp skill_validation`; generate empty,
  singleton, nested, matching, missing, and reordered cases without excessive
  filtering. Include witnesses for each class and a mutation that swaps path or
  flag equality; the property must reject it. This is proportionate for finite
  exact lookup and can fail on omitted, widened, or mis-scoped checks.
- `INV-EFFECTIVE-INPUTS`: table-test that the consumer/bridge adapter passes
  explicit per-command effective inputs exactly, including an explicit global
  option and a similarly named root-only non-global option. Artefact:
  `cargo-orthohelp/src/skill_validation/tests_surface.rs`; the adapter arrived
  after `tests.rs` neared the file-size cap, and the rename is recorded in the
  decision log. A root-option-union mutant must fail. This test covers the real
  adapter boundary; the oracle alone does not prove clap metadata.
- `INV-EXAMPLE-BINDING`: an example must bind to a specific reference ordinal
  in its owning manifest, and each example option must be in that reference's
  declared flags as well as the real effective inputs for that path. Generated
  and table tests preserve option name, value presence, values, example source
  ordinal, and referenced-command ordinal. Cover attached/separate and repeated
  values, booleans, positionals, unknown options, wrong owner, missing index,
  unsupported extraction, and malicious path/argv strings. Apply values only to
  value-sensitive 7.1 rules (for example enum values or a global option's
  canonical shape); preserve other domain values without validating them. A
  duplicate-path pair with disjoint flags must not allow an example to union
  their flags. A mutant dropping values or rebinding a child example to root
  options must fail. No process or filesystem effects may occur.
- `INV-POLICY-SEPARATION`: test modes `off`, `warn`, and `deny`, exceptions,
  noncanonical vocabulary, global-option near-misses, and structural errors.
  Structural findings remain in all modes; exceptions only affect policy
  results. Each mode must have structural and policy witnesses. A mutant hiding
  structural errors under `off` or accepting a nonexistent exception flag must
  fail.
- `INV-FIXTURE-EXTRACTION`: `rstest-bdd` plus end-to-end fixture adapter
  verifies the extracted records match source fixtures. A drift mutation that
  edits fixture text without changing extracted claims must fail. Use generic
  operation-skill and build-workflow skill fixtures; do not embed either
  application's domain semantics. Empty coverage must be represented honestly.

### Recorded evidence

- Focused library suite: `cargo test -p cargo-orthohelp --lib skill_validation`
  passed 35 tests. The cases cover empty input, nested resolution, unknown path
  without flag cascade, per-flag single reporting, duplicate manifest ID on the
  repeat, exact option comparison, partial status for unsupported extraction,
  ordering, byte stability, surface-order independence, and the four typed
  ambiguous-surface rejections.
- Behavioural suite:
  `cargo test -p cargo-orthohelp --test skill_validation_bdd` passed 6 tests,
  including a test that pins the finding codes quoted by the feature file to
  their exported constants.
- Consumer end-to-end suite:
  `cargo test -p cargo-orthohelp --test skill_validation_e2e` passed 5 tests.
  The drift test re-parses mutated fixture text and asserts both the extracted
  record and the resulting findings, so a stale adapter fails before the
  finding assertion.
- Package Clippy:
  `cargo clippy -p cargo-orthohelp --all-targets --all-features -- -D warnings`
  finished clean after the repair of seven published-API and test-hygiene
  findings (`missing_const_for_fn`, `needless_pass_by_value`, `ref_option`,
  `too_many_arguments`, `shadow_reuse`, `excessive_nesting`, `assert_is_empty`,
  `panic_in_result_fn`).
- Logs are kept under `/tmp/*-ortho-config-6-3-2-skill-manifest-validation.out`.
- Repository gates on the implementation candidate (2026-11-13):
  `make check-fmt` reported 100 files unchanged; `make typecheck` finished with
  `MAKE_EXIT:0`; `make lint` completed rustdoc, Clippy, and Whitaker with
  `MAKE_EXIT:0`; `make markdownlint` (with the spelling gate) and `make nixie`
  both exited 0. `make test` was interrupted twice by session boundaries rather
  than by a failing assertion; the two real failures it did expose were the
  users-guide transcript fence without a marker and its missing
  `EXPECTED_EXAMPLE_IDS` entry, both repaired and re-checked with
  `cargo test -p ortho_config --test documentation_examples_tests --test
  documentation_examples_rust_tests`
  (`CARGO_EXIT:0`). The final complete `make test` run on the frozen candidate
  `e4038718` then passed with `MAKE_EXIT:0` and 113 `test result: ok` suites;
  the whole gate sweep was repeated on that commit with a clean working tree,
  so every gate certificate now refers to one candidate.
- Repository gates on the replayed candidate (2026-11-14, head `a465f690`,
  based on `origin/main` `724839c4`): `make check-fmt` reported 103 files
  unchanged, `make markdownlint` (with spelling) reported zero issues, and
  `make nixie` validated every diagram. `make typecheck` finished with
  `MAKE_EXIT:0`; `make lint` completed rustdoc, Clippy, Whitaker, and the
  docstring-coverage gate (1011 documented items) with `MAKE_EXIT:0`;
  `make test` finished with `MAKE_EXIT:0`, 114 `test result: ok` suites, no
  failing test, and the Python suite at 128 passed with 5 skipped. The replay
  invalidated the earlier certificates by candidate identity, so these replace
  them. Each gate wrote
  `/tmp/<gate>-ortho-config-6-3-2-skill-manifest-validation.out`.

No introduced contractual business logic is selected for formal verification:
exact finite membership and reuse of completed policy rules have direct,
independent executable oracles, while the uncertain integration boundary is
consumer extraction and real bridge metadata. Verus would add little assurance
for the membership helper and would not prove the integration obligations. If
implementation adds a stateful or non-trivial all-input contract, return to
proof-sketch-first design, select Verus only if warranted, reuse the repository
pins and Make targets from PR #651, and add a feature-owned proof milestone
before implementation. Do not add `assume`, `admit`, or trusted proof shortcuts.

### Existing prover tooling and conditional proof acceptance

PR #651 already pins Kani through `tools/kani/VERSION` and Verus through
`tools/verus/VERSION` and `tools/verus/SHA256SUMS`. The Makefile pins the shared
`rust-prover-tools` package. Reuse these sources of truth rather than copying
versions into feature code. The lane is validated on Linux x86_64. No prover
installation or execution is required for this planning revision or for the
currently selected membership/property-test strategy.

If an approved revision selects a substantive unbounded contract, first state
its abstract model, satisfiable premises, representation relationship to the
production Rust symbol, and semantic lemma sketches. Register the feature-owned
proof in `VERUS_PROOF_FILES`, naming its owner, source symbol, assumptions,
expected check count, and rejecting negative control as ADR-0017 requires.
Require `make verus` with that non-empty inventory to succeed, alongside
`make check-fmt`, `make typecheck`, `make lint`, and `make test`. A focused run
may use `make verus VERUS_FLAGS='--proof-file path/to/proof.rs'`, replacing the
placeholder with the approved proof file. Verification must establish the
property from its premises, include executable-to-model refinement evidence,
and reject the negative control; fixture proofs alone cannot discharge it. Do
not add speculative proof bodies or trust shortcuts.

Provisioning is separate: `make install-verus` installs the pinned bundle and
its required Rust toolchain. Execution uses the bundled Z3 and both
`--no-install-missing` and `--no-ensure-toolchain`; missing prerequisites must
fail without installation. Do not replace these boundaries or run installation
implicitly. `make test-prover-tools-integration` is an opt-in runner check, not
a production proof, and requires tools to be installed first.

If a later approved design instead needs bounded exhaustive exploration,
register its Kani harnesses in `KANI_SMOKE_HARNESSES` and `KANI_ALL_HARNESSES`,
document symbolic inputs and unwind bounds, and require `make kani-check`,
`make kani`, and `make kani-full` with a rejecting control. Use
`make install-kani` only for explicit provisioning. Neither empty-inventory
`FORMAL-EMPTY` failures nor skipped integration tests count as verification.
Tests still cover extraction, diagnostics, policy integration, and externally
observable behaviour even if a future proof establishes an underlying kernel.

External assumptions: clap's generated command tree represents its configured
commands; the consumer or bridge supplies a correct explicit effective-option
surface; the consumer adapter extracts intended claims; Serde carries input
values as modelled. Do not verify third-party internals. Exercise repository-
owned bridge and adapter logic against real fixtures and record residual
extraction limitations.

## Plan of work

### EP-M0: Recheck prerequisites and reconcile design

Verify 6.3.1, 7.1.2, and 7.1.3 on the implementation base. PR #651 completes
tooling only; it does not satisfy the vocabulary prerequisites. Recheck
ADR-0017 and retain the selected test strategy unless new semantic obligations
justify an approved proof milestone. Confirm 7.1 exports canonical rules and
exceptions. Roadmap 7.1.3 does not promise inherited-option metadata, so also
identify an explicit consumer- or bridge-resolved effective option set for each
command, including global-option classification. Reconcile
`agent-native-cli-design.md` §3.4 with current Rustdoc and policy pipeline:
define a reusable validator API invoked by consumers, with downstream I/O and
extraction. Record any required design or ADR update before code work.

Go only when prerequisites, inputs, owners, and approved design agree. If not,
mark `BLOCKED`, record evidence and options, and stop. Do not implement
prerequisites or work around missing metadata.

### EP-M1: Specify normalized input and passing contract tests

Candidate module: `cargo-orthohelp/src/skill_validation/`. Define supplied
manifest claims, structured examples, effective command inputs, separated
structural/policy findings, deterministic coverage counts, stable codes, and
finding identity based on manifest ID plus source ordinal. Keep paths inert and
diagnostics free of raw argv values. Represent unsupported/unchecked examples
explicitly. Empty reference/example lists are valid and yield zero counts.

Add passing `rstest` contract tests for construction, deterministic ordering,
empty input, and typed rejection of ambiguous command surfaces. The
missing-path regression belongs to EP-M2 so this plateau ends green. Establish
the abstract equivalence contract and independent test oracle before production
implementation.

### EP-M2: Implement structural resolution

Implement exact command-path and scoped long-flag checks. Diagnose duplicate
manifest IDs deterministically. Structural findings remain visible in `off`
mode; exceptions cannot suppress them. Do not normalize case, aliases,
punctuation, or `--` prefixes because 6.3.1 specifies exact path segments and
long names without the prefix.

Run focused `rstest`, `proptest` against the independent linear oracle, and
mutation controls for swapped predicates, omitted references, and wrong-command
lookup. This plateau provides correct structural validation without vocabulary
or example policy integration.

### EP-M3: Validate examples, apply policy, and exercise consumer fixtures

Connect to completed 7.1 policy and explicit effective command inputs. An
example path must be declared by its owning manifest, and its options must be
declared by that path's command reference and exist on the real command.
Preserve value presence and contents; inspect values only for value-sensitive
7.1 policy rules, not general option arity or domain validity. Apply canonical
vocabulary and global-option rules with configured modes and exceptions. Do not
parse raw `AgentExample.command` as shell.

Add generic operation and build-workflow skill fixtures. A narrow test adapter
extracts the deliberately small fixture convention and sends those records to
the API; it is integration evidence, not a supported general Markdown parser.
Store the fixtures at
`cargo-orthohelp/tests/fixtures/skills/operation/SKILL.md` and
`cargo-orthohelp/tests/fixtures/skills/build-workflow/SKILL.md`. Use realistic
workflow-shaped examples with opaque resource names and values; do not encode
application semantics or execute commands. In these test fixtures only, use a
`Command index` heading followed by one JSON array of `SkillCommandRef`
records, and a `Checked examples` heading followed by one JSON array of the
normalized example records defined below. Each array occupies a fenced `json`
block. The test adapter requires each heading exactly once, rejects malformed
arrays, and extracts directly from these blocks; surrounding workflow prose
remains opaque. This convention is fixture-owned, not a new public manifest
format. A source mutation replacing a known option with `missing-option` must
change the extracted record and yield an unknown-option finding. A mutant
adapter returning stale records must fail the source-to-record assertions.

Add an `insta` report snapshot only if multivariant report output is part of
the approved contract. Add end-to-end tests because this is a consumer
integration boundary.

The feature specification lives at
`cargo-orthohelp/tests/features/skill_manifest_validation.feature`; step glue
lives in `cargo-orthohelp/tests/skill_validation_bdd.rs`; consumer extraction
end-to-end coverage lives in `cargo-orthohelp/tests/skill_validation_e2e.rs`.
Run `cargo test -p cargo-orthohelp --test skill_validation_bdd` and
`cargo test -p cargo-orthohelp --test skill_validation_e2e`. The feature is:

```gherkin
Feature: Validate skill claims against an agent command contract

  Scenario: Report a command reference with an unknown flag
    Given a command contract with a nested command and effective options
    And an operation skill fixture with a valid reference and an unknown flag
    When the consumer adapter submits the extracted claims
    Then the report resolves the valid reference
    And reports the unknown flag for the referenced command
    And identifies the manifest and source ordinal

  Scenario: Check normalized example values and global options
    Given a command contract with explicit effective global options
    And a build workflow example with an option name and value
    When the adapter submits the normalized example
    Then the option is checked on that command
    And value presence and contents are preserved

  Scenario: Report a noncanonical option under deny policy
    Given a real command accepting the declared option "colour-policy"
    And an example using "colour-policy" with value "always"
    And deny policy without an exception for that option
    When the consumer submits the normalized example
    Then structural validation succeeds
    And the policy finding recommends "--color"

  Scenario: A root-only option is unavailable on a child
    Given a root command with a non-global option "root-only"
    And a child whose explicit effective options omit "root-only"
    And a child example declaring and using "root-only"
    When the consumer submits it for validation
    Then an unknown-option structural finding is returned

  Scenario: Accept a prose-only skill without implying coverage
    Given a manifest with no command references or examples
    When the consumer submits it for validation
    Then no coverage error is reported
    And checked command and example counts are zero
```

### EP-M4: Document and close acceptance

Update `docs/agent-native-cli-design.md` with ownership, normalized example
contract, structural-versus-policy results, and extraction limits. Update
`docs/developers-guide.md` for internal interfaces/practice and
`docs/users-guide.md` for the consumer API and observable validation behaviour.
Create/update an ADR if ownership or result separation is a substantive
architecture decision. Update `docs/contents.md`. Mark roadmap 6.3.2 done only
after all accepted behaviour and gates pass.

At every major milestone run `make check-fmt`, `make typecheck`, `make lint`,
and `make test` sequentially. Final acceptance also runs `make markdownlint` and
`make nixie`. Use `/tmp` logs per repository convention, inspect terminal
summaries, and do not parallelize gates.

## Milestones and plateaus

### EP-M0 — prerequisite and design gate

Requirements: 6.3.1, 7.1.2, 7.1.3 and ownership reconciliation. End state:
verified prerequisites, explicit API source, and accepted design. Evidence:
roadmap readback and API/design references. Recovery: resume when upstream work
lands. Remaining gaps: all implementation. Compatibility decision: none.
Conformance: requirements complete; ownership explicit; assumptions valid; no
API/dependency invented; no trust or persisted format changed; links current.

### EP-M1 — normalized contract and passing specification

Requirements: reference, example, policy-separation, and prose-only contracts.
End state: input/result contract, empty-input success, and deterministic-order
tests pass. Evidence: ambiguous-surface cases fail with typed errors and
contract tests pass. Recovery: retain tests/spec and revise only through
approved design. Remaining gaps: reference validation. Compatibility decision:
none absent a named consumer. Conformance: exact scoping and values
represented; no file/shell boundary added; no wire schema changed without
approval; links current.

### EP-M2 — structural validator

Requirements: `ROADMAP-6.3.2-PATH-FLAG`. End state: exact structural
resolution, deterministic findings, empty prose-only support. Evidence: the
missing-path red test fails before code and passes after; unit/property tests
pass and seeded wrong-path/flag mutations fail. Recovery: roll back
implementation while retaining approved specification. Remaining gaps: policy
and examples. Compatibility decision: none. Conformance: result corresponds to
exact metadata; consumer ownership preserved; effective-input assumptions hold;
no unapproved API/dependency or I/O; links current.

### EP-M3 — examples, policy, and integration

Requirements: `ROADMAP-6.3.2-EXAMPLES-VOCAB-GLOBAL` and
`ROADMAP-6.3.2-FIXTURES`. End state: examples and policy checked with generic
operation/build-workflow fixtures. Evidence: unit, property, BDD, E2E, and
required snapshots pass; drift and policy-separation mutants fail. Recovery:
retain EP-M2 and roll back integration. Remaining gaps: docs and final gates.
Compatibility decision: none. Conformance: values and paths preserved; policy
reused from 7.1; no domain semantics or file loading; no unapproved
schema/dependency; links current.

### EP-M4 — docs and acceptance

Requirements: all roadmap bullets and documentation obligations. End state:
docs match implementation and all sequential gates pass. Evidence:
`make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, `make nixie`, and BDD/E2E targets pass. Mark roadmap item
done only now. Recovery: repair from cited logs and rerun affected plus
downstream gates in sequence. Remaining gaps: none except recorded follow-up.
Compatibility decision: none. Conformance: requirements/docs agree; deviations
accepted; assumptions and residual gaps recorded; dependencies authorized;
trust/persistence unchanged; trace and evidence current.

## Concrete steps

Run commands from the repository root. Before each code milestone, read the
current roadmap and this plan's milestone; record branch and base commit. Keep
gate output in `/tmp` with the action/project/branch template in `AGENTS.md`.

At EP-M0 inspect the exact 7.1.2/7.1.3 APIs. The BDD and E2E commands are
specified in EP-M3; confirm their integration targets exist when implemented.
At each implementation milestone follow Red-Green-Refactor: run the focused
regression before production changes and record the expected failure; make the
smallest fix; rerun focused tests; then refactor and rerun focused plus wider
checks. Keep the red interval inside its milestone and commit only after the
milestone is green. Run format, lint, and test gates sequentially. No Verus
command is planned because no formal proof milestone is selected.

## Validation and acceptance

A real `AgentContext` command tree plus consumer-supplied claims must show a
valid reference resolves; an unknown path and a flag absent on the exact
command produce findings; normalized example names resolve against the owning
reference and effective options, while values are preserved and checked only by
applicable 7.1 value-sensitive policy rules; policy applies canonical
vocabulary/global-option rules. Structural findings remain visible in all
policy modes. Exceptions cannot authorize absent paths or flags. A prose-only
skill succeeds with zero checked references and examples.

Use `rstest` for finite cases, `googletest` and `pretty_assertions` for clear
assertions, and `proptest` for generated lists and normalization-preservation
relations. Use `rstest-bdd` for the feature in EP-M3. Add end-to-end coverage
for the consumer adapter and actual validator API. Use `insta` where stable,
multivariant serialized output is part of the approved report contract. Kani is
unnecessary: there is no bounded machine/state invariant. The independent
oracle and negative controls explain why the property suite detects wrong
membership behaviour; generator cases must cover empty, matching, missing,
nested, and reordered values without excessive filtering.

Red: focused unknown-path test fails because no finding is produced. Green: the
same test passes after structural resolution. Refactor: focused tests, BDD/E2E,
property tests, and sequential repository gates still pass. Record actual test
names, counts, and output in this section during implementation.

## Idempotence and recovery

The validator is pure and can be rerun safely. Fixture extraction reads only
repository-controlled fixtures. Do not run it against arbitrary manifest paths.
On a gate failure, retain the log, repair the smallest affected milestone, and
rerun that gate followed by dependent gates sequentially. If a prerequisite or
design assumption fails, mark the plan `BLOCKED` and preserve the last
validated plateau.

## Artefacts and notes

The report shape must follow the completed policy/report contract. It should
include stable finding code/category, manifest ID, source ordinal, exact
command path, and option/example identity. It must omit raw paths and argv
values. Coverage counts deterministically distinguish claims seen from claims
resolved. Capture concise positive and negative report examples during
implementation; do not freeze output shape before EP-M0.

Illustrative finding fields (not a wire-schema commitment):

```json
{
  "category": "structural",
  "code": "skill.unknown_flag",
  "manifest_id": "operation-skill",
  "source_ordinal": 2,
  "command_path": ["tool", "records", "get"],
  "option": "format"
}
```

A report with unsupported examples must expose a partial-validation status and
separate unchecked counts; absence of findings alone is not a complete
validation verdict. A prose-only manifest with no supplied examples remains a
valid zero-coverage case. Duplicate manifest IDs prevent examples referring to
that ID from choosing an arbitrary owner; report the ambiguity and skip those
dependent checks.

Report order is stable by manifest input order, source ordinal, then finding
code. Do not include manifest filesystem paths or example argument values.

## Interfaces and dependencies

Proposed module: `cargo-orthohelp/src/skill_validation/`. Candidate entry
point, subject to EP-M0 API reconciliation:

```rust
pub fn validate(
    input: &SkillValidationInput,
    commands: &EffectiveCommandSurface,
    policy: &ResolvedAgentPolicy,
) -> Result<SkillValidationReport, CommandSurfaceError>;
```

`SkillValidationInput` carries manifest IDs, exact `SkillCommandRef` values,
and normalized examples. Each normalized example carries manifest ID,
`command_reference_ordinal`, path, option uses (name, value presence, values),
positionals, example source ordinal, and option source ordinals.
`EffectiveCommandSurface` explicitly associates each path with the effective
inputs resolved by its consumer or bridge adapter. It rejects duplicate paths
and ambiguous option metadata. It does not infer inheritance. The completed
7.1.3 policy/glossary supplies canonical vocabulary and global-option rules;
`ResolvedAgentPolicy` contains the exact mode, rule levels, and exceptions.
Results separate structural from policy findings. Names and visibility may
change during approved reconciliation; ownership, scoping, and non-I/O
boundaries may not.

No runtime dependency is proposed. Verify existing workspace test dependencies
at EP-M1; add the authorized `googletest` and `pretty_assertions` as
dev-dependencies if missing, using caret requirements. `rstest-bdd` is already
used and follows the repository's existing runner. The repository already owns
prover tooling from PR #651; this feature adds no proof tool, installation
path, or runtime crate.

## Documentation and skill signposts

Consult/update the relevant portions of `docs/design.md`,
`docs/cli-localization-design.md`, `docs/cargo-orthohelp-design.md`,
`docs/clap-dispatch-and-ortho-config-integration.md`,
`docs/agent-native-cli-design.md`, `docs/improved-error-message-design.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`, and
`docs/rstest-bdd-users-guide.md`. The request's `docs/rtest-bdd-users-guide.md`
spelling is not present; the correctly named repository guide is
`docs/rstest-bdd-users-guide.md`. `docs/scripting-standards.md` now exists;
consult it if a later approved change introduces scripts. No script is
currently proposed. Consult [ADR-0017][adr-0017] and the developers' guide
before any proof adoption.

Read `$execplans`, `$rust-router`, `$rust-verification`, `$rust-unit-testing`,
`$proptest`, and `$domain-cli-and-daemons` before their implementation facets.
Use `$codegraph-mcp` for structural navigation and `$firecrawl-mcp` for
external prior art. The Agent Skills source above is prior art only, not an
OrthoConfig format mandate.

## Revision note

- 2026-10-14: Initial DRAFT from roadmap, current context/policy code, Wyvern
  reconnaissance, Firecrawl prior art, and architecture/contracts/operations
  review. No implementation has started; explicit approval and EP-M0 remain
  mandatory.
- 2026-10-14: Reconciled final expert findings, specified fixture extraction
  and partial-validation reporting, and recorded passing planning gates.
  Implementation scope and prerequisite/approval gates remain unchanged.

- 2026-10-14: Reworded two policy-exception statements after the publication
  spelling check rejected their wording. Meaning and remaining work are
  unchanged.

- 2026-11-08: Rebased onto PR #651, preserved both conflicting index entries,
  refreshed moved ADR and scripting references, and specified reuse of the
  existing prover lane for any future approved proof milestone. The selected
  test strategy, incomplete vocabulary prerequisites, and approval gate remain.

- 2026-11-09: Restacked on PR #664 to incorporate the nested Cargo timeout
  fix. Recorded the temporary target, exact replay boundary, and implementing
  agent's return-to-main procedure without changing feature scope. Refreshed
  the parent to `501ab831` after its process-ownership improvements landed
  during validation; the initial-parent results remain historical evidence.

- 2026-11-14: Replayed the branch's own commits onto `main` after PR #664
  squash-merged, retargeted the pull request, and recorded the evidence for the
  superseded-parent boundary. EP-M4 had already closed; the replay invalidates
  the earlier gate certificates by candidate identity, so the gates were run
  again on the replayed head.

- 2026-11-14: Closed EP-M4. Recorded the green full `make test` run and the
  repeated gate sweep on candidate `e4038718`, the third documentation-example
  registry, the capability-filesystem repair of the Whitaker finding, and the
  roadmap 6.3.2 tick. No scope or design change; hosted review and CI remain
  with the PR lifecycle.

- 2026-11-14: Replayed the branch onto main's new head `54dab732`, recorded
  the six-gate sweep on `1594f70f`, and confirmed the feature paths are
  unchanged by the replay. Superseded the hosted completeness verdict given
  against `0acc69e5`; a fresh assessment is required for this revision.

[adr-008]: ../adrs/adr-0008-agent-native-policy-configuration.md
[adr-0017]: ../adrs/adr-0017-consumer-side-prover-tooling.md
