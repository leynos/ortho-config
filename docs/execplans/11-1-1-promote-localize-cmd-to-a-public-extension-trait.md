# Promote `LocalizeCmd` to a public extension trait on `clap::Command`

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

Roadmap item: **11.1.1** (see [docs/roadmap.md](../roadmap.md) §11.1). Design
source: [cli-localization-design.md](../cli-localization-design.md) §4, §4.1,
§4.2, §10.

## Purpose / big picture

Today the load-bearing helper that translates a `clap` command tree lives in the
`hello_world` example, at `examples/hello_world/src/cli/localization.rs`.
Every application that wants localized command-line help copies that trait
verbatim. This plan promotes the helper into the library as a first-class,
documented public API so applications re-use it instead of copying it.

After this change a consumer can write:

```rust,ignore
use clap::CommandFactory;
use ortho_config::{LocalizeCmd, Localizer};

fn build(localizer: &dyn Localizer) -> clap::Command {
    // `with_base` names the catalogue root; `localize` walks the whole tree.
    MyCli::command().with_base("my_app.cli").localize(localizer)
}
```

Observable success is concrete and test-driven:

1. `ortho_config::LocalizeCmd`, `ortho_config::WithBase`, and
   `ortho_config::message_id_for` are public and documented.
2. The `hello_world` example deletes its local `LocalizeCmd` trait and
   re-exports the crate one through the existing path
   `hello_world::cli::LocalizeCmd`, and every existing example test still
   passes unchanged in assertion content.
3. `make check-fmt`, `make typecheck`, `make lint`, and `make test` all pass,
   including doctests (`cargo test --doc`).

This plan covers **only** 11.1.1: the promoted `LocalizeCmd` trait (widened to
the full clap surface), `LocalizeCmd::with_base`, and the public
`message_id_for` function. The blanket `LocalizedParse` trait (11.1.2), the
`OrthoConfigLocalization` derive (11.1.3), the widened clap-error matrix
(11.2), and the `BootLocalizer`/`BootHandle` lifecycle (11.3) are explicitly
out of scope and are named where they touch this work.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

1. **Domain boundary.** `ortho_config` must not depend on the `hello_world`
   example or on any application crate. The promoted code moves *into*
   `ortho_config`; the example depends on the crate, never the reverse. No
   circular workspace dependency may be introduced.
2. **Additive, non-destructive localization.** A missing catalogue key must
   leave clap's stock value untouched (the localizer returns `None` → the
   setter is not called). The walker must never reset a field to clap's
   default; it only overwrites on a `Some` lookup. This preserves §10 backward
   compatibility.
3. **Byte-for-byte identifier agreement.** Identifiers produced by
   `message_id_for` must, after the existing load-time normalization
   (`normalize_resource_ids`), match the keys in the shipped catalogues
   (`examples/hello_world/src/locales/en-US/messages.ftl` and
   `.../ja/messages.ftl`). The existing example tests are the regression gate
   for this.
4. **Public path stability for the deprecation window.** The path
   `hello_world::cli::LocalizeCmd` must keep resolving (now to the re-exported
   crate trait) so the `localizer.rs` doctest and `cli/tests/localisation.rs`
   keep compiling for one release (§10: collapse to a re-export in 0.9, remove
   in 0.10).
5. **No flag corruption.** Rewriting `value_name` on a `clap::Arg` implicitly
   sets `ArgAction::Set` (see Surprises). The walker must only set `value_name`
   on arguments that already take a value; it must never change a flag's action.
6. **clap API reality.** `clap::Command` has no `get_arguments_mut`.
   Per-argument edits go through `Command::mut_arg(id, f)`; child commands are
   walked via `get_subcommands()` (read) + `find_subcommand_mut` +
   `std::mem::take`. See Surprises and the Interfaces section.
7. **File size.** No code file may exceed 400 lines (AGENTS.md). The new
   `localizer/clap_command.rs` and any `identifier` submodule must respect
   this; split helpers if needed.
8. **Spelling and style.** Comments and docs use en-GB-oxendict spelling, except
   references to external APIs. Documentation follows
   [documentation-style-guide.md](../documentation-style-guide.md).

## Tolerances (exception triggers)

Stop and escalate (record in `Decision Log`, await direction) when any of these
is breached.

1. **Scope.** If the implementation requires touching more than ~12 files or
   more than ~600 net lines of non-test code, stop and escalate.
2. **Public API shape.** The trait/function signatures in the Interfaces section
   are the contract. If implementation forces a *different* public signature
   (for example `message_id_for` must return `Result` rather than panic, or the
   `WithBase` wrapper proves unworkable), stop and escalate — this changes the
   ADR.
3. **New runtime dependency.** Adding any non-dev dependency to `ortho_config`
   is out of tolerance. Adding the `proptest` **dev**-dependency is in scope
   and expected.
4. **Catalogue churn.** If making the example pass would require editing the
   shipped `.ftl` catalogue keys (rather than wiring `with_base`), stop: that
   is a breaking change to translators and needs sign-off.
5. **Iterations.** If a milestone's tests still fail after 3 focused attempts,
   stop and escalate with the transcript.
6. **Cross-crate ripple.** If promoting the trait forces changes to
   `ortho_config_macros`, `cargo-orthohelp`, or `test_helpers` beyond a
   re-export, stop — that signals scope creep into 11.1.3.

## Risks

