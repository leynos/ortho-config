# Promote `try_parse_localized*` to a generic blanket trait (11.1.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Today, any application that wants localized command-line parsing must copy a
four-step ritual out of the `hello_world` example: build the `clap::Command`,
override its identifier base, localize the command tree, then parse arguments
while routing both the `try_get_matches_from_mut` error and the
`from_arg_matches` error through the crate's error localizer. That ritual lives
in `examples/hello_world/src/cli/mod.rs` as the inherent methods
`try_parse_localized`, `try_parse_localized_env`, and
`try_parse_localized_with_matches_env`. The design document calls this the
"copy this from the example" step and wants it gone
(`docs/cli-localization-design.md` §4.2).

After this change, a consumer who has a `#[derive(clap::Parser)]` type and a
`Localizer` can write:

```rust,ignore
use ortho_config::LocalizedParse;

let cli = MyCli::try_parse_localized(&localizer)?;          // reads std::env
let cli = MyCli::try_parse_localized_from(args, &localizer)?;
let (cli, matches) = MyCli::try_parse_localized_with_matches(args, &localizer)?;
```

with no hand-written parsing glue. Applications that need a custom identifier
base (the multi-binary "share one catalogue across several binaries" case that
`LocalizeCmd::with_base` exists for) call one public free function and still
avoid copying the error-localization glue:

```rust,ignore
use clap::CommandFactory;
use ortho_config::{parse_localized_command, LocalizeCmd};

let command = MyCli::command().with_base("acme.tool").localize(&localizer);
let (cli, matches): (MyCli, clap::ArgMatches) =
    parse_localized_command(command, args, &localizer)?;
```

You can observe success three ways. First, `examples/hello_world` deletes its
hand-written parsing methods and its `ParsedCommandLine` struct, yet
`cargo run -p hello_world -- greet` and the localized `--help`/error output
behave exactly as before (the snapshot suite in
`examples/hello_world/tests/localised_help.rs` stays green). Second, a new
crate-level integration test parses a fixture `#[derive(Parser)]` type through
the blanket trait and asserts the localized output. Third, an identifier
coverage test proves that every message identifier the trait queries at runtime
equals `ortho_config::message_id_for(path, suffix)` for a fixture command tree,
locking the runtime lookups to the public identifier contract.

This is roadmap item **11.1.2**. It is "quality-of-life first" with no policy
risk: it adds surface and removes duplication without changing identifier
semantics. It requires 11.1.1 (already shipped: `LocalizeCmd`, `with_base`,
`message_id_for`).

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

1. Do not change identifier semantics. `message_id_for`, `normalize_segment`,
   and `LocalizeCmd`'s base derivation
   (`ortho_config/src/localizer/identifier.rs`,
   `ortho_config/src/localizer/clap_command/mod.rs`) must remain byte-for-byte
   compatible. 11.1.3 depends on this agreement
   (`docs/cli-localization-design.md` §8.2).
