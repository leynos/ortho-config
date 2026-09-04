# Add the `OrthoConfigLocalization` trait and derive emission (11.1.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

Today an application using `#[derive(OrthoConfig)]` that wants localized
command-line help must hand-author every Fluent identifier: it calls
`ortho_config::message_id_for(["hello_world", "cli"], "about")` (or worse,
concatenates strings) and hopes the result matches what the runtime localizer
looks up. Nothing checks the identifiers at compile time, and a typo or a
collision between two fields only surfaces as a runtime panic (see
`docs/adr-006-identifier-derivation-panics.md`).

After this change:

- Every `#[derive(OrthoConfig)]` struct implements a new public trait,
  `OrthoConfigLocalization`, carrying the command's Fluent identifiers as
  associated constants: the catalogue base (`LOCALIZATION_BASE`), the
  command-level identifiers (`ABOUT_ID`, `LONG_ABOUT_ID`, `USAGE_ID`,
  `VERSION_ID`, `LONG_VERSION_ID`, `AFTER_HELP_ID`, `AFTER_LONG_HELP_ID`), and
  a per-argument `ARG_IDS` table of named `ArgLocalizationIds` entries.
  Application code and tests refer to identifiers by constant, never by string
  concatenation, and pass `T::LOCALIZATION_BASE` to `with_base` so the derive
  is the single source of truth for the catalogue root.
- Two fields whose identifiers normalize to the same Fluent id fail to
  *compile*, with the error pointing at the offending field, instead of
  panicking at runtime. This discharges (for derived argument identifiers) the
  derive-time guard promised in ADR-006.
- The documentation intermediate representation (IR) emitted by the derive
  (`OrthoConfigDocs::get_doc_metadata`) reports root defaults equal to the
  localization trait constants and nested command/argument defaults equal to
  the runtime ids for their mounted command paths. The docs pipeline and
  runtime localizer therefore agree byte-for-byte for every field represented in
  `ARG_IDS` throughout a derive-generated tree. Docs-only, `skip_cli`, and
  flattened fields have no corresponding runtime argument id and retain their
  existing docs identifier defaults.
- When explicitly requested, the derive emits a build-time declaration and
  source-span inventory at `${OUT_DIR}/ortho-config/cli-identifiers.json`
  (split across capped files for large crates). Entries are explicitly marked
  as standalone per-struct identifiers; future `cargo-orthohelp` translator
  tooling combines those spans with the path-aware compiled docs IR rather than
  treating the JSON alone as a mounted-tree inventory (Decision D-3).

Observable success: the fixture and example crates compile with derived
identifier constants; `ortho_config/tests/localized_parse.rs` proves the
derived constants match `message_id_for` output over a real
`#[derive(OrthoConfig)]` tree; a trybuild compile-fail test shows the collision
diagnostic; and an end-to-end test drives `cargo build` on a fixture crate and
inspects the emitted JSON artefact. The nested docs fixture proves that root,
first-level, second-level, and renamed-subcommand ids match the runtime-mounted
paths.