1. Risk: **Identifier path divergence.** The example's old `message_id` omits
   the root command segment (it hard-codes `hello_world.cli` and joins only
   subcommand names), whereas the design's `<base> ::= <root> <segment>...`
   includes the normalized binary name. A faithful `message_id_for` therefore
   emits `hello-world-greet-about`, not the catalogue's
   `hello_world-cli-greet-about`. Severity: high. Likelihood: high (certain if
   unaddressed). Mitigation: the example calls `with_base("hello_world.cli")`
   so the derived ids reproduce the existing keys. The existing example tests
   are the gate.
2. Risk: **Underscore normalization contradicts the design prose.** §4.1 says
   "underscores become hyphens", but the shipped keys preserve `_`
   (`hello_world-cli-about`). If `message_id_for` converts `_`→`-`, every
   lookup misses. Severity: high. Likelihood: high. Mitigation: preserve `_`
   (it is a legal Fluent character); amend the §4.1 prose as Milestone 0; add a
   byte-for-byte agreement test. See Decision Log.
3. Risk: **Doctest breakage.** The rustdoc doctest on `DemoLocalizer::new`
   (`examples/hello_world/src/localizer.rs:55-63`) calls
   `.localize(&localizer)` without a base and asserts `get_about().is_some()`.
   After promotion the default base (binary name `hello-world`) will not
   resolve the `hello_world.cli.*` keys, so the doctest fails under
   `cargo test --doc`. Severity: medium. Likelihood: high. Mitigation: update
   the doctest to `.with_base("hello_world.cli").localize(...)`; gate
   `cargo test --doc` in acceptance.
4. Risk: **`value_name` action side-effect** corrupts flags (Constraint 5).
   Severity: high. Likelihood: medium. Mitigation: guard on a takes-value
   predicate captured from the read-only `&Arg`; explicit unit test asserting a
   `SetTrue` flag keeps its action.
5. Risk: **Dead-code lint-as-error.** Removing the example's helpers orphans
   `CLI_GREET_ABOUT_MESSAGE_ID` / `CLI_TAKE_LEAVE_ABOUT_MESSAGE_ID` (currently
   unused) and possibly others; `make lint` denies warnings. Severity: low.
   Likelihood: medium. Mitigation: remove genuinely unused consts; retain only
   those the in-file tests reference.
6. Risk: **Load-time vs derive-time normalizer drift.** `normalize_resource_ids`
   currently *tolerates* non-ASCII ids (there is a Cyrillic test in
   `fluent.rs`). A strict `message_id_for` and a tolerant load path can
   disagree on exotic input. Severity: low (no shipped catalogue uses non-ASCII
   keys). Likelihood: low. Mitigation: keep the tolerant load path unchanged
   for 11.1.1; only correct the misleading `is_valid_fluent_id_char` doc
   comment; record full unification as a deferred open question (do **not**
   delete the Cyrillic test).
7. Risk: **Panic as public-API contract.** `message_id_for` and `localize` panic
   on unrepresentable / colliding command paths. Reviewers flagged this as a
   hard-to-reverse choice. Severity: medium. Likelihood: low (inputs are
   compile-time-fixed command trees). Mitigation: this is mandated by §4.1 and
   the roadmap; record ADR-006 capturing the decision and the rejected `Result`
   alternative. See Decision Log.

## Progress

- [x] (2026-06-11) Milestone 0 — design-doc reconciliation (§4.1 prose,
  ADR-006). Updated `docs/cli-localization-design.md`, added
  `docs/adr-006-identifier-derivation-panics.md`, indexed ADR-006 in
  `docs/contents.md`, and began implementation status tracking.
- [x] (2026-06-11) Milestone 1 — `normalize_segment` + `message_id_for`
  (red→green, proptest). Added `ortho_config/src/localizer/identifier.rs`,
  exported `message_id_for`, added table and property tests, and added the
  `proptest` dev-dependency to `ortho_config`.
- [x] (2026-06-11) Milestone 2 — `LocalizeCmd` trait, `WithBase`, walker
  (red→green). Added `ortho_config/src/localizer/clap_command/mod.rs`,
  recursive command and argument localization, `with_base`, `localize_self`,
  collision checks, and tests for flag-action preservation.
- [ ] (pending) Milestone 3 — crate re-exports and `fluent.rs` doc-comment fix.
- [ ] (pending) Milestone 4 — collapse the example onto the promoted trait.
- [ ] (pending) Milestone 5 — snapshot + BDD acceptance; docs; final gates.

Each milestone ends with `make check-fmt typecheck lint test` (via `tee` logs)
and a CodeRabbit review (`coderabbit review --agent`) with all concerns cleared
before the next milestone.

## Surprises & discoveries

Recorded upfront from the planning research so the implementer does not
rediscover them.

1. Observation: **`clap::Command::get_arguments_mut()` does not exist** in clap
   4.x (repo pins 4.5.60). Evidence: clap docs.rs `Command` API; the only
   `*_mut` accessors are `find_subcommand_mut`, `get_subcommands_mut`,
   `get_matches_mut`, `try_get_matches_from_mut`, `borrow_mut`. `Arg` exposes no
   `get_*_mut` setters. Impact: per-argument edits must use
   `Command::mut_arg(id, |arg| ...)` (closure takes an owned `Arg`, returns
   `Arg`). `mut_arg` panics on an unknown id, so ids must be enumerated
   up-front via the read-only `get_arguments()`.
2. Observation: **`Arg::value_name` implicitly sets `ArgAction::Set`.**
   Evidence: clap `Arg::value_name` docs. Impact: Constraint 5 — never call
   `value_name` on a flag; guard on a takes-value predicate.
3. Observation: **`Command::override_usage` has no getter.**
   Evidence: clap `Command` API (no `get_override_usage`). Impact: the walker
   can only overwrite usage wholesale on a `Some` lookup; it cannot
   read-modify-translate. The `<base>-usage` key is a full-string replacement
   contract.