2. Do not change the rendered output of `examples/hello_world`. The example
   keeps its `with_base("hello_world.cli")` base and its existing
   `hello_world-cli-*` Fluent catalogue keys; the `localised_help.rs` snapshots
   and the BDD feature output must not change. (This plan does **not** migrate
   the example's catalogue — see Decision Log D-1.)
3. Preserve `clap::error::ErrorKind` through localization so
   `ortho_config::is_display_request` keeps detecting `DisplayHelp` and
   `DisplayVersion`, and `--help`/`--version` keep exiting with status `0` to
   stdout.
4. The crate must remain free of circular dependencies. `LocalizedParse` and
   the free function live in `ortho_config` and may depend only on existing
   `ortho_config` internals (`LocalizeCmd`, `localize_clap_error_with_command`,
   `message_id_for`). The `examples/hello_world` crate depends on
   `ortho_config`, never the reverse.
5. No new external dependency. clap is already a dependency (resolved 4.5.60).
6. The `*_with_matches` variant must surface the raw `clap::ArgMatches`
   unconsumed, because OrthoConfig's `load_and_merge_with_matches` requires it.
7. The public method names are exactly those in
   `docs/cli-localization-design.md` §4.2: `try_parse_localized` (reads the
   environment), `try_parse_localized_from` (takes an iterator), and
   `try_parse_localized_with_matches` (takes an iterator, returns matches). The
   example's `_env`-suffixed names are removed, not preserved.

## Tolerances (exception triggers)

Stop and escalate (do not work around) when any of these is reached.

1. Scope: if the production (non-test, non-doc) change exceeds roughly 250 net
   lines or touches more than 12 files, stop and re-scope.
2. Interface: if delivering the design requires changing the signature of any
   existing public item other than the planned additions (the trait, the free
   function, their re-exports) and the planned example deletions, stop and
   escalate.
3. Dependencies: if any new crate dependency appears necessary, stop.
4. Snapshot drift: if any `examples/hello_world` snapshot or BDD scenario
   changes output, stop — Constraint 2 has been breached; investigate before
   accepting.
5. Iterations: if a milestone's gates (`make check-fmt typecheck lint test`)
   still fail after 3 focused attempts, stop and record the blocker.
6. Ambiguity: if the identifier-coverage test requirement (roadmap wording
   "compare derive-emitted identifiers with `message_id_for`") cannot be
   satisfied under the interpretation in Decision Log D-4, stop and present
   options.

## Risks

1. Risk: the blanket impl globalizes `message_id_for`/`LocalizeCmd::localize`
   panics (on Fluent-unsafe argument ids, duplicate normalized ids, or a
   non-letter-initial root) to **every** `clap::Parser`, so a consumer with a
   valid-for-clap but invalid-for-Fluent identifier panics at parse time. clap
   itself never panics on a valid command definition. Severity: high.
   Likelihood: medium. Mitigation: document the Fluent-safe identifier contract
   on every new public item with a `# Panics` section; add a `#[should_panic]`
   negative test at the trait/free-function level; record the panic-as-contract
   decision and tie its acceptability to 11.1.3's compile-time `compile_error!`
   guard (Decision Log D-5, ADR-006 amendment).
2. Risk: the migration silently drops the load-bearing `.with_cmd(&command)`
   enrichment on the `from_arg_matches` error path, degrading the
   missing-subcommand translation (it loses `valid_subcommands`). The two error
   paths are asymmetric. Severity: medium. Likelihood: medium. Mitigation: keep
   `.with_cmd` in the free function; add a test asserting a
   `from_arg_matches`-originated error still carries `valid_subcommands`.
3. Risk: inherent methods on `CommandLine` silently shadow the blanket trait
   methods (inherent wins) with incompatible signatures, so the promotion
   becomes a no-op for the example and the old behaviour persists. Severity:
   medium. Likelihood: medium. Mitigation: delete every inherent
   `try_parse_localized*` method on `CommandLine`; the example's primary path
   uses the free function (custom base), and a crate-level fixture exercises
   the trait.
4. Risk: snapshot tests assert text only, not exit code or stream, so a
   regression to exit code or stderr could pass. Severity: low. Likelihood:
   low. Mitigation: add explicit `success()`/exit-code and stdout assertions for
   `--help`/`--version`.
5. Risk: the example's `main.rs` and tests destructure a named
   `ParsedCommandLine`; replacing it with a tuple touches several call sites.
   Severity: low. Likelihood: high. Mitigation: the migration sites are fully
   enumerated in "Plan of work"; `(cli, matches)` binding names match the old
   field names.

## Progress

- [x] (2026-06-15) Milestone 0: design-doc and ADR reconciliation.
- [x] (2026-06-15) Milestone 1: red tests for the free function and blanket
      trait
  (crate-level), failing for the expected reasons.
- [x] (2026-06-15) Milestone 2: implement `parse_localized_command` free
      function and the
  `LocalizedParse` blanket trait; re-export both.
- [x] (2026-06-15) Milestone 3: identifier-coverage test and panic/negative
      tests.
- [x] (2026-06-15) Milestone 4: migrate `examples/hello_world` onto the free
      function;
  delete inherent methods and `ParsedCommandLine`.
- [x] (2026-06-15) Milestone 5: documentation sweep (users' guide, developers'
      guide,
  design doc, README), final gates, CodeRabbit review, roadmap tick.

Each milestone ends with `make check-fmt typecheck lint test` passing and a
commit. Run gates sequentially (build caching), never in parallel.

## Surprises & discoveries

- Observation: 2026-06-15 initial implementation pass started on branch
  `11-1-2-promote-try-parse-localized-to-a-generic-blanket-trait`; the worktree
  was clean before edits and the branch was already task-named.
- Observation: 2026-06-15 red run
  `cargo test -p ortho_config --test localized_parse` failed for the expected
  missing root exports (`LocalizedParse`, `parse_localized_command`) before the
  implementation existed. The test double also exposed that `FluentValue` does
  not implement `Display`; the localizer test now formats that diagnostic-only
  value with `Debug`.
- Observation: 2026-06-15 green run
  `cargo test -p ortho_config --test localized_parse` passed with 6 tests. The
  identifier coverage expectation is intentionally built with
  `message_id_for(...)` rather than hand-normalized literals so the test locks
  the runtime lookup set to the public helper.
- Observation: 2026-06-15 milestone gates passed after implementation:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test`. Logs are
  under
  `/tmp/*-ortho-config-11-1-2-promote-try-parse-localized-to-a-generic-blanket-trait.out`.
- Observation: 2026-06-15 `coderabbit review --agent` was attempted twice
  after the milestone gates. Both invocations connected and then stalled at
  `preparing_sandbox` with no findings or rate-limit message; logs are
  `/tmp/coderabbit-ortho-config-11-1-2-promote-try-parse-localized-to-a-generic-blanket-trait-milestone-2.out`
  and
  `/tmp/coderabbit-ortho-config-11-1-2-promote-try-parse-localized-to-a-generic-blanket-trait-milestone-2-retry.out`.
  The stalled local processes were terminated before continuing so no orphaned
  task remains from this milestone.
- Observation: 2026-06-15 example migration deleted `ParsedCommandLine` and the
  inherent `CommandLine::try_parse_localized*` helpers. The example now calls
  `parse_localized_command` with
  `CommandLine::command().with_base("hello_world.cli").localize(&localizer)` so
  the existing catalogue keys and localized snapshots remain unchanged.
  `cargo test -p hello_world` passed after the migration.
- Observation: 2026-06-15 Milestone 4 gates passed: `make check-fmt`,
  `make typecheck`, `make lint`, and `make test`.
- Observation: 2026-06-15 CodeRabbit was attempted again after Milestone 4
  gates and again stalled at `preparing_sandbox` without findings or a
  rate-limit message. Log:
  `/tmp/coderabbit-ortho-config-11-1-2-promote-try-parse-localized-to-a-generic-blanket-trait-milestone-4.out`.
- Observation: 2026-06-15 final documentation sweep updated the users' guide,
  developers' guide, design document, ADR-006, `hello_world` README, and
  roadmap. `localised_help.rs` now asserts `--help` and `--version` display
  requests exit successfully, write stdout, and leave stderr empty. Final gates
  passed: `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie`.
- Observation: 2026-06-15 final `coderabbit review --agent` attempt also
  stalled at `preparing_sandbox` without findings or a rate-limit message. Log:
  `/tmp/coderabbit-ortho-config-11-1-2-promote-try-parse-localized-to-a-generic-blanket-trait-final.out`.
- Observation: 2026-06-16 `docs/roadmap.md` was updated after implementation
  to carry the 11.1.2 decisions, findings, progress, validation status, and
  CodeRabbit stall observation alongside the completed checklist.

## Decision log

- Decision (D-1): **Do not migrate the example's identifier base.** The example
  keeps `with_base("hello_world.cli")` and its `hello_world-cli-*` catalogue.
  Rationale: the community-of-experts review (Pandalump, Wafflecat) showed that
  flattening the example to the default single-segment base (`hello-world-*`)
  deletes the multi-segment base demonstration that 11.1.3's
  `OrthoConfigLocalization` derive depends on, and contradicts the existing
  users' guide guidance and ADR-006. The free function (D-2) lets the example
  drop the copy-pasted glue *without* abandoning its base. Date/Author:
  2026-06-14, planning session (logisphere-experts panel).

- Decision (D-2): **Ship a base-agnostic public free function as the
  load-bearing primitive; the blanket trait is a thin default-base convenience
  over it.** Signature:

  ```rust,ignore
  pub fn parse_localized_command<P, I, T>(
      command: clap::Command,
      iter: I,
      localizer: &dyn Localizer,
  ) -> Result<(P, clap::ArgMatches), clap::Error>
  where
      P: clap::FromArgMatches,
      I: IntoIterator<Item = T>,
      T: Into<std::ffi::OsString> + Clone;
  ```

  The caller hands in an already-localized `Command` (default base via
  `Self::command().localize(l)`, or custom base via
  `Self::command().with_base("…").localize(l)`); the function owns only the
  parse-and-localize-errors glue. This factors the genuine duplication (the
  error dance), serves custom-base apps, and is forward-compatible: 11.1.3's
  derived-identifier path reuses the same seam. Rationale: unanimous panel
  recommendation. Rejected alternatives: `*_with_base` trait methods (doubles
  the surface; obsoleted by 11.1.3's derived constants; ambiguous override
  semantics), a builder type (one optional knob does not justify it), and a
  derive (parsing is uniform across types, so a blanket impl dominates).
  Date/Author: 2026-06-14, planning session.

- Decision (D-3): **Trait shape and signatures.**
  `pub trait LocalizedParse: clap::Parser` (no redundant `+ Sized`;
  `Parser: Sized` already), three methods with default bodies, empty blanket
  impl `impl<P: clap::Parser> LocalizedParse for P {}` (itertools precedent).
  Methods take `localizer: &dyn Localizer` (matches the crate-wide convention;
  avoids monomorphization with no benefit). `*_with_matches` returns the tuple
  `(Self, ArgMatches)` (value first, mirroring clap), not a named wrapper
  struct. Use `from_arg_matches` (borrow), not `from_arg_matches_mut`, so the
  matches stay returnable. Rationale: Telefono contract review. Date/Author:
  2026-06-14, planning session.

- Decision (D-4): **Identifier-coverage test interpretation.** The roadmap
  wording "compare derive-emitted identifiers with `message_id_for` output
  across a fixture command tree" is read, for 11.1.2, as: the *derive* is clap's
  `#[derive(Parser)]` (which emits the command tree); OrthoConfig's
  identifier-emitting derive arrives in 11.1.3. The test builds a fixture
  `#[derive(Parser)]` tree, localizes it with a recording `Localizer` that
  captures every `lookup(id, _)` call, and asserts the recorded identifier set
  equals the set computed from `message_id_for(path, suffix)` over the
  fixture's known structure (default base). This locks the trait's runtime
  lookups to the public identifier contract and guards the 11.1.3 transition.
  Date/Author: 2026-06-14, planning session.

- Decision (D-5): **Panic-as-contract, tied to 11.1.3.**
  `parse_localized_command` and the `LocalizedParse` methods inherit
  `LocalizeCmd::localize`'s panic on Fluent-unsafe or colliding identifiers. We
  keep the panic (an illegal identifier is a command-declaration bug, per
  ADR-006 and `identifier.rs`'s module doc) rather than introducing a fallible
  `try_*` variant in 11.1.2. We document it loudly and note the ordering risk:
  until 11.1.3 lands the compile-time `compile_error!` guard, 11.1.2 ships a
  runtime panic with no compile-time check. ADR-006 is amended to record that
  the blanket trait widens the reachable panic surface to every `clap::Parser`.
  Rationale: Doggylump failure-mode review. A fallible variant is recorded as
  possible future work, out of scope here. Date/Author: 2026-06-14, planning
  session.

## Outcomes & retrospective

11.1.2 is implemented. `ortho_config` now exposes `LocalizedParse` and
`parse_localized_command`; `examples/hello_world` uses the free function with
its existing `hello_world.cli` base and no longer carries local parsing glue.
The identifier-coverage and panic-contract tests live in
`ortho_config/tests/localized_parse.rs`, and the roadmap item is checked off.
On 2026-06-16, the roadmap was expanded with the same decision, finding,
progress, validation, and CodeRabbit observations recorded here so both living
documents reflect the current implementation status.

CodeRabbit could not complete in this environment: repeated invocations stalled
at `preparing_sandbox` without findings or a rate-limit message. Deterministic
quality gates passed before each attempted review; the final stalled invocation
is recorded in the observations above.

## Context and orientation

You are working in the `ortho_config` workspace. The relevant crates are
`ortho_config` (the library) and `examples/hello_world` (a demonstration
binary). The workspace builds with a `Makefile`; the gates are `make check-fmt`,
`make typecheck`, `make lint`, and `make test`. Run them sequentially to
benefit from build caching; never run them in parallel.

Key terms:

1. **Localizer** — `ortho_config::Localizer`, an object-safe `Send + Sync`
   trait with `lookup(&self, id, args) -> Option<String>`. Returning `None`
   means "no translation"; callers fall back to clap's stock English. Defined in
   `ortho_config/src/localizer/mod.rs`.
2. **`LocalizeCmd`** — extension trait on `clap::Command` (shipped in 11.1.1).
   `localize(self, &dyn Localizer) -> Command` walks the command tree and
   replaces metadata (about, long_about, usage, version, after_help, per-arg
   help/long_help/value_name) when the localizer has a translation. It uses a
   **default base** derived from the command's `bin_name` (falling back to its
   `name`). `with_base(self, impl Into<String>) -> WithBase<Command>` overrides
   that base. `localize` **panics** on duplicate identifiers. Defined in
   `ortho_config/src/localizer/clap_command/mod.rs`.
3. **`message_id_for(command_path, suffix) -> String`** — the public identifier
   builder. Normalizes each segment to lowercase ASCII `[A-Za-z0-9_-]`, joins
   with `-`, and **panics** on an unrepresentable segment or a
   non-letter-initial identifier. Defined in
   `ortho_config/src/localizer/identifier.rs`.
4. **
   `localize_clap_error_with_command(err, &dyn Localizer, Some(&Command)) -> clap::Error`
   ** — rewrites a clap error's message via the localizer while preserving its
   `ErrorKind` (it rebuilds with `clap::Error::raw(err.kind(), message)`), and
   returns the error unchanged for `DisplayHelp`/`DisplayVersion` and when no
   translation differs from the stock text. Defined in
   `ortho_config/src/localizer/clap_error.rs`. It is idempotent by construction
   (translated == fallback → early return).
5. **`is_display_request(&clap::Error) -> bool`** — true for `DisplayHelp` and
   `DisplayVersion`. The example calls `err.exit()` on these so help/version
   exit `0` to stdout. Defined in `ortho_config/src/error/helpers.rs`,
   re-exported at the crate root.

The clap 4.5.60 facts this plan relies on (verified against the resolved
source): `Parser: FromArgMatches + CommandFactory + Sized`;
`CommandFactory::command() -> Command`;
`Command::try_get_matches_from_mut(&mut self, itr) -> Result<ArgMatches, Error>`
where `itr: IntoIterator<Item = T>, T: Into<OsString> + Clone`;
`FromArgMatches::from_arg_matches(&ArgMatches) -> Result<Self, Error>`;
`Error::with_cmd(self, &Command) -> Self` preserves the kind.

The current example glue to be replaced is
`examples/hello_world/src/cli/mod.rs` lines 60–123 (the `ParsedCommandLine`
struct and the three inherent methods), its single thin wrapper
`localize_parse_error` in `examples/hello_world/src/cli/localization.rs`, and
the call sites in `examples/hello_world/src/main.rs` (lines 8, 19, 43–45) and
`examples/hello_world/src/cli/tests/localisation.rs` (lines 51, 57, 77).

Signposted documentation and skills:

1. Design: `docs/cli-localization-design.md` §4.1 (identifier convention), §4.2
   (the `try_parse_localized` helpers — the contract for this task), §8.1/§8.2
   (the 11.1.3 derive that this must stay compatible with).
2. ADR: `docs/adr/adr-006-identifier-derivation-panics.md` (the panic contract
   to amend, per D-5).
3. Testing guides: `docs/rust-testing-with-rstest-fixtures.md`,
   `docs/rust-doctest-dry-guide.md`,
   `docs/reliable-testing-in-rust-via-dependency-injection.md`,
   `docs/rtest-bdd-users-guide.md`,
   `docs/localizable-rust-libraries-with-fluent.md`,
   `docs/complexity-antipatterns-and-refactoring-strategies.md`.
4. Skills: `rust-router` then `rust-types-and-apis` (trait/blanket-impl design)
   and `arch-crate-design` (public-surface/re-export placement);
   `rust-unit-testing` (rstest fixtures, table tests, googletest/insta); `leta`
   for navigation and the example migration; `arch-decision-records` for the
   ADR-006 amendment; `pr-creation` for the draft PR.

## Plan of work

Work proceeds in stages with go/no-go validation at each boundary. Stages map
to the milestones in `Progress`.

### Stage A — understand and propose (no code changes)

Already complete: the design is captured in this document and reviewed by the
community-of-experts panel. The output is Decisions D-1 through D-5. Go/no-go:
the user approves this plan (the approval gate).

### Stage B — red tests (Milestones 1 and 3)

Add failing tests before production code. The new tests live in a fresh
crate-level integration test file, `ortho_config/tests/localized_parse.rs`,
because the blanket trait spans the derive layer (`CommandFactory` +
`FromArgMatches`) and is most naturally exercised through a real
`#[derive(clap::Parser)]` fixture defined in that test. A small fixture tree:

```rust,ignore
#[derive(clap::Parser)]
#[command(name = "fixture", bin_name = "fixture")]
struct Fixture {
    #[arg(long, value_name = "PATH")]
    config: Option<std::path::PathBuf>,
    #[command(subcommand)]
    command: FixtureCommand,
}

#[derive(clap::Subcommand)]
enum FixtureCommand {
    Greet(GreetArgs),
}

#[derive(clap::Args)]
struct GreetArgs {
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
}
```

Write these tests (each must fail to compile or assert before Stage C):

1. `try_parse_localized_from_parses_subcommand`: parse `["fixture", "greet"]`
   through the trait, assert the `FixtureCommand::Greet` variant. Red reason:
   `LocalizedParse` does not exist yet.
2. `try_parse_localized_with_matches_returns_matches`: assert the returned
   `ArgMatches` is usable (e.g. `subcommand_name() == Some("greet")`).
3. `noop_localizer_matches_stock_clap`: parse a deliberately failing input
   (`["fixture"]`, missing subcommand) with `NoOpLocalizer` through the trait,
   and assert the error string equals
   `Fixture::command().try_get_matches_from(["fixture"]).unwrap_err().to_string()`.
   This is the crate-level transparency guard (the example has an equivalent;
   the blanket machinery needs its own).
4. `from_arg_matches_error_retains_valid_subcommands`: drive a localizer whose
   `clap-error-missing-subcommand` translation interpolates
   `{valid_subcommands}`, force a `from_arg_matches`-path error, and assert the
   rendered message lists the subcommands — proving the `.with_cmd` enrichment
   survives.
5. `identifier_coverage_matches_message_id_for`: localize the fixture with a
   `RecordingLocalizer` (a test double that pushes every queried `id` into a
   `RefCell<Vec<String>>` and returns `None`), then assert the recorded set
   equals the expected set built from `message_id_for` over the fixture
   structure (root `["fixture"]`; suffixes `about`, `long_about`, `usage`,
   `version`, `long_version`, `after_help`, `after_long_help`,
   `args.config.help|long_help|value_name`; recursively for the `greet`
   subcommand). See Decision D-4.
6. `fluent_unsafe_identifier_panics`
   (`#[should_panic(expected = "invalid Fluent identifier segment")]`): a
   fixture with `#[arg(id = "bad.id")]` parsed through the trait must panic,
   pinning the panic contract (D-5).

Use `rstest` fixtures for the localizer doubles, `pretty_assertions` for
equality, and `googletest` matchers where they read better than `assert_eq!`.
Where the `RecordingLocalizer` and the fixture parsers double as shared
helpers, place them in the test file (or `ortho_config/tests/support/` if an
existing support module is the better home — check
`ortho_config/tests/support/localizers.rs`). No `proptest`/`kani`/`verus` is
warranted here: the new surface is thin glue over already-property-tested
identifier logic; the invariants are covered by the coverage test and the panic
test.

Go/no-go: `make test` shows the six new tests failing for the expected reasons
(missing symbol, then assertion). Commit the red tests.

### Stage C — implementation (Milestone 2)

1. Create `ortho_config/src/localizer/clap_command/parse.rs`. Define the free
   function `parse_localized_command` (D-2) and the `LocalizedParse` trait
   (D-3). The free function body:

   ```rust,ignore
   pub fn parse_localized_command<P, I, T>(
       command: clap::Command,
       iter: I,
       localizer: &dyn Localizer,
   ) -> Result<(P, clap::ArgMatches), clap::Error>
   where
       P: clap::FromArgMatches,
       I: IntoIterator<Item = T>,
       T: Into<std::ffi::OsString> + Clone,
   {
       let mut command = command;
       let matches = command
           .try_get_matches_from_mut(iter)
           .map_err(|err| localize_clap_error_with_command(err, localizer, Some(&command)))?;
       let value = P::from_arg_matches(&matches).map_err(|err| {
           localize_clap_error_with_command(err.with_cmd(&command), localizer, Some(&command))
       })?;
       Ok((value, matches))
   }
   ```

   The trait default bodies delegate:

   ```rust,ignore
   pub trait LocalizedParse: clap::Parser {
       fn try_parse_localized(localizer: &dyn Localizer) -> Result<Self, clap::Error> {
           Self::try_parse_localized_from(std::env::args_os(), localizer)
       }
       fn try_parse_localized_from<I, T>(iter: I, localizer: &dyn Localizer)
           -> Result<Self, clap::Error>
       where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone {
           Self::try_parse_localized_with_matches(iter, localizer).map(|(value, _)| value)
       }
       fn try_parse_localized_with_matches<I, T>(iter: I, localizer: &dyn Localizer)
           -> Result<(Self, clap::ArgMatches), clap::Error>
       where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone {
           parse_localized_command(Self::command().localize(localizer), iter, localizer)
       }
   }

   impl<P: clap::Parser> LocalizedParse for P {}
   ```

   Document every item. Each method and the free function gets a `# Panics`
   section pointing at the Fluent-safe identifier contract (D-5) and a
   `# Errors` section. The trait gets a runnable doctest showing the
   zero-config common case (a small `#[derive(Parser)]` plus a `NoOpLocalizer`,
   asserting it parses). Keep doctests DRY per `docs/rust-doctest-dry-guide.md`.
2. Wire the module: add `mod parse;` (or `pub use` as appropriate) in
   `ortho_config/src/localizer/clap_command/mod.rs`, re-export `LocalizedParse`
   and `parse_localized_command` from `ortho_config/src/localizer/mod.rs`, and
   add both to the single existing `pub use localizer::{ … }` block in
   `ortho_config/src/lib.rs` (lines 118–122) — do not start a second re-export
   statement.

Go/no-go: the Stage B tests 1–5 now pass; test 6 (panic) passes.
`make check-fmt typecheck lint test` is green. Commit.

### Stage D — example migration (Milestone 4)

1. In `examples/hello_world/src/cli/mod.rs`: delete the `ParsedCommandLine`
   struct (60–67) and the three inherent methods `try_parse_localized_env`,
   `try_parse_localized_with_matches_env`, `try_parse_localized` (69–123).
   Remove the now-unused `CommandFactory`/`FromArgMatches` imports (line 8) if
   they become orphaned.
2. In `examples/hello_world/src/cli/localization.rs`: delete the
   `localize_parse_error` wrapper; the crate's `parse_localized_command` now
   owns the error glue. Keep the `pub use ortho_config::LocalizeCmd;` re-export
   if other modules rely on it; otherwise remove.
3. In `examples/hello_world/src/main.rs`: keep the private `parse_command_line`
   helper but have it call the free function with the example's custom base,
   and return a tuple:

   ```rust,ignore
   fn parse_command_line() -> Result<(CommandLine, clap::ArgMatches)> {
       let localizer = DemoLocalizer::default();
       let command = CommandLine::command()
           .with_base("hello_world.cli")
           .localize(&localizer);
       // matches is needed by load_globals_and_merge_selected_subcommand below.
       match parse_localized_command::<CommandLine, _, _>(command, std::env::args_os(), &localizer) {
           Ok(parsed) => Ok(parsed),
           Err(err) => {
               if is_display_request(&err) { err.exit(); }
               Err(err.into())
           }
       }
   }
   ```

   Update the caller (`let (cli, matches) = parse_command_line()?;`) and remove
   `ParsedCommandLine` from the `use` on line 8.
4. In `examples/hello_world/src/cli/tests/localisation.rs`: replace the three
   `try_parse_localized` call sites with
   `parse_localized_command::<CommandLine, _, _>(command, args, &loc)`, where
   `command` is
   `CommandLine::command().with_base("hello_world.cli").localize(&loc)`,
   destructuring `(cli, _)` where the test inspects `cli.command`. The two
   error-path tests (lines 57, 77) only call `.expect_err()` and need only the
   call-shape update.

Go/no-go: `make test` (workspace, including `examples/hello_world`) is green;
the `localised_help.rs` snapshots and BDD scenarios are unchanged (Constraint
2; Tolerance 4). Commit.

### Stage E — failure-mode hardening and docs (Milestone 5)

1. Strengthen the example's end-to-end assertions: extend the existing
   `assert_cmd` tests in `examples/hello_world/tests/localised_help.rs` so
   `--help` and `--version` assert `.success()` (exit `0`) and non-empty stdout
   with empty stderr, not just snapshot text (Risk 4).
2. Documentation sweep:
   - `docs/users-guide.md`: add a `LocalizedParse` section showing the
     zero-config trait path for the common case (catalogue keys match
     `bin_name`), and present `parse_localized_command` + `with_base` as the
     escape hatch for namespaced/multi-binary catalogues. State the rule
     plainly: *the bare trait assumes catalogue keys match your `bin_name`; for
     a different root use `with_base` and `parse_localized_command`.* Update the
     stale `try_parse_localized_env` reference (around line 313).
   - `docs/developers-guide.md`: add a short "localized parsing" note pointing
     at the trait and the free-function seam, and the asymmetry of the two
     error paths (`with_cmd` enrichment).
   - `docs/cli-localization-design.md` §4.2: reconcile the prose with the
     shipped surface — the blanket impl is empty with default-bodied methods,
     and the base-agnostic `parse_localized_command` primitive is documented as
     the seam the trait wraps. Record the naming clarification (env-based method
     is the unsuffixed `try_parse_localized`).
   - `docs/adr/adr-006-identifier-derivation-panics.md`: amend per D-5 to note
     the blanket trait widens the reachable panic surface and the 11.1.3
     ordering relationship.
   - `examples/hello_world/README.md`: update the two `try_parse_localized_env`
     references (around lines 50 and 91).
3. Final gates: `make check-fmt typecheck lint test`, each sequentially, via
   `tee` to a per-action log under `/tmp` for review.
4. Run `coderabbit review --agent`; clear every concern before requesting human
   review (deterministic gates must already pass first).
5. Mark roadmap item 11.1.2 done in `docs/roadmap.md` (tick the four
   sub-bullets and the item).

Go/no-go: all gates green; CodeRabbit clean; roadmap updated. Commit.

## Concrete steps

Run from the worktree root. Use `tee` so truncated output stays reviewable:

```bash
# Gate template (run each sequentially, never in parallel):
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-ortho-config-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-ortho-config-$(git branch --show-current).out"
```

Focused test runs during red/green (cargo-nextest is available; see the
`nextest` skill):

```bash
# Red: expect the six new tests to fail to compile / assert.
cargo test -p ortho_config --test localized_parse 2>&1 | tee /tmp/red-localized-parse.out
# Green after Stage C:
cargo test -p ortho_config --test localized_parse 2>&1 | tee /tmp/green-localized-parse.out
# Example after Stage D:
cargo test -p hello_world 2>&1 | tee /tmp/example-hello-world.out
```

Expected green transcript shape (illustrative):

```plaintext
test identifier_coverage_matches_message_id_for ... ok
test noop_localizer_matches_stock_clap ... ok
test fluent_unsafe_identifier_panics ... ok
test result: ok. 6 passed; 0 failed
```

## Validation and acceptance

Acceptance is behavioural:

1. A consumer with `#[derive(clap::Parser)]` and a `Localizer` can call
   `MyCli::try_parse_localized(&localizer)`,
   `MyCli::try_parse_localized_from(args, &localizer)`, and
   `MyCli::try_parse_localized_with_matches(args, &localizer)` with no
   hand-written glue. Proven by the crate-level integration tests.
2. `parse_localized_command(cmd, args, &localizer)` parses against a
   pre-localized (optionally `with_base`) command and localizes errors. Proven
   by the example using it and by the `with_cmd` enrichment test.
3. `NoOpLocalizer` yields byte-identical output to stock clap. Proven by
   `noop_localizer_matches_stock_clap`.
4. `--help`/`--version` still exit `0` to stdout. Proven by the strengthened
   `assert_cmd` assertions.
5. Every runtime-queried identifier equals `message_id_for(path, suffix)`.
   Proven by `identifier_coverage_matches_message_id_for`.
6. A Fluent-unsafe identifier panics with the documented message. Proven by
   `fluent_unsafe_identifier_panics`.

Red-Green-Refactor evidence to record in `Progress`/`Artifacts` as work
proceeds: the Stage B red run (expected failures), the Stage C green run, and
the post-refactor green run plus `make lint`/`check-fmt`.

Quality criteria ("done"):

1. Tests: `make test` passes across the workspace; the six new tests pass; all
   existing example snapshots and BDD scenarios are unchanged.
2. Lint/typecheck: `make lint` and `make typecheck` clean (warnings denied).
3. Formatting: `make check-fmt` clean.
4. Review: `coderabbit review --agent` reports no outstanding concerns.

Quality method: the gate commands above, run sequentially, plus the CodeRabbit
pass after deterministic gates are green.

## Idempotence and recovery

Each milestone is a separate commit, so any stage can be rolled back with
`git revert`/`git reset` to the prior commit. The red tests are additive and
safe to re-run. The example migration is the only destructive edit (deleting
methods and a struct); it is recoverable from git history. No data migrations,
no external side effects.

## Artifacts and notes

To be filled with red/green transcripts and the final diff summary as work
proceeds.

## Interfaces and dependencies

Final public surface added to `ortho_config` (re-exported at the crate root):

```rust
// ortho_config/src/localizer/clap_command/parse.rs

/// Parses `iter` against an already-localized `command`, localizing any clap
/// error through the supplied localizer, and returns the parsed value with the
/// raw `ArgMatches`.
///
/// # Panics
/// The supplied `command` must already be localized; building that command via
/// `LocalizeCmd`/`message_id_for` panics on Fluent-unsafe identifiers.
///
/// # Errors
/// Returns the localized `clap::Error` when argument parsing fails.
pub fn parse_localized_command<P, I, T>(
    command: clap::Command,
    iter: I,
    localizer: &dyn Localizer,
) -> Result<(P, clap::ArgMatches), clap::Error>
where
    P: clap::FromArgMatches,
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone;

/// Localized counterparts to `clap::Parser`'s `try_parse*` methods.
pub trait LocalizedParse: clap::Parser {
    /// Like `Parser::try_parse`, with localized metadata and errors.
    ///
    /// # Panics
    /// Panics if the command tree contains a Fluent-unsafe or duplicate
    /// identifier (see `message_id_for`).
    ///
    /// # Errors
    /// Returns a localized `clap::Error` on parse failure.
    fn try_parse_localized(localizer: &dyn Localizer) -> Result<Self, clap::Error>;

    /// Like `Parser::try_parse_from`, with localization. Same panics/errors.
    fn try_parse_localized_from<I, T>(iter: I, localizer: &dyn Localizer)
        -> Result<Self, clap::Error>
    where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone;

    /// As `try_parse_localized_from`, also returning the raw `ArgMatches`
    /// required by `load_and_merge_with_matches`. Same panics/errors.
    fn try_parse_localized_with_matches<I, T>(iter: I, localizer: &dyn Localizer)
        -> Result<(Self, clap::ArgMatches), clap::Error>
    where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone;
}

impl<P: clap::Parser> LocalizedParse for P {}
```

Dependencies: only existing `ortho_config` internals
(`localize_clap_error_with_command`, `LocalizeCmd`, `Localizer`) and clap
4.5.60. No new crates. The `examples/hello_world` crate consumes
`parse_localized_command` and (in its doctests/tests) `LocalizedParse`.

## Revision note

Initial draft (2026-06-14). Authored after a codebase-mapping research pass and
a logisphere-experts community-of-experts review (Pandalump, Telefono,
Dinolump, Wafflecat, Doggylump). The review changed the central design from
"default base only; migrate the example's catalogue" to "ship a base-agnostic
`parse_localized_command` primitive; the blanket trait wraps it; the example
keeps its `with_base` base" (Decision D-2/D-1), and added the failure-mode
requirements (panic contract D-5, `with_cmd` enrichment, NoOp transparency,
exit-code assertions). Status remains DRAFT pending user approval before any
implementation.