This is roadmap item 11.1.3 (`docs/roadmap.md`, "Promote and widen the CLI
localization surface"). The governing design is
`docs/cli-localization-design.md` §8.1 and §8.2, with the identifier convention
in §4.1. This plan deviates from the design document in named, recorded ways
(Decision Log D-1, D-3, D-7, D-9); Milestone 6 amends the design document so
the two stay reconciled.

This plan was revised after a six-lens pre-implementation design review; the
review's findings are folded into the Decision Log and milestones below.

## Constraints

- Do not create a *build* dependency from `ortho_config_macros` on
  `ortho_config`. The macro crate is a `proc-macro` crate consumed by
  `ortho_config`; a reverse build dependency is circular and forbidden. A
  *dev*-dependency cycle (`ortho_config_macros` dev-depending on `ortho_config`
  for its own tests) is permitted by Cargo and by this plan (Decision D-8).
- The identifier convention is fixed by
  `docs/cli-localization-design.md` §4.1 and implemented by
  `ortho_config::message_id_for` (`ortho_config/src/localizer/identifier.rs`).
  Derive-generated identifiers must agree byte-for-byte with `message_id_for`
  output. The convention itself must not change.
- The runtime localization surface shipped by 11.1.1 and 11.1.2
  (`LocalizeCmd`, `LocalizedParse`, `parse_localized_command`,
  `message_id_for`) must remain source-compatible. Additive changes only.
- The existing `OrthoConfigDocs::get_doc_metadata` and
  `OrthoConfigSubcommandDocs::get_subcommand_doc_metadata` signatures remain
  intact. Milestone 4 may add provided, path-aware methods to those traits and
  override them in derive-generated implementations (Decision D-9); changing or
  removing the existing entry points is not authorized.
- The derive must not write to the filesystem during ambient builds. Artefact
  emission is opt-in (Decision D-3): it requires both `OUT_DIR` to be present
  and an explicit environment opt-in. Unconditional proc-macro filesystem
  writes break `docs.rs` (read-only sandbox), `rust-analyzer`, and the nightly
  derive-expansion cache, and are contrary to Cargo team guidance
  (rust-lang/cargo#9084).
- The workspace lint policy is strict (`clippy::unwrap_used`,
  `expect_used`, `indexing_slicing`, `panic_in_result_fn`, `missing_docs`, and
  friends are denied). New code must pass `make check-fmt`, `make typecheck`,
  `make lint`, and `make test` at every milestone. Note that denied
  `indexing_slicing` is itself a reason `ARG_IDS` entries are named structs,
  not positional tuples (Decision D-7).
- All prose follows `docs/documentation-style-guide.md` and en-GB-oxendict
  spelling; Markdown is gated by `make markdownlint` and `make nixie`.
- No single code file may exceed 400 lines.

## Tolerances (exception triggers)

- Scope: if the implementation (excluding tests, fixtures, snapshots, and
  documentation) requires changes to more than 25 files or more than ~2,500 net
  lines, stop and escalate.
- Interface: if any *existing* public API signature must change (as opposed
  to the additive, provided path-aware methods authorized by D-9), stop and
  escalate. The docs IR content change in Milestone 4 is pre-authorized by this
  plan: only the *values* of existing identifier-valued fields throughout a
  derive-generated command tree, plus the IR version string, may change. Any
  change to field semantics (for example `CliMetadata.value_name`, which is
  display text, not an identifier) is out of bounds.
- Dependencies: adding `serde`/`serde_json` as unconditional dependencies of
  `ortho_config_macros`, and `ortho_config` as a *dev*-dependency of
  `ortho_config_macros`, are pre-authorized. Any other new external dependency:
  stop and escalate.
- Iterations: if a gate still fails after three fix attempts on the same
  failure, stop, record the failure mode, and escalate.
- Ambiguity: if the docs-IR reconciliation (Milestone 4) turns out to require
  changes to `cargo-orthohelp` beyond regenerating golden snapshots, stop and
  present options before touching generator logic. Likewise if the trait-to-IR
  mapping table in Milestone 4 does not match the actual `DocMetadata` field
  semantics on inspection.
- CodeRabbit: if `coderabbit review --agent` stalls at `preparing_sandbox`
  for more than two attempts (a known failure mode recorded in the 11.1.1 and
  11.1.2 ExecPlans), record the attempt and continue; do not block the
  milestone on it.

## Risks

- Risk: divergence between the macro crate's identifier normalization and
  `ortho_config::message_id_for` (they cannot share build-time code; see
  Constraints). Severity: high. Likelihood: medium. Mitigation: (a) a
  cross-implementation property test *inside the macro crate* via the
  dev-dependency cycle, driving both functions with generated segment lists;
  (b) cross-crate agreement tests in `ortho_config/tests/` comparing derive
  output against `message_id_for` across fixture trees; (c) a marker-comment
  version gate — both implementations carry a
  `NORMALIZATION-RULES-VERSION: <n>` comment and a test fails if the numbers
  differ, so editing one file mechanically points at the other (Decision D-8).
- Risk: the docs IR identifier change breaks consumers of the IR beyond the
  workspace, and downstream Fluent Translation List (FTL) catalogues keyed on
  the old dotted ids stop resolving. Severity: medium. Likelihood: medium.
  Mitigation: bump `ORTHO_DOCS_IR_VERSION` to "2.0" (Decision D-2), update all
  pinned snapshots in one commit, and write a migration note in `CHANGELOG.md`
  and the users' guide mapping old id shapes to new.
- Risk: recursive docs generation loses, duplicates, or misorders a command
  path, yielding identifiers that are syntactically canonical but do not match
  the runtime walker's lookups. Severity: high. Likelihood: medium. Mitigation:
  make the accumulated command path an explicit parameter of the generated docs
  methods; append the clap-resolved variant label exactly once in
  `OrthoConfigSubcommandDocs`; and lock root, first-level, second-level, and
  renamed-variant paths against `message_id_for` and the recording localizer.
- Risk: handwritten implementations of the docs traits inherit the provided
  path-aware fallback and therefore retain their existing context-free metadata
  rather than gaining derive-only path rewriting. Severity: low. Likelihood:
  low. Mitigation: state that Milestone 4 guarantees path-aware identifiers for
  derive-generated trees, document how handwritten impls can override the new
  method, and add a compatibility test proving their existing methods continue
  to compile and return unchanged metadata.
- Risk: a future tool mistakes the build-time JSON's standalone per-struct ids
  for mounted command-tree ids. Severity: high. Likelihood: medium. Mitigation:
  include `path_scope: "standalone"` in every entry, describe the limitation in
  ADR-008 and the schema documentation, and require roadmap 11.5.2 to obtain
  mounted ids from the path-aware compiled docs IR before exposing translator
  output.
- Risk: artefact emission interacts badly with incremental compilation —
  the opt-in environment variable is invisible to Cargo's rebuild fingerprint,
  so setting it against a warm `target/` re-expands nothing and writes nothing;
  stale fragments from renamed or deleted types persist. Severity: high.
  Likelihood: high (this *is* the realistic usage sequence). Mitigation: the
  documented workflow forces recompilation and starts from a clean artefact
  directory; fragments are pruned against recorded source paths at merge; the
  end-to-end test covers the warm-cache case (Decision D-11, Milestone 5).
- Risk: `make lint` (Whitaker suite) may be red on `main` for reasons
  unrelated to this change. Severity: low. Likelihood: medium. Mitigation:
  establish the baseline in Milestone 0; only failures citing files in this
  branch's diff block progress.
- Risk: trybuild `.stderr` expectations are toolchain-sensitive, and the
  collision diagnostic is a multi-span rendering (error plus "first defined
  here" note) whose format rustc has reshaped before. Severity: low.
  Likelihood: medium. Mitigation: one struct per `.rs` case file with minimal
  surrounding code (smallest stable render surface); treat a
  `TRYBUILD=overwrite` regeneration commit as routine on toolchain bumps, not
  as a failure.

## Progress

> DECISION RESOLVED (2026-08-17): the user selected the path-aware redesign.
> Milestone 4 will pass the accumulated command path through derive-generated
> docs methods, append clap-resolved subcommand labels during recursion, and
> derive every non-overridden identifier from that mounted path. Reading A was
> rejected because root versus payload is a use-site property and a marker
> would drift; dropping field reconciliation would miss the milestone's core
> outcome. Reading B was rejected because a payload's standalone base can
> produce a canonical-looking id that differs from the runtime-mounted path.
> Decision D-9 and the Milestone 4 acceptance criteria now encode the approved
> redesign; implementation may resume.

- [x] Milestone 0: baseline gates and orientation.
  Baseline gates green on branch tip 66b3bf2: `make check-fmt`,
  `make typecheck`, `make lint` (rustdoc + clippy -D warnings + Whitaker), and
  `make test` (all workspace targets, Python suite 106 passed / 1 skipped). The
  Whitaker suite is green on this baseline (no outside-diff failures to
  quarantine). Verification findings for `usage`-per-node and flatten handling
  are recorded above.
- [x] Milestone 1: `OrthoConfigLocalization` trait in `ortho_config`.
  - `ortho_config/src/localizer/localization_ids.rs` defines
    `ArgLocalizationIds`
    and `OrthoConfigLocalization` (D-7 surface), re-exported at the crate root and
    from `localizer::`.
  - Unit tests assert the constants agree with `message_id_for` and round-trip
    through a `FluentLocalizer`; a doc example demonstrates
    `with_base(T::LOCALIZATION_BASE)`; trybuild `localization_public_paths.rs`
    locks the public crate-root paths.
- [x] Milestone 2: macro-side identifier generation and collision detection.
  - `generate/localization/` (identifier twin, model pass, collision detection)
    with the `NORMALIZATION-RULES-VERSION` twin gate, the dev-dependency cycle,
    `clap_field_is_flattened` detection (D-12) and `localization_base` parsing
    plus `localized_default` rejection (D-4).
  - Derive emission of `OrthoConfigLocalization` is wired in
    (`emit_localization_impl`),
    so the model is consumed by production code (Milestone 3 fold-in): this
    avoids a dead-code stage while making all existing derive consumers emit
    compiled constants (verified by the full workspace test suite).
- [x] Milestone 3: derive emission of `OrthoConfigLocalization` impls plus
  cross-crate agreement tests.
  - Derive emission is wired and all existing derive consumers compile
    (verified by the full workspace test suite).
  - Flat derived fixture: `FlatCli` in `ortho_config/tests/localized_parse.rs`,
    with `flat_command_constants_equal_message_id_for`,
    `flat_argument_constants_equal_message_id_for`, and
    `flat_walker_coverage_equals_derived_constants` green (the last proves a
    flat derived tree's constants equal the runtime walker's recorded set).
  - Subcommand+flatten fixture: derived `TreeCli` (replacing a handwritten
    impl) with `subcommand_flatten_constants_are_subset_with_documented_remainder`
    green, accounting for subcommand-node ids (D-9) and flattened-arg ids (D-12).
  - Collision trybuild: `ortho_config/tests/ui/localization_id_collision.rs`
    with the pinned message contract and `first defined here` note.
  - rstest-bdd Fluent-localizer scenario added (`localizer.feature` +
    `localizer_steps.rs`): the given step keys a `FluentLocalizer` catalogue on
    the derived constants (`LOCALIZATION_BASE`, `ABOUT_ID`, `ARG_IDS`), the
    when step builds `LocalizedDemoArgs::command().with_base(...).localize(...)`
    and runs `parse_localized_command`, and the then step asserts the about
    text resolves. Verified: `rstest_bdd` 57 passed 0 failed.
  - Full gate set green (fmt, typecheck, lint incl. Whitaker, test) before
    review; `coderabbit review --agent --committed` (Milestones 1-3 scope,
    compared against `main`) completed with 0 findings across 31 files.
    Note: the `scrutineer` sub-agent harness failed twice at "parse planner
    response" before tool execution (recorded infra failure per Tolerances);
    the review was run directly with the authenticated local `coderabbit`
    CLI instead.
- [x] Milestone 3a: migrate `examples/hello_world` to the derived constants.
  - `CommandLine` now derives `OrthoConfig` with
    `#[ortho_config(prefix = "HELLO_WORLD", localization_base = "hello_world.cli")]`;
    the flattened `globals` field is marked `#[ortho_config(skip_cli)]` (D-12)
    and the subcommand selector `#[serde(skip)]`; `Commands` gained
    `Deserialize`/`Serialize` so `CommandLine: DeserializeOwned` holds.
  - `localizer.rs` constants (`CLI_BASE_MESSAGE_ID`, `CLI_ABOUT_MESSAGE_ID`,
    `CLI_LONG_ABOUT_MESSAGE_ID`, `CLI_USAGE_MESSAGE_ID`) are now aliases of the
    derived `CommandLine::LOCALIZATION_BASE` / `ABOUT_ID` / `LONG_ABOUT_ID` /
    `USAGE_ID` — no hand-built strings remain.
  - Every `.with_base("hello_world.cli")` replaced with
    `.with_base(CommandLine::LOCALIZATION_BASE)` (main.rs, doc example,
    cli/tests/localisation.rs, tests/localised_help.rs). Insta snapshots stay
    byte-identical; hello_world 67 lib tests + integration + snapshot tests
    pass, clippy `-D warnings` clean. `CommandLine` derives `Parser` and
    `OrthoConfig` simultaneously without conflict (verified empirically).
  - CodeRabbit (`coderabbit review --agent --base-commit 40e5aa6`): 0
    findings across the 6 changed files.
- [ ] Milestone 4: path-aware docs IR delegation to the localization
  identifiers (redesign approved 2026-08-17).
- [ ] Milestone 5: opt-in build-time identifier artefact.
- [ ] Milestone 6: documentation, ADR-008, roadmap completion, final gates.

## Surprises & discoveries

- Observation: `OUT_DIR` is not set during proc-macro expansion unless the
  *consuming* crate has a `build.rs`, and the Cargo team explicitly rejects
  proc macros persisting data to disk (rust-lang/cargo#9084, closed wontfix;
  rust-lang/cargo#14035 likewise). Nightly now caches derive expansions, so a
  cached expansion never runs the macro body and any ambient side effect
  silently stops happening. Environment variables read by proc macros are not
  tracked by Cargo's rebuild fingerprint at all. Evidence: pre-planning
  research pass over the Cargo issue tracker and internals threads, 2026-08-06.
  Impact: the design document's §8.2 artefact bullet cannot be implemented as
  an unconditional write, and even the opt-in write needs a documented
  forced-recompile workflow; Decisions D-3 and D-11, recorded in ADR-008.
- Observation: the existing docs derive emits dotted identifiers
  (`{app}.about`, `{app}.fields.{field}.help`) under a `fields.` namespace,
  while `message_id_for` emits dash-joined identifiers under an `args.`
  namespace (`{app}-args-{field}-help`). Evidence:
  `ortho_config_macros/src/derive/generate/docs/sections.rs` and
  `docs/fields/defaults.rs` versus `ortho_config/src/localizer/identifier.rs`.
  Impact: Milestone 4 is a reconciliation, not a mechanical delegation; every
  pinned IR snapshot changes and `ORTHO_DOCS_IR_VERSION` must be bumped.
- Observation: the runtime walker (`LocalizeCmd::localize`) requests more
  command-level identifiers than design §8.1 defines constants for — `version`,
  `long_version`, `after_help`, and `after_long_help` in addition to `about`,
  `long_about`, and `usage` — and requests full id sets for every subcommand
  node. Evidence: `apply_command_metadata` in
  `ortho_config/src/localizer/clap_command/mod.rs` and the recorded coverage
  set in `ortho_config/tests/localized_parse.rs`. Impact: a trait limited to
  the §8.1 four constants could never satisfy a constants-equal-coverage test;
  Decision D-7 widens the trait and Milestone 3 states the coverage contract
  precisely.
- Observation: subcommand docs metadata is generated by the separate
  `SubcommandDocs` derive with no knowledge of the parent command path, so
  path-dependent canonical ids for subcommand arguments cannot be produced
  context-free. Evidence: `ortho_config_macros/src/derive/generate/docs/mod.rs`
  delegates to `<SubTy>::get_subcommand_doc_metadata()`. Impact: the original
  D-9 scoped subcommand IR ids out, but the accepted 2026-08-17 revision now
  adds path-aware generated methods and carries the parent path through this
  delegation.
- Observation (Milestone 0): `apply_command_metadata` requests every
  command-level suffix — `about`, `long_about`, `usage`, `version`,
  `long_version`, `after_help`, and `after_long_help` — for *every* node in the
  tree, not just the root. `localize_command` recurses through subcommands and
  calls `apply_command_metadata` at each node with that node's path. Evidence:
  `apply_command_metadata` and the recursion in `localize_command`
  (`ortho_config/src/localizer/clap_command/mod.rs`), and the recorded hit set
  in `parse_localized_command_uses_translated_metadata_on_success`
  (`ortho_config/tests/localized_parse.rs`) which lists both
  `custom-fixture-usage` and `custom-fixture-greet-usage`. Impact: confirms
  Decision D-7's widened trait; `USAGE_ID` (and the other command-level
  constants) must exist per node, so the flat-fixture equality test in
  Milestone 3 can assert full-set equality on a tree with subcommands only if
  the subcommand-node constants are accounted for. A flat struct (no
  subcommands) has exactly one node, so equality is direct there.
- Observation (Milestone 0): the existing derive has *no* handling of
  `#[command(flatten)]` / `#[clap(flatten)]` fields at all. A repository-wide
  search of `ortho_config_macros/src/` finds no reference to flatten;
  `build_cli_struct_fields` (`derive/build/cli/cli_flags.rs`) processes every
  non-subcommand, non-`skip_cli` field uniformly, so a flatten field would be
  emitted as a single `#[arg(long, short)]` over the flattened struct type
  rather than expanded into its constituent arguments. No workspace struct that
  derives `OrthoConfig` currently uses flatten (the only flatten uses —
  `CommandLine` in `examples/hello_world` and `FlatArgs` in the rstest-bdd
  fixtures — derive `Parser`/`Args` only, not `OrthoConfig`). Impact: for D-12
  the derive must add explicit flatten *detection* (mirroring
  `clap_field_is_subcommand`) and exclude those fields from `ARG_IDS`; there is
  no existing flatten semantics to preserve, and no current consumer regresses.

- Observation (Milestone 3): a derived fixture can combine a subcommand and a
  flattened struct by marking the flattened field `#[command(flatten)]` plus
  `#[ortho_config(skip_cli)]`. The derive's CLI builder skips the field via
  `skip_cli` (it has no flatten awareness in `cli_flags.rs`), so the flattened
  type itself must supply the clap `Args` semantics it already does, and the
  runtime walker (built from the clap `Parser` tree) still surfaces the
  flattened arguments under the parent's `args.` namespace. Verified
  empirically with a throwaway crate: a derived `ProbeCli` emitted
  `ARG_IDS = [config]` (flatten `extra` excluded by `clap_field_is_flattened`)
  while the walker recorded `probe-args-extra-help`/`long_help`/`value_name`
  plus the full subcommand-node set. This confirms the D-12 subset contract is
  achievable *with a genuine derive* on a tree that has both a subcommand and a
  flattened group, so the Milestone 3 fixture does not need the handwritten
  impl fallback.
- Observation (Milestone 4): canonicalizing every deriving struct from its own
  `localization_base` does not canonicalize a mounted command tree. For
  example, a standalone payload base can yield `audit-args-dry_run-help`, while
  the runtime walker mounted below the root requests
  `nested-app-admin-audit-args-dry_run-help`. The existing
  `OrthoConfigSubcommandDocs` implementation obtains already-materialized
  payload metadata and then overwrites only `app_name` and `about_id`; at that
  point it cannot distinguish defaults from explicit field overrides. Impact:
  the command path must be threaded into metadata construction before field ids
  are materialized. This resolved the Ambiguity tolerance through revised
  Decision D-9 rather than by changing pinned assertions alone.

## Decision log

Decisions D-1 through D-6 were taken while drafting; D-7 onwards resolve
findings from the pre-implementation design review (panel lenses: structure,
contracts, alternatives, scaling, failure modes, viability).

- Decision D-1: the "blanket `OrthoConfigDocs` impl" named in the roadmap is
  realized as *generated delegation*, not a literal Rust blanket impl.
  Rationale: a blanket `impl<T: OrthoConfigLocalization> OrthoConfigDocs for T`
  is impossible — it would conflict with the derive-emitted `OrthoConfigDocs`
  impls under coherence rules, and `get_doc_metadata` requires per-field data
  that associated constants cannot supply (`OrthoConfigDocs` has no constants
  today; it is a single-method trait). Instead, the derive's docs generator
  emits delegation that uses the same identifier rules as the localization
  model. For standalone metadata it starts from `LOCALIZATION_BASE`; for
  mounted subcommands it uses the accumulated path supplied by D-9. The roadmap
  intent ("the docs IR picks up the same identifiers") is met without a
  conflicting blanket implementation. Milestone 6 amends the stale blanket-impl
  sentence in design §8.1 as well as §8.2. Date/Author: 2026-08-06, planning
  session; revised 2026-08-17 after the Milestone 4 ambiguity decision.
- Decision D-2: command identifiers and localization-eligible field identifiers
  throughout a derive-generated command tree change from the dotted
  `{app}.fields.{field}.help` form to the canonical `message_id_for` form
  (`{mounted-path}-args-{field}-help`), and `ORTHO_DOCS_IR_VERSION` bumps from
  "1.1" to "2.0". Explicit `help_id`/ `long_help_id`/`about_id`/`synopsis_id`
  attribute overrides keep working unchanged. Rationale: two identifier
  conventions for the same strings is precisely the defect this roadmap item
  exists to remove. The bump is to "2.0", not "1.2": existing id values change
  meaning for consumers keyed on the old shapes, which is a major-flavoured
  change and the version string should say so. Decision D-9 defines how nested
  metadata obtains the mounted command path, and the mapping is pinned by the
  Milestone 4 table. Fields excluded from `ARG_IDS` by `skip_cli` or flatten
  handling have no runtime localization id to delegate to and retain their
  existing docs defaults. Date/Author: 2026-08-06, planning session; revised
  2026-08-17 after the Milestone 4 ambiguity decision.
- Decision D-3: the `${OUT_DIR}/ortho-config/cli-identifiers.json` artefact
  is emitted only when both (a) `OUT_DIR` is present in the environment at
  expansion time and (b) `ORTHO_CONFIG_EMIT_IDENTIFIERS` is set to exactly `1`.
  Any other value (including `0`, empty, or unset) disables emission. Ambient
  `cargo build`/`cargo check`/rust-analyzer runs never write. A proc-macro
  expansion knows a struct's standalone base but not every parent path where
  that type may be mounted. Each JSON entry therefore carries
  `path_scope: "standalone"`; it is a declaration and source-span inventory,
  not an authoritative mounted-tree inventory. The authoritative consumption
  path for future `cargo-orthohelp` translator tooling (§11 of the design) is
  the path-aware compiled docs IR through the existing bridge, joined to JSON
  entries for source spans. The artefact schema is *provisional until its first
  consumer lands (roadmap 11.5.x)*; that consumer may add the stable join key
  or relationship data its implementation proves necessary. The
  `schema_version` envelope (D-11) makes this revision explicit. Rationale:
  prior art (sqlx's `cargo sqlx prepare` gating, uniffi's extract-from-binary
  model) and Cargo team guidance both reject ambient proc-macro writes; an
  env-gated write neutralizes the docs.rs, rust-analyzer, and expansion-cache
  failure modes while still honouring the design document's artefact contract.
  Recorded as ADR-008 in Milestone 6, together with the guidance that the
  variable is set per-invocation and never exported in shell profiles or
  CI-wide environment blocks. Date/Author: 2026-08-06, planning session;
  revised 2026-08-17 after D-9 made mounted docs identifiers path-aware.
- Decision D-4: `localized_default` embedding (design §8.2, second bullet)
  is out of scope. The roadmap checklist for 11.1.3 does not include it; the
  artefact schema reserves an `embedded_default` field that this task always
  emits as `null`. The attribute is *recognized and rejected* with a deliberate
  spanned error ("`localized_default` is not yet implemented; see
  cli-localization-design.md §8.2") rather than falling through the derive's
  silent unknown-key path, so readers of the published design get an honest
  diagnostic. Milestone 6 marks the design bullet as deferred. Date/Author:
  2026-08-06, planning session; revised same day after review.
- Decision D-5: the derive gains an optional struct-level attribute
  `#[ortho_config(localization_base = "hello_world.cli")]` naming the
  identifier root as a dotted path. When absent, the base defaults to the same
  application-name resolution the docs derive already uses (the `app_name` doc
  attribute when present, otherwise the kebab-cased struct name). The resolved
  base is exposed as `OrthoConfigLocalization::LOCALIZATION_BASE`, and the
  sanctioned runtime pattern is `with_base(T::LOCALIZATION_BASE)`, making
  derive-versus- runtime base drift unrepresentable for applications that
  follow it. Rationale: 11.1.1 shipped `LocalizeCmd::with_base` for
  multi-segment catalogue roots; the runtime default base is the clap command
  name while the derive default is the docs app name, so silent disagreement is
  otherwise possible and would degrade every lookup to en-US fallback with no
  error — the review pre-mortem's highest-likelihood consumer incident. The
  users' guide documents the drift hazard explicitly (Milestone 6).
  Date/Author: 2026-08-06, planning session; revised same day after review.
- Decision D-6: no Kani or Verus harnesses. The interesting invariants
  (normalization agreement between two implementations; split-file round-trip)
  range over unbounded strings and are exercised with `proptest` property tests
  instead. There is no introduced lemma or contractual business logic that a
  bounded model check or proof would cover more rigorously than the property
  tests plus the cross-crate agreement suite. Date/Author: 2026-08-06, planning
  session.
- Decision D-7: the trait is wider than design §8.1's sketch, and `ARG_IDS`
  entries are a named struct rather than anonymous triples. Added constants:
  `LOCALIZATION_BASE` (D-5), `VERSION_ID`, `LONG_VERSION_ID`, `AFTER_HELP_ID`,
  `AFTER_LONG_HELP_ID` — every command-level suffix the runtime walker actually
  requests. `ARG_IDS` becomes `&'static [ArgLocalizationIds]` where
  `ArgLocalizationIds` carries `name` (the argument id), `help_id`,
  `long_help_id`, and `value_name_id`. Rationale: §4.1's grammar and the walker
  already cover `version` and friends; shipping a knowingly incomplete surface
  invites the hand-concatenation this task exists to remove. Positional tuples
  are unreadable, break silently on field reorder, and cannot even be indexed
  in this workspace (denied `clippy::indexing_slicing`); the named struct
  enables lookup-by-name and costs nothing before the trait's first release.
  Milestone 6 amends §8.1 to match. Date/Author: 2026-08-06, post-review
  revision.
- Decision D-8: the macro-side normalizer is a test-locked twin of
  `normalize_segment`, not shared source. Alternatives rejected: a shared
  zero-dependency `ortho_config_identifiers` crate (a third published crates.io
  artefact in perpetuity, publish-order choreography, and a semver surface for
  a ~20-line function §4.1 declares frozen), and `include!`-sharing of a source
  file across crate roots (breaks crates.io packaging of independently
  published crates). Locks: the dev-dependency-cycle property test inside
  `ortho_config_macros` (Cargo permits dev-dep cycles) calling
  `ortho_config::message_id_for` directly against the internal normalizer; the
  fixture-mediated agreement tests in `ortho_config/tests/`; and a
  `NORMALIZATION-RULES-VERSION: <n>` marker comment in both files with a test
  that fails when the numbers differ. The earlier draft's idea of a
  `#[doc(hidden)] pub` re-export from the macro crate is impossible —
  `proc-macro = true` crates may only export macros — and is withdrawn.
  Date/Author: 2026-08-06, post-review revision.
- Decision D-9 (revised): Milestone 4 makes derive-generated docs metadata
  path-aware instead of excluding subcommands. Add provided methods
  `OrthoConfigDocs::get_doc_metadata_for_path(command_path: &[String])` and
  `OrthoConfigSubcommandDocs::get_subcommand_doc_metadata_for_path(
  parent_path: &[String])`,
  without changing the existing method signatures. Generated
  `get_doc_metadata()` starts from `LOCALIZATION_BASE` and delegates to the
  path-aware method. Generated subcommand code appends the clap-resolved
  variant label exactly once, then asks the payload for metadata at that child
  path; nested payloads repeat the same operation. The existing methods remain
  the source-compatible entry points, and their provided path-aware fallbacks
  preserve handwritten implementations. Explicit id overrides are emitted
  literally and are never rebased; defaults use the accumulated mounted path.
  This design was chosen because root versus subcommand payload is a use-site
  property: a marker cannot model type reuse, and uniform per-struct
  canonicalization produces ids based on a standalone base rather than the
  runtime-mounted path. Derive-time collision detection still covers argument
  identifiers within one deriving struct; sibling-subcommand-name collisions
  remain a runtime panic per ADR-006. Date/Author: 2026-08-06, post-review
  revision; replaced by explicit user decision on 2026-08-17 after the
  Ambiguity tolerance was reached.
- Decision D-10 (revised): context-free localization constants and explicit
  docs overrides remain identifier literals produced by the Milestone 2 pass.
  Mounted default ids cannot be literals because the parent path is a use-site
  input; generated path-aware docs methods therefore call
  `ortho_config::message_id_for` with the accumulated path and the fixed
  suffix. This avoids const slice indexing, preserves overrides byte-for-byte,
  and makes the runtime function the final authority for mounted ids. Agreement
  is locked by the path-aware nested tests and the existing cross-crate
  property tests. Date/Author: 2026-08-06, post-review revision; revised
  2026-08-17 after D-9 changed.
- Decision D-11: artefact robustness contract. Fragment files are named
  `<TypeIdent>-<hash>.json` where `<hash>` hashes the expansion-site source
  path (two same-named types in different modules must not collide; a proc
  macro cannot see the module path, so the `type` field in entries is
  `CARGO_CRATE_NAME` plus the type ident). All writes (fragments, merged files,
  index) are write-to-temp-then-rename, atomic on one filesystem. The merged
  `cli-identifiers.json` and the index carry a top-level `schema_version` field
  so consumers can distinguish truncation, staleness, and schema drift. At
  merge time, fragments whose recorded source file no longer exists are pruned.
  The artefact is documented as authoritative only for the workflow: `rm -rf`
  the `${OUT_DIR}/ortho-config` directory is unnecessary for consumers because
  the documented invocation is
  `cargo clean -p <crate> && ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build`,
  which recreates `OUT_DIR` content from scratch and defeats both the
  untracked-env-var problem and the expansion cache. Residual staleness for
  renames within one file is accepted and documented in ADR-008. Date/Author:
  2026-08-06, post-review revision.
- Decision D-12: fields introduced through `#[clap(flatten)]` /
  `#[command(flatten)]` are excluded from `ARG_IDS` in this task, and the
  exclusion is documented in the trait rustdoc and the users' guide, with the
  flattened type expected to carry its own `OrthoConfigLocalization` impl. The
  Milestone 3 coverage contract accounts for this (subset semantics with a
  documented remainder). Rationale: flattened arguments surface at runtime
  under the parent command, but the parent derive cannot enumerate another
  type's fields; erroring on flatten would regress existing derive users. A
  flatten support follow-up is recorded when ticking the roadmap. Before
  implementing, Milestone 2 verifies how the existing derive treats flattened
  fields and records the finding here. Date/Author: 2026-08-06, post-review
  revision.

## Outcomes & retrospective

(To be completed as milestones land.)

## Context and orientation

The workspace (`Cargo.toml`, version 0.8.0, edition 2024) contains:

- `ortho_config/` — the main library crate. The localization runtime lives
  under `ortho_config/src/localizer/`: the `Localizer` trait and Fluent
  implementations (`mod.rs`, `fluent.rs`), the identifier convention
  (`identifier.rs`: `message_id_for` public, `normalize_segment`
  crate-private), command-tree localization (`clap_command/mod.rs`:
  `LocalizeCmd`, `WithBase`, `default_base_for`, per-parent collision
  `assert!`s), and localized parsing (`clap_command/parse.rs`: `LocalizedParse`,
  `parse_localized_command`). The docs IR lives under `ortho_config/src/docs/`
  (`OrthoConfigDocs` with a single `get_doc_metadata()` method, `DocMetadata`
  and friends in `ir.rs`, `ORTHO_DOCS_IR_VERSION = "1.1"`).
- `ortho_config_macros/` — the proc-macro crate implementing
  `#[derive(OrthoConfig)]` (`src/lib.rs`, `derive_ortho_config`). Parsing lives
  under `src/derive/parse/` (`StructAttrs`, `FieldAttrs`, `clap_attrs.rs` with
  `clap_arg_id` reading `#[arg(id = "…")]`); generation under
  `src/derive/generate/` (docs emission in `generate/docs/`, with the spanned
  duplicate-id helper `ensure_unique` in `generate/docs/fields/validation.rs`).
  Kebab-casing uses `heck` (`derive/build/cli/cli_flags.rs`). Errors are
  reported as `syn::Result` converted via `to_compile_error()` — never `panic!`.
- `cargo-orthohelp/` — the docs/agent-context CLI; consumes the IR through
  a generated bridge shim (`src/bridge.rs`).
- `examples/hello_world/` — the canonical localized example (catalogue base
  `hello_world.cli`, set via `with_base`); insta snapshots under
  `examples/hello_world/tests/snapshots/`.
- `tests/fixtures/orthohelp_fixture/` — a workspace-member fixture crate
  with its own `locales/` tree.
- `test_helpers/` — shared test utilities (`ortho_config_test_helpers`).

Terms used below:

- "Fluent" is the Mozilla localization system; an FTL (Fluent Translation
  List) file maps identifiers to translated strings. A Fluent identifier matches
  `[a-zA-Z][a-zA-Z0-9_-]*`.
- "Identifier convention" (design §4.1): author-facing FTL keys are dotted
  (`hello_world.cli.about`); the runtime id joins normalized segments with `-`
  (`hello_world-cli-about`). Argument ids insert an `args` segment:
  `hello_world-cli-args-recipient-help`.
- "Docs IR": the JSON-serializable `DocMetadata` structure the derive emits
  for documentation generators.

Prior work this task builds on: 11.1.1 promoted `LocalizeCmd` and
`message_id_for`; 11.1.2 promoted `LocalizedParse` and added an
identifier-coverage test (`ortho_config/tests/localized_parse.rs`, the
`RecordingLocalizer` and `identifier_coverage_matches_message_id_for`) that
this task extends. ADR-006 records that runtime identifier derivation panics on
invalid or colliding segments *until this task* adds the compile-time guard for
derived argument identifiers.

Relevant skills for the implementer: `leta` (code navigation), `rust-router`
(then `rust-types-and-apis` for the trait surface and `rust-unit-testing` for
the test work), `execplans` (this document's maintenance), `commit-message`,
`comenq-coderabbit` (review loop), `arch-decision-records` (ADR-008),
`proptest`, and `en-gb-oxendict` (prose). Relevant repository documentation:
`docs/design.md`, `docs/cli-localization-design.md`,
`docs/localizable-rust-libraries-with-fluent.md`,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
`docs/rust-doctest-dry-guide.md`,
`docs/reliable-testing-in-rust-via-dependency-injection.md`, and
`docs/complexity-antipatterns-and-refactoring-strategies.md`.

## Conformance basis

The upstream requirement is `docs/roadmap.md` item 11.1.3: derive localization
constants, make docs IR use the same identifiers, emit an identifier artefact,
and reject collisions at compile time. The governing design is
`docs/cli-localization-design.md` §4.1 (path-based identifier grammar), §8.1
(trait ownership), and §8.2 (derive behaviour). ADR-006 governs which invalid
or colliding identifiers fail at derive time and which hand-built command-tree
cases may still panic at runtime.

Traceability for the revised work is:

- roadmap 11.1.3 docs-delegation requirement → Decisions D-1, D-2, D-9, and
  D-10 → Milestone 4 → `nested_docs_ir.rs` path assertions and the docs IR
  snapshot sweep;
- design §4.1 mounted-path grammar → Decision D-9 → Milestone 4 → agreement
  with `message_id_for` and the runtime recording localizer;
- explicit-override compatibility → Decisions D-2 and D-9 → Milestone 4 →
  root and nested override regression tests;
- IR consumer migration → Decision D-2 → Milestones 4 and 6 → IR version
  "2.0", reviewed snapshots, changelog, and users' guide migration note.

The approved deviation from the current design text is generated, path-aware
delegation rather than the impossible blanket implementation. The design, ADR,
and roadmap are reconciled in Milestone 6 before this plan can be marked
complete.

## Verification plan

Milestone 4 introduces one state variable: the accumulated command path used
while recursively assembling docs metadata. Its obligations are:

- **V-M4-1 — mounted-path agreement.** Every default `about_id` and
  `synopsis_id`, plus each `help_id` and `long_help_id` for a field represented
  in `ARG_IDS`, equals `message_id_for` over the root localization base
  followed by each clap-resolved subcommand label. Docs-only, `skip_cli`, and
  flattened fields retain their pre-2.0 docs defaults because no corresponding
  runtime argument id exists. A focused integration test in
  `ortho_config/tests/nested_docs_ir.rs` covers root, first-level,
  second-level, and `#[command(name = "…")]` paths. The test must first fail on
  the existing dotted or standalone-base values, then pass after the generated
  path methods are wired. A negative assertion rejects
  `audit-args-dry_run-help`, proving that the test would catch Reading B rather
  than merely checking valid syntax.
- **V-M4-2 — append exactly once.** Each subcommand label occurs once, in
  declaration order, and sibling or nested traversal does not leak path state.
  Parameterized assertions cover two siblings and a two-level branch; expected
  ids are calculated independently with `message_id_for`. The witnesses are the
  existing `greet`, `admin audit`, and renamed `grant-access` fixture branches.
- **V-M4-3 — override preservation.** Explicit `about_id`, `synopsis_id`,
  `help_id`, and `long_help_id` values remain byte-for-byte unchanged at root
  and nested levels. Add one nested fixture override and assert both the
  override and a neighbouring generated default, preventing a vacuous test that
  exercises only one resolution branch. Assert a `skip_cli` or flattened field
  keeps its prior dotted default, proving that absence from `ARG_IDS` does not
  fabricate a runtime localization id.
- **V-M4-4 — compatibility entry points.** Existing handwritten
  `OrthoConfigDocs` and `OrthoConfigSubcommandDocs` implementations compile
  without defining the new provided methods and retain their current return
  values. A trybuild pass case or ordinary integration fixture discharges this
  source-compatibility obligation.
- **V-M4-5 — consumer coherence.** The complete snapshot and golden-file sweep
  changes only the IR version and identifier-valued defaults. A diff containing
  display text, field semantics, ordering, or an explicit override is a
  failure, even if tests can be regenerated to accept it.

The non-trivial external axiom is that `clap_variant_name` returns the same
effective variant label that clap mounts in the runtime command tree. Existing
renamed-variant tests exercise that boundary against real clap derives. The
implementation calls the already property-tested `message_id_for`; it does not
attempt to re-prove that function's normalization internally. No Kani or Verus
harness is warranted: the new risk is deterministic path threading through a
finite generated tree, covered directly by integration tests, while the
unbounded normalization invariant remains under the existing proptest suite.

The remaining plan-wide obligations are unchanged: the macro and runtime
normalizers agree for generated valid segment lists (Milestone 2 proptest and
marker-version gate); normalized field ids are unique within a deriving struct
(trybuild collision test with a known colliding witness); and artefact output
is opt-in, deterministic, atomic, round-trippable across split files, capped at
1 MiB except for one indivisible oversized entry, and explicitly standalone in
scope (Milestone 5 unit, property, snapshot, and end-to-end tests). The
artefact's `path_scope` assertion is the negative control against treating a
context-free proc-macro fragment as a mounted-tree inventory.

## Plan of work

### Milestone 0: baseline and orientation

No code changes. Run the four gates (`make check-fmt`, `make typecheck`,
`make lint`, `make test`) via the `scrutineer` subagent to record the baseline,
especially whether the Whitaker lint gate is red on files outside this task's
diff (a known possibility). Verify, and record in `Surprises & discoveries`,
two facts the review flagged: whether `USAGE_ID` (`usage` suffix) is looked up
for every node or only the root by `apply_command_metadata`, and how the
existing derive treats `#[clap(flatten)]` fields (D-12). Record the results in
`Progress`.

### Milestone 1: the `OrthoConfigLocalization` trait

New file `ortho_config/src/localizer/localization_ids.rs` (module registered in
`ortho_config/src/localizer/mod.rs`, re-exported from `ortho_config/src/lib.rs`
alongside the existing localizer exports):

```rust
/// Fluent identifiers for one argument of a derived command-line surface.
pub struct ArgLocalizationIds {
    /// The argument's clap id (explicit `#[arg(id = "…")]` or the
    /// kebab-cased field name).
    pub name: &'static str,
    /// Identifier for the argument's `help` text.
    pub help_id: &'static str,
    /// Identifier for `long_help`.
    pub long_help_id: &'static str,
    /// Identifier for the value name placeholder.
    pub value_name_id: &'static str,
}

/// Compile-time Fluent identifiers for a derived command-line surface.
pub trait OrthoConfigLocalization {
    /// The catalogue base as a dotted path, for `with_base`.
    const LOCALIZATION_BASE: &'static str;
    /// Identifier for the command's `about` text.
    const ABOUT_ID: &'static str;
    /// Identifier for `long_about`.
    const LONG_ABOUT_ID: &'static str;
    /// Identifier for the override usage string.
    const USAGE_ID: &'static str;
    /// Identifier for `version`.
    const VERSION_ID: &'static str;
    /// Identifier for `long_version`.
    const LONG_VERSION_ID: &'static str;
    /// Identifier for `after_help`.
    const AFTER_HELP_ID: &'static str;
    /// Identifier for `after_long_help`.
    const AFTER_LONG_HELP_ID: &'static str;
    /// Identifier records for every own argument, in declaration order.
    /// Flattened fields are excluded (see the trait documentation).
    const ARG_IDS: &'static [ArgLocalizationIds];
}
```

(Exact rustdoc wording at implementation time; the widened surface and the
named entry struct are Decision D-7; `LOCALIZATION_BASE` is Decision D-5.)

Red: a unit test module (rstest, `googletest` assertions) with a handwritten
impl asserting the constants round-trip through a `FluentLocalizer` lookup,
plus a doc example demonstrating `with_base(T::LOCALIZATION_BASE)`. The test
fails to compile until the trait exists; then make it pass. A trybuild "pass"
case under `ortho_config/tests/trybuild/` locks the public paths
`ortho_config::OrthoConfigLocalization` and `ortho_config::ArgLocalizationIds`.

Validation: `make test`, then the full gate set. Commit.

### Milestone 2: macro-side identifier generation and collision detection

New module `ortho_config_macros/src/derive/generate/localization/` with:

- `identifier.rs`: a strict segment normalizer mirroring
  `ortho_config::localizer::identifier::normalize_segment` (lowercase ASCII
  alphanumerics pass through, `-` and `_` pass through, anything else is an
  error; empty segments are errors; the *joined* identifier must start with an
  ASCII letter). Unlike the runtime twin it returns `syn::Result<String>` with
  errors spanned to the offending field or attribute
  (`syn::Error::new_spanned`), never panicking. Both files carry the
  `NORMALIZATION-RULES-VERSION: <n>` marker comment and module-level
  documentation naming the twin and the locking tests (Decision D-8).
- `mod.rs`: the identifier-generation pass. Inputs: the resolved base
  segments (Decision D-5) and the field list. For each own, non-subcommand,
  non-`skip_cli`, non-flattened field (D-12), the argument id is the field's
  clap `id` override (`clap_arg_id` in `derive/parse/clap_attrs.rs`) or the
  kebab-cased field name (same rule as `cli_flags.rs`). Outputs a struct-shaped
  model
  (`LocalizationIds { base, command: CommandIds, args: Vec<ArgIdsModel> }`)
  used by Milestones 3–5.
- Collision detection: normalized argument ids are checked for uniqueness
  with the `ensure_unique` pattern from `generate/docs/fields/validation.rs`.
  The message contract is pinned: the error names the colliding *normalized id*
  and both field names, and carries the remediation hint "rename the field or
  set `#[arg(id = \"…\")]`"; the note at the earlier field reads "first defined
  here". The trybuild `.stderr` review checks against this contract, not taste.
  Scope: argument ids within the deriving struct; Decision D-9 records that
  sibling-subcommand-name collisions remain runtime checks.
- Attribute parsing: extend `StructAttrs` and `parse_struct_attrs`
  (`derive/parse/mod.rs`) with `localization_base: Option<String>`, validated
  at parse time (each dotted segment must normalize cleanly; errors are spanned
  to the attribute). Additionally recognize `localized_default` and reject it
  with the deliberate deferral message (Decision D-4).

Red: rstest unit tests in the macro crate (inline test modules, following
`ortho_config_macros/src/derive/parse/tests/`) for the normalizer, base
resolution, argument-id selection, flatten exclusion, `localized_default`
rejection, and collision detection — written first so they fail, then
implemented. Property tests via the dev-dependency cycle (add `ortho_config` as
a dev-dependency of `ortho_config_macros`, Decision D-8): for generated valid
segment lists, the macro normalizer's joined output equals
`ortho_config::message_id_for`, and normalization is idempotent and yields a
valid Fluent identifier (`[a-zA-Z][a-zA-Z0-9_-]*`).

Validation: full gate set. Commit.

### Milestone 3: derive emission and cross-crate agreement

Wire the Milestone 2 model into `derive_ortho_config`
(`ortho_config_macros/src/lib.rs`, alongside the existing `generate_docs_impl`
call): emit `impl #krate::OrthoConfigLocalization for #ident` with all constant
values computed at expansion time as string literals, including
`LOCALIZATION_BASE` and `ARG_IDS` as `&[ArgLocalizationIds { … }, …]`.

Red first, in this order:

1. A trybuild compile-fail case
   `ortho_config/tests/ui/localization_id_collision.rs` (one struct, two fields
   whose ids normalize identically, for example `foo_bar: String` and
   `#[arg(id = "foo-bar")] other: String`; minimal surrounding code) with a
   `.stderr` expectation matching the pinned message contract. This fails the
   harness (compiles cleanly) until emission and collision wiring land.
2. Extend `ortho_config/tests/localized_parse.rs` with two fixtures:
   - A *flat* `#[derive(OrthoConfig)]` fixture (no subcommands, no
     flatten): assert with `googletest`/`pretty_assertions` that every
     constant equals the corresponding `message_id_for(...)` call, and
     that the `RecordingLocalizer` coverage set after a
     `parse_localized_command` run *equals* the set of derived constants
     (the walker's command-level suffixes are all trait constants after
     D-7, so equality is achievable on a flat tree — Milestone 0's
     `usage` verification feeds the exact expectation).
   - A fixture *with a subcommand and a flattened struct*: assert derived
     constants are a *subset* of the recorded coverage, and that every
     recorded id not among the constants is accounted for by a documented
     remainder list (subcommand-node ids per D-9, flattened-type ids per
     D-12). This is the cross-crate agreement lock required by design
     §8.2, upgraded from 11.1.2's clap-derive fixture as that plan's
     Decision D-4 reserved.
3. An rstest-bdd scenario in
   `ortho_config/tests/features/localizer.feature` plus steps in
   `tests/rstest_bdd/behaviour/steps/localizer_steps.rs`: "Given a derived
   configuration struct with a localization catalogue, When the command line is
   parsed with a Fluent localizer, Then the help text resolves through the
   derive-generated identifiers" — the end-to-end behavioural contract.

Validation: full gate set; `coderabbit review --agent` for Milestones 1–3 as
one review unit (gates must be green first). Commit per sub-step.

### Milestone 3a: migrate `examples/hello_world`

Replace hand-built identifier references in the example with the derived
constants and `with_base(CommandLine::LOCALIZATION_BASE)` (adding
`#[ortho_config(localization_base = "hello_world.cli")]`), keeping the existing
insta snapshots green — identifier *values* do not change, so any snapshot diff
is a defect. Validation: full gate set. Commit.

### Milestone 4: path-aware docs IR delegation

Per Decisions D-1, D-2, D-9, and D-10, make docs metadata construction carry
the mounted command path before identifier defaults are materialized. This is
one atomic architecture change: do not first canonicalize every struct from its
standalone base and then attempt to repair nested metadata afterwards, because
post-processing cannot distinguish generated defaults from explicit overrides.

Add provided path-aware methods without changing the existing signatures:

```rust
pub trait OrthoConfigDocs {
    fn get_doc_metadata() -> DocMetadata;

    fn get_doc_metadata_for_path(_command_path: &[String]) -> DocMetadata {
        Self::get_doc_metadata()
    }
}

pub trait OrthoConfigSubcommandDocs {
    fn get_subcommand_doc_metadata() -> Vec<DocMetadata>;

    fn get_subcommand_doc_metadata_for_path(
        _parent_path: &[String],
    ) -> Vec<DocMetadata> {
        Self::get_subcommand_doc_metadata()
    }
}
```

Exact rustdoc wording may improve during implementation, but the signatures and
fallback semantics are the approved interface. The defaults preserve
handwritten implementations. The derives override both path-aware methods:

1. Generated `OrthoConfigDocs::get_doc_metadata()` splits
   `OrthoConfigLocalization::LOCALIZATION_BASE` into owned path segments and
   delegates to `get_doc_metadata_for_path`.
2. Generated `get_doc_metadata_for_path` constructs the current node's
   metadata. Default identifiers call `message_id_for(command_path, suffix)`;
   explicit attributes remain literal. Its subcommand field calls
   `get_subcommand_doc_metadata_for_path(command_path)`.
3. Generated `OrthoConfigSubcommandDocs::get_subcommand_doc_metadata()` calls
   its path-aware counterpart with an empty parent path, giving direct enum
   callers a deterministic standalone tree rooted at each variant label.
4. Generated `get_subcommand_doc_metadata_for_path` clones the parent path,
   appends the clap-resolved variant label exactly once, and calls the payload
   type's `get_doc_metadata_for_path` with that child path. It then sets the
   returned metadata's display `app_name` to the variant label without
   rewriting any identifier. Recursive payloads repeat the same operation.

The pinned mapping applies at every node in a derive-generated tree:

- default `DocMetadata.about_id` ← `message_id_for(path, "about")`;
- default `DocMetadata.synopsis_id` ←
  `Some(message_id_for(path, "usage"))`. Inspection confirmed this is a
  catalogue identifier: `cargo-orthohelp/src/ir.rs` resolves it through the
  localizer;
- default `FieldMetadata.help_id` / `long_help_id` ←
  `message_id_for(path, "args.<clap-id>.help")` and
  `message_id_for(path, "args.<clap-id>.long_help")` for fields represented in
  the localization model's `ARG_IDS`;
- docs-only, `skip_cli`, and flattened fields that are absent from `ARG_IDS`
  retain their existing dotted docs defaults;
- explicit `help_id`, `long_help_id`, `about_id`, `synopsis_id`, and heading
  ids are untouched;
- `CliMetadata.value_name` is display text, not an identifier, and remains
  untouched;
- no IR fields are added.

Bump `ORTHO_DOCS_IR_VERSION` to "2.0" in `ortho_config/src/docs/mod.rs` (D-2).

Red-Green-Refactor sequence:

1. Red: change `ortho_config/tests/nested_docs_ir.rs` to require the full
   mounted ids for `greet`, `admin audit`, and renamed `admin grant-access`,
   plus a negative assertion rejecting the standalone `audit-args-dry_run-help`
   shape. Add nested explicit overrides and assert they remain literal. Run the
   focused test and record the expected dotted-id mismatch.
2. Green: add the provided trait methods, thread path arguments through
   `generate/docs/`, and update `subcommand_docs.rs` to append the resolved
   label. Pass the existing localization model into docs generation so field
   suffixes use the same clap ids and eligibility rules as `ARG_IDS`, with
   existing defaults retained for excluded fields. Run the focused test until
   it passes.
3. Refactor: isolate path construction and identifier token generation only if
   repository sweeps confirm no equivalent helper exists; document any new
   helper's ownership and reuse policy in `docs/developers-guide.md` as
   required by the repository abstraction policy. Re-run the focused test.
4. Add the handwritten-implementation compatibility test from V-M4-4, then
   update `ortho_config/tests/docs_ir.rs` and the IR-version assertion.
5. Sweep `docs_ir_subcommands.rs`, `subcommand_docs.rs`, the
   `cargo-orthohelp` golden files (`cargo-orthohelp/tests/golden/` and
   `tests/snapshots/`), agent-context snapshots
   (`ortho_config/src/agent_context/snapshots/`), and `hello_world` snapshots.
   Regenerate snapshots deliberately and inspect every diff. Only the version
   and non-overridden identifier values may change; display text, ordering,
   field semantics, or explicit overrides changing is a defect.

Acceptance evidence: V-M4-1 through V-M4-5 are discharged; a nested IR tree's
generated command ids and localization-eligible field ids equal independently
computed `message_id_for` values for their mounted paths; the recording
localizer requests the same ids; excluded-field defaults and explicit overrides
remain unchanged; IR version is "2.0"; and handwritten implementations retain
source compatibility. Run the full gate set and `coderabbit review --agent`,
perform the Tolerances and Conformance basis checks, then commit.

### Milestone 5: opt-in build-time identifier artefact

New module `ortho_config_macros/src/derive/generate/localization/artefact.rs`
(plus a sibling split if the 400-line cap demands):

- A pure, filesystem-free core (dependency-injection style, per
  `docs/reliable-testing-in-rust-via-dependency-injection.md`): given the
  Milestone 2 model plus span data, produce an in-memory artefact set — either
  `[("cli-identifiers.json", contents)]` or, when the merged JSON would exceed
  1 MiB (1,048,576 bytes), a split set `cli-identifiers.<n>.json` plus
  `cli-identifiers.index.json` naming the parts. Both the merged file and the
  index carry a top-level `schema_version` field (starting at `1`; the schema
  is provisional until its first consumer, D-3). Entry schema per identifier:
  `id`, `kind` (`about`/`long_about`/`usage`/`version`/`long_version`/
  `after_help`/`after_long_help`/`help`/`long_help`/`value_name`), `type`
  (`CARGO_CRATE_NAME` plus the deriving type's ident — the module path is not
  visible to a proc macro, D-11), `field`, `path_scope` (always `"standalone"`,
  D-3), `source` (file, line, column — from `proc_macro2::Span`), and
  `embedded_default` (always `null`; D-4). Serialization uses `serde_json`
  (promoted to an unconditional macro-crate dependency). Do not claim that
  these per-struct entries enumerate mounted subcommand paths; roadmap 11.5.2
  joins them to the compiled docs IR.
- A thin filesystem shell: runs only when `OUT_DIR` is set and
  `ORTHO_CONFIG_EMIT_IDENTIFIERS` equals exactly `1` (D-3). Each derive
  expansion writes a fragment
  `${OUT_DIR}/ortho-config/cli-identifiers.d/<TypeIdent>-<hash>.json` (hash of
  the expansion-site source path, D-11), then re-merges all fragments into the
  capped top-level file set, pruning fragments whose recorded source file no
  longer exists. Every write is temp-file-then-rename (atomic). I/O failures
  are reported as spanned derive errors carrying remediation text ("unset
  `ORTHO_CONFIG_EMIT_IDENTIFIERS` or fix permissions on `<path>`") — only
  possible when emission was explicitly requested, never in an ambient build.

Red tests, in order:

1. rstest unit tests on the pure core: schema shape including
   `schema_version` and `path_scope: "standalone"` (locked with an `insta` JSON
   snapshot), the 1 MiB cap boundary (one byte under stays single-file; one
   byte over splits), deterministic ordering, and fragment pruning against a
   synthetic missing-source entry. (Sizing arithmetic from the review: ~800
   bytes per field-equivalent means the cap engages near ~1,300 fields — far
   beyond realistic trees — so the boundary test plus the round-trip property
   below is the correct minimum; do not elaborate further.)
2. A `proptest` property: for any generated entry set, merging the split
   output reproduces exactly the input entries, and every emitted file except a
   single oversized entry respects the cap.
3. An end-to-end test (new file
   `ortho_config/tests/identifier_artefact_e2e.rs`, `serial_test`-guarded
   because it drives Cargo): add a minimal `build.rs` to
   `tests/fixtures/orthohelp_fixture` (so `OUT_DIR` exists), then run the
   fixture build as a subprocess into a dedicated scratch target directory
   (`--target-dir target/identifier-e2e`) so it neither fights the workspace
   build lock during test execution nor inherits a warm cache that would mask
   the untracked-env-var problem. Cases:
   - Fresh build with `ORTHO_CONFIG_EMIT_IDENTIFIERS=1`
     (`--message-format=json` to locate `OUT_DIR`): assert the artefact is
     absent beforehand, exists afterwards, parses, carries
     `schema_version`, marks every entry with `path_scope: "standalone"`, and
     contains the fixture's derived identifiers.
   - Warm-cache case: rebuild *without* the variable (no write, artefact
     from the previous case untouched), then demonstrate the documented
     forced-recompile invocation
     (`cargo clean -p orthohelp_fixture` then build with the variable)
     refreshes the artefact. This is the realistic user sequence the
     review identified as the silent-failure path.

   Record cold/warm wall-clock in `Artefacts and notes` on first run and add a
   test-runner timeout override if the cold build needs one. Uses the shared
   default Cargo *package* cache (never an isolated one) and tolerates the
   package-cache lock; only the target directory is scratch.

Validation: full gate set; `coderabbit review --agent`. Commit.

### Milestone 6: documentation, ADR, roadmap, and closure

- Write `docs/adr-008-opt-in-identifier-artefact-emission.md`
  (Y-Statement, per `arch-decision-records`): ambient proc-macro writes
  rejected; env-gated emission chosen (exact-`1` semantics, per-invocation-only
  guidance); the forced-recompile workflow and expansion-cache caveat; the
  clean-build freshness contract and residual staleness acceptance;
  `schema_version`, `path_scope: "standalone"`, the provisional-schema status,
  and the rule that future translator tooling takes mounted ids from the
  path-aware compiled docs IR; alternatives (unconditional write,
  extract-from-binary, source-parsing CLI, shared normalizer crate) recorded
  with the evidence from the research pass. Index it in `docs/contents.md`.
- Amend `docs/cli-localization-design.md`: §8.1 (widened trait surface and
  named `ArgLocalizationIds` per D-7, `LOCALIZATION_BASE` per D-5, and replace
  the impossible blanket-impl sentence with the generated delegation of D-1);
  §8.2 (artefact emission is opt-in, reference ADR-008; mark the
  `localized_default` bullet deferred per D-4; specify D-9's path-aware nested
  docs generation and D-12's flatten exclusion). Update ADR-006's known-risk
  paragraph to state precisely which collisions moved to compile time (argument
  ids within a deriving struct) and which remain runtime panics (hand-built
  trees; sibling subcommand names).
- `docs/users-guide.md` ("Localizing CLI copy" section): document the
  trait, the derived constants, `localization_base` and the
  `with_base(T::LOCALIZATION_BASE)` pattern *including the drift hazard it
  prevents*, the collision compile error, the flatten exclusion, and the
  artefact opt-in with the exact forced-recompile invocation and a warning
  never to export the variable in shell profiles or CI-wide blocks. Explain
  that JSON ids have standalone scope and that mounted ids come from docs IR.
  Include the IR "1.1" → "2.0" migration note mapping old dotted ids to new
  dashed ids.
- `docs/developers-guide.md` ("Schema ownership" area): document the
  twin-normalizer rule (both implementations, the marker-comment version gate,
  the dev-dep-cycle property test, and the agreement tests as the lock), the
  path-aware docs-IR delegation and its handwritten-implementation fallback,
  and the artefact fragment/merge design. Update the paragraph that says
  runtime panic tests remain "until derive-emitted identifiers move validation
  to compile time".
- `CHANGELOG.md`: entries for the new trait, the collision diagnostic,
  the IR identifier change (with the "2.0" bump and migration pointer), and the
  artefact opt-in.
- `docs/roadmap.md`: tick all five 11.1.3 checkboxes and the parent item,
  with Decision/Finding notes mirroring this plan's Decision Log — in
  particular that the "blanket impl" bullet was realized as generated
  delegation (D-1), that the artefact schema is provisional until its first
  consumer (D-3), that nested docs identifiers use the mounted command path
  (D-9), and a proposed follow-up item for flatten support (D-12).
- Final full gate run (`make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`) via `scrutineer`; final
  `coderabbit review --agent`; clear all findings.

## Concrete steps

All commands run from the repository root. Long outputs go through `tee`, for
example:

```sh
make test 2>&1 | tee "/tmp/test-ortho-config-$(git branch --show-current).out"
```

- Gates (every milestone): `make check-fmt`, `make typecheck`,
  `make lint`, `make test`; documentation milestones add `make markdownlint` and
  `make nixie`. Prefer delegating the full run to the `scrutineer` subagent
  and reading its cited logs on failure.
- Focused loops: `cargo test -p ortho-config --test localized_parse`,
  `cargo test -p ortho_config_macros`,
  `cargo test -p ortho-config --test compile_fail` (set `TRYBUILD=overwrite`
  only to intentionally regenerate `.stderr`).
- Snapshot review (Milestones 4–5): `cargo insta test`, then accept via
  explicit inspection of each `.snap.new`.
- E2E artefact test (Milestone 5):
  `cargo test -p ortho-config --test identifier_artefact_e2e`.
- Commit after every green sub-step with `commit-message`-skill-formatted
  messages; never commit on a red gate.

Expected red-stage evidence examples: the trybuild collision case initially
*fails the harness* by compiling successfully; `localized_parse.rs`'s new
assertions initially fail with a missing-trait compile error (Milestone 1 red)
or identifier mismatch (Milestone 3 red); the warm-cache e2e case fails until
the documented forced-recompile invocation is what the test exercises. Record
actual transcripts in `Artefacts and notes` as they occur.

## Validation and acceptance

Acceptance is behavioural:

1. `make test` passes. The new tests — the flat-fixture equality and
   subcommand-fixture subset tests in `localized_parse.rs`, the trybuild case
   `tests/ui/localization_id_collision.rs`, the macro-crate localization unit
   and property tests (including the dev-dep-cycle agreement property and the
   marker-version gate), the artefact unit/property/snapshot tests, and both
   e2e artefact cases — all exist and pass; each failed first for the
   documented reason.
2. A collision reproduces a compile error at the offending field span
   naming the normalized id and both fields with the remediation hint, plus a
   "first defined here" note at the earlier field (the pinned message contract
   of Milestone 2).
3. The documented invocation
   `cargo clean -p orthohelp_fixture && ORTHO_CONFIG_EMIT_IDENTIFIERS=1
   cargo build -p orthohelp_fixture`
   produces `${OUT_DIR}/ortho-config/cli-identifiers.json` with
   `schema_version` and the fixture's identifiers; the same build *without* the
   variable writes nothing.
4. `cargo run -p hello_world --bin emit_docs` (docs IR) reports root metadata
   identifiers equal to the `OrthoConfigLocalization` constants and IR version
   "2.0". Every nested command id and localization-eligible field id equals
   `message_id_for` over its mounted command path, including the two-level and
   renamed-variant fixtures; excluded-field defaults and explicit overrides
   remain byte-identical (D-9 and D-12).
5. `make check-fmt`, `make typecheck`, `make lint`, `make markdownlint`,
   and `make nixie` all pass; CodeRabbit findings are cleared.

## Idempotence and recovery

Every milestone is an ordinary additive code change committed on a green gate;
`git revert` of the milestone commits is the rollback path. Snapshot
regeneration is repeatable (`cargo insta test` and re-review). The e2e test
builds into a scratch target directory (`target/identifier-e2e`) and is
`serial_test`-guarded; if it is interrupted, re-running it is safe because
fragment writes are atomic (temp-then-rename) and the merge is a deterministic,
pruning fold. No step mutates state outside the repository and `/tmp` logs.

## Artefacts and notes

Milestone 0 baseline (2026-08-13): all four gates green on branch tip 66b3bf2.
Logs: `/tmp/ms0-checkfmt-*.out` (fmt), `/tmp/ms0-typecheck-*.out`,
`/tmp/ms0-lint-*.out`, `/tmp/ms0-test-*.out`. Both Milestone 0 verification
facts confirmed: (a) `apply_command_metadata` resolves the `usage` suffix for
every node via `localize_command` recursion; (b) `ortho_config_macros/src` has
zero references to `flatten` (grep sweep 2026-08-13), so D-12 flatten exclusion
is additive, not behaviour-preserving.

(To be populated during implementation: red/green transcripts, the collision
diagnostic as rendered, a sample `cli-identifiers.json`, e2e cold/warm
wall-clock.)

## Interfaces and dependencies

At completion the following exist:

- `ortho_config::OrthoConfigLocalization` and
  `ortho_config::ArgLocalizationIds` (shapes in Milestone 1), defined in
  `ortho_config/src/localizer/localization_ids.rs`, re-exported at the crate
  root.
- `#[derive(OrthoConfig)]` additionally emits
  `impl ortho_config::OrthoConfigLocalization for T` with literal-valued
  constants, honouring `#[ortho_config(localization_base = "…")]` and rejecting
  `localized_default` with a deferral message.
- `ortho_config_macros::derive::generate::localization` (private): strict
  normalizer (`syn::Result`-based twin of `normalize_segment`, marker version
  comment), identifier model, collision detection with the pinned message
  contract, artefact core and shell.
- `OrthoConfigDocs` and `OrthoConfigSubcommandDocs` retain their existing
  methods and gain the provided path-aware methods specified in Milestone 4.
  Derive-generated implementations override them so command defaults and
  localization-eligible field defaults across a mounted tree use
  `message_id_for`; `ORTHO_DOCS_IR_VERSION == "2.0"`.
- Opt-in artefact under `${OUT_DIR}/ortho-config/` as specified in
  Milestone 5, with `schema_version`, `path_scope: "standalone"`, and atomic
  writes. Mounted-tree ids remain owned by the path-aware docs IR.
- New unconditional macro-crate dependencies: `serde`, `serde_json`
  (already in the workspace dependency set); new macro-crate *dev*-dependency:
  `ortho_config`. No other dependency changes.

## Revision note

2026-08-06: revised after the six-lens pre-implementation design review
(structure, contracts, alternatives, scaling, failure modes, viability).
Material changes: widened the trait and named the `ARG_IDS` entry struct (D-7);
added `LOCALIZATION_BASE` and the sanctioned `with_base` pattern (D-5 revised);
replaced the impossible macro-crate re-export with a dev-dependency-cycle
property test and a marker-version gate (D-8); narrowed the docs-IR
reconciliation to own metadata with a pinned mapping table and moved the
version bump to "2.0" (D-2 revised, D-9, D-10); made the artefact robust to
incremental compilation, same-named types, stale fragments, and torn writes,
with exact env-var semantics and a `schema_version` envelope (D-3 revised,
D-11); scoped flattened fields out with a recorded policy (D-12); pinned the
collision-message contract; and split the `hello_world` migration into
Milestone 3a. Remaining work is unchanged in intent: trait, derive emission,
docs delegation, artefact, collision guard, documentation.

2026-08-17: revised after the Milestone 4 Ambiguity tolerance was reached and
the user selected the redesign option. Replaced D-9's context-free subcommand
exclusion with additive path-aware methods on the docs traits; updated D-1,
D-2, and D-10 to distinguish standalone constants from mounted defaults;
expanded Milestone 4 into a Red-Green-Refactor path-threading change; added
mounted-path, append-once, override, compatibility, and snapshot verification
obligations; and updated risks, acceptance, interfaces, and Milestone 6
documentation work. The downstream artefact contract now marks JSON entries as
standalone declaration ids and makes compiled docs IR authoritative for mounted
paths, avoiding a second context-free overclaim. Remaining Milestone 4 work now
includes subcommand metadata rather than deferring it to a follow-up.