4. Observation: **`mut_args` / `mut_subcommands` do not affect the
   auto-generated `--help` / `--version` arguments.** Evidence: clap docs.
   Impact: localizing the built-in flag descriptions is out of scope for 11.1.1
   (would require `disable_help_flag` + custom args). Deferred and documented.
5. Observation: **`get_subcommands_mut` is shallow** (immediate children only).
   Evidence: clap docs. Impact: recurse manually; because clap setters consume
   `self` and a `&mut Command` cannot call them directly, pull each child out
   with `std::mem::take`, localize by value, and write it back via
   `find_subcommand_mut`.
6. Observation: **Fluent identifiers are ASCII-only:**
   `Identifier ::= [a-zA-Z] [a-zA-Z0-9_-]*`; `.` is a structural attribute
   separator, not an id character. Evidence: the canonical Fluent EBNF
   (`projectfluent/fluent/spec/fluent.ebnf`). Impact: the crate's
   `is_valid_fluent_id_char` doc comment (claiming Unicode and `.` are legal)
   is wrong and is corrected. `_` *is* legal, which is why the catalogues use
   it and `message_id_for` preserves it.
7. Observation: **`hello_world.cli` base mismatch.** The binary name is
   `hello-world`; the catalogue base is `hello_world.cli`. The default base
   derivation cannot reproduce the example's keys. Evidence:
   `examples/hello_world/src/cli/mod.rs` (`name = "hello-world"`) vs the
   `CLI_BASE_MESSAGE_ID = "hello_world.cli"` consts. Impact: the example must
   use `with_base("hello_world.cli")`; this is the canonical "multi-segment
   catalogue" demonstration of `with_base`.
8. Observation: **ADR location follows the existing flat docs convention.**
   Evidence: `docs/contents.md` indexes ADR-001 through ADR-005 at
   `docs/adr-00x-...md`, not under `docs/adr/`. Impact: ADR-006 was added as
   `docs/adr-006-identifier-derivation-panics.md` to match the repository's
   current convention despite the generic ADR skill mentioning `docs/adr/`.
9. Observation: **CodeRabbit can wedge during sandbox preparation.**
   Evidence: the second Milestone 0 review stayed at
   `{"phase":"setup","status":"preparing_sandbox"}` for about 18 minutes after
   the two trivial ADR concerns were fixed. Impact: the stuck review process
   was terminated and the review was retried cleanly; this did not bypass the
   review gate.
10. Observation: **Empty command paths need an explicit root guard.**
    Evidence: the first Milestone 1 green attempt let
    `message_id_for(&[], "about")` produce `about`, because the suffix supplied
    the leading ASCII letter.
    Impact: `message_id_for` now panics before suffix handling when
    `command_path` is empty, preserving the contract that `command_path[0]` is
    the root.
11. Observation: **Suffix splitting must be counted before preallocation.**
    Evidence: CodeRabbit flagged that `Vec::with_capacity(command_path.len() +
    1)` under-allocated for suffixes such as `args.reason.help`.
    Impact: `message_id_for` now reuses one `suffix.split('.')` iterator and
    counts its clone for exact capacity before extending the segment vector.
12. Observation: **Consecutive suffix dots are an empty-segment edge case.**
    Evidence: CodeRabbit requested explicit coverage for `args..help`, which
    reaches the empty segment path after suffix splitting.
    Impact: `message_id_for_rejects_unrepresentable_segments` now documents
    that consecutive dots panic with `invalid Fluent identifier segment`.
13. Observation: **Dynamic clap version and value-name strings need the
    `string` feature.**
    Evidence: the first Milestone 2 green attempt failed because
    `Command::version`, `Command::long_version`, and `Arg::value_name` did not
    accept `String` without clap's `string` feature.
    Impact: the existing `clap` dependency now enables `string` alongside
    `derive`; no new runtime dependency was added.
14. Observation: **A test can accidentally trigger the `value_name` side
    effect it is meant to guard.**
    Evidence: the initial flag fixture set `.value_name(...)` on a
    `SetTrue` flag, which would itself change clap's action before the walker
    ran.
    Impact: the flag fixture now omits `value_name`, and the test asserts that
    localization does not add one while preserving `ArgAction::SetTrue`.
15. Observation: **Clippy requires directory modules for self-named modules.**
    Evidence: the first Milestone 2 `make lint` run failed with
    `clippy::self-named-module-files` for
    `ortho_config/src/localizer/clap_command.rs`.
    Impact: `clap_command` now uses `clap_command/mod.rs` plus
    `clap_command/tests.rs`, keeping both files under 400 lines while matching
    the repository lint policy.
16. Observation: **Milestone 2 CodeRabbit setup can repeat the sandbox wedge.**
    Evidence: three post-gate CodeRabbit attempts for Milestone 2 stayed at
    `{"phase":"setup","status":"preparing_sandbox"}` with no findings or
    rate-limit output before being terminated; later attempts advanced after a
    long setup phase.
    Impact: only the stuck review processes were stopped; deterministic gates
    remained green, and the review gate was kept open until CodeRabbit returned
    zero findings.
17. Observation: **Builder-style `must_use` needs messages.**
    Evidence: Clippy rejected plain `#[must_use]` on `with_base` because the
    return type is already `#[must_use]`, while CodeRabbit requested caller
    warnings for discarded builder values.
    Impact: `with_base` now uses message-bearing `#[must_use = "..."]`
    attributes so both the lint policy and API ergonomics are satisfied.

