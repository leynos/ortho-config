# Add an `ortho_config::cargo` helper for hand-built clap commands (8.3.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE (all milestones delivered 2026-08-09; PR #419 ready for
review)

## Purpose / big picture

Cargo invokes an external subcommand `cargo <name> [OPTIONS]` by executing the
binary `cargo-<name>` with the subcommand name injected as the second argument:
argv becomes `["<path-to>/cargo-<name>", "<name>", OPTIONS...]`. A hand-built
`clap::Command` that models only `cargo-<name> [OPTIONS]` rejects that injected
`<name>` token, so every Cargo subcommand author currently hand-rolls the same
wrapper: a parent `clap::Command::new("cargo")` with the real options nested
one level down under a `<name>` subcommand.

After this change, a crate that builds its `clap::Command` by hand can write:

```rust,ignore
use ortho_config::cargo::external_subcommand;

let args_command = clap::Command::new("demo")
    .version("1.2.3")
    .arg(clap::Arg::new("verbose").long("verbose").num_args(0));
let cli = external_subcommand("cargo-demo", "demo", args_command);
let matches = cli.try_get_matches_from(["cargo-demo", "demo", "--verbose"])?;
// The inner options live one level down, under the subcommand:
let demo = matches
    .subcommand_matches("demo")
    .expect("subcommand_required guarantees a subcommand");
assert!(demo.get_flag("verbose"));
```

and the returned `clap::Command` accepts both invocation forms with the same
inner options and no duplicated parser setup:

1. `cargo demo --verbose` (Cargo dispatch: argv `["cargo-demo", "demo",
   "--verbose"]`), and
2. `cargo-demo demo --verbose` (direct invocation with the same injected
   token).

Note the adoption cost shown above: options move one level down, so callers
read them through `subcommand_matches("<name>")` rather than from the
top-level matches. Every example in the rustdoc and the users' guide must run
all the way to extracting an option value, not stop at construction.

You can observe success three ways. First, integration tests parse both argv
forms through the wrapper and assert the inner option values match a parse of
the unwrapped command. Second, help output renders the Cargo dispatch shape
(`Usage: cargo <COMMAND>` at the top level and `Usage: cargo demo [OPTIONS]`
for the subcommand) and `cargo demo --version` renders `cargo-demo 1.2.3`,
captured as insta snapshots and string assertions. Third, behavioural
(rstest-bdd) scenarios drive the wrapper from a consumer's point of view and
assert both the happy path and the bare-invocation failure paths.

This is roadmap item **8.3.1** (`docs/roadmap.md` §8.3.1). It is the first step
of §8.3 "Standardize Cargo external-subcommand entry points" and unblocks
8.3.2 (derive template documentation), 8.3.3 (macro prototype decision), and
8.3.4 (shared regression fixtures). The design contract is `docs/design.md`
§4.17 and `docs/adr-004-cargo-external-subcommand-entry-point.md` (accepted).

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

1. Entry-point shape stays at the command boundary. Do not move Cargo dispatch
   semantics into the `OrthoConfig` trait, the configuration merge pipeline, or
   any loader path. Configuration precedence remains defaults → files →
   environment → explicit command-line arguments (`docs/design.md` §4.17).
2. The helper must preserve the caller's existing inner command options
   verbatim. It must not add, remove, rename, or re-parse any inner argument,
   and it must not introduce a second configuration-loading pathway. The only
   inner properties the helper touches are the command's `name` (set to the
   injected subcommand name — that is its job), its `bin_name` (reset so
   clap derives consistent `cargo <name>` usage; see D-2 and Risk 2), and its
   `display_name` (set to the installed binary name so version output renders
   it; see D-2).
3. No new external dependency. clap 4.6 (locked at clap 4.6.1 in `Cargo.lock`;
   `derive` and `string` features) is already a dependency of `ortho_config`.
   In particular, do not add `clap-cargo` for styling; styling stays out of
   scope (see Decision D-3).
4. No change to `cargo-orthohelp`'s user-visible behaviour in this task. The
   binary is derive-based and is not migrated here; its dispatch snapshots in
   `cargo-orthohelp/tests/cli_dispatch.rs` must remain untouched and green.
5. Existing public API of `ortho_config` remains stable; this task is purely
   additive (a new `cargo` module and its contents).
6. No circular dependencies. `ortho_config::cargo` depends on clap only — it
   uses no other `ortho_config` internals. `ortho_config` must never depend on
   `cargo-orthohelp` or the test fixture crates.
7. The helper must not install a tracing subscriber or any global state:
   libraries must not install global recorders/subscribers (AGENTS.md). The
   observability expectation in ADR-004 (subscriber before parsing, debug event
   at the dispatch boundary) is a binary-level obligation and is handled here
   as documentation only (see Decision D-4).
8. All module files start with `//!` docs; every public item carries `///`
   rustdoc with runnable examples; no code file exceeds 400 lines (AGENTS.md).
9. Every commit passes the gates (`make check-fmt`, `make typecheck`,
   `make lint`, `make test`). Red-test evidence is captured as transcripts in
   `Artefacts`, not as a failing commit (see D-8).

## Tolerances (exception triggers)

Stop and escalate (do not work around) when any of these is reached.

1. Scope: if the production (non-test, non-doc) change exceeds roughly 150 net
   lines or touches more than 10 files, stop and re-scope. This helper is
   deliberately small; growth beyond that signals scope creep towards 8.3.3.
2. Interface: if delivering the helper requires changing the signature of any
   existing public item, stop and escalate.
3. Dependencies: if any new crate dependency appears necessary, stop.
4. Snapshot drift: if any existing snapshot (in `cargo-orthohelp` or
   elsewhere) changes, stop — Constraint 4 has been breached; investigate.
5. Iterations: if a milestone's gates still fail after 3 focused attempts,
   stop and record the blocker.
6. Ambiguity: the usage-rendering decision (D-2) deviates from the literal
   `bin_name("cargo-<name>")` wording in `docs/roadmap.md` §8.3.1 and
   `docs/design.md` §4.17. This plan's approval covers that deviation; if
   during implementation the deviation turns out to require behaviour the plan
   does not describe, stop and present options.
7. clap behaviour: the plan pins several clap 4.6 rendering behaviours
   verified against the vendored sources (display-name derivation, bin-name
   build paths, error kinds). If implementation observes different behaviour,
   stop, record the discrepancy, and re-verify before adapting tests.

## Risks

1. Risk: help/usage rendering differs between clap patch releases, making
   snapshots brittle. Severity: low. Likelihood: low. Mitigation: snapshot
   only the load-bearing `Usage:` lines and the bare-invocation error
   rendering, not whole help screens, mirroring
   `cargo-orthohelp/tests/cli_dispatch.rs`; assert error kinds as membership
   in a small set rather than a single kind (clap has shifted kinds across
   minor versions; `cargo-orthohelp/src/main.rs` hedges the same way).
2. Risk: a caller who already set `name`, `bin_name`, or `display_name` on
   the inner command gets surprising output. clap preserves an inner
   `display_name` in both build paths but overwrites an inner `bin_name`
   inconsistently (preserved in the help path, overwritten in the
   parse-descent path — verified in clap_builder 4.6 sources). Severity: low.
   Likelihood: medium. Mitigation: the helper renames the inner command and
   resets its `bin_name` so both paths derive the same `cargo <name>` shape;
   the rustdoc states this contract and unit tests pin it.
