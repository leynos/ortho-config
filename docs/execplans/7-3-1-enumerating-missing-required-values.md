# Enumerate missing required configuration values

Status: APPROVED — implementation complete (approved 2026-11-09).

This execution plan (ExecPlan) is a living document. Keep the constraints,
progress, discoveries, decisions, verification, conformance, and outcomes
current at every milestone.

## Purpose / big picture

A caller with several absent configuration values should receive one typed
error listing every missing requirement and the actual ways to supply it. An
agent should be able to use that diagnostic to construct a successful second
invocation, without discovering one more absent field on each attempt. Roadmap
task 7.3.1 follows the completed 5.1.1 truth audit.

Success means two absent fields, including a supported nested field such as
`database.host`, appear together, with verified CLI, environment, and file
supply paths where those sources are supported. Supplying those values makes
the next load succeed. Invalid values remain invalid-value errors; no values or
secrets are copied into the missing-values report.

## Constraints

- Obtain explicit approval of this plan before changing runtime code, macros,
  tests, dependencies, or the design's implemented-status claims.
- Preserve source precedence, collection merge behaviour, profile selection,
  injected source isolation, clap help/version behaviour, and source failures.
- Never derive requiredness from documentation-only overrides. Never classify
  every deserialization failure as absence or parse an English error string.
- Never advertise a source spelling that the actual provider cannot consume.
  A field excluded from CLI parsing must have no invented CLI suggestion.
- Enumerate supported nested requirements; do not pretend that arbitrary
  custom Serde implementations expose a schema. The coverage contract below
  must be documented and tested explicitly.
- Do not introduce source-API compatibility wrappers for this pre-1.0 crate.
  Update affected callers together. Do not change documentation IR, agent
  context, or policy-report schema versions merely to carry runtime errors.
- Use capability-based filesystem helpers and injected environment sources.
  Process-environment mutations require existing shared guards. Keep Rust files
  under 400 lines, dependencies on caret requirements, and warnings denied.
- Run gates sequentially through the scrutineer agent, with logs in `/tmp`.
  Preserve compiler admission and the shared Cargo cache. Do not install or
  alter Whitaker. Do not use `/tmp` as a build directory.
- Keep prover execution opt-in and feature-owned, per
  `docs/adrs/adr-0017-consumer-side-prover-tooling.md`. Do not add `make kani`,
  `make kani-full`, or `make verus` to this plan's ordinary gates. With an
  empty inventory they exit 2 with `FORMAL-EMPTY` by design; that is neither a
  pass nor a failure of this work. `make test-prover-tools-integration` is
  evidence about the runners and must never be cited as evidence about this
  feature.

## Tolerances (exception triggers)

Stop and record a proposed deviation, with status BLOCKED, if recursive schema
coverage cannot meet the contract without changing unrelated provider or Serde
behaviour; if a new production dependency is needed; if error handling requires
replaying custom deserializers; if source naming cannot be made truthful
without a broader configuration-format migration; or if a re-evaluation under
D5 selects Kani or Verus. Obtain explicit approval of the deviation before
continuing.

The new typed error and the narrowly scoped nested-schema API proposed here are
part of the approval request. M1 signature refinements implementing the
approved trait and marker contract are in scope; changes to supported cases,
bounds, or source semantics require a recorded deviation. Other public API
changes require renewed approval. Test dependencies named in this plan are
authorized. No arbitrary time limit applies. Report infrastructure failures
without bypassing them. After two targeted repairs of the same failing gate,
reassess the hypothesis and record evidence before further changes; do not
repeatedly rerun it.

## Risks

The highest risk is false requiredness: optional fields, defaults, collections,
and custom deserialization do not all follow a non-Option rule. Mitigate with
an explicit semantic matrix and real generated-loader fixtures in M1.

Nested types and flattened fields are a high-likelihood design risk. A derive
macro cannot inspect another type's syntax. Use explicit schema composition;
make unsupported opaque cases visible in documentation and the diagnostic
coverage contract, and stop if this cannot satisfy the approved scope.

Source names can diverge from documentation metadata, especially Serde renames,
clap overrides, subcommands, and environment separators. Test each advertised
route by actually supplying a value through it. Metadata snapshots alone are
insufficient.

An abstract model can describe the wrong loader. Keep the runtime-to-model
mapping explicit and test its adapters. PR #651 imported pinned Kani and Verus
runners, but no production harness or proof exists. The proposed collector is
straightforward filtering; property tests are proportionate, and no formal
verification result is claimed.

Available tooling creates its own risk: a proof added because a runner now
exists would be well-formed and vacuous about the actual hazards, schema
fidelity and source resolution. D5 therefore ties adoption to the kernel's
content. Kani's ability to compile the production crate also remains unclaimed.
ADR-0017 kept that question out of its fixture evidence, so any harness starts
with a compatibility probe rather than an assumption.

## Progress

- [x] (2026-10-14) Read repository guidance and requested planning skills.
- [x] (2026-10-14) Rename the branch and configure its requested upstream.
- [x] (2026-10-14) Use three Wyvern investigations for runtime, requirements,
  and verification evidence, exchanged through context pack `pk_knocbild`.
- [x] (2026-10-14) Draft contracts, proof sketch, milestones, and acceptance.
- [x] (2026-10-14) Complete six-perspective community-of-experts review
  across three agents and reconcile its findings.
- [x] (2026-10-14) Preliminary formatting, Markdown/spelling, and diagram
  validation passed; the final candidate will be rechecked before commit.
- [x] (2026-10-14) Commit and publish the reviewed plan as draft PR #589.
- [x] (2026-11-06) Rebase onto `ddba3578` (PR #651 landed) and revise this plan:
  prover command surface, inventory obligations, D5, and refreshed baseline.
- [x] (2026-11-09) Rebase both plan commits onto the timeout fix in PR #664;
  retain their patches unchanged and record the temporary parent boundary.
- [x] (2026-11-14) Return to `main` after #664 merged (`eaf4c028`,
  `9ede81a1`): rebase onto `959dbd7e` from the boundary `26275a4e`, dropping
  the pre-merge copies of the timeout fix, and renumber ADR-0018 to ADR-0020
  after main took 0018 and 0019.