## Decision log

- Decision: **Preserve `_` in normalized identifiers; amend the design-doc prose
  rather than re-key the catalogues.** Rationale: `_` is a legal Fluent
  character and the shipped en-US/ja catalogues plus `normalize_resource_ids`
  already rely on it. Converting `_`→`-` would break every translator. Amending
  §4.1 prose is documentation-only and lower risk. Date/Author: 2026-06-09,
  planning agent (pending user approval).
- Decision: **`message_id_for` and `LocalizeCmd::localize` panic** on
  unrepresentable segments and on identifier collisions, rather than returning
  `Result`. Rationale: §4.1 and the roadmap explicitly mandate
  "panic-on-collision behaviour" / "runtime panic in `LocalizeCmd::localize`".
  Inputs are compile-time-fixed command trees authored by the binary developer,
  so a colliding/unrepresentable id is a build-time bug surfaced at first run,
  mirroring clap's own `mut_arg` panic-on-undefined-id. The reviewers' `Result`
  alternative is recorded as rejected in **ADR-006** so the trade-off is
  auditable; if a downstream consumer needs recoverability later, a
  `try_message_id_for -> Result` can be added without breaking the panicking
  function. Date/Author: 2026-06-09, planning agent (pending user approval and
  ADR-006 sign-off).
- Decision: **Keep the example single-phase; do not wire `BootLocalizer`.**
  Rationale: `BootLocalizer`/`BootHandle` are 11.3 deliverables and do not
  exist yet. 11.1.1 keeps the example's current
  `Self::command().with_base(...).localize(localizer)` shape and adds a doc
  note pointing at §5.3 for the full two-phase lifecycle. Date/Author:
  2026-06-09, planning agent.
- Decision: **Leave `normalize_resource_ids` tolerant** of non-ASCII ids for
  11.1.1; only correct the `is_valid_fluent_id_char` doc comment and introduce
  a strict `normalize_segment` for `message_id_for`. Rationale: unifying both
  onto one strict normalizer would break the existing Cyrillic-id load test and
  risk silently dropping consumer catalogue entries. Full unification is a
  separate, breaking-risk change recorded as an open question. Date/Author:
  2026-06-09, planning agent.
- Decision: **Leading-`[a-zA-Z]` check is a whole-id invariant**, enforced after
  joining segments, not per-segment. Rationale: a middle segment of `123` is
  legal; only the final id's first character must be a letter. Keeping
  `normalize_segment` pure (no composition-aware panic) lets the derive
  (11.1.3) and load path reuse it. Date/Author: 2026-06-09, planning agent.
- Decision: **Keep ADR-006 in the flat `docs/` ADR series.**
  Rationale: the repository's accepted ADRs and documentation index already use
  flat `docs/adr-00x-...md` paths. Matching that local convention preserves
  discoverability and avoids an unnecessary documentation-layout change.
  Date/Author: 2026-06-11, implementation agent.
- Decision: **Merge the ADR Y-Statement into the canonical outcome section.**
  Rationale: CodeRabbit flagged a standalone `Y-Statement` heading as
  non-standard for this repository's ADR format. The Y-Statement wording now
  lives under `Decision outcome / proposed direction`, preserving the
  architectural decision-record requirement without adding a duplicate section.
  Date/Author: 2026-06-11, implementation agent.
- Decision: **Implement identifier derivation in a dedicated
  `localizer::identifier` module.** Rationale: the helper is pure and will be
  reused by the command-tree walker, derive work, and documentation tests.
  Keeping it out of `fluent.rs` avoids coupling strict derive-time identifier
  rules to the intentionally tolerant load-time catalogue normalizer.
  Date/Author: 2026-06-11, implementation agent.
- Decision: **Enable clap's `string` feature for `ortho_config`.**
  Rationale: the promoted walker receives owned localized `String` values from
  `Localizer`. Clap's dynamic `version`, `long_version`, and `value_name`
  setters accept those owned values only when the existing dependency enables
  `string`. This keeps the public API additive without leaking static-string
  requirements onto localizer implementations. Date/Author: 2026-06-11,
  implementation agent.

## Outcomes & retrospective

Milestone 0 is complete. The design document now preserves underscores in
runtime Fluent ids, documents the 11.1.1 non-localized clap surfaces, and
references ADR-006 for the public panic contract. The promoted trait is not yet
implemented; Milestone 1 begins the red-green identifier helper work.

Milestone 1 is complete through focused validation. The red command
`cargo test -p ortho_config localizer::tests::message_id_for` failed with
`cannot find function message_id_for in this scope`. After implementation,
`cargo test -p ortho_config localizer::tests` passed 29 localizer tests,
including table tests, panic tests, and three proptest properties.

After moving the identifier tests into `localizer::identifier` to keep code
files under 400 lines, the focused command
`cargo test -p ortho_config localizer::identifier::tests` passed 14 tests. The
Milestone 1 gates `make check-fmt`, `make typecheck`, `make lint`, `make test`,
and `make markdownlint` passed, and CodeRabbit reported zero findings after the
suffix-capacity and consecutive-dot fixes.

Milestone 2 is complete through focused validation. The red command
`cargo test -p ortho_config localizer::clap_command::tests` failed with missing
`with_base` / `localize` / `localize_self` methods. After implementation and
enabling clap's `string` feature, the same command passed three focused tests:
recursive localization, non-recursive `localize_self`, and subcommand collision
panic coverage.