3. Risk: the `make lint` Whitaker gate has recently been red on `main` for
   files unrelated to a change (see memory note "Whitaker lint gate red on
   main"). Severity: medium. Likelihood: medium. Mitigation: when the lint
   gate fails, check whether the cited files are in this branch's diff before
   treating the failure as caused by this work; escalate if the failure is
   pre-existing.
4. Risk: `coderabbit review --agent` has previously stalled at
   `preparing_sandbox` in this environment (see the 11.1.2 ExecPlan
   retrospective). Severity: low. Likelihood: medium. Mitigation: run it after
   deterministic gates; if it stalls without findings twice, record the stall
   with logs and continue rather than blocking delivery.
5. Risk: the roadmap wording "Return the standard `Command::new("cargo")`
   shape with `bin_name("cargo-<name>")`" could be read as a strict acceptance
   criterion for the parent's `bin_name` value, conflicting with correct usage
   rendering (D-2). Severity: medium. Likelihood: low once the plan is
   approved. Mitigation: the deviation is called out explicitly in D-2 and in
   the design-doc reconciliation milestone; when ticking the roadmap, the
   `bin_name` sub-bullet is annotated with a pointer to the ADR-004 amendment
   so the roadmap and the shipped code do not silently disagree.
6. Risk: the helper's top-level `--version` behaviour diverges from the
   derive reference: `cargo-orthohelp` sets `#[command(version)]` on its
   parent, so `cargo-orthohelp --version` works, while helper consumers get an
   `UnknownArgument` error at the top level (the synthetic parent is
   version-less per D-3). Severity: low. Likelihood: certain (by design).
   Mitigation: pin the current behaviour in a unit test so any clap change
   surfaces, state it in the users' guide, and flag the divergence for 8.3.2
   to reconcile when the derive template is documented.

## Progress

- [x] (2026-08-06) Milestone 0a: reconnaissance (design docs, current
      `cargo-orthohelp` entry point, testing and documentation conventions,
      external prior art) and initial draft.
- [x] (2026-08-06) Milestone 0b: community-of-experts design review (five
      lenses) and plan revision; see the Decision Log and the revision note.
- [x] (2026-08-06) Milestone 0c: user approval of this plan. **Hard gate: no
      implementation before approval.** Approved by the task instruction to
      proceed with implementation.
- [x] (2026-08-07) Milestone 1: red tests written and their failure
      transcripts captured; helper implemented; unit, snapshot, and
      behavioural tests green; gates pass; single commit `9ca4adb`
      (see D-8; transcripts in Artefacts). The branch was then rebased onto
      `origin/main` and the commit re-landed with two lint fixes folded in
      (clippy `doc_markdown` backticks, `shadow_unrelated` rename).
- [x] (2026-08-09) Milestone 2: documentation sweep (users' guide, design
      doc, ADR-004 amendment, developers' guide); `make markdownlint` and
      `make nixie` green in addition to the code gates; committed as
      `6286f1c`. The four edits were authored by the `scribe` subagent from
      the Stage C brief and accepted unchanged after review; all six gates
      passed in a single `scrutineer` run (see Stage C notes).
- [x] (2026-08-09) Milestone 3: CodeRabbit review clean (0 findings across
      17 files, no `preparing_sandbox` stall and no rate limit — the first
      clean completion in this environment since the 11.1.x stalls); roadmap
      8.3.1 marked done with the D-2 annotation per Risk 5; final gates
      green; PR ready for review.

Each milestone ends with the gates run sequentially (never in parallel,
because the environment relies on build caching) and a commit. Prefer
delegating full gate runs to the `scrutineer` subagent, which logs each gate
under `/tmp` and returns a bounded report.

## Surprises & discoveries

- Observation: the workspace does not use `googletest` or `pretty_assertions`
  anywhere; the house unit-assertion style is `rstest` plus plain
  `assert_eq!`/`assert!`, with `insta` for multi-variant output.
  Evidence: no such dev-dependencies in any workspace `Cargo.toml`.
  Impact: this plan follows house style (see D-6) rather than introducing new
  assertion crates.
- Observation: `cargo-orthohelp` already implements the wrapper shape via
  derive (`#[command(name = "cargo", bin_name = "cargo")]` with a
  single-variant subcommand enum) and pins `Usage: cargo <COMMAND>` /
  `Usage: cargo orthohelp [OPTIONS]` in unit tests and insta snapshots,
  including true end-to-end `cargo orthohelp` dispatch tests.
  Evidence: `cargo-orthohelp/src/cli/mod.rs`,
  `cargo-orthohelp/tests/cli_dispatch.rs`.
  Impact: the helper for hand-built commands must render the same usage shape
  or the two paths would disagree; this drives D-2.
- Observation: no published crate wraps an existing `clap::Command` for Cargo
  external-subcommand use. `clap-cargo` and `cargo-options` provide reusable
  flag structs only; real tools hand-roll one of three patterns
  (nested subcommand as in clap's cookbook `cargo-example`; conditional argv[1]
  strip guarded by the `CARGO` environment variable as in `cargo-insta`;
  unconditional argv[1] filter as in `cargo-deny`).
  Evidence: prior-art survey of clap cookbook, clap-cargo docs, cargo-nextest,
  cargo-insta, and cargo-deny sources (2026-08-06).
  Impact: the helper fills a genuine gap; the nested-subcommand pattern is the
  one prescribed by ADR-004 and matches clap's official example.
- Observation: in clap 4.6, a subcommand without an explicit `display_name`
  derives it as `{parent_display_name}-{subcommand_name}`, and
  `render_version` prints the display name. With parent name `cargo` and
  subcommand `demo`, clap derives `cargo-demo` — the installed binary name —
  with zero configuration. Setting `display_name` on the synthetic parent
  (as this plan's first draft sketched) would corrupt the derivation to
  `cargo-demo-demo` in `--version` output.
  Evidence: `clap_builder` 4.6 sources, `src/builder/command.rs`
  (`_build_subcommand` display-name derivation and `_render_version`);
  found independently by two design-review lenses.
  Impact: drives the corrected plumbing in D-2 and the rendered-version
  assertion in the test matrix.
- Observation (implementation, 2026-08-06): `clap` is locked at 4.6.1 but its
  builder dependency is `clap_builder` 4.6.0; the vendored `clap_builder-4.6.0`
  sources were re-verified for every pinned fact (Tolerance 7) and all held:
  `name` takes `impl Into<Str>` while `bin_name`/`display_name` take
  `impl IntoResettable<String>` (`None` resets); `_build_subcommand`
  unconditionally overwrites the subcommand `bin_name` in the parse-descent
  path and derives `display_name` as `{parent_display_name}-{name}` only when
  unset; `_render_version` prints `{display_name} {version}\n` with fallback
  to the command name; `get_flag`/`subcommand_matches` take `&str`;
  `Command::is_subcommand_required_set`, `find_subcommand(&self, ..)`,
  `render_help(&mut self)`, and `get_display_name`/`get_bin_name` all exist.
  Impact: no plan changes needed.
- Observation (implementation, 2026-08-06): `clap_builder::Str` implements
  `From<&'static str>`, `From<String>`, and `From<&String>` (with the
  `string` feature), but **not** `From<&str>` for non-static references;
  `Id` likewise implements `From<String>` and `From<&'static str>`.
  Impact: the BDD step code must pass owned `String`s (or `&String`) to
  `Arg::new`/`Arg::long` rather than borrowed slices.
- Observation (implementation, 2026-08-06): the plan's code sketch wrote
  `.bin_name(None)` to reset the inner `bin_name`, but clap_builder 4.6.0
  has **no** `impl IntoResettable<String> for Option<String>` (only
  `char`/`usize`/`ArgAction`/`ValueHint`/`ValueParser`/`&'static str`
  Options reset); String-typed setters reset through
  `impl IntoResettable<T> for Resettable<T>` instead.
  Impact: the helper passes `clap::builder::Resettable::Reset`; observable
  behaviour is unchanged from the plan (the inner `bin_name` is reset), so
  this is recorded as a sketch correction under Tolerance 7 rather than an
  escalation. Fact list item 1 amended accordingly.
- Observation (implementation, 2026-08-06): workspace lints deny
  `str_to_string` and `indexing_slicing` on all targets including tests, and
  `clippy.toml` sets `allow-expect-in-tests = true`, so test code uses
  `.to_owned()` instead of `.to_string()` on string slices, iterator access
  instead of `slice[i]`, and `.expect(...)` freely. `rstest-bdd` step
  functions receive placeholder captures with their surrounding quotes
  intact, so steps normalize captured values with the shared
  `value_parsing` helpers, and step-parameter names must match the
  `{placeholder}` names in the step pattern exactly.
  Impact: test and step style follows these constraints; no plan changes.
- Observation (implementation, 2026-08-06): clap 4.6's
  `ArgMatches::subcommand_matches(name)` **panics** when `name` is not among
  the subcommands matched by that parse (clap's own "not a name of a
  subcommand" assertion); it does not return `None` for unmatched names.
  Evidence: `clap_builder` 4.6.0, `arg_matches.rs` (`get_subcommand` wraps
  `MatchesError::unwrap`).
  Impact: the rename unit test checks `Command::find_subcommand` instead of
  querying unmatched names on `ArgMatches`; the documented adoption shape is
  unaffected because `subcommand_required(true)` guarantees the injected
  subcommand is always the one matched on success.
- Observation (implementation, 2026-08-07): adding `CargoContext` and its
  `#[fixture]` provider to the shared `scenario_state` module pushed that
  file towards the repository's 400-line module cap enforced by the Whitaker
  gate, so both were relocated into
  `tests/rstest_bdd/behaviour/steps/cargo_steps.rs`, beside the steps that
  consume them (`scenarios.rs` imports them from there).
  Impact: this matches the developers' guide's isolation guidance for
  fixture-specific step modules; Milestone 2's developers'-guide wording
  should describe the state as shipped.

## Decision log

- Decision (D-1): **The helper is a plain function that reshapes a
  `clap::Command`; parsing stays with the caller.** Signature (see
  "Interfaces and dependencies" for the documented form):
  `external_subcommand(installed_bin_name, subcommand_name, command) ->
  clap::Command`. It performs no argv inspection, no environment reads, and no
  parsing. Rationale: `docs/design.md` §4.17 prescribes exactly this shape;
  keeping the helper free of argv/environment access keeps it deterministic,
  trivially testable, and macro-generatable by a possible 8.3.3 attribute
  (plain `impl Into<_>` parameters, no generics over iterators, no
  lifetimes). The argv-stripping alternatives (cargo-insta, cargo-deny
  patterns) were rejected on two independent anchors: ADR-004 chose the
  nested-subcommand shape, and the written acceptance criteria predating this
  plan require it — design.md §4.17's success criterion and the 8.3.4 fixture
  spec both demand that `cargo-<name> <name> [OPTIONS]` parse on direct
  invocation *without* Cargo in the loop, which a conditional argv strip
  (guarded by the `CARGO` environment variable) cannot deliver. Newtype
  parameters were considered for the two adjacent string arguments and
  rejected: the roadmap fixes the three-argument form, and plain `Into`
  bounds keep the call macro-generatable; the transposition hazard is covered
  by the debug assertion and tests in D-7 instead. Date/Author: 2026-08-06,
  planning session, amended after panel review.

- Decision (D-2): **The parent is `Command::new("cargo").bin_name("cargo")`
  and sets no display name; `installed_bin_name` is applied to the *inner*
  command as its `display_name`, and the inner `bin_name` is reset.** This
  deviates from the literal `bin_name("cargo-<name>")` text in
  `docs/roadmap.md` §8.3.1 and the code sketch in `docs/design.md` §4.17.
  Rationale: `bin_name` is what clap prints in `Usage:` lines.
  `bin_name("cargo")` renders `Usage: cargo <name> [OPTIONS]`, which matches
  clap's official `cargo-example` cookbook pattern, matches Cargo's help
  protocol (`cargo help <name>` runs `cargo-<name> <name> --help`), and
  matches the behaviour `cargo-orthohelp` already pins in its snapshots.
  `bin_name("cargo-<name>")` would render `Usage: cargo-<name> <name>
  [OPTIONS]`, which is accurate only for direct invocation and would make the
  hand-built and derive paths disagree. The installed binary name has a real,
  observable job on the *inner* command: clap's `render_version` prints the
  display name, so `cargo demo --version` renders `cargo-demo 1.2.3`. (A
  parent-level `display_name` — the first draft's sketch — is never rendered,
  because the parent is version-less, and it corrupts the inner derivation to
  `cargo-demo-demo`; see Surprises. Explicitly setting the inner
  `display_name` is equivalent to clap's derivation when the names are
  consistent and honours unusual installed names when they are not.) The
  inner `bin_name` is reset because clap treats a caller-set inner `bin_name`
  inconsistently between the help and parse-descent build paths; resetting
  makes both derive `cargo <name>`. Milestone 2 reconciles the design doc's
  code sketch with this decision and appends an ADR-004 amendment. Plan
  approval ratifies the deviation (Tolerance 6). Date/Author: 2026-08-06,
  planning session, corrected after panel review (two lenses independently
  found the display-name derivation flaw).

- Decision (D-3): **No styling, no version propagation, no error-hint
  augmentation inside the helper.** The helper sets exactly:
  `Command::new("cargo")`, `bin_name("cargo")`, `subcommand_required(true)`,
  and nests the renamed inner command (with `display_name` set and `bin_name`
  reset per D-2) as the sole subcommand. Rationale: Cargo's colour styling
  would require the `clap-cargo` dependency (Constraint 3); version belongs
  on the inner command where the caller controls it (clap's cookbook sets
  `version` on the subcommand, keeping the synthetic parent version-less —
  the resulting top-level `--version` divergence from `cargo-orthohelp` is
  Risk 6); the friendly "invoke via `cargo <name>`" hint that
  `cargo-orthohelp` prints on `MissingSubcommand` is binary-level error
  handling, not command shape, and stays with binaries — but the users' guide
  carries a copy-pasteable hint snippet (Milestone 2), not a one-line
  mention. Each exclusion is documented in the helper's rustdoc so callers
  know where those concerns live. Date/Author: 2026-08-06, planning session,
  amended after panel review.

- Decision (D-4): **ADR-004's observability expectation is documented, not
  implemented.** ADR-004 says Cargo-facing binaries should initialize a
  tracing subscriber before parsing and emit a debug event at the dispatch
  boundary. A library helper must not install subscribers (AGENTS.md), and the
  helper never parses, so it has no dispatch boundary of its own. The users'
  guide section (Milestone 2) states the expectation and points at
  `cargo-orthohelp/src/main.rs` as the reference implementation. Date/Author:
  2026-08-06, planning session.

- Decision (D-5): **Coverage levels.** Unit tests (rstest) pin the shape,
  both argv forms, both bare-invocation failure paths (no arguments at all,
  and a flag without the injected token), the inner rename, the
  `bin_name`/`display_name` contract including the rendered `--version`
  string, a nested inner subcommand, and a required inner argument; insta
  snapshots pin the `Usage:` lines for top-level and subcommand help and the
  no-arguments error rendering (insta is retained over bare substring asserts
  to match the `cli_dispatch.rs` house precedent; the substring alternative
  was considered); an rstest-bdd feature exercises the consumer-visible happy
  and unhappy paths; argv-equivalence is covered by table-driven
  `#[rstest] #[case]` parameterized cases rather than a property test. The
  first draft proposed a proptest for "wrapped parse equals unwrapped parse
  for arbitrary option values"; the panel cut it: the helper never touches
  argument values, so the property holds by construction and would exercise
  clap's subcommand dispatch, not this crate's code — exactly the
  non-substantive-invariant case `docs/developers-guide.md` says property
  tooling must not be added for. No end-to-end process-spawning tests here:
  roadmap item 8.3.4 owns the shared on-`PATH` regression fixtures, and
  `cargo-orthohelp`'s existing `cli_dispatch.rs` already proves real Cargo
  dispatch works for the nested shape. No `kani`/`verus`: the helper contains
  no unsafe code, no state machine, and no arithmetic lemma. Date/Author:
  2026-08-06, planning session, amended after panel review.

- Decision (D-6): **Assertion style follows the house style.** The task brief
  asks for `googletest` and `pretty_assertions`; neither is used anywhere in
  this workspace, and prior ExecPlans (11.1.1, 11.1.2) resolved the same
  tension by citing the `rust-unit-testing` skill while using `rstest` plus
  plain assertions and `insta`. Introducing two new dev-dependencies for one
  small module would contradict AGENTS.md's dependency discipline and
  Constraint 3's spirit. Date/Author: 2026-08-06, planning session.

- Decision (D-7): **The name-consistency invariant is a documented
  precondition backed by debug assertions.** The protocol only works when
  `installed_bin_name == "cargo-<subcommand_name>"` (Cargo derives the
  injected token from the binary's file name). The helper carries
  `debug_assert_eq!` for that relationship and `debug_assert!` that the
  subcommand name is non-empty, with the preconditions stated in rustdoc
  (including a note that the name `help` is reserved by clap's auto-generated
  help subcommand). Debug assertions rather than a `Result`: a violation is a
  programming error in the caller's build description, mirroring clap's own
  panic-on-misuse builder philosophy, and an error type would burden every
  correct caller. This also neutralizes the adjacent-string-parameter
  transposition hazard: swapping the arguments trips the assertion in debug
  builds and the rustdoc example makes the distinct roles visually obvious.
  Date/Author: 2026-08-06, panel review (contracts and structure lenses).

- Decision (D-8): **`external_subcommand` is not re-exported at the crate
  root, and red-test evidence is transcripts, not a failing commit.** Two
  housekeeping decisions promoted from asides. (a) Every existing public
  module in `ortho_config/src/lib.rs` re-exports its key items at the root;
  `cargo` deliberately breaks that convention because `external_subcommand`
  at the root is vague and collides conceptually with clap's
  `#[command(external_subcommand)]` attribute, while
  `ortho_config::cargo::external_subcommand` reads as a sentence.
  `ortho_config/tests/reexports.rs` (if it enumerates the root surface) is
  checked and updated accordingly. (b) The repository rule is that every
  commit passes the gates; red tests that fail to compile cannot be committed
  on their own. Red evidence is therefore captured as transcripts in
  `Artefacts` during Milestone 1, and the first commit lands with tests and
  implementation together at the end of that milestone. Date/Author:
  2026-08-06, panel review (structure and viability lenses).

- Decision (D-9): **A future argv-normalizing companion is recorded as
  deliberately out of scope, not rejected.** Tools that want cargo-insta's
  transparent dual-mode (bare `cargo-demo --verbose` working without the
  injected token) cannot use this wrapper shape; the module documentation
  names that pattern as out of scope for 8.3.1 so the gap reads as a choice.
  The module namespace leaves room for a later `normalized_args`-style helper
  without disturbing `external_subcommand`. Date/Author: 2026-08-06, panel
  review (alternatives lens).

## Outcomes & retrospective

Delivered: `ortho_config::cargo::external_subcommand` ships in the crate with
unit, snapshot, and behavioural coverage, the public documentation is
reconciled with the shipped shape, the roadmap is ticked, and the PR is ready
for review.

What went well:

- The plan's clap 4.6 fact list held without revision during implementation
  (Tolerance 7 was not triggered), and the two benign sketch corrections
  (`Resettable::Reset` for the inner `bin_name` reset; owned `String`s in BDD
  step code) were recorded as observations rather than escalations.
- The red-evidence discipline (D-8) worked as designed: transcripts captured
  the compile failures against the missing module, and the first commit landed
  green with tests and implementation together.
- The `scrutineer` delegation kept full gate output out of the planning
  context; every milestone gate run came back green on the first attempt.
- CodeRabbit completed clean on the first 8.3.1 attempt — 0 findings across
  17 files, no `preparing_sandbox` stall and no rate limit — the first clean
  completion recorded in this environment since the 11.1.x stalls, so Risk 4
  did not materialize.
- The `scribe` delegation for the Milestone 2 doc sweep produced all four
  edits to the Stage C brief, accepted unchanged after review, and every
  Markdown gate passed on the first run.

Costs and frictions:

- The documentation sweep is the largest single effort in this task measured
  in prose, not code: the helper is ~120 lines of production code, but the
  plan's documentation obligations (users' guide section with hint snippet,
  design sketch reconciliation, ADR amendment, developers' guide) are where
  the deliverable's discoverability lives.
- The `typos.local.toml` inline-code exclusion was originally landed on this
  branch as a temporary hold (pre-rebase commit `41f2a53`) inherited from the
  upstream spelling-dictionary change. The 2026-08-09 rebase dropped that
  commit as empty because main's `d3c9bdf` had meanwhile re-added the same
  pattern as a policy position; the pattern now comes from main, so the
  temporary-hold framing is superseded.

Deferred to follow-up items (all recorded in the plan body):

- 8.3.2 owns the derive template documentation and full README examples; the
  README now signposts the shipped helper.
- 8.3.4 owns the shared on-`PATH` regression fixtures; `cargo-orthohelp`'s
  existing `cli_dispatch.rs` already proves real Cargo dispatch for the
  nested shape.
- The helper's top-level `--version` divergence from the derive reference
  (Risk 6) is pinned in tests and documented in the users' guide for 8.3.2 to
  reconcile.

## Context and orientation

The `ortho_config` workspace uses Rust edition 2024 and workspace version
0.8.0. Workspace members: `ortho_config` (the library),
`ortho_config_macros`, `cargo-orthohelp` (the reference CLI binary),
`examples/hello_world`, `test_helpers`, and
`tests/fixtures/orthohelp_fixture`. The gates are `make check-fmt`,
`make typecheck`, `make lint` (clippy plus Whitaker), and `make test`; docs
changes additionally need `make markdownlint` and `make nixie`. Run gates
sequentially.

Key terms:

1. **Cargo external subcommand** — Cargo runs `cargo <name>` by executing
   `cargo-<name>` from `PATH` with argv
   `["<path>/cargo-<name>", "<name>", ...]`; the second element is the
   *injected subcommand name*. `cargo help <name>` runs
   `cargo-<name> <name> --help` (The Cargo Book, "External tools").
2. **The wrapper shape** — a synthetic parent `clap::Command::new("cargo")`
   whose only subcommand carries the tool's real options. clap then consumes
   the injected token as the subcommand selector; no argv massaging is
   needed. This is clap's official `cargo-example` cookbook pattern and what
   ADR-004 adopted.
3. **Hand-built command** — a `clap::Command` constructed with the builder
   API rather than `#[derive(clap::Parser)]`. Derive-based callers use the
   pattern documented by 8.3.2 instead (wrap the `Args` struct in a
   single-variant `#[command(subcommand)]` enum, as `cargo-orthohelp` does in
   `cargo-orthohelp/src/cli/mod.rs`).
4. **Display name** — clap's `Command::display_name`, printed by version
   output. When unset on a subcommand, clap 4.6 derives it as
   `{parent_display_name}-{subcommand_name}` (parent default: the parent's
   name), so the wrapper's inner command derives `cargo-<name>` naturally;
   the helper sets it explicitly to the installed binary name (D-2).

The `ortho_config` crate's `src/lib.rs` currently declares these top-level
modules: `agent_context`, `csv_env`, `declarative`, `discovery`, `docs`,
`error`, `file`, `localizer` (private), `merge`, `post_merge`, `result_ext`,
`subcommand`. This task adds a new public `cargo` module (the module refers to
Cargo-the-tool's dispatch protocol; its `//!` documentation's first sentence
must say so, because the bare name is opaque in the rustdoc module index).
clap is already a direct dependency with the `derive` and `string` features.

Verified clap 4.6 facts this plan relies on (checked against the vendored
`clap_builder` sources; re-verify per Tolerance 7 if behaviour differs):

1. `Command::name` takes `impl Into<clap::builder::Str>`; `bin_name` and
   `display_name` take `impl IntoResettable<String>`; resetting a
   String-typed setter uses `clap::builder::Resettable::Reset` (clap 4.6.0
   has no `IntoResettable<String>` impl for `Option<String>`).
2. Usage lines render from `bin_name`; subcommand usage joins the parent
   `bin_name` with the subcommand name (`cargo demo`).
3. A subcommand's unset `display_name` derives as
   `{parent_display_name}-{sc_name}`; `render_version` prints the display
   name.
4. An inner `display_name` set by the caller is preserved in both build
   paths; an inner `bin_name` is preserved in the help path but overwritten
   in the parse-descent path (hence the reset in D-2).
5. With `subcommand_required(true)`: zero arguments yields
   `ErrorKind::MissingSubcommand` ("'cargo' requires a subcommand…"); a
   leading long flag yields `ErrorKind::UnknownArgument`; clap only adds
   `-V/--version` where a version is set, so top-level `--version` on the
   version-less parent is `UnknownArgument` (Risk 6).

Testing infrastructure this plan reuses:

1. Unit tests: `rstest` 0.26.1 (dev-dependency of `ortho_config`); unit tests
   for a module live in a sibling `tests.rs` included with
   `#[cfg(test)] mod tests;` (for example `ortho_config/src/error/tests.rs`).
2. Integration tests: files under `ortho_config/tests/` (for example
   `clap_subcommand.rs`, `localized_parse.rs`).
3. Behavioural tests: a single registered test binary
   (`[[test]] name = "rstest_bdd" path = "tests/rstest_bdd/mod.rs"` in
   `ortho_config/Cargo.toml`), feature files under
   `ortho_config/tests/features/*.feature`, step definitions under
   `ortho_config/tests/rstest_bdd/behaviour/steps/` (17 existing
   `*_steps.rs` files), scenario-local state via `Slot<T>` in
   `#[derive(ScenarioState)]` structs
   (`ortho_config/tests/rstest_bdd/scenario_state.rs`), scenarios bound with
   `scenarios!(...)`. Adding a feature means touching three places: the
   feature file, the steps module (declared in the steps `mod.rs`), and the
   scenario-state/fixture wiring plus `scenarios!` binding. The `rstest_bdd`
   group is serialized to one thread by `.config/nextest.toml`.
4. Snapshots: `insta` 1, snapshots beside the owning test; review with
   `cargo insta review` (or `INSTA_UPDATE=always` non-interactively, then
   verify no `.snap.new`/`.pending-snap` files remain).

Signposted documentation and skills:

1. Design and decisions: `docs/design.md` §4.17 (the contract for this task),
   `docs/adr-004-cargo-external-subcommand-entry-point.md`,
   `docs/roadmap.md` §8.3, `docs/agent-native-cli-design.md` §7 (why
   `cargo-orthohelp` is the dogfooding target), `docs/contents.md` (doc
   index), `docs/documentation-style-guide.md` (en-GB-oxendict, heading and
   wrapping rules).
2. Testing guides: `docs/rust-testing-with-rstest-fixtures.md`,
   `docs/rust-doctest-dry-guide.md`,
   `docs/reliable-testing-in-rust-via-dependency-injection.md`,
   `docs/rtest-bdd-users-guide.md`,
   `docs/complexity-antipatterns-and-refactoring-strategies.md`.
3. Skills: `rust-router`, then `rust-types-and-apis` (API shape) and
   `arch-crate-design` (module placement and public surface);
   `rust-unit-testing` (fixtures, table tests, snapshot discipline); `leta`
   for navigation; `commit-message` for commits; `pr-creation` for the PR;
   `comenq-coderabbit` if the CodeRabbit loop needs driving through a PR.

## Plan of work

Work proceeds in stages with go/no-go validation at each boundary. Stages map
to the milestones in `Progress`.

### Stage A — understand and propose (no code changes)

Complete: this document, informed by a four-agent reconnaissance pass (design
docs, current `cargo-orthohelp` implementation, testing and documentation
conventions, and external prior art) and revised by a five-lens
community-of-experts design review (contracts, structure, alternatives,
failure modes, viability; the scaling lens was waived — a pure
`Command → Command` function has no load profile). The outputs are Decisions
D-1 through D-9. Go/no-go: the user approves this plan (the approval gate).
Do not start Stage B without it.

### Stage B — red evidence, then implementation (Milestone 1)

Write the tests first, against the planned `ortho_config::cargo` API, so they
fail to compile because the module does not exist yet. Run the focused test
commands and capture the failure transcripts into `Artefacts` as the red
evidence. Per D-8 there is no red-only commit; tests and implementation land
together at the end of this stage once everything is green.

Red tests:

1. Unit tests in `ortho_config/src/cargo/tests.rs` (included from the module
   with `#[cfg(test)] mod tests;`), using `rstest`:
   - `wraps_command_under_cargo_parent`: the returned command is named
     `cargo`, has `bin_name` `cargo`, requires a subcommand, and contains
     exactly one subcommand named with the injected name.
   - `parses_cargo_dispatch_argv`: `try_get_matches_from(["cargo-demo",
     "demo", "--verbose"])` succeeds and
     `subcommand_matches("demo")` carries `verbose` — the assertion goes
     through the subcommand extraction so the documented adoption shape is
     what the test exercises.
   - `parses_direct_invocation_argv`: the same argv shape with a direct-path
     argv[0] (for example `"./target/debug/cargo-demo"`) succeeds — proving
     argv[0] is irrelevant to the wrapper.
   - `rejects_flag_without_injected_token`:
     `try_get_matches_from(["cargo-demo", "--verbose"])` fails, with the
     error kind asserted as membership in
     `{UnknownArgument, InvalidSubcommand}` (hedged per Risk 1).
   - `rejects_zero_argument_invocation`:
     `try_get_matches_from(["cargo-demo"])` fails with
     `ErrorKind::MissingSubcommand` — the most common consumer mistake, on a
     different clap code path from the flag case.
   - `renames_inner_command_to_subcommand_name`: an inner command originally
     named `something-else` surfaces as the `demo` subcommand.
   - `sets_inner_display_name_and_resets_bin_name`: the nested subcommand's
     `display_name` is the installed binary name and a caller-set inner
     `bin_name` does not leak into usage (both parse-descent and help paths
     render `cargo demo`).
   - `version_renders_installed_binary_name`:
     `try_get_matches_from(["cargo-demo", "demo", "--version"])` yields
     `ErrorKind::DisplayVersion` **and** the rendered error string starts
     with `cargo-demo` followed by a space — pinning the rendered text,
     because the kind alone cannot detect display-name corruption.
   - `top_level_version_is_rejected`:
     `try_get_matches_from(["cargo-demo", "--version"])` fails (the parent is
     version-less; Risk 6), asserted as a hedged kind set.
   - `supports_nested_inner_subcommands`: an inner command that itself has a
     subcommand parses `["cargo-demo", "demo", "build"]` and renders a sane
     `cargo demo build` usage line.
   - `preserves_required_inner_arguments`: an inner command with a required
     argument errors cleanly through the wrapper when the argument is
     missing and parses when present.
   - Argv-equivalence table (replaces the first draft's proptest, per D-5):
     `#[rstest]`-parameterized cases asserting that for representative argv
     tails (flags, options with values, positionals), the wrapped parse of
     `["cargo-demo", "demo", tail...]` yields the same option values via
     `subcommand_matches("demo")` as the unwrapped inner command parsing
     `["demo", tail...]`.
2. Snapshot tests in `ortho_config/tests/cargo_entry_point.rs` using `insta`,
   asserting the load-bearing renderings only (D-5):
   - top-level help contains `Usage: cargo <COMMAND>`;
   - subcommand help contains `Usage: cargo demo [OPTIONS]`;
   - the zero-argument error rendering (`'cargo' requires a subcommand…`
     plus the usage line), because that is what a confused user actually
     sees.
   Render via `Command::render_help` / the error's `to_string()` on the
   wrapped command (no process spawning). Snapshots live beside the test.
3. Behavioural coverage: new feature file
   `ortho_config/tests/features/cargo_entry_point.feature`, a steps module
   `ortho_config/tests/rstest_bdd/behaviour/steps/cargo_steps.rs` (declared
   in the steps `mod.rs`), a `Slot`-based `ScenarioState` struct with its
   `#[fixture]` provider following
   `ortho_config/tests/rstest_bdd/scenario_state.rs` conventions, and a
   `scenarios!` binding for the feature file — all three wiring points, not
   just the steps file. Scenarios:

   ```gherkin
   Feature: Cargo external-subcommand entry point
     Scenario: Cargo dispatch invocation parses the inner options
       Given a hand-built clap command named "demo" with a "--verbose" flag
       When the command is wrapped for the installed binary "cargo-demo"
       And the wrapper parses the Cargo-injected arguments "demo --verbose"
       Then parsing succeeds and the "demo" subcommand sees "--verbose"

     Scenario: Bare invocation without the injected token is rejected
       Given a hand-built clap command named "demo" with a "--verbose" flag
       When the command is wrapped for the installed binary "cargo-demo"
       And the wrapper parses the arguments "--verbose"
       Then parsing fails because the subcommand token is missing
   ```

Implementation:

1. Create `ortho_config/src/cargo/mod.rs`. The `//!` doc's first sentence:
   "Helpers for Cargo external-subcommand (`cargo-<name>`) entry points." It
   explains Cargo's injected-token protocol (with the argv example), the
   wrapper shape, the `subcommand_matches` adoption shape, what the helper
   deliberately does not do (D-3, D-4), that the argv-normalizing pattern is
   deliberately out of scope (D-9), and a runnable doctest that goes all the
   way to extracting an option value. The function body:

   ```rust,ignore
   #[must_use]
   pub fn external_subcommand(
       installed_bin_name: impl Into<String>,
       subcommand_name: impl Into<clap::builder::Str>,
       command: clap::Command,
   ) -> clap::Command {
       let installed = installed_bin_name.into();
       let name = subcommand_name.into();
       debug_assert!(!name.as_str().is_empty(), "subcommand name is empty");
       debug_assert_eq!(
           installed,
           format!("cargo-{name}"),
           "installed binary name must be cargo-<subcommand name>",
       );
       clap::Command::new("cargo")
           .bin_name("cargo")
           .subcommand_required(true)
           .subcommand(
               command
                   .name(name)
                   .bin_name(clap::builder::Resettable::Reset)
                   .display_name(installed),
           )
   }
   ```

   `Resettable::Reset` replaces the draft's `.bin_name(None)`: clap 4.6.0
   provides no `IntoResettable<String>` conversion for `Option<String>`;
   see Surprises & discoveries.

   Parameter types follow their sinks (`display_name` takes
   `IntoResettable<String>`, so the installed name is `impl Into<String>`;
   `Command::name` takes `impl Into<Str>`); `IntoResettable` itself is not
   exposed in the signature. The rustdoc documents: the rename and
   `bin_name` reset contract (Risk 2); the preconditions and debug
   assertions (D-7, including the reserved `help` name); that the returned
   command is a plain `clap::Command` the caller may customize further; and
   where version, styling, error hints, and tracing live (D-3, D-4).
2. Declare `pub mod cargo;` in `ortho_config/src/lib.rs` alongside the other
   public modules. Per D-8, do not re-export `external_subcommand` at the
   crate root; check `ortho_config/tests/reexports.rs` for any
   root-surface enumeration that needs a deliberate exception noted.
3. Make the red unit, snapshot, and behavioural tests pass.

Go/no-go: focused tests green, then full gates green
(`make check-fmt`, `make typecheck`, `make lint`, `make test`, sequentially).
Single commit containing tests and implementation, with the red transcripts
recorded in `Artefacts`.

### Stage C — documentation sweep (Milestone 2)

1. `docs/users-guide.md`: add a new `##` section "Cargo external-subcommand
   entry points" (placed after "Documentation metadata (OrthoConfigDocs)" and
   before "Additional notes"), covering: why Cargo injects the subcommand
   name; the helper with a worked example that runs to extracting an option
   value through `subcommand_matches`; the plain statement that the wrapper
   is CLI entry-point structure, not configuration loading (design.md
   §4.17's documentation obligation); the derive-based alternative in one
   paragraph with a pointer to `cargo-orthohelp`; the binary-level
   obligations the helper leaves with the caller — version on the inner
   command (including that top-level `--version` intentionally errors,
   Risk 6), a copy-pasteable `MissingSubcommand`/`UnknownArgument` hint
   snippet mirroring `exit_for_clap_error` and `write_augmented_clap_error`
   from `cargo-orthohelp/src/main.rs`, and the tracing expectation per
   ADR-004 (D-4). Add a one-line cross-reference from the existing
   "Subcommand configuration" section, where Cargo-tool authors will look
   first. The README now signposts the shipped helper; full derive-template
   examples remain deferred to 8.3.2.
2. `docs/design.md` §4.17: update the code sketch to the shipped shape
   (parent `bin_name("cargo")`, inner `display_name`, per D-2) and note that
   the helper shipped in 8.3.1; keep the section's constraint prose intact.
3. `docs/adr-004-cargo-external-subcommand-entry-point.md`: append a short
   amendment note recording D-2 (usage renders the Cargo dispatch form; the
   installed binary name is carried as the inner command's display name;
   the inner `bin_name` is reset).
4. `docs/developers-guide.md`: extend the "Behavioural test layout" prose
   (the paragraph about keeping richer fixture families isolated is the
   natural insertion point) with the new feature file and steps module, so
   future contributors extend rather than duplicate.
5. `docs/contents.md`: index this ExecPlan alongside the other execution
   plans.

Go/no-go: run `make fmt` after the documentation changes, then require
`make markdownlint` and `make nixie` green in addition to the code gates,
recording the formatting result. Commit.

### Stage D — review, roadmap, delivery (Milestone 3)

1. Run the full gate suite sequentially via `scrutineer`; all green.
2. Run `coderabbit review --agent` (log under `/tmp`); clear every concern.
   Deterministic gates must already be green before the review is requested.
   If the review stalls at `preparing_sandbox` twice, record the stall (Risk
   4) and continue.
3. Tick roadmap item 8.3.1 in `docs/roadmap.md` (the four sub-bullets and
   the item; mark "done" per the item's convention), annotating the
   `bin_name("cargo-<name>")` sub-bullet with a pointer to the ADR-004
   amendment (Risk 5) so the roadmap text and the shipped code do not
   silently disagree.
4. Update this ExecPlan's `Progress`, `Outcomes & retrospective`, and append
   a revision note; final commit; push and mark the PR ready for review.

## Concrete steps

Run from the worktree root. Use `tee` so truncated output stays reviewable:

```bash
# Gate template (run each sequentially, never in parallel):
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-ortho-config-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-ortho-config-$(git branch --show-current).out"
```

Focused runs during red/green:

```bash
# Red: expect unresolved module `ortho_config::cargo`.
cargo test -p ortho_config --test cargo_entry_point 2>&1 \
  | tee /tmp/red-cargo-entry-point.out
cargo test -p ortho_config --test rstest_bdd 2>&1 \
  | tee /tmp/red-cargo-bdd.out
# Green after implementation:
cargo test -p ortho_config cargo:: 2>&1 | tee /tmp/green-cargo-unit.out
cargo test -p ortho_config --test cargo_entry_point 2>&1 \
  | tee /tmp/green-cargo-entry-point.out
cargo test -p ortho_config --test rstest_bdd 2>&1 \
  | tee /tmp/green-cargo-bdd.out
```

Expected red transcript shape (illustrative):

```plaintext
error[E0432]: unresolved import `ortho_config::cargo`
```

Expected green transcript shape (illustrative):

```plaintext
test parses_cargo_dispatch_argv ... ok
test rejects_zero_argument_invocation ... ok
test version_renders_installed_binary_name ... ok
test result: ok. 12 passed; 0 failed
```

## Validation and acceptance

Acceptance is behavioural:

1. A hand-built `clap::Command` wrapped by
   `ortho_config::cargo::external_subcommand("cargo-demo", "demo", cmd)`
   parses both `["cargo-demo", "demo", OPTIONS...]` (Cargo dispatch) and the
   identical direct-invocation argv, yielding the same option values (read
   through `subcommand_matches("demo")`) as the unwrapped command — proven
   by the unit tests and the argv-equivalence table.
2. Bare invocation fails safely on both paths: zero arguments yields
   `MissingSubcommand`, and a leading flag yields an unknown-argument-class
   error — proven by unit tests, the error-rendering snapshot, and a BDD
   scenario.
3. Help renders the Cargo dispatch shape (`Usage: cargo <COMMAND>`,
   `Usage: cargo demo [OPTIONS]`) — proven by insta snapshots.
4. `cargo demo --version` renders `cargo-demo <version>` (the installed
   binary name, not `cargo-demo-demo`), and top-level `--version` errors —
   proven by the rendered-string assertions.
5. The inner command's options are preserved, not duplicated — proven by
   `preserves_required_inner_arguments`, `supports_nested_inner_subcommands`,
   and the argv-equivalence table.
6. No existing behaviour changes: `cargo-orthohelp`'s dispatch tests and
   snapshots are untouched and green — proven by `make test`.

Red-Green-Refactor evidence to record in `Artefacts` as work proceeds: the
Stage B red run (expected compile failure), the green run, and the
post-refactor green run plus lint/format gates. Per D-8 the red stage is
evidenced by transcripts rather than a failing commit.

Quality criteria ("done"):

1. Tests: `make test` passes across the workspace; all new tests pass.
2. Lint/typecheck: `make lint` and `make typecheck` clean (warnings denied).
3. Formatting: `make check-fmt` clean; docs pass `make markdownlint` and
   `make nixie`.
4. Review: `coderabbit review --agent` reports no outstanding concerns (or a
   recorded environment stall per Risk 4).
5. Roadmap: 8.3.1 marked done with the D-2 annotation.

## Idempotence and recovery

Each milestone is a separate commit; any stage rolls back with `git revert`
or `git reset` to the prior commit. All edits are additive except the
documentation reconciliation in `docs/design.md` §4.17 (a sketch update,
recoverable from git history). Snapshot generation is repeatable
(`INSTA_UPDATE=always` then verify no pending snapshots remain). No data
migrations, no external side effects.

## Artefacts and notes

### Stage B red transcripts (per D-8, evidence rather than a failing commit)

Both red runs fail to compile because the module did not exist yet.

`cargo test -p ortho_config --test cargo_entry_point`
(log: `/tmp/red-cargo-entry-point.out`):

```plaintext
error[E0432]: unresolved import `ortho_config::cargo`
  --> ortho_config/tests/cargo_entry_point.rs:11:19
   |
11 | use ortho_config::cargo::external_subcommand;
   |                   ^^^^^ could not find `cargo` in `ortho_config`

error: could not compile `ortho_config` (test "cargo_entry_point") due to 1 previous error
```

`cargo test -p ortho_config --test rstest_bdd`
(log: `/tmp/red-cargo-bdd.out`):

```plaintext
error[E0432]: unresolved import `ortho_config::cargo`
  --> ortho_config/tests/rstest_bdd/behaviour/steps/cargo_steps.rs:13:19
   |
13 | use ortho_config::cargo::external_subcommand;
   |                   ^^^^^ could not find `cargo` in `ortho_config`

error: could not compile `ortho_config` (test "rstest_bdd") due to 1 previous error
```

### Stage B green transcripts

- `cargo test -p ortho_config --lib cargo::`
  (log: `/tmp/green-cargo-unit.out`):

  ```plaintext
  test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 154 filtered out
  ```

- `INSTA_UPDATE=always cargo test -p ortho_config --test cargo_entry_point`
  (log: `/tmp/green-cargo-entry-point.out`) created the three baselines under
  `ortho_config/tests/snapshots/` (`cargo_entry_point__top_level_help_usage`,
  `cargo_entry_point__subcommand_help_usage`,
  `cargo_entry_point__zero_argument_error`); no `.snap.new` or
  `.pending-snap` files remained. Subsequent runs without `INSTA_UPDATE`
  pass against the committed baselines:

  ```plaintext
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- `cargo test -p ortho_config --test rstest_bdd`
  (log: `/tmp/green-cargo-bdd.out`) runs both new scenarios
  (`cargo_entry_point_cargo_dispatch_invocation_parses_the_inner_options`,
  `cargo_entry_point_bare_invocation_without_the_injected_token_is_rejected`)
  plus the existing suite:

  ```plaintext
  test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

### Delivery notes

- Milestone 1 shipped as commit `9ca4adb` on top of the rebased branch
  (rebased onto `origin/main` on 2026-08-07); the maintenance pass folded in
  the two clippy fixes and the `CargoContext` relocation recorded above.
- Milestone 2 shipped as commit `6286f1c` (2026-08-09): the documentation
  sweep across `docs/users-guide.md`, `docs/design.md` §4.17,
  `docs/adr-004-cargo-external-subcommand-entry-point.md`, and
  `docs/developers-guide.md`, drafted by the `scribe` subagent. The
  `scrutineer` recorded all six green gate logs under `/tmp`.
- The branch was rebased onto `origin/main` again on 2026-08-09 to pick up
  the injectable environment-source feature (#410/#411) and the proc-macro2
  bump (#406). The post-rebase commit hashes listed above supersede the
  pre-rebase ones (`25be87a`, `04d548f`). The one-line conflict was
  `typos.local.toml`: main's `d3c9bdf` had already added the same inline-code
  exclusion pattern as a policy position, so the branch's separate
  "Pin the inline-code spelling exclusion locally" commit was dropped as
  empty after resolution and the pattern now comes from main. Two
  double-blank-line artefacts in `docs/users-guide.md` from the weave merge
  were fixed in the rebase-validation commit.

## Interfaces and dependencies

Final public surface added to `ortho_config`:

```rust
// ortho_config/src/cargo/mod.rs

/// Wraps a hand-built `clap::Command` in the standard Cargo
/// external-subcommand shape.
///
/// Cargo runs `cargo <name>` by executing `cargo-<name>` with the
/// subcommand name injected as the second argument. The returned command
/// models that protocol: a synthetic `cargo` parent that requires the
/// `<name>` subcommand, so both `cargo <name> [OPTIONS]` and
/// `cargo-<name> <name> [OPTIONS]` parse with the caller's original
/// options, which callers read through
/// `matches.subcommand_matches("<name>")`. The wrapper is command-line
/// entry-point structure only; it performs no configuration loading and
/// reads no environment state.
///
/// The inner command is renamed to `subcommand_name`, its `bin_name` is
/// reset so usage renders `cargo <name>`, and its `display_name` is set
/// to `installed_bin_name` so `--version` output names the installed
/// binary. All other inner options, help text, and the inner version are
/// preserved verbatim. The returned command is an ordinary
/// `clap::Command` that the caller may customize further. Styling,
/// top-level version flags, invocation-hint error text, and tracing setup
/// remain the caller's responsibility (see the users' guide).
///
/// # Preconditions
///
/// `installed_bin_name` must equal `"cargo-"` followed by
/// `subcommand_name` (Cargo derives the injected token from the binary
/// file name), and `subcommand_name` must be non-empty and must not be
/// `help` (reserved by clap's auto-generated help subcommand). Violations
/// are programming errors and trip debug assertions.
#[must_use]
pub fn external_subcommand(
    installed_bin_name: impl Into<String>,
    subcommand_name: impl Into<clap::builder::Str>,
    command: clap::Command,
) -> clap::Command;
```

Dependencies: clap 4.6 only (already a dependency with the `derive` and
`string` features). Dev-only additions: none (rstest, rstest-bdd, and insta
are already dev-dependencies of `ortho_config`). No new crates, no new
features, no changes to `ortho_config/Cargo.toml` expected; if one turns out
to be needed, Tolerance 3 applies.

## Revision note

Initial draft (2026-08-06): authored after a four-agent reconnaissance pass
(design documents; current `cargo-orthohelp` entry point; testing and
documentation conventions; external prior art via web research).

Revision 1 (2026-08-06): revised after a five-lens community-of-experts
design review (contracts, structure, alternatives, failure modes, viability;
scaling waived as inapplicable to a pure function). Material changes:

1. Corrected the `display_name` plumbing (D-2): the first draft set it on
   the synthetic parent, which two lenses independently showed corrupts
   subcommand version output to `cargo-demo-demo` in clap 4.6; it now goes
   on the inner command, giving `installed_bin_name` a real observable job,
   with the rendered `--version` string pinned in tests.
2. Added the missing zero-argument bare-invocation test and error-rendering
   snapshot (a different clap code path from the flag case), the inner
   `bin_name` reset contract, hedged error-kind assertions, nested-inner
   and required-argument cases, and the top-level `--version` divergence
   from the derive path (Risk 6).
3. Replaced the proptest milestone with table-driven rstest equivalence
   cases (D-5): the property held by construction and exercised clap, not
   this crate, contrary to the developers' guide's own rule.
4. Promoted the no-crate-root-re-export choice and the red-commit gating
   resolution to Decision D-8; added D-7 (name-consistency debug
   assertions, `#[must_use]`, per-sink parameter types) and D-9 (the
   argv-normalizer pattern recorded as out of scope, not rejected).
5. Consolidated five milestones to three post-approval milestones; spelled
   out the three-file BDD wiring; added the users'-guide hint-snippet
   deliverable, the README-deferral-to-8.3.2 note, the "Subcommand
   configuration" cross-reference, and the roadmap-tick annotation for the
   D-2 deviation.

The plan was approved on 2026-08-06 (Milestone 0c) and is now COMPLETE; see
the status line at the top of the document.

Revision 2 (2026-08-09): implementation complete. Milestones 1–3 all
delivered; this revision records the delivery in `Progress`, replaces the
placeholder `Outcomes & retrospective` with the delivery retrospective, and
updates the status line. The plan body (Decisions D-1–D-9, Constraints,
Tolerances) was unchanged throughout implementation — no deviation required
escalation under Tolerance 6 or 7; the two benign clap facts recorded as
implementation observations amended the verified-fact list only.