- [x] (2026-11-15) Rebase onto `f086edc9` (PR #615 landed): main took 0020 and
  0021 for the artefact-lock records, so renumber this task's decisions from
  0020/0021 to ADR-0023/ADR-0024 with the file names, titles, and every
  reference updated.
- [x] (2026-11-16) Rebase onto `7b750601`: main took 0023 for the
  structured-output record, so renumber this task's decisions again, from
  0023/0024 to ADR-0024/ADR-0025, with the file names, titles, and every
  reference updated.
- [x] (2026-11-09) Obtain explicit plan approval.
- [x] (2026-11-09) M1 audit: derive field-metadata map, supply-route map, and
  error-classification sites. Settled the schema, requiredness, and route
  contract in ADR-0024 and the improved-error design.
- [x] (2026-11-09) M1 aggregate red reproducer recorded against the unchanged
  loader: two absent required fields produced
  `Failed to merge CLI with configuration: missing field 'reference'`, naming
  only the first. Characterization checks passed: a single absent requirement
  is an `OrthoError::Merge`, and absent `Option`/defaulted fields load. Log:
  `/tmp/m1-red-ortho-config-7-3-1.out`. The probe file was removed.
- [x] (2026-11-09) Implemented the schema trait, pure collector, typed error
  payload, English display, and bounded telemetry classification. The program
  label travels on `LayerComposition`, and every generated merge entry point
  calls the labelled constructor. The standalone `RequiredConfigSchema` derive
  serves nested child types.
- [x] (2026-11-09) V1 real-loader matrix passes: 14 integration tests cover
  required leaves, aliases, CLI and environment supply, present-empty `Option`,
  renamed keys without a CLI route, `skip_cli`, inferred clap defaults, nested
  parent activation, a present non-object, and the recursive-schema guard.
  Focused unit tests pass (41), and the declarative token fixtures were updated
  for the new state field and constructor.
- [x] (2026-11-09) V2 property tests pass: `proptest` cases over 0–64
  requirement paths compare the collector against an independently computed
  model for membership, declaration order, repair, and nested activation.
  Seeded omission, inversion, and reordering controls each fail their assertion.
  `cargo test -p ortho_config --lib required_values` reported 26 passing tests.
- [x] (2026-11-09) V3 macro-expansion and compile evidence passes: 12 token
  tests cover crate renaming, caller-declared generic bounds, nested
  composition, wrapper stripping, Serde naming, requiredness, skipped and
  unmarked-flatten fields, and CLI hints. The new trybuild pass fixture proves
  the standalone derive compiles from a renamed downstream crate; all three
  `crate_path_trybuild` fixtures passed.
- [x] (2026-11-09) V4 display and consumer-policy evidence passes: 10 tests pin
  the English rendering with an `insta` snapshot, exact multiline output, path
  quoting, and route-section numbering, while `googletest` matchers classify
  membership. Five child-process tests show a consumer exiting non-zero with
  the diagnostic on standard error and exiting zero once any advertised route
  supplies the values. No localized presentation path is exposed for this
  error, so only the English rendering is pinned.
- [x] (2026-11-09) V5 behavioural evidence passes: four `rstest-bdd` scenarios
  cover one-diagnostic recovery through the command line and through a
  file-plus-environment mix, the absence of reports for optional and supplied
  values, and an invalid nested value remaining a deserialization error. The
  `rstest_bdd` target reported 96 passing tests. The previous `Merge`
  expectation in `clap_integration/error_cases.rs` and the missing-field
  message assertion in `declarative_merge_empty_layers.rs` were updated to the
  enumerated diagnostic.
- [x] (2026-11-09) First full-gate attempt exposed eight deterministic Clippy
  failures in `ortho_config_macros`, none of which the test builds reported:
  `single_match_else` and `option_if_let_else` in the schema generator,
  `struct_excessive_bools` on `FieldAttrs`, `too_many_lines` on the derive
  entry point, `too_many_arguments` on the schema builder,
  `self_named_module_files`, and two `needless_pass_by_value` helpers in the
  schema tests. Repaired by mapping the prefix with `map_or_else`, folding
  `skip_cli` and `is_subcommand` into a `CliFieldMode` state machine, extracting
  `expand_ortho_config` and `expand_required_config_schema`, taking
  `RequiredSchemaSource` in one argument, moving the generator to
  `required_schema/mod.rs`, and borrowing in the test helpers. A focused
  `cargo clippy -p ortho_config_macros --all-targets --all-features --
  -D warnings`
  run then passed. Red log: `/tmp/lint-ortho-config-7-3-1.out`; green log:
  `/tmp/clippy-macros-ortho-config-7-3-1.out`.
- [x] (2026-11-09) M4 documentation reconciliation: the improved-error design
  now reports the implemented state and the realized value-based collector;
  `docs/design.md` carries the ownership split; the users' guide, developers'
  guide, agent-native CLI design, contents index, and changelog are updated;
  roadmap 7.3.1 and its three subtasks are checked. `mapsplice replace` was
  previewed for the roadmap edit and rejected because it renumbered every phase
  from 1, so the four checkbox lines were changed directly.
- [x] (2026-11-09) Second full-gate attempt reported 32 `ortho_config` Clippy
  diagnostics in the new modules, none visible to the test builds. Repaired by
  moving the payload to `error/missing/mod.rs`, making rebuilt accessors
  `const`/`#[must_use]`, replacing a panicking slice index with `split_last`,
  introducing the `Collector` recursion state to shed the extra arguments,
  borrowing the schema error, splitting the route assertions to reduce
  cognitive complexity, and adding failure messages to the `assert_is_empty`
  sites (that lint only fires on the single-argument form). Later passes
  surfaced `missing_docs_in_private_items`, `print_stdout`/`print_stderr` in
  the child probe, `similar_names`, `use_self`, and `err_expect`; each was
  repaired at source without suppressing a lint. A focused
  `cargo clippy -p ortho_config --all-targets --all-features -- -D warnings`
  run now passes. Red log: `/tmp/lint-ortho-config-7-3-1.out`; green log:
  `/tmp/clippy-focus-ortho-config-7-3-1.out`.
- [x] (2026-11-09) The first Whitaker pass after that repair reported four
  `conditional_max_n_branches` sites and two test modules without inner
  documentation. Repaired by extracting `accepts_cli_hint`, `cli_hint_for`, and
  `has_no_route_lines` predicates and adding `//!` module docs. A further pass
  reported one `no_unwrap_or_else_panic` in the display fixture; the fixture
  now returns a `Result` and its callers thread `?`, since `expect_used` is
  also denied outside `#[test]` bodies. Scoped
  `whitaker --all -p ortho_config_macros` and `-p ortho_config` runs now pass;
  `make docstring-coverage` reports 100%. Red evidence:
  `/tmp/lint-ortho-config-7-3-1.out` for the macros site, and the quoted scoped
  `ortho_config` diagnostics above (that log was overwritten by the green
  re-run). Green logs: `/tmp/whitaker-macros-ortho-config-7-3-1.out` and
  `/tmp/whitaker-ortho-config-7-3-1.out`.
- [x] (2026-11-09) Gate 4 (`make test`) exposed an unmarked fence the
  documentation-example harness rejects: the new `plaintext` rendering under
  "Recover from missing required values" had no `tested-example` marker. Added
  `guide-missing-values-output`, registered it in `EXPECTED_EXAMPLE_IDS`, and
  re-ran both documentation-example targets green. Log:
  `/tmp/doc-examples-ortho-config-7-3-1.out`.
- [x] (2026-11-14) Final candidate `1fbf28cf` passed all six gates sequentially
  after the return to `main`: `make check-fmt` (2 s), `make lint` (157 s),
  `make typecheck` (36 s), `make test` (1748 s; 113 targets reported
  `test result: ok`, none failed), `make markdownlint` (22 s, including
  spelling), and `make nixie` (1 s). Logs:
  `/tmp/<gate>-ortho-config-7-3-1.out`. The preceding candidate `ea184c20`
  failed only the spelling stage, on one British verb form and one hyphenated
  compound; both corrections landed in `1fbf28cf`, and the whole suite re-ran
  green.
- [x] (2026-11-15) Assessment repair round on PR #589. The completeness
  assessment returned "not yet complete or correct against the approved plan",
  and each gap is now closed against executable evidence:
  - Three `rstest-bdd` scenarios load a combined tree with a nested requirement,
    an optional parent, and a scalar where an object is required; all eight
    scenarios in `missing_required_values.feature` pass.
  - `prop_tests.rs` gained insertion controls beside the existing omission one,
    so the comparison fails for both false-negative directions.
  - `an_indirect_cycle_is_detected_at_the_repeated_type` pins a mutually
    recursive schema, asserting the chain repeats exactly the entry type.
  - `tests/missing_required_evaluation.rs` instruments a custom `Deserialize`
    and a `#[ortho_config(default = ...)]` expression and measures `(0, 1)`,
    `(1, 1)`, and `(0, 0)` evaluations across the missing, successful, and
    compose-only paths, which is the plan's no-double-evaluation evidence.
  - Two trybuild fixtures with committed `.stderr` files pin the
    `RequiredConfigSchema` bound for a nested field and the rejection of
    `#[command(subcommand)]` combined with `#[ortho_config(nested)]`.
  - [ADR-0025](../adrs/adr-0025-enumerate-missing-values-for-selected-subcommands.md)
    records selected-subcommand enumeration; ADR-0024 gains an amendment
    banner and `## Amendments` section, and the design document, users' guide,
    developers' guide, migration guide, changelog, and contents index follow.
- [x] M1: establish exact schema and source contracts with executable evidence
  (ADR-0024, characterization evidence, and the recorded red reproducer).
- [x] M2: implement and property-test collection with typed diagnostics.
- [x] M3: integrate all supported generated loading paths and recovery tests.
- [x] M4: reconcile documentation, gates, and roadmap completion.
- [x] (2026-11-15) Second completeness assessment (round c2,
  `7-3-1-completeness-2026-11-15-c2`) found three correctness defects and three
  verification gaps against `95e8e314`. Repaired in this round:
  - `SchemaField::optional_subtree` demotes a whole flattened composition,
    including nested objects, so a partially supplied `Option<Child>` no longer
    invents a report Serde would never raise.
  - `is_optional` now consults the key the loader reads, so a materialized
    default counts only when the Rust identifier is that key or an alias.
  - `toml_key` renders every configuration-file segment as TOML accepts it,
    escaping quoted keys instead of emitting `api@key` unquoted.
  - New oracles: an exact object-plus-leaf body compared with a parsed TOML
    document, quoting and parsing for punctuation, quotes, and backslashes, a
    file-route recovery through a discovered configuration file, a partially
    supplied nested flatten, and a renamed default beside an unrelated
    requirement.
  - Two trybuild fixtures with generated `.stderr` files: an unsupported
    directional `rename_all` value and a tuple-struct `OrthoConfig`.
  - Recorded the renamed-default limitation in the design document and the
    migration guide, and corrected the compound spelling the hosted word list
    rejects in the module header of `missing_required_evaluation.rs`.
- [x] (2026-11-15) Third completeness assessment (round c3,
  `7-3-1-completeness-2026-11-15-c3`) found two correctness defects against
  `3f66cb81`. Repaired in this round:
  - A flattened field is demoted only when its type is `Option`. A
    `#[serde(default)]` on a flattened struct never rescues a missing child
    leaf, because Serde deserializes the flattened field from the unmatched
    keys and propagates the child's failure.
  - `SerdeRenameAll::apply` now reproduces Serde's own field rules instead of a
    general case conversion: `lowercase` and `snake_case` leave an identifier
    unchanged, and `PascalCase` only capitalizes each underscore-separated
    word.
  - New evidence: real-loader cases for an absent and a partially supplied
    defaulted flattening, a `lowercase` rule proved through a discovered
    configuration file, a quoted dotted key replayed through the same path, a
    serialize-direction `rename_all` trybuild fixture, the alias file route
    replayed beside the environment value that produced the report, and a unit
    table covering all eight rename rules on a mixed-case identifier.
- [x] (2026-11-15) Fourth completeness assessment (round c4,
  `7-3-1-completeness-2026-11-15-c4`) found four correctness defects against
  `dc8e7d2d`. Repaired in this round:
  - A field-level custom deserializer outranks the declared type. Serde's
    `expr_is_missing` checks the field default, then the container default, and
    only then `deserialize_with`; when a deserializer is installed the missing
    branch returns `missing_field` instead of the helper that yields `None` for
    an `Option`. `serde_field_has_custom_deserializer` recognizes both
    `deserialize_with` and `with`, and `is_optional` treats such a field as
    required unless a Serde default applies.
  - `#[ortho_config(nested)]` combined with a custom deserializer becomes an
    opaque leaf under its own key, because the child's `Deserialize` may never
    run; a flattened one contributes no requirement at all, having no key of
    its own. `composes_children` carries that decision for both the composed
    and the reported shape.
  - `is_expressible` rejects a segment containing `=` or a NUL. Every process
    environment separates name from value at `=`, and a NUL terminates the
    platform call's C string, so neither can name a variable whatever the
    provider's case and separator transforms do. `render_segment` now quotes
    and escapes any segment outside TOML's bare-key alphabet, so the file route
    that remains is a spelling the file can carry.
  - The defaulted flatten gained a two-leaf child: supplying one leaf still
    reports the other, which separates "the flatten is inspected" from "one
    supplied value silences the whole composition".
  - New evidence: a macro unit table over both deserializer spellings, a
    defaulted custom-deserialized `Option`, an opaque nested child, a flattened
    custom-deserialized field, a real-loader module
    (`tests/support/missing_required_deserializers.rs`) proving requiredness,
    the default escape, and the opaque-leaf recovery through `MCX_DATABASE`, a
    provider-expressibility unit table over six rejected spellings beside the
    positive control, a rendering case for quoting and escaping, and a real
    alias replica of a `=` key that keeps only its quoted file route and then
    recovers from that file. The same `=` scenario is replayed through the
    process environment: the child-process harness
    (`tests/missing_required_child.rs`) loads from the platform environment
    with a quoted table as its only input, asserts that no environment section
    is rendered, and then recovers `host` and `port` from the advertised table.
    ADR-0022 gains a third amendment; the design document and the
    improved-error design record the same rules.
- [x] (2026-11-15) Fifth completeness assessment (round c5,
  `7-3-1-completeness-2026-11-15-c5`) found one environment-route defect against
  `3dbd552b`. Repaired in this round:
  - `environment_variable` now validates the whole path, not each segment
    alone. The provider replaces every `__` in the *joined* name with a dot, so
    a segment edge merges with the separator: `database_` plus `host` gives
    `database___HOST`, which the provider reads as `database` plus `_host`.
    The advertised variable named a different key and would have repaired
    nothing. `provider_reads_the_same_path` replays that split and offers a
    route only when it reproduces the canonical path.
  - New evidence: a `required_values` unit table pairing the expressible and
    leading-underscore paths (both routable) with the trailing-underscore path
    (not routable), and the generated-loader case
    `a_trailing_underscore_parent_has_no_environment_route` in
    `tests/support/missing_required_keys.rs`. It reports `database_.host` with
    no `env:` route, keeps the `file:database_.host` route, recovers from a
    `[database_]` table, and uses a separator-free root as the positive control
    by recovering `MRT_DATABASE__HOST`. ADR-0022 gains a fourth amendment; the
    design document and the improved-error design record the whole-path rule.

- [x] (2026-11-15) Sixth completeness assessment (round c6,
  `7-3-1-completeness-2026-11-15-c6`) found one correctness defect and two
  coverage gaps against `55454b31`. Repaired in this round:
  - `lookup` selected the canonical key first and never noticed a second
    accepted spelling. Supplying `MRA_DATABASE__PORT` beside `MRA_DB__HOST`
    made the collector report `database.host` and advertise a route under
    `database`; following it left both spellings in the merged value, so the
    loader rejected the duplicate and the report had described an absence it
    could not repair. `presence` now classifies each node as absent, uniquely
    present, or conflicting, and `collect_level` skips a conflicting node
    without descending. The deserializer's own error is preserved, which for an
    incomplete first spelling is its missing-field error and for two complete
    spellings is the duplicate-field error. Spellings are deduplicated before
    counting, because a rename can leave the canonical key listed as its own
    alias.
  - New evidence: a `required_values` unit case over both orderings of the
    duplicate plus the canonical-only and alias-only controls, a deduplication
    control whose value is present once under a key that is also its alias, and
    the real-loader module `tests/support/missing_required_conflicts.rs`. It
    asserts the loader's missing-field error for the partial case, its
    duplicate-field error for two complete spellings, and that either spelling
    on its own still recovers the value. ADR-0022 gains a fifth amendment; the
    design document and the improved-error design record the same rule.
  - V1 real-loader gaps closed by `tests/support/missing_required_declared.rs`:
    a container `#[serde(default)]` supplying every field, an explicit `null`
    through a JSON5 file, an empty collection under `merge_strategy = "append"`
    beside the absent-collection control, a `#[serde(skip)]` field that is
    never inspected, and a `doc(required = false)` override that does not
    relax a real requirement. The container-default and null cases were
    previously covered only by macro and collector tests; the JSON5 case is
    feature-gated because no TOML file can spell null.
  - The round-c5 acceptance deviation is recorded under "Concrete steps and
    acceptance commands". Issue #695 owns the prerequisite repair and now
    carries the wider inventory in its evidence comment, so the follow-up issue
    earlier rounds planned is unnecessary. The `apply_field_attr` diagnostic is
    not this branch's: it appears identically on `main`'s `f086edc9`.

- [x] (2026-11-15) CodeScene cleanup of the branch-only code. The hosted check
  reported two rule violations and one duplication finding, all introduced by
  this branch:
  - *String heavy function arguments* on
    `required_schema/tests.rs` (57.1% of arguments were string literals). The
    cause was this round's own assertion helper, which took the forbidden token
    and a case label as strings. A `Demotion` enum now names the two ways a
    flatten could weaken a child's requirements, so the assertion takes no
    string at all. The two case tables moved to `tests_serde_keys.rs`
    behind `RenameCase` and `UnreadField`, each pairing a spelling with the
    key it must produce. That also splits "which keys does Serde read?" from
    "what does the generator emit?", and keeps both files under the size cap.
  - *Code duplication* between the required and defaulted flatten tests. They
    are one `#[rstest]` over a `FlattenCase` now; the argument list and the
    demotion it must not apply travel together, so no case can assert without
    its expectation.
  - *Code duplication* in `subcommand/sources.rs`. Four exported wrappers
    repeated the announce/build/finish/report sequence. `recorded_load` owns
    the sequence and `load_extracted` names the raw entry points' policy,
    leaving `finish_with_required_values` as the schema-aware counterpart.
    CodeScene still reports this file at 9.09 with the duplication marked
    *unchanged* (3.0 → 3.0): the residual similarity is between the two public
    `_with_sources_at` signatures, and the change both introduced and reduced
    blocks, so the finding predates and outlives the repair. It is dispositioned
    as unchanged-from-base rather than hidden behind further indirection.
  - Two helpers were strengthened while they were being extracted: the custom
    deserializer cases now assert that the advertised route recovers the leaf,
    and `recovered_reference` fails when the selected variant is not `serve`.

- [x] (2026-11-15) Rebased onto `main`'s `1f4a5775`, the landing of PR #698.
  That commit is the #695 prerequisite repair, so the reconciliation of the
  rebase also closes the acceptance deviation recorded below: the ordered gate
  sequence now reaches the end with `make lint` green rather than stopping at
  Whitaker. One conflict arose, in `apply_field_attr`
  (`ortho_config_macros/src/derive/parse/field_attrs.rs`), where #698 split the
  key match into setters while this branch had added `nested` and moved
  `skip_cli` onto `CliFieldMode`. The resolution keeps the setters and carries
  both branch decisions into them; `range-diff` shows every other commit
  unchanged, and `git diff --check` is clean. The rebase is the reason the
  plan's base/head pair changes, so every earlier assessment verdict is
  evidence for its own candidate only and round c7 re-covers the new pair.

- [x] (2026-11-15) Ordered gates on the rebased candidate `c0a7e991`.
  `make check-fmt`, `make typecheck`, `make lint-clippy` and the whole
  `make lint` passed, Whitaker and its 100% docstring coverage included. The
  suite then ran with `--no-fail-fast` across 112 test binaries: 2785 passed,
  46 ignored, one failed, and pytest passed. The one failure is `ortho_l10n`'s
  charged-wait boundary probe; "Surprises & discoveries" records why it is host
  load rather than this branch.

- [x] (2026-11-15) Round c7's reply found no confirmed defect and asked for a
  passing local suite, the six gates including `make nixie`, and a reassessment
  once the branch was mergeable again. `main` had advanced to `934c4c99`, so
  the series was replayed onto it. One conflict arose in
  `docs/v0-11-0-migration-guide.md`, where both sides had added table rows and
  numbered the sections that follow: the table carries the union and this
  branch's two sections are renumbered `## 11.` and `## 12.`. Weave produced an
  unparsable `config_merge.rs` during the same replay, so that file was rebuilt
  from `main`'s version plus this branch's `RequiredConfigSchema` bounds.
  `range-diff` against the previous candidate shows every commit unchanged
  except the conflict resolution, whose only extra content is table padding that
  `mdtablefix` requires. A comment-only correction to
  `ortho_config/tests/missing_required_values.rs` now records that the
  `declared` module does test a documentation override. On `8ca5dd81` all six
  gates then passed in the plan's order: `make check-fmt`, `make lint`
  (rustdoc, Clippy, Whitaker and 100% docstring coverage), `make typecheck`,
  `make test`, `make markdownlint` and `make nixie`. `make test`'s Cargo
  command ran with `--no-fail-fast` over 114 binaries: 2814 passed, 46 ignored,
  none failed, and its pytest command passed 128 with 5 skipped. The
  charged-wait boundary probe passed here, which makes the earlier failure
  intermittent rather than a deterministic property of this host.

## Surprises & discoveries

The full local suite aborted once at round c7 on
`identifier_artefact_lease_cargo_build_tests::a_real_cargo_build_keeps_the_lease_after_its_helper_dies`
with "timed out waiting for the second test process to write build_started".
The test waits 120 s for a probe `cargo build` to write a marker. Build
admission at the time reported `capacity 4, held 4, healthy` with roughly forty
queued requests from other repositories, so the probe could not reach a compile
slot inside its window. Re-running that test alone passed in 60.77 s, and the
full suite with `--no-fail-fast` then passed every binary (2786 tests, no
failures). The failing log is kept as
`/tmp/test-ortho-config-7-3-1-enumerating-missing-required-values.run1.out` and
the clean log beside it, so the abort is evidence about the host, not about
this branch.

The charged-wait boundary probe is intermittent on this host rather than
deterministically failing. `a_wait_for_a_sibling_is_charged_to_the_allowance`
holds the process-wide serialization lock, releases it, and expects the child's
bound to be the remainder of the shared allowance; at `c0a7e991` it reported
instead that the 900 second allowance was exhausted while the child waited for
a sibling nested Cargo invocation. That wait is nested Cargo, so it queues
behind shared compile admission, and a long-enough wait consumes the remainder.
The branch changes nothing under `ortho_l10n`
(`git diff 1f4a5775 HEAD -- ortho_l10n` is empty); the same test failed
identically at the unmodified base `1f4a5775` on this host, in 553.65 s with
the same message; and the same binary had passed in 302.31 s at `f086edc9`. On
`8ca5dd81` the probe passed inside a boundary binary that took 456.06 s, in a
run that logged 1717 admission waits with up to 47 queued requests, so queue
depth alone does not predict the outcome. Hosted `build-test` remains the
arbiter for this contract, and no local edit should paper over it.

Two `make lint` runs on `8ca5dd81` aborted in the Whitaker stage with
`sccache: Compile terminated by signal 11` and no rustc diagnostic, first on
`ortho_config_macros` and then on the `rstest_bdd` test target.
`build-limits-status` reported `capacity 4, held 4, healthy` on both occasions.
`make lint-whitaker` then passed on its own in 9m 10s, which identifies the
aborts as cache-side crashes rather than lint findings; the retry is the gate
result and the two aborts are recorded as infrastructure noise.

The `make lint` failure that began in round c5 is an estate-wide effect of a
shared lint installation, not a property of any branch. The suite checkout at
`~/.local/share/whitaker` is at `880939d1`, fourteen commits past the `v0.2.12`
tag, and its macro-aware cognitive complexity rule lets a function cross the
weighted threshold without any of its own lines changing. Hosted lint passed at
03:40 UTC and failed at 05:05 UTC on 2026-11-15 for the same commit `f086edc9`.
The local run reports 28 sites; the hosted job reports the eleven
`ortho_config_macros` ones and aborts there. Since the repository must not
modify the installed suite, the repair is a refactor tracked by #695.

The historical proposal names `ortho_config_macros/src/derive/load_impl.rs` and
a final Figment extraction. The current path is `derive/load_impl/source.rs` to
declarative merging, then generated `finish_tokens` in
`derive/generate/declarative/merge_tokens.rs`, followed by
`ortho_config/src/declarative/convert.rs::from_value_merge` on a JSON value.
Integrate at that final value boundary after collection overlays, rather than
adding a parallel Figment pipeline.

At the M1 audit, `OrthoError::MissingRequiredValues` did not exist. The
regression in `ortho_config/tests/clap_integration/error_cases.rs` then expected
`Merge` for an absent field; M3 updated it to the enumerated diagnostic.
Discovery's `missing_required_error` concerns missing files, which is a
different condition and retains its existing meaning.

`derive/generate/docs/fields/resolution.rs::resolve_required` accepts a
`doc.required` override and treats collections as non-required. It is useful
prior art, not an authoritative validation predicate. Documentation metadata is
recursive for subcommands, not for arbitrary nested data structures.

The requested `docs/rtest-bdd-users-guide.md` spelling refers to the existing
`docs/rstest-bdd-users-guide.md`; use the latter. The live Makefile's test gate
includes all targets/features and Python tests, beyond the older summary in
AGENTS.md. Ordinary config fields are wrapped in Option in the generated CLI,
allowing missing values to reach final merge; explicit clap constraints remain
separate.

PR #651 removes this plan's earlier premise that no prover tooling exists. Make
targets, pins, and a CI lane are present, while the production inventory is
deliberately empty, and `docs/execplans/import-prover-tools.md` names this plan
as a motivating consumer without reversing D4. That lane's workflow triggers on
tooling and fixture paths only. A production proof would not run on a change to
the code it reasons about until those paths are extended. No workspace manifest
declares `cfg(kani)`, so a production harness would trip the denied
`unexpected_cfgs` warning until one does; the fixture declares its own.

The rebase also corrected three baseline assumptions unrelated to provers.
`docs/scripting-standards.md` now exists. `OrthoError` has no consumer in
`ortho_config/src/localizer/` or `ortho_l10n/`, and ADR-0015 forbids the latter
from depending on configuration, so the classification sites to update are the
two `error_category` functions in `ortho_config/src/merge_telemetry.rs`
(exhaustive) and `ortho_config/src/discovery/telemetry.rs` (wildcard to its
"other" category). Behavioural scenarios bind through the `rstest_bdd` test
target in `tests/rstest_bdd_support/behaviour/scenarios.rs`, rather than a
per-feature runner.

The M1 audit found three facts that shape the implementation.

1. The generated defaults and CLI structs copy no Serde naming attributes, so
   their layers key values by the Rust identifier. A field with
   `#[serde(rename = "...")]` therefore has no working CLI route, because the
   merged layer key does not match its deserialization key. The diagnostic must
   omit that flag rather than invent one.
2. Runtime requiredness differs from the documentation IR. The IR treats every
   collection as non-required, but the loader emits an empty collection only
   through an explicit default or a merge overlay, so a `Vec<T>` without a
   default is required by Serde. Runtime decisions follow the loader.
3. A third exhaustive `error_category` match exists in
   `ortho_config/src/subcommand/paths_telemetry.rs`. The plan named two
   classification sites; the compiler locates the third when the variant is
   added.

The 2026-11-15 assessment repair round produced four findings worth keeping.

1. `rstest-bdd` step texts are registered globally, not per feature. The plan's
   `When the configuration is loaded` already belongs to
   `tests/rstest_bdd_support/behaviour/steps/env_steps.rs`, so each new
   scenario names its own load (`the combined configuration is loaded`, and so
   on). This is a deliberate departure from the plan's step wording, recorded
   in the new steps module's doc comment.
2. `scenarios!` binds fixture names, not types, and Cargo does not track the
   `.feature` file. Editing the feature without touching `scenarios.rs` leaves
   a stale test binary that silently keeps running the old scenarios.
3. A load that stops at a missing value evaluates a
   `#[ortho_config(default = ...)]` expression exactly once and never runs a
   custom deserializer, and compose-plus-collect alone touches neither. The
   evaluation test therefore asserts those counts directly rather than
   inferring them from the code.
4. The subcommand loaders cannot reuse the type's own `RouteHints`: their
   environment variable is `PREFIX_CMDS_<NAME>_<FIELD>` and their file root is
   `[cmds.<name>]`. Advertising the type's prefix and file root would name
   spellings the loader never reads, which is exactly the false-route failure
   the design forbids. ADR-0025 records the loader-scope decision and the
   widened `RequiredConfigSchema` bound it forces on the four
   `load_and_merge_subcommand_for*` helpers and the `SubcmdConfigMerge` methods.

The c2 repair produced two findings worth keeping. First, a flattened `Option`
absorbs *any* descendant failure, not just an entirely absent child, so the
demotion has to reach nested objects; demoting only immediate fields still
reported a leaf the loader never requires. Second,
`derive/parse/struct_attrs.rs::discard_unknown` parsed an unrecognized value as
a token stream, which runs to the end of the attribute. In
`#[serde(rename = "ref", alias = "reference")]` that swallowed the alias, so
the alias was invisible to both the schema and the key check. The value is now
parsed as one expression, which stops at the comma.

The c3 round added two more Serde details that decide correctness. A field
default is applied only to a field Serde never saw, so
`#[serde(flatten, default)]` is inert: the flattened struct is still
deserialized from the unmatched keys and a missing leaf propagates. And Serde's
`rename_all` rules for struct fields are not case conversions — `lowercase` and
`snake_case` leave the identifier alone, and `PascalCase` capitalizes each
underscore-separated word without lowering the rest. The earlier converter
matched Serde for snake_case identifiers and diverged for every other shape.

The c4 round closed the deserializer gap and, while doing so, found that a `=`
key is reachable only through an alias. The derive validates every *serialized*
file key path against the alphanumeric, `_`, and `-` alphabet, so
`#[serde(rename = "Host=Name")]` fails to compile as an `OrthoConfig`
implementation; `#[serde(alias = "Host=Name")]` is accepted, because the
validation never looks at aliases. The quoting and escaping in `render_segment`
is still required, since an alias and a schema-only child can both produce such
a segment, and the real-loader case therefore drives the alias route rather
than the rename one. The environment-route exclusion for `=` and NUL is
likewise a platform property, not a provider setting: no configuration could
make either spelling expressible.

## Decision log

D1 (2026-10-14, proposed): add structured missing-value data rather than the
historical string-only variant. Callers must inspect missing paths without
parsing localized prose. Update the improved-error design and record an ADR
when implementation begins; this plan approval includes that design direction.

D2 (2026-10-14, proposed): audit and reuse existing key/flag/environment
resolution helpers, but keep runtime requiredness separate from descriptive
metadata. Avoid coupling loading to `OrthoConfigDocs` or `Default` merely to
obtain diagnostics.

D3 (2026-10-14, proposed): introduce explicit recursive schema composition for
nested config types, with an opt-in field marker and a companion trait, rather
than guessing nested leaves from JSON or repeatedly deserializing failures. M1
is a feasibility gate for this proposal, not permission to silently reduce
7.3.1 to top-level fields. Unsupported custom deserialization must retain its
existing error behaviour and be documented as outside schema enumeration.

D4 (2026-10-14, revised after expert review): use property tests and negative
controls for ordered aggregation. A formal proof of a Boolean filter would
prove straightforward control flow while leaving the actual risks, schema
fidelity and source resolution, outside the proof. Retain the independent
specification below but do not introduce an artificial Verus milestone. If M1
reveals non-trivial contractual logic requiring an exhaustive guarantee, stop
and revise the proof sketch and implementation plan together for approval.

D5 (2026-11-06, proposed, after PR #651): retain D4. The import removes a cost
and leaves its reason intact, since a proof of an ordered Boolean filter still
says nothing about schema fidelity or source resolution. `proptest` satisfies
the repository's invariant rule, which permits it or Kani. No harness, proof,
or inventory entry is planned, and none may be added to populate the new lane.

Verdict (2026-11-09, at M1 and M2): no prover. The kernel as written is an
ordered filter over a finite schema tree plus ancestor-stack bookkeeping. The
sampled V1 matrix and the seeded omission, insertion, and ordering controls
exercise every branch, which satisfies the repository's invariant rule. No
harness, proof, or inventory entry exists, and none was added.

Re-evaluate at the M1 and M2 checkpoints against the kernel actually written,
recording the verdict and its evidence in this section. Propose Kani if the
kernel grows beyond ordered filtering, for example if activation and
blocked-ancestor classification become logic where every path within a stated
bound matters, and it can be isolated from Serde values, strings, and maps.
Propose Verus only if a stable pure kernel needs a guarantee without a bound.
Either verdict triggers the tolerance above and a dedicated, approved
milestone, following the conditions under "Proof sketch before implementation".

D6 (2026-11-15, accepted): extend enumeration to selected subcommands through a
loader-scoped `RouteScope` rather than by widening `RouteHints`, and record the
decision in
[ADR-0025](../adrs/adr-0025-enumerate-missing-values-for-selected-subcommands.md).
`RouteHints` is a statement about the type; the subcommand loaders read a
different prefix and file root, so the scope has to travel with the loader. The
consequence is accepted deliberately: `load_and_merge_subcommand_for*` and
`SubcmdConfigMerge` now require `RequiredConfigSchema`, which every type
deriving `OrthoConfig` already satisfies. The verdict above is unchanged — the
subcommand path adds a scope parameter, not new logical structure, so the
sampled matrix and the seeded controls still exercise every branch.

D7 (2026-11-15, accepted): demote an optional flattened composition as a whole
subtree rather than field by field, and classify a materialized default by the
key the loader reads. Both follow one rule: the report must describe what the
loader does. Serde converts any descendant failure inside a flattened `Option`
into absence, so a partially supplied child raises no missing-value error and
must not be reported. The generated defaults struct keys its values by the Rust
identifier, so a default only supplies a renamed field when that identifier is
also an accepted alias; otherwise the field stays required and the report says
so. The alternative — teaching the defaults struct to serialize under the
deserialized key — would change merge precedence for every renamed field, which
is a wider behavioural change than this plan covers.

D8 (2026-11-15, accepted, refining D7): treat only an `Option`-typed flatten as
tolerant of absence, and follow Serde's own field rename rules. A
`#[serde(default)]` on a flattened struct is inert: Serde applies a field
default to a field it never saw, and a flattened field is always deserialized
from the unmatched keys, so a missing child leaf propagates and must be
reported. Reading "carries a default" as tolerant of absence over-generalized
D7's first clause. Separately, the `rename_all` rules are Serde's field rules,
not case conversions: `lowercase` and `snake_case` leave a struct field
unchanged, and `PascalCase` only capitalizes each underscore-separated word
without lowering the rest. A general converter matched Serde for snake_case
identifiers and diverged for every other shape — the same class of defect,
where the schema names a key the loader never reads.

D9 (2026-11-15, accepted): let an installed deserializer outrank the declared
type, and treat a segment outside the variable-name alphabet as having no
environment route. Both follow D7's rule that the report describes the loader.
`Option<T>` is optional only because `de::missing_field` yields `None` for it,
and installing `deserialize_with` or `with` replaces that call with
`missing_field`, so the field is required unless a Serde default intercepts it
first. For the same reason the child's `Deserialize` may never run, so a
`nested` marker on such a field cannot promise that the composed keys are the
ones read: the field becomes an opaque leaf under its own key, which is the one
route the deserializer is guaranteed to receive, and a flattened equivalent
contributes nothing. On the route side, `=` and NUL are excluded by the
platform rather than by any provider setting, so no configuration could make
them expressible; the file route therefore carries the whole advertising duty,
and it quotes and escapes whatever a TOML bare key cannot spell. Rejecting the
combination outright was considered for the nested case and rejected: it would
turn existing user code into a compile error for a diagnostic limitation.

D10 (2026-11-15, accepted, refining D9): decide an environment route from the
whole path. D9 checked each segment against the provider's transforms, but the
provider does not transform segments: it joins them with `__` and then replaces
every `__` in the joined name with a dot. A segment edge therefore merges with
the separator, and `database_.host` was advertised as `APP_DATABASE___HOST` — a
variable the provider reads back as `database._host`, so supplying it repaired
nothing. The collector now replays the split on the joined name and offers a
route only when the replay reproduces the path exactly. A leading underscore
keeps its route, because the prefix strip exposes it rather than absorbing it,
so the rule rejects the overlap rather than any underscore. The alternative —
changing the separator or escaping underscores in the provider — would alter
the variable names of existing configurations, which is a wider behavioural
change than this plan covers.

D11 (2026-11-15, accepted, refining D7 and D10): leave a node spelled two
accepted ways to the deserializer. D7 promised that an accepted alias is never
reported as missing, which held when exactly one spelling was present but not
when a canonical key and an alias both were. M1's canonical-plus-alias row
makes the merged value ambiguous, and Serde resolves it by erroring: a
duplicate field when both values deserialize, and the first value's
missing-field error when it does not. Reporting beneath the node was wrong in
both directions — it invented a requirement the loader never reaches, and every
route it advertised would have left both spellings in place. The collector
therefore classifies presence before visiting a node and skips a conflicting
one. Rejecting only the nested case was considered and rejected: a duplicated
leaf is equally the deserializer's error, and a rule that depends on the node's
shape would be harder to reason about than one that depends on the merged value.

## Outcomes & retrospective

The Wyvern team established the current merge boundary. Pandalump/Wafflecat
review removed disproportionate proof tooling and separated child schemas from
CLI derivation; Telefono/Buzzy Bee review corrected parent presence, naming,
activation, finite schema construction, and duplicate evaluation;
Doggylump/Dinolump review clarified checkpoint gates and classification tests.
The trade-off favours real-provider evidence over proving a trivial filter. No
feature implementation or proof has been executed. Documentation validation and
publication evidence is supplied in the draft PR. All six review perspectives
confirmed their findings resolved. Preliminary `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie` passed. A final wording correction
requires a final documentation gate run before commit; the PR records that
result without implying runtime validation. Implementation completion requires
every acceptance item below; a plan PR cannot mark roadmap item 7.3.1 done.

The 2026-11-06 revision is a documentation change after a clean rebase. It adds
no proof and validates no runtime behaviour. Its design consequence is D5,
which replaces a tooling obstacle with an explicit content test for adopting a
prover.

Implementation outcome (2026-11-14): roadmap task 7.3.1 is delivered. The
derive emits `RequiredConfigSchema` observations, the pure collector in
`ortho_config/src/required_values/` compares them with the merged value, and
every generated loader reports through `OrthoError::MissingRequiredValues`
after composition. The V1–V5 evidence above stayed green through the rebase onto
`main` and the ADR renumbering to 0020, and the final candidate `1fbf28cf`
passed `make check-fmt`, `make lint`, `make typecheck`, `make test`,
`make markdownlint`, and `make nixie` sequentially. Per the D5 verdict, no
proof or prover inventory entry was added. What remains is delivery: publish
the rebased branch and carry PR #589 through review.

Assessment repair outcome (2026-11-15): the hosted completeness assessment
reported the plan not yet complete, naming gaps that were real rather than
stylistic — the combined-load behavioural scenarios were absent, the
no-double-evaluation claim had no measurement behind it, the macro bound and
the subcommand conflict had no compile-fail pin, and selected-subcommand
coverage was neither implemented nor recorded as a decision. Every one is now
closed with the evidence listed in Progress, and ADR-0025 records the loader
scope and the widened bound it forces. The round also retired an assumption the
earlier plan carried: that the subcommand helpers could report through the
type's own route hints. They cannot, and saying so in an ADR is the honest
outcome rather than leaving the exclusion in place. The D5 verdict stands: the
added scope parameter is not new logical structure, so the sampled matrix and
seeded controls still discharge the invariant rule.

## Context and orientation

`ortho_config` owns runtime loading, errors, source providers, and merge logic.
`ortho_config_macros` generates configuration loading and merge implementations.
`test_helpers` contains shared test infrastructure. `cargo-orthohelp` consumes
documentation metadata; it must not become a runtime dependency.

Read `docs/contents.md` and `docs/repository-layout.md` first. The central
integration points are `ortho_config_macros/src/derive/load_impl/source.rs`,
`ortho_config_macros/src/derive/generate/declarative/merge_tokens.rs`,
`ortho_config/src/declarative/convert.rs`, and
`ortho_config/src/error/types.rs`. Inspect error re-exports, localization, and
telemetry matches when adding a variant. Existing naming helpers are in
`ortho_config_macros/src/derive/parse/serde_attrs.rs`, `derive/build/cli/`, and
`derive/generate/docs/fields/`.

Kani is a bounded model checker: a passing harness says nothing beyond its
unwind bound. Verus proves logical obligations over a specified model, which
supports a runtime claim only through an explicit refinement relation to
production code. Both are contributor tools pinned in `tools/`, delegated to
`rust-prover-tools`, and never Cargo dependencies. Read `docs/design.md` §4.20
and "Kani and Verus verification tools" in `docs/developers-guide.md` before
changing any verification decision here.

### Temporary base and return to main

As requested on 2026-11-09, this branch and draft PR #589 temporarily target
`issue-644-nested-cargo-timeouts`, whose inherited head is
`26275a4efe31d3cf84cf8ec3e265782a28a832f3` in
[PR #664](https://github.com/leynos/ortho-config/pull/664). That parent
addresses the nested Cargo timeouts that prevented implementation against the
latest main. Its code remains parent-owned; this branch adds only the plan and
its documentation revisions. The two existing plan commits replayed unchanged,
with no conflicts, from their exclusive old base `ddba3578`.

After #664 merges, the implementing agent can fetch `origin/main`, verify the
parent's landing, and rebase this branch back onto main using the `rebase`
skill. Preserve the inherited parent boundary above: replay only this branch's
commits after it, rather than replaying the timeout fix after a squash merge.
If the parent branch changes first, reconcile its history before choosing a new
boundary. Preserve recovery refs and the expected remote head, audit the
range-diff, run gates appropriate to the resulting changes, and push with an
explicit force-with-lease. Retarget PR #589 to `main` and verify its final
diff. This base change does not approve feature implementation; M1 still
requires explicit plan approval.

Status (2026-11-14): PR #664 merged, landing as `eaf4c028` with follow-up
`9ede81a1`, and this branch returned to `main`. It was rebased onto `959dbd7e`,
the then-current `origin/main`, replaying only its own commits after the
exclusive boundary `26275a4e` and dropping the two pre-merge copies of the
timeout fix. The plan commit that recorded the temporary base remains as
history. Main had meanwhile taken ADR-0018 and ADR-0019, so this task's
decision record was renumbered from 0018 to 0020; its file name, title, and
every reference were updated during the same replay. The range-diff audit
showed only that renumbering and `docs/contents.md` context as differences from
the pre-rebase series. A later rebase onto `f086edc9` took the numbers to 0022
and 0023, because main took 0020 and 0021 for the artefact-lock records; a final
rebase onto `7b750601` took them to 0024 and 0025, because main took 0023 for
the structured-output record. See
[ADR-0024](../adrs/adr-0024-enumerate-missing-required-values.md).

## Conformance basis

The baseline is commit `ddba3578fae04227e426ea3a61aa2b24aa043232`, the landing
of PR #651, rebased from `dc979878b5728bbb23866df9c05c2570bdb5629e`. There is
no separate terms-of-reference document for this task. Governing requirements
are roadmap 7.3.1 and the user's planning and testing instructions. The
completed `docs/execplans/5-1-1-reconcile-design-with-actual-error-surface.md`
is audit evidence, not an implementation specification.

- R1: all missing requirements together, from
  `docs/improved-error-message-design.md` §§1–3. M1–M3 discharge through typed
  error tests, collection properties, and nested behavioural recovery.
- R2: truthful CLI/environment/file routes and precedence, from that design
  and `docs/design.md`. M1 and M3 discharge by replaying advertised routes.
- R3: an agent can self-correct from one diagnostic, from roadmap 7.3.1 and
  `docs/agent-native-cli-design.md` §6.3. M3 discharges by a second successful
  child-process invocation.
- R4: error and metadata ownership, from `docs/cli-localization-design.md`,
  `docs/cargo-orthohelp-design.md`, and
  `docs/clap-dispatch-and-ortho-config-integration.md`. M2–M4 preserve existing
  error classification outside this feature and avoid schema ownership drift.
- R5: quality and documentation requirements. M1–M4 use the testing references
  below and M4 updates user, developer, and architecture guidance.
- R6: honest formal-verification claims, from ADR-0017 and the production proof
  inventory in `docs/developers-guide.md`. M1 and M2 discharge it by recording
  a D5 verdict. M4 confirms that any claim names its tool version and bound,
  and that the inventory matches the verdict.

Read `docs/rust-testing-with-rstest-fixtures.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/complexity-antipatterns-and-refactoring-strategies.md`,
`docs/rstest-bdd-users-guide.md` before writing the corresponding tests,
helpers, or localization. No new script is needed for this implementation; if
that changes, follow `docs/scripting-standards.md`, which is now present.

Use the `codegraph-mcp` and `rust-router` skills for navigation and Rust work;
route error design to `rust-errors`, fixtures to `rust-unit-testing`, and
verification to `rust-verification`, `proptest`, `kani`, and `verus`. Use
`execplans` to maintain this file, `firecrawl-mcp` for external gaps, and
`logisphere-experts` to review material design changes. Follow `commit-message`
and `pr-creation` for delivery. Read each skill's SKILL.md before applying it.

## Behavioural contract and abstract specification

Define a requirement by its canonical sequence of configuration key segments,
its activation condition, and its supported supply routes. Segments distinguish
a literal key containing a dot from a nested path. Rendering joins or quotes
segments only at the presentation boundary. Schema order is declaration order,
with nested children visited in declaration order. Canonical paths are unique.
Distinguish canonical diagnostic identities, accepted deserialization
keys/aliases, and source serialization names. The existing Serde helpers
resolve serialization names and do not establish the complete deserialization
contract. M1 must account for deserialize-only and asymmetric renames and
aliases, including canonical-plus-alias duplicate input. Never label an
accepted alias as missing; preserve existing errors for unsupported name
combinations instead of inventing a mapping.

The final merged value includes defaults, file/profile layers, environment,
CLI, and collection overlays according to existing precedence. For each
activated requirement, classify its observation as absent, present, or blocked
by an invalid ancestor. Only absent observations belong in the missing set.
Explicit null is present and is left to Serde's type rules; an empty string or
collection is likewise present. A scalar where a nested object is expected is
invalid, not a collection of missing children.

Optional absent parents deactivate children. An absent required nested parent
activates its declared required leaves. If there are no such leaves, report the
parent itself with an object-valued supply route (including an empty
table/object): otherwise an absent required object containing only
optional/defaulted children would escape the report. A defaulted parent follows
the actual loader's default semantics. Selected subcommands activate only their
own requirements. A non-selected branch must never contribute missing fields.

The report is the ordered subsequence of activated requirements observed
absent. It contains no duplicates, no supplied values, and no unrelated
requirements. Every route in a reported item must be accepted by the actual
CLI/provider for that canonical path. Route precedence is CLI, environment,
configuration file; profile layering remains within the file contribution. Lack
of a route removes that suggestion, not the missing requirement.

Source/discovery/parse failures retain precedence because no trustworthy merged
value exists. For a valid merged object containing both absent requirements and
invalid present values, report the complete absence set first; after
correction, normal deserialization may report invalid values. One-round
recovery is promised when the sole defect is missing required values, not for
arbitrary invalid input. Help, version, unknown arguments, and clap-only
requirements keep clap semantics.

## Verification plan

### Proof sketch before implementation

Let D be a finite sequence of distinct canonical requirement identities, and
let A and P be equal-length Boolean sequences representing activation and
presence. The adapter admits only observations whose ancestors are valid;
blocked observations remain outside this kernel and preserve invalid-value
handling. Define the specification independently as the subsequence of D at
indices i where A[i] is true and P[i] is false.

The executable collector consumes these observations once in schema order.
Inspect the final value without replaying defaults or custom deserialization.
Collector cost is linear in observation count; path lookup also costs path
depth. Materialize route strings and report payloads only for missing entries
where practical. Use instrumented default/deserializer fixtures to detect
double evaluation, without adding benchmarks or a telemetry subsystem. Its
preconditions are equal sequence lengths and distinct identities. Its
postconditions are soundness (every output is activated and absent),
completeness (every activated absent identity occurs), uniqueness, and
preservation of input order. Representation relates a runtime path to exactly
one schema identity; identity assignment cannot depend on hash iteration or
rendered path strings.

Candidate semantic lemmas are prefix completeness after processing k entries,
order preservation when appending the next missing entry, and fixed-schema
repair: with activation and ancestor validity unchanged, changing an absent
requirement to present removes exactly that identity and cannot add another.
Supplying an optional parent can activate children; it is deliberately outside
that relation. Sketch induction on processed length with an empty-prefix base
and separate selected/unselected cases. These are hypotheses about proof
structure, not a requirement to retain redundant lemmas. Do not decide solver
triggers or auxiliary collection algebra before seeing verification conditions.

These candidate reasoning steps pressure-test the API; they are not introduced
formal theorem declarations. The collector is straightforward control flow, not
new contractual business logic warranting a prover. Discharge its executable
contracts through V1/V2 and deliberate omission, insertion, and ordering
faults. Use inhabited witnesses: empty input, no missing values, two missing
values separated by a supplied value, and an absent required object with no
required children. No unbounded proof is claimed by sampled tests.

A prover becomes warranted only if a later approved design introduces a stable,
non-trivial semantic kernel, as D5 defines it. Before that implementation,
revise this section with preconditions, postconditions, representation
relationships, and semantic lemma sketches. Add a dedicated milestone using the
`kani` or `verus` skill. PR #651 supplies the pins, installers, and runners, so
that milestone adds production entries rather than tooling. Each entry extends
`KANI_SMOKE_HARNESSES`, `KANI_ALL_HARNESSES`, or `VERUS_PROOF_FILES` in the
Makefile and names, in its pull request, the owning feature and crate, the
production source file, the harness or proof file and exact symbol, the
symbolic inputs and bounds including the Kani unwind depth, every assumption,
the expected check count, and the negative control that must be rejected. Its
acceptance adds the applicable commands below to `make check-fmt`, `make lint`,
`make typecheck`, `make test`, and `make test-workflow-contracts`:

```bash
make install-kani   # Once per CARGO_HOME: installs the pinned release.
make kani           # Runs the registered smoke harnesses after a version check.
make kani-full      # Runs every registered harness.
make install-verus  # Once: needs network access and may change rustup.
make verus          # Runs VERUS_PROOF_FILES; never installs anything.
```

A Kani harness must exercise the real pure kernel behind `cfg(kani)`, with the
configuration declared to the lint. Probe first that Kani's bundled compiler
can build the owning crate, including its dependencies. If it cannot, record a
deviation: extracting a kernel crate is a new boundary needing approval, and a
copied model would need the refinement relation below.

A Verus proof stays outside the Cargo workspace and runs under the toolchain
its release embeds, currently 1.93.0 rather than the repository's 1.99. It must
verify the production kernel or establish an explicit refinement relation, with
adapter tests covering the executable integration. It must be substantive and
non-vacuous, and use no `assume`, `admit`, external proof bodies, or equivalent
trusted shortcuts. Leave speculative theorem statements in documentation until
that approved milestone.

The same milestone must extend `.github/workflows/prover-tools.yml` so its path
triggers cover the harness or proof and its production dependencies, adding a
bounded Kani pull-request smoke lane, a full scheduled lane, and scheduled or
manual Verus execution. Promoting Verus to a required check needs measured
stability and a separate decision. Evidence must name the tool version and
bound, and record that the lane is validated on Linux x86_64 only.

Do not add passing proof stubs or placeholder inventory entries, and do not
treat `FORMAL-EMPTY`, tool absence, or fixture evidence as a successful proof
now. Formatting, localization, and integration plumbing remain conventionally
tested.

### Executable assurance and external assumptions

V1 tests requiredness against real generated loaders, using `rstest` tables for
Option, field/container defaults, inferred clap defaults, collection
strategies, renames, aliases, skipped fields, explicit null, empty values, and
nested parent activation. Documentation overrides must not affect runtime
decisions. Red failures must identify wrong membership, not merely an absent
API symbol.

V2 uses `proptest` for 0–64 distinct requirement paths, all presence/activation
combinations generated directly, and deterministic order and repair relations.
The expected set is computed independently. Include empty, all-present,
all-missing, interleaved, and nested cases without rejection-heavy generators.
Tests connect runtime observations to the abstract model; they are sampled
integration evidence, not an unbounded proof. Seeded omission, inversion, and
reordering faults must fail the appropriate assertion.

V3 uses macro expansion unit tests and trybuild fixtures to validate crate
renaming, generic fields, explicit nested schema bounds, compile diagnostics,
and supported Serde naming. Compile-fail tests are only for invalid attributes
or schema contracts; missing runtime values are not compile errors.

V4 uses `insta` snapshots for English display and the supported localized
presentation path if one is exposed. Keep typed assertions alongside snapshots.
Use `googletest` matchers for membership/classification and `pretty_assertions`
for exact ordered paths and multiline output. Reuse workspace versions; avoid
migrating unrelated tests. The upstream GoogleTest guidance supports placing
`#[gtest]` before `#[rstest]`; verify the pinned crate's integration with a
small compiling fixture.

V5 uses `rstest-bdd` and a child-process fixture for one-diagnostic recovery
through CLI, environment, file, and mixed sources. Reuse injected sources and
capability-based temporary directories. Child environment isolation must not
mutate the parent's process environment. Assert nonzero failure, diagnostic on
stderr, no success output or side effects, and zero exit on corrected input.
The fixture tests a consumer's exit policy; the library itself must not exit.

Third-party assumptions: Serde accepts its documented default/rename rules;
clap's argument definitions match accepted names and parse errors; existing
providers implement the documented source mappings. Exercise those boundaries
with real loaders, not mocks alone. Figment's `find_value` returns raw values
and can fail on combined-provider errors; it is not a safe Boolean absence
oracle and is not the proposed final-value implementation boundary.[^figment]

## Interfaces and dependencies

Propose `OrthoError::MissingRequiredValues(Box<MissingRequiredValues>)`, with
public inspectable payload types exported through the existing error facade.
The payload owns an ordered non-empty list of `MissingRequiredValue` entries,
canonical segment paths, and optional real supply routes. Keep actual values
out of it. Construct non-empty reports through a fallible/private constructor,
not unchecked caller assumptions. Display formats the payload; it does not
perform discovery or read process arguments/environment.

Propose a companion `RequiredConfigSchema` trait with a schema-construction
method and derive emission from `OrthoConfig`. A field marker
`#[ortho_config(nested)]` opts into composing a child's schema and adds a bound
only to that field's type. Offer a schema-only derive of `RequiredConfigSchema`
for child types, plus a documented manual implementation contract for custom
types; children must not need to derive a stand-alone CLI or implement
OrthoConfig merely to expose a schema. Avoid conflicting implementations when
OrthoConfig already emits the companion trait. M1 must demonstrate this wiring
and the real source path for nested values without imposing clap parsing bounds
on nested object types. No fabricated `--database-host` flag is permitted. This
marker composes diagnostic schemas only; it does not add nested provider
capabilities. Include a nested file-only requirement with no CLI suggestion,
generic children, and optional children in M1.

Schemas must be finite. Reject cyclic opt-in schema participation with a typed
schema error and a bounded traversal using a type-identity recursion stack; do
not eagerly expand optional boxed recursive children forever. Distinguish
revisiting a type on the current ancestry from reusing it in sibling fields.
Test direct and indirect cycles, finite repeated children, and document this
restriction. If this requires broader provider or type-system changes, stop
under the existing tolerance rather than silently pruning requirements.
Represent fields as leaf or nested schema nodes, including optional/default
activation. Flattened composition must be explicit and tested; do not infer it
from a dotted string. M1 fixes the precise signatures and attribute grammar in
the design document before M2 consumes them.

Do not require arbitrary Serde types to implement the companion trait. Such
opaque fields can be diagnosed as absent direct values, but absent leaves
inside a supplied opaque value remain Serde errors. Custom deserializers and
unsupported flatten contracts must not acquire fabricated recursive reports.
This restriction must be prominent in public documentation, with an example
showing how nested schema participation obtains full enumeration.

Put runtime schema and collector code in a focused
`ortho_config/src/required_values/` module; presentation belongs under
`ortho_config/src/error/`. Generated code needs a documented support surface
reachable from downstream macro expansion. Ownership and permitted call-sites
must be recorded in `docs/design.md`. Sweep for equivalent helpers before
adding each abstraction. Reuse naming functions where their semantics match;
never consume `doc.env` or `doc.file_key` overrides without provider evidence.

No production dependency is planned. `googletest`, `pretty_assertions`,
`insta`, and `proptest` are already `ortho_config` dev dependencies; authorized
additions apply only to another crate that lacks one. No verifier dependency is
introduced: Kani and Verus never appear in a `Cargo.toml`.

## Milestones and plateaus

### M1 — settle schema fidelity before integration

After approval, add focused characterization fixtures and the smallest red
regression for multiple absent fields. Audit default production, Serde naming,
CLI omission, and nested/flatten generation. Test the proposed schema approach
with required nested leaves and optional/defaulted parents. Run the aggregate
red reproducer as a temporary working-tree experiment, retain its failure log,
then remove it before the M1 checkpoint. Reintroduce the permanent regression
in the integration milestone and observe red again. Do not commit failing tests
or disable them to manufacture a green checkpoint.

Record a support matrix and exact API signatures in the improved-error design
and the next available ADR under `docs/adrs/`, using the four-digit filename
convention and linked from `docs/design.md` and `docs/contents.md`. This
milestone must show the nested schema is feasible without evaluating custom
defaults or deserializers twice, or it stops for an approved deviation. Do not
retrofit arbitrary Serde reflection. Resolve clap eager-required parsing
explicitly: configuration-backed fields must reach merging, while help/version
and genuine CLI-only constraints retain their existing paths.

Acceptance E1 is passing characterization coverage, a recorded expected red
failure for the aggregate contract, and a D5 verdict against the settled
schema. R1/R2/R4 are specified, not yet delivered. Run all ordinary gates
before the coherent schema/design checkpoint commit. Remaining gap: production
diagnostics and collection property coverage. Recovery is to revert the
checkpoint atomically; no migration layer is needed.

### M2 — typed report and property-tested collection

Implement runtime schema observations, the pure collector, the error payload,
and display. Write V1/V2 tests before their production logic, observe the
expected failure, then make the smallest implementation pass. Audit the
runtime-to-model mapping, especially canonical paths and ancestor validity.

Acceptance E2 comprises the schema/collector-only subset of V1 and V2 evidence
(the real-loader assertions become acceptance in M3), typed payload/display
snapshots, inhabited boundary witnesses, and rejection of deliberate omission,
insertion, and ordering faults, plus a D5 verdict against the written kernel.
Restore the correct code after each negative control. Run `make check-fmt`,
`make lint`, `make typecheck`, and `make test` sequentially. Remaining gap:
generated loader wiring and user recovery. Revert M2 as one coherent unit if
its API or representation cannot satisfy the contract.

### M3 — generated loaders and one-diagnostic recovery

Generate schema observations in the declarative finish path after collection
overlays and before final deserialization. Route ordinary loads,
injected-source loads, profile loads, and selected-subcommand merge entrypoints
through the shared mechanism wherever they have the approved schema. Preserve
raw generic `from_value_merge<T>` behaviour when only `DeserializeOwned` is
known; do not claim it has metadata it cannot possess. Add tests proving this
boundary.

Add error classification tests for both `error_category` functions, in
`ortho_config/src/merge_telemetry.rs` and
`ortho_config/src/discovery/telemetry.rs`. The second has a wildcard arm, so
the compiler will not point at it. Preserve existing source attribution and
metric labels for old variants, choose a stable low-cardinality category for
the new variant, and never emit supplied values or secrets. Any translated
rendering stays in `ortho_config`, because `ortho_l10n` must not depend on
configuration (ADR-0015). The initial display contract is English; existing
localization entrypoints must retain it deliberately when no dedicated
translation exists, without panics or rewriting the canonical typed paths.
Document that fallback rather than expanding phase 11 localization scope.

Use the audited parsing and key-generation helpers to produce real routes.
Account for renamed keys, custom long/short flags, prefixes, no CLI route,
subcommand invocation context, and literal dotted keys. Take a program label
from the actual command/loading context; deterministic direct-merge fallback
uses the configuration type name. Never read ambient `std::env::args` inside
pure diagnostics or injected loaders. Keep canonical data locale-independent;
use existing localizer interfaces for any supported translated rendering.

Add the following specification at
`ortho_config/tests/features/missing_required_values.feature`, bound by a
`scenarios!` entry in
`ortho_config/tests/rstest_bdd_support/behaviour/scenarios.rs` with steps under
`behaviour/steps/`. `scenarios!` is not tracked by Cargo, so touch that file
after editing the feature, or the binary will run stale scenarios:

```gherkin
Feature: Recover all missing configuration from one diagnostic
  Scenario: Correct a root value and two nested required values together
    Given a schema-enabled configuration with reference, database.host and database.port required
    And no source supplies those values
    When the configuration is loaded
    Then one missing-values error names all three paths exactly once
    And every suggested route is accepted by its provider
    When valid values are supplied using the reported routes
    Then the next load succeeds without another missing-values error

  Scenario: Optional and supplied values are not reported
    Given an absent optional parent and a supplied required value
    When the configuration is loaded
    Then neither value nor the optional parent's children are reported

  Scenario: Invalid nested input is not absence
    Given a scalar where a nested configuration object is required
    When the configuration is loaded
    Then the existing invalid-value error is retained
```

Use scenario outlines or parameterized runners for each supply route. Add
unit/macro/e2e coverage for selected versus unselected subcommands and preserve
existing invalid-input and help/version regressions. An assertion must prove
all required paths appear, not merely that the message contains “missing”.

The implemented feature file departs from this block in one respect. Step texts
are registered globally across the `rstest_bdd` target, and
`behaviour/steps/env_steps.rs` already owns `the configuration is loaded`, so
each scenario names its own load: `the combined configuration is loaded`,
`the configuration with an optional parent is loaded`, and
`the configuration with a scalar parent is loaded`. Do not "restore" the plan's
single wording; it collides.

Acceptance E3 is V3–V5 passing, all standard gates passing, and E2 property
tests rerun if its kernel or representation changes. Commit this integrated
plateau; no private compatibility adapters are necessary. Remaining gap: final
public guidance and roadmap closeout.

Amendment (2026-11-15): M3 also delivered selected-subcommand enumeration,
which this milestone originally left inside the ADR-0024 coverage exclusion. A
loader-scoped `RouteScope` now carries the `PREFIX_CMDS_<NAME>_` environment
prefix and the `[cmds.<name>]` file section, and
[ADR-0025](../adrs/adr-0025-enumerate-missing-values-for-selected-subcommands.md)
supersedes that exclusion. R1–R4 are delivered with no coverage exclusion.

### M4 — documentation and final conformance

Update `docs/users-guide.md` for typed matching, coverage limits, and a worked
one-round correction. Update `docs/developers-guide.md` for schema ownership,
source injection, tests, and the verification scope and limits. Reconcile
`docs/improved-error-message-design.md`, `docs/design.md`, and the phase-5
status paragraph in `docs/agent-native-cli-design.md`. Reference the ADR where
schema representation and proof obligations influence architecture. Unless D5
selected a prover, the guide's statement that no production inventory entries
exist stays true; do not reword it to imply this feature was verified. Update
the changelog using its skill; update documentation examples/registries if
their contracts change.

Only after E1–E3 and all gates pass, mark roadmap task 7.3.1 and its three
subtasks done (`[x]`), using the `mapsplice` skill for structural roadmap
edits. Run documentation gates and ordinary gates after this final milestone.
Record E4, reconcile every upstream discrepancy, and mark this plan COMPLETE
only when implementation is complete. The current planning PR leaves all
feature checkboxes unchecked.

At each milestone check assigned requirements, approved design, external
assumptions, public/dependency boundaries, unchanged persisted/wire schemas,
and current evidence links. Record deviations before proceeding. Changes to
these decisions require updating both the design/ADR and this plan.

## Concrete steps and acceptance commands

Run from the repository root on `7-3-1-enumerating-missing-required-values`.
Record `git status --short` and the head before edits. Use scrutineer for
sequential gates and `set -o pipefail` with `tee` to preserve failures. Example
final gate sequence:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-7-3-1.out
make lint 2>&1 | tee /tmp/lint-ortho-config-7-3-1.out
make typecheck 2>&1 | tee /tmp/typecheck-ortho-config-7-3-1.out
make test 2>&1 | tee /tmp/test-ortho-config-7-3-1.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-7-3-1.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-7-3-1.out
```

Run commands individually and stop at failure. No prover milestone is selected
for this design, so these gates exclude `make kani`, `make kani-full`, and
`make verus`. If an approved D5 revision adds one, run its proof gates in
addition to these commands. Run `make fmt` after documentation edits, inspect
its diff, and retain only relevant changes. For implementation, run relevant
baseline tests before edits; use a focused red command before production
changes, the same command after the fix, then full gates. Suggested new targets
are:

```bash
cargo test -p ortho_config --test missing_required_values
cargo test -p ortho_config --test rstest_bdd missing_required_values
cargo test -p ortho_config --test missing_required_values_e2e
cargo test -p ortho_config_macros
```

Create those test entrypoints in M2/M3; they do not exist at the baseline.
Include property tests in the first target. Record concrete test names, red
assertion failures, green counts, property/negative-control results, and log
paths in Progress. A zero-test run, skipped proof, or incomplete compiler
invocation is not a pass.

For this documentation-only plan publication, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie`. No runtime code changes justify a new
full Rust compilation during planning. Implementation milestones must run the
four ordinary code gates listed above, and the Markdown gates for documentation.

### Recorded acceptance deviation

The round-c5 gate run continued after a red `make lint` instead of stopping
there, so that the remaining gates could show whether the pre-existing failure
masked any defect of this branch. It did not, and every later gate passed.

This was a deviation, not the sequence above, and it is now closed. `make lint`
failed at its Whitaker stage on sites unchanged from `main`: the shared suite
checkout at `~/.local/share/whitaker` is at `880939d1`, which adds the
macro-aware cognitive complexity rule described in its own
`docs/v0-3-0-migration-guide.md`, and that rule lets a function cross the
weighted threshold without any of its own lines changing. Commit `f086edc9`
passed a hosted lint run at 03:40 UTC and failed one at 05:05 UTC on 2026-11-15
with no repository change between them, and the eleven diagnostics were
identical on `main` and on this branch's head.

Issue #695 owned the repair and carried the full inventory. PR #698 landed it as
`1f4a5775` on `main`, and this branch is rebased onto that commit, so a whole
ordered `make lint` now runs to completion: `lint-clippy`, `lint-whitaker` and
`docstring-coverage` all pass on the rebased head. The prerequisite is no
longer outstanding and no acceptance requirement changed.

The rebase needed one conflict resolution. `apply_field_attr` in
`ortho_config_macros/src/derive/parse/field_attrs.rs` was split by #698 into
one setter per recognized key, while this branch had added the `nested` key and
moved `skip_cli` onto `CliFieldMode`. The resolution keeps #698's setter
structure and carries both branch decisions into it, so `nested` is recorded and
`skip_cli` still selects `CliFieldMode::Skipped`.

## Idempotence and recovery

Use temporary capability-controlled fixture directories and injected sources;
rerunning tests must not alter the caller's files or environment. Revert failed
experiments and deliberate mutations before gates. Preserve meaningful red
failure evidence in logs, not failing committed tests. Commit each passing
logical change using the commit-message skill. Reverting a milestone must
revert its tests, API changes, and documentation together.

Push with the standard SSH Git client after verifying the GitHub identity.
Refresh the explicit `origin` tracking ref after URL-based pushes and confirm
the remote SHA. Draft PR #589, titled
`Plan: Enumerate missing required values (7.3.1)`, publishes this plan with a
branch-wise plan link and a terminal References section linking the Lody
session. Approval must be explicit before M1 starts.

## Artefacts and research notes

Wyvern evidence is available in context pack `pk_knocbild`. The durable code
paths and decisions are repeated above so execution does not depend on that
pack's lifetime. Firecrawl was used to inspect upstream Figment and GoogleTest
documentation on 2026-10-14.[^figment][^googletest]

[^figment]: [Figment 0.10.19 API](https://docs.rs/figment/0.10.19/figment/struct.Figment.html#method.find_value).

[^googletest]: [GoogleTest Rust interoperability](https://github.com/google/googletest-rust#interoperability).

Revision note (2026-10-14): initial draft replaces the obsolete Figment
preflight assumption with the live declarative merge boundary, and makes
recursive schema coverage an explicit approval decision. Expert review removed
an artificial filter-proof milestone and strengthened nested parent and repair
semantics.

Revision note (2026-11-06): rebased onto the landing of PR #651. Records that
pinned Kani and Verus runners now exist with an empty production inventory,
replaces the direct `prover-tools` command with the Make surface and inventory
obligations, and adds D5 and R6 while retaining D4. Also corrects the baseline,
the scripting-standards note, telemetry and localization sites, the BDD
binding, dev dependencies, and the gate list, which now includes
`make typecheck`. Remaining work is unchanged: approval, then M1–M4.

Revision note (2026-11-09): restack onto PR #664 and record its inherited
boundary and the return-to-main procedure after merge. Preserve the earlier
prover-tooling decisions and the implementation approval gate.