After Clippy required the directory module shape, the walker was moved to
`localizer/clap_command/mod.rs`, tests stayed in
`localizer/clap_command/tests.rs`, and helper context was grouped to satisfy
the argument-count lint. Milestone 2 gates `make check-fmt`, `make typecheck`,
`make lint`, `make test`, and `make markdownlint` passed before CodeRabbit
review. Three CodeRabbit attempts then wedged in sandbox preparation without
findings or rate-limit output. A later review found missing builder
`must_use` warnings, panic docs, and an Oxford spelling issue. After fixes and
full gates, CodeRabbit requested explicit argument-collision coverage; that
test was added, full gates were rerun, and CodeRabbit requested two clarifying
comments plus one ownership comment for the subcommand take-and-replace path.
After those comments and another full gate pass, CodeRabbit reported zero
findings.

## Context and orientation

The reader is assumed to know nothing about this repository. Key facts:

- This is a Cargo workspace. Members:
  `ortho_config` (the library), `ortho_config_macros` (the derive),
  `examples/hello_world`, `cargo-orthohelp`, `test_helpers`,
  `tests/fixtures/orthohelp_fixture`. Workspace version is `0.8.0`; the
  promoted helpers ship in `0.9` (§10).
- **Localization lives in `ortho_config/src/localizer/`.** `mod.rs` defines the
  object-safe `Localizer` trait (`lookup(&self, id, args) -> Option<String>`),
  `NoOpLocalizer`, `FluentLocalizer`, and re-exports the clap-error helpers.
  `fluent.rs` holds `normalize_identifier` (dots→hyphens, lookup-time),
  `normalize_resource_ids` (load-time), and the `is_valid_fluent_id_char` /
  `is_valid_fluent_identifier` helpers. `clap_error.rs` maps `clap::ErrorKind`
  to Fluent ids.
- **The thing being promoted** is
  `examples/hello_world/src/cli/localization.rs`: a `LocalizeCmd` trait with
  one method `localize(self, &dyn Localizer) -> Self`, an
  `impl LocalizeCmd for clap::Command`, and private helpers
  `localize_command_tree`, `apply_command_metadata`, `localization_args_for`,
  `base_message_id_for_suffix`, and `message_id`. It currently covers only
  `about`, `long_about`, and `override_usage`, and omits the root segment from
  the id path.
- **Where the example wires it:** `examples/hello_world/src/cli/mod.rs:30`
  re-exports the local `LocalizeCmd`; `try_parse_localized` (around line 111)
  calls `Self::command().localize(localizer)`. The message-id constants
  (`CLI_BASE_MESSAGE_ID = "hello_world.cli"`, `CLI_ABOUT_MESSAGE_ID`, …) live in
  `examples/hello_world/src/localizer.rs`.
- **Catalogues** are `examples/hello_world/src/locales/en-US/messages.ftl` and
  `.../ja/messages.ftl`, keyed with dotted ids such as `hello_world.cli.about`,
  normalized at load time to `hello_world-cli-about`.
- **Crate re-exports** are in `ortho_config/src/lib.rs` around lines 118-122
  (`pub use localizer::{ ... }`).

Terms used in this plan:

- **Identifier / id:** a Fluent message key, e.g. `hello_world-cli-greet-about`.
- **Segment:** one component of a command path (a command name) or of a base.
- **Base / `<root>`:** the leading namespace of an id, set by `with_base` or
  derived from the binary name.
- **Walker:** the recursive routine that rewrites a `clap::Command` tree.

## Interfaces and dependencies

Be prescriptive. At the end of this work the following must exist.

New module `ortho_config/src/localizer/clap_command.rs` (re-exported from
`localizer/mod.rs` and the crate root):

```rust,ignore
/// Extension trait that applies a `Localizer` to a `clap::Command` tree.
pub trait LocalizeCmd: Sized {
    /// Apply the localizer to this command and every subcommand recursively.
    #[must_use]
    fn localize(self, localizer: &dyn Localizer) -> Self;

    /// Apply the localizer to this command only (no recursion). Intended for
    /// re-rendering a single node after subcommand selection.
    #[must_use]
    fn localize_self(self, localizer: &dyn Localizer) -> Self;

    /// Override the `<root>` segment(s) used to derive identifiers for this
    /// tree. Accepts a human-facing dotted base (e.g. `"my_app.cli"`), split on
    /// `'.'` into raw segments and normalized at lookup time.
    #[must_use]
    fn with_base(self, base: impl Into<String>) -> WithBase<Self>;
}

impl LocalizeCmd for clap::Command { /* default root = bin name or name */ }

/// Carrier returned by `LocalizeCmd::with_base`, command-specific.
#[must_use]
pub struct WithBase<C> { /* command: C, base: Vec<String> */ }

impl WithBase<clap::Command> {
    #[must_use] pub fn localize(self, localizer: &dyn Localizer) -> clap::Command;
    #[must_use] pub fn localize_self(self, localizer: &dyn Localizer) -> clap::Command;
    /// Replaces the base (idempotent override; does not accumulate).
    #[must_use] pub fn with_base(self, base: impl Into<String>) -> Self;
}
```

Public free function (in `clap_command.rs` or a sibling `identifier.rs`,
re-exported from the crate root):

```rust,ignore
/// Builds the Fluent identifier for a command path and suffix.
///
/// `command_path[0]` is `<root>`; later elements are command-name segments.
/// `suffix` is the leaf token (`"about"`, `"long_about"`, `"usage"`,
/// `"version"`, `"long_version"`, `"after_help"`, `"after_long_help"`, or an
/// argument path such as `"args.<arg-id>.help"`). Each segment is normalized
/// independently and joined with `-`.
///
/// # Panics
/// Panics if a segment contains a character outside `[A-Za-z0-9_-]` after
/// lowercasing, or if the final id does not start with an ASCII letter. These
/// are unrepresentable-identifier programmer errors. Collision detection is
/// **not** performed here (the function cannot see sibling ids); the walker
/// panics on collisions.
pub fn message_id_for(command_path: &[impl AsRef<str>], suffix: &str) -> String;
```

Supporting (crate-internal) helper, factored so the derive (11.1.3) and the
load path can reuse it:

```rust,ignore
/// Normalizes one raw segment to Fluent-legal ASCII: ASCII letters lowercased
/// and passed through; ASCII digits, '-' and '_' passed through; any other
/// character panics. Pure and composition-agnostic (the leading-letter rule is
/// a whole-id invariant enforced by `message_id_for`, not here).
pub(crate) fn normalize_segment(raw: &str) -> String;
```

Identifier grammar (final, ASCII-only — corrects §4.1):

```plaintext
<root>        ::= command_path[0] (binary name or with_base override)
<segment>     ::= each later command name, kebab-case
<base-id>     ::= <root> ("-" <segment>)*
<command id>  ::= <base-id> "-" ("about"|"long_about"|"usage"|"version"
                              |"long_version"|"after_help"|"after_long_help")
<argument id> ::= <base-id> "-args-" <arg-id> "-" ("help"|"long_help"|"value_name")
```

The author-facing FTL form uses `.` separators (`hello_world.cli.about`); the
load-time `normalize_resource_ids` rewrites `.`→`-`, so derive output,
`message_id_for` output, and hand-authored FTL agree after load.

Dependencies: add `proptest` to `ortho_config`'s `[dev-dependencies]` only.
`rstest`, `rstest-bdd`, `rstest-bdd-macros`, and `insta` are already present in
`ortho_config`'s dev-deps; `proptest` is absent and must be added (verify with
a dry `cargo add --dev proptest -p ortho_config --dry-run`). No runtime
dependency changes.

## Plan of work

Staged, each stage ending in validation. Do not advance past a failing stage.

### Milestone 0 — Reconcile the design doc (no code)

1. Amend [cli-localization-design.md](../cli-localization-design.md) §4.1 prose:
   replace "underscores become hyphens" with wording that states underscores
   are preserved (a legal Fluent character per the EBNF
   `[a-zA-Z][a-zA-Z0-9_-]*`); only `.` and out-of-grammar characters are
   transformed or rejected. Show both the author-facing dotted form and the
   final hyphenated id form side by side.
2. Add a short "What is not localized in 11.1.1" note to §4 (built-in
   `--help`/`--version` argument text; per-`PossibleValue` help; the
   `help_template` layout string), with a pointer that the footer slots
   (`after_help`/`after_long_help`) *are* localized.
3. Author **ADR-006** (`docs/adr-006-identifier-derivation-panics.md`) in
   Y-Statement form (use the `arch-decision-records` skill): "In the context of
   deriving Fluent identifiers from compile-time-fixed clap command trees,
   facing the need to surface unrepresentable or colliding ids, we decided to
   panic (matching clap's `mut_arg` convention and the §4.1 mandate) and
   neglected a `Result`-returning API, accepting that hand-built dynamic trees
   must validate names before localizing, because the inputs are
   developer-authored constants surfaced at first run." Reference ADR-006 from
   the design doc and from `message_id_for`'s rustdoc. Add it to
   [contents.md](../contents.md).
4. Validation: `make markdownlint` passes; the design doc no longer contradicts
   the planned code.

Go/no-go: Milestone 0 is documentation only and de-risks the contentious
decisions before any code lands.

### Milestone 1 — `normalize_segment` + `message_id_for` (Red → Green)

1. **Red.** Add `ortho_config/src/localizer/tests.rs` cases (or a new
   `identifier` test submodule) for the not-yet-existing `message_id_for`. Use
   `rstest` `#[case]`s for the happy path, `#[should_panic]` for
   unrepresentable input, and a byte-for-byte agreement assertion against
   `normalize_resource_ids`. Confirm they fail to compile / fail for the
   expected reason.
2. **Green.** Implement `normalize_segment` and `message_id_for` (split `suffix`
   on `.` into trailing segments; normalize each segment; join with `-`;
   enforce the leading-`[a-zA-Z]` whole-id invariant). Re-run; expect green.
3. Add `proptest` dev-dep and the invariant tests (validity regex, idempotence,
   collision-detectability at the helper level).
4. **Refactor.** Keep `normalize_segment` pure and under the 400-line limit.
5. Validation: `make check-fmt typecheck lint test` (with `tee` logs); then
   `coderabbit review --agent`, clearing all concerns.

### Milestone 2 — `LocalizeCmd` trait, `WithBase`, and the walker (Red → Green)

1. **Red.** In a `clap_command` test submodule, build a hand-rolled
   `clap::Command` (root + one subcommand + one value-bearing arg + one
   `SetTrue` flag) and drive it with a stub `Localizer` that echoes the
   requested id. Assert: root `about`/`long_about`/`usage` set; subcommand
   `about` set (recursion); the value-bearing arg's `help`/`value_name` set;
   the flag's `value_name` *not* set and its action still `SetTrue`; `version`
   untouched when no version id is echoed and overwritten when it is;
   `after_help` set; `localize_self` leaves the subcommand at stock values. Add
   a `#[should_panic]` collision test. These use a hand-built command, keeping
   `ortho_config` free of the example (Constraint 1).
2. **Green.** Implement the trait, `WithBase`, and the walker per the Interfaces
   and the clap-API Surprises: command-level setters applied by value;
   per-argument edits via `mut_arg` with the takes-value guard; children pulled
   via `get_subcommands()` + `find_subcommand_mut` + `std::mem::take`;
   per-parent `HashSet<String>` collision panic naming both raw segments and
   the shared id.
3. **Refactor.** Extract `apply_command_metadata` / `apply_arg_metadata` /
   `localize_command` to keep files small and single-responsibility.
4. Validation: gates + CodeRabbit, all concerns cleared.

### Milestone 3 — Crate re-exports and `fluent.rs` correction

1. Add `mod clap_command;` and `pub use clap_command::{LocalizeCmd, WithBase};`
   plus `pub use ...::message_id_for;` to `localizer/mod.rs`.
2. Extend the `ortho_config/src/lib.rs` localizer re-export block (lines
   118-122) with `LocalizeCmd`, `WithBase`, and `message_id_for`.
3. Correct the `is_valid_fluent_id_char` doc comment in `fluent.rs` to state the
   ASCII-only grammar and to stop claiming `.` is legal; adjust its unit tests
   to assert a non-ASCII char and `.` are rejected by the *strict* path while
   the tolerant `normalize_resource_ids` load behaviour (including the Cyrillic
   test) is retained unchanged (Decision Log / Risk 6).
4. Validation: gates + CodeRabbit.

### Milestone 4 — Collapse the example onto the promoted trait

1. In `examples/hello_world/src/cli/localization.rs`, delete the local
   `LocalizeCmd` trait, its impl, and the private helpers
   (`localize_command_tree`, `apply_command_metadata`, `localization_args_for`,
   `base_message_id_for_suffix`, `message_id`). Keep `localize_parse_error` as
   a thin shim over `ortho_config::localize_clap_error_with_command`, or inline
   it at its single call site in `cli/mod.rs`.
2. In `examples/hello_world/src/cli/mod.rs`, change the line-30 re-export to
   `pub use ortho_config::LocalizeCmd;` (preserving the public path
   `hello_world::cli::LocalizeCmd`), and change `try_parse_localized` to
   `Self::command().with_base("hello_world.cli").localize(localizer)`.
3. In `examples/hello_world/src/localizer.rs`: update the `DemoLocalizer::new`
   doctest (lines 55-63) to
   `.with_base("hello_world.cli").localize(&localizer)`. Retain the `CLI_*`
   consts the in-file tests reference (`CLI_ABOUT_MESSAGE_ID`,
   `CLI_LONG_ABOUT_MESSAGE_ID` at lines 221/232/290); remove the genuinely
   unused `CLI_GREET_ABOUT_MESSAGE_ID` / `CLI_TAKE_LEAVE_ABOUT_MESSAGE_ID` to
   avoid dead-code lint-as-error (Risk 5).
4. Validation: the existing `examples/hello_world/src/cli/tests/localisation.rs`
   and the `localizer.rs` inline tests + doctest pass unchanged in assertion
   content (this is the byte-for-byte id-agreement gate, Constraint 3). Run
   `cargo test --doc -p hello_world` explicitly. Gates + CodeRabbit.

### Milestone 5 — Acceptance, snapshots, docs, final gates

1. Add an `insta` snapshot test in `examples/hello_world` rendering
   `CommandLine::command().with_base("hello_world.cli").localize(&loc)` long
   help for three variants — en-US `DemoLocalizer`, `ja` `DemoLocalizer`, and
   `NoOpLocalizer` — through an insta filter that strips clap version/ANSI
   noise. Three named snapshots prove translated copy appears, ja copy appears,
   and the no-op output equals stock clap.
2. Add an `rstest-bdd` scenario (`examples/hello_world`) — feature "Localised
   CLI help": Given locale `ja`, When the user renders `hello-world --help`,
   Then the about line contains the ja copy and the greet subcommand help
   contains the ja greeting; And Given locale `en-US`, the about contains the
   en-US copy. Embed the feature text in this plan (below) and keep it
   synchronized.
3. Documentation:
   - [users-guide.md](../users-guide.md) §"Localizing CLI copy": document
     `LocalizeCmd`, `with_base`, `message_id_for`, the identifier convention, and
     a DON'T/DO base-mismatch example.
   - [developers-guide.md](../developers-guide.md): record the
     `normalize_segment` ownership/reuse policy (single source of truth for
     derive-time and lookup-time ids) per the AGENTS.md abstraction policy.
   - The `localizer` component architecture notes in
     [design.md](../design.md): document the walker's clap-API constraints
     (no `get_arguments_mut`; `value_name` action guard; shallow
     `get_subcommands_mut`) and the per-parent collision scope.
4. Mark roadmap item 11.1.1 done in [docs/roadmap.md](../roadmap.md).
5. Final validation: `make check-fmt typecheck lint test markdownlint nixie`
   (i.e. `make all`) via `tee`; `cargo test --doc`; a final
   `coderabbit review --agent` with all concerns cleared.

BDD feature specification (to embed at
`examples/hello_world/tests/features/localised_help.feature` and keep in sync):

```plaintext
Feature: Localised CLI help
  Scenario: Japanese locale localises the command tree
    Given the user's locale is "ja"
    When the user renders the hello-world long help
    Then the about line contains the Japanese greeting copy
    And the greet subcommand help contains the Japanese greeting copy

  Scenario: English locale localises the command tree
    Given the user's locale is "en-US"
    When the user renders the hello-world long help
    Then the about line contains the English localised copy
```

## Concrete steps

Run from the repository root
(`/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/91b9b87e-f134-4f9b-b42a-769ba714c237`).

Gating (run after each milestone; capture with `tee` per the global command
guidance):

```bash
# Action ∈ {check-fmt, typecheck, lint, test}
make check-fmt 2>&1 | tee "/tmp/check-fmt-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-$(git branch --show-current).out"
```

Doctest gate (the known breakage point, Risk 3):

```bash
cargo test --doc -p ortho_config 2>&1 | tee /tmp/doctest-crate.out
cargo test --doc -p hello_world  2>&1 | tee /tmp/doctest-example.out
```

Add the dev-dependency (Milestone 1):

```bash
cargo add --dev proptest -p ortho_config --dry-run   # verify, then drop --dry-run
```

Expected transcript shape for the example regression gate (Milestone 4): the
existing localisation tests pass without assertion edits, e.g.

```plaintext
test cli::tests::localisation::command_with_localizer_overrides_copy ... ok
test cli::tests::localisation::localizes_subcommand_tree ... ok
test cli::tests::localisation::try_parse_with_localizer_localises_errors ... ok
```

CodeRabbit (after gates are green, before advancing a milestone):

```bash
coderabbit review --agent 2>&1 | tee "/tmp/coderabbit-$(git branch --show-current).out"
```

## Validation and acceptance

Acceptance is behavioural:

1. **New public API exists and is documented.** `cargo doc -p ortho_config`
   builds; `ortho_config::{LocalizeCmd, WithBase, message_id_for}` are public.
2. **Red-Green evidence.** Milestone 1: `message_id_for` tests fail to compile
   before the function exists and pass after; the `#[should_panic]` cases panic
   for the documented reason. Milestone 2: the walker tests fail before the
   walker exists and pass after; the flag-action and collision assertions hold.
3. **Example regression.** With no assertion-content changes to
   `cli/tests/localisation.rs` or the `localizer.rs` inline tests, the suite is
   green — proving byte-for-byte id agreement via
   `with_base("hello_world.cli")`.
4. **Doctests pass:** `cargo test --doc -p ortho_config` and
   `cargo test --doc -p hello_world` both succeed (the updated `DemoLocalizer`
   doctest renders localized `about`).
5. **Snapshots and BDD.** `insta` snapshots for en-US/ja/no-op are committed and
   pass; the `rstest-bdd` "Localised CLI help" scenarios pass.
6. **Gates.** `make all` passes.

Quality criteria ("done"): all four make gates plus doctests, snapshots, and
BDD green; CodeRabbit concerns cleared at each milestone; ADR-006 recorded and
referenced; roadmap 11.1.1 marked done; users-guide, developers-guide, and the
design/component docs updated.

## Idempotence and recovery

- All steps are re-runnable. `make` targets are cache-friendly and idempotent.
- `cargo insta` snapshots: on first run review with `cargo insta review`; commit
  only intended snapshots. Re-running does not drift.
- If a milestone's gate fails, fix forward; the work is committed per milestone
  so `git` provides rollback points. Do not advance milestones with a red gate.
- The design-doc and ADR edits (Milestone 0) are independent of code and can be
  redone safely.

## Artifacts and notes

- The promotion is a *move*, not a rewrite: the example's existing behaviour
  (about/long_about/usage) is the floor; the widened coverage
  (args/version/footer/recursion) is additive and falls back gracefully on
  missing keys.
- The single most likely failure is the base mismatch (Risk 1). If any example
  localisation assertion goes red after Milestone 4, check first that
  `with_base("hello_world.cli")` is wired at every parse/help site.

## Signposted documentation and skills

Documentation:

- [cli-localization-design.md](../cli-localization-design.md) (§4, §4.1, §4.2,
  §10)
- [localizable-rust-libraries-with-fluent.md](../localizable-rust-libraries-with-fluent.md)
- [users-guide.md](../users-guide.md)
- [developers-guide.md](../developers-guide.md)
- [design.md](../design.md)
- [documentation-style-guide.md](../documentation-style-guide.md)
- [rust-testing-with-rstest-fixtures.md](../rust-testing-with-rstest-fixtures.md)
- [rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md)
- [reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md)
- [complexity-antipatterns-and-refactoring-strategies.md](../complexity-antipatterns-and-refactoring-strategies.md)
- [rstest-bdd-users-guide.md](../rstest-bdd-users-guide.md)
- ADR-005 (subcommand docs companion trait) for the existing companion-trait
  pattern.

Skills: `rust-router` → `rust-types-and-apis` (trait/API shape, `WithBase`
wrapper), `arch-crate-design` (public-vs-internal surface, boundary),
`rust-errors` (panic boundary rationale), `arch-decision-records` (ADR-006),
`rust-unit-testing` (rstest fixtures, googletest/pretty_assertions, insta),
`proptest` (the normalization invariants), `rstest-bdd` skill via the
users-guide, `leta` for navigation/refactor, and `arch-supply-chain` (the
`proptest` dev-dep / SemVer of the new public surface).

## Revision note

Initial draft. Authored from a planning workflow that combined clap-4.x API
research, Fluent-identifier/prior-art research (Firecrawl), a code-surface map,
an architect design synthesis, and a five-lens community-of-experts review. The
review surfaced five must-fix corrections now baked into the plan: the
non-existent `get_arguments_mut` (use `mut_arg`), the `value_name` action
side-effect guard, the underscore-preservation prose contradiction (Milestone
0), the default-base mismatch forcing `with_base` in the example (Risk 1 /
Milestone 4), and the panic-as-public-API contract (ADR-006). The two-phase
`BootLocalizer` lifecycle and built-in `--help`/`--version` text localization
are explicitly scoped out (11.3 and a later item respectively).
