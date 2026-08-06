# Add an `ortho_config::cargo` helper for hand-built clap commands (8.3.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

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
    .arg(clap::Arg::new("verbose").long("verbose").num_args(0));
let cli = external_subcommand("cargo-demo", "demo", args_command);
```

and the returned `clap::Command` accepts both invocation forms with the same
inner options and no duplicated parser setup:

1. `cargo demo --verbose` (Cargo dispatch: argv `["cargo-demo", "demo",
   "--verbose"]`), and
2. `cargo-demo demo --verbose` (direct invocation with the same injected
   token).

You can observe success three ways. First, a new integration test parses both
argv forms through the wrapper and asserts the inner option values match a
parse of the unwrapped command. Second, help output renders the Cargo dispatch
shape (`Usage: cargo <COMMAND>` at the top level and `Usage: cargo demo
[OPTIONS]` for the subcommand), captured as insta snapshots. Third, a
behavioural (rstest-bdd) scenario drives the wrapper from a consumer's point of
view and asserts both the happy path and the bare-invocation failure path.

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
   and it must not introduce a second configuration-loading pathway.
3. No new external dependency. clap 4 (resolved 4.6.1, `derive` and `string`
   features) is already a dependency of `ortho_config`. In particular, do not
   add `clap-cargo` for styling; styling stays out of scope (see Decision D-3).
4. No change to `cargo-orthohelp`'s user-visible behaviour in this task. The
   binary is derive-based and is not migrated here; its dispatch snapshots in
   `cargo-orthohelp/tests/cli_dispatch.rs` must remain untouched and green.
5. Existing public API of `ortho_config` remains stable; this task is purely
   additive (a new `cargo` module and its contents).
6. No circular dependencies. `ortho_config::cargo` may depend only on clap and
   `ortho_config` internals. `ortho_config` must never depend on
   `cargo-orthohelp` or the test fixture crates.
7. The helper must not install a tracing subscriber or any global state:
   libraries must not install global recorders/subscribers (AGENTS.md). The
   observability expectation in ADR-004 (subscriber before parsing, debug event
   at the dispatch boundary) is a binary-level obligation and is handled here
   as documentation only (see Decision D-4).
8. All module files start with `//!` docs; every public item carries `///`
   rustdoc with runnable examples; no code file exceeds 400 lines (AGENTS.md).

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
5. Iterations: if a milestone's gates (`make check-fmt`, `make typecheck`,
   `make lint`, `make test`) still fail after 3 focused attempts, stop and
   record the blocker.
6. Ambiguity: the usage-rendering decision (D-2) deviates from the literal
   `bin_name("cargo-<name>")` wording in `docs/roadmap.md` §8.3.1 and
   `docs/design.md` §4.17. This plan's approval covers that deviation; if
   during implementation the deviation turns out to require behaviour the plan
   does not describe, stop and present options.

## Risks

1. Risk: help/usage rendering differs between clap patch releases, making the
   insta snapshots brittle. Severity: low. Likelihood: low. Mitigation:
   snapshot only the load-bearing `Usage:` lines and the subcommand list, not
   whole help screens, mirroring the existing practice in
   `cargo-orthohelp/tests/cli_dispatch.rs`.
2. Risk: `Command::name` on the inner command interacts with a caller who
   already set a conflicting name, producing surprising help output. Severity:
   low. Likelihood: medium. Mitigation: the helper documents that it renames
   the inner command to the injected subcommand name (that is its job), and a
   unit test pins the rename behaviour.
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
   the design-doc reconciliation milestone; plan approval decides it.

## Progress

- [ ] Milestone 0: plan drafted, reviewed by expert panel, approved by user.
      (Drafting and panel review done 2026-08-06; awaiting user approval.)
- [ ] Milestone 1: red tests (unit, snapshot, behavioural) failing for the
      expected reasons.
- [ ] Milestone 2: implement `ortho_config::cargo::external_subcommand`;
      green tests; gates pass; commit.
- [ ] Milestone 3: property test for option-preservation equivalence; gates
      pass; commit.
- [ ] Milestone 4: documentation sweep (users' guide, design doc, developers'
      guide, contents index); gates including `make markdownlint` and
      `make nixie` pass; commit.
- [ ] Milestone 5: CodeRabbit review clean; roadmap 8.3.1 marked done; final
      gates; draft PR updated.

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

## Decision log

- Decision (D-1): **The helper is a plain function that reshapes a
  `clap::Command`; parsing stays with the caller.** Signature (see
  "Interfaces and dependencies" for the documented form):
  `external_subcommand(installed_bin_name, subcommand_name, command) ->
  clap::Command`. It performs no argv inspection, no environment reads, and no
  parsing. Rationale: `docs/design.md` §4.17 prescribes exactly this shape;
  keeping the helper free of argv/environment access keeps it deterministic,
  trivially testable, and macro-generatable by a possible 8.3.3 attribute
  (plain `impl Into<clap::builder::Str>` parameters, no generics over
  iterators, no lifetimes). The argv-stripping alternatives (cargo-insta,
  cargo-deny patterns) were rejected: ADR-004 already chose the
  nested-subcommand shape, and argv manipulation would put process-level
  behaviour inside a library helper. Date/Author: 2026-08-06, planning
  session.

- Decision (D-2): **The parent command renders Cargo dispatch usage:
  `Command::new("cargo").bin_name("cargo")`, with the installed binary name
  applied via `Command::display_name`.** This deviates from the literal
  `bin_name("cargo-<name>")` text in `docs/roadmap.md` §8.3.1 and the code
  sketch in `docs/design.md` §4.17. Rationale: `bin_name` is what clap prints
  in `Usage:` lines. `bin_name("cargo")` renders `Usage: cargo <name>
  [OPTIONS]`, which matches clap's official `cargo-example` cookbook pattern,
  matches Cargo's help protocol (`cargo help <name>` runs
  `cargo-<name> <name> --help`), and matches the behaviour `cargo-orthohelp`
  already pins in its snapshots. `bin_name("cargo-<name>")` would render
  `Usage: cargo-<name> <name> [OPTIONS]`, which is accurate only for direct
  invocation and would make the hand-built and derive paths disagree. The
  installed binary name is still honoured: it becomes the command's
  `display_name` (used in version output) and appears in the helper's rustdoc
  guidance for direct invocation. Milestone 4 reconciles the design doc's code
  sketch with this decision. Plan approval ratifies the deviation
  (Tolerance 6). Date/Author: 2026-08-06, planning session.

- Decision (D-3): **No styling, no version propagation, no error-hint
  augmentation inside the helper.** The helper sets exactly:
  `Command::new("cargo")`, `bin_name("cargo")`,
  `display_name(installed_bin_name)`, `subcommand_required(true)`, and nests
  `command.name(subcommand_name)` as the sole subcommand. Rationale: Cargo's
  colour styling would require the `clap-cargo` dependency (Constraint 3);
  version belongs on the inner command where the caller controls it (clap's
  cookbook sets `version` on the subcommand, keeping the synthetic parent
  version-less); the friendly "invoke via `cargo <name>`" hint that
  `cargo-orthohelp` prints on `MissingSubcommand` is binary-level error
  handling, not command shape, and stays with binaries. Each exclusion is
  documented in the helper's rustdoc so callers know where those concerns
  live. Date/Author: 2026-08-06, planning session.

- Decision (D-4): **ADR-004's observability expectation is documented, not
  implemented.** ADR-004 says Cargo-facing binaries should initialize a
  tracing subscriber before parsing and emit a debug event at the dispatch
  boundary. A library helper must not install subscribers (AGENTS.md), and the
  helper never parses, so it has no dispatch boundary of its own. The users'
  guide section (Milestone 4) states the expectation and points at
  `cargo-orthohelp/src/main.rs` as the reference implementation. Date/Author:
  2026-08-06, planning session.

- Decision (D-5): **Coverage levels.** Unit tests (rstest) pin the shape and
  both argv forms; insta snapshots pin the `Usage:` lines for top-level and
  subcommand help; an rstest-bdd feature exercises the consumer-visible
  happy and unhappy paths; one proptest pins the option-preservation
  invariant (for arbitrary simple option values, parsing
  `["cargo-<name>", "<name>", args...]` through the wrapper yields the same
  matches as parsing `["<name>", args...]` through the unwrapped inner
  command). No end-to-end process-spawning tests here: roadmap item 8.3.4
  owns the shared on-`PATH` regression fixtures, and `cargo-orthohelp`'s
  existing `cli_dispatch.rs` already proves real Cargo dispatch works for the
  nested shape. No `kani`/`verus`: the helper contains no unsafe code, no
  state machine, and no arithmetic lemma; the single invariant is
  input-domain-shaped and proptest is the proportionate adversary
  (`docs/developers-guide.md` discourages heavier tooling without a
  substantive invariant). Date/Author: 2026-08-06, planning session.

- Decision (D-6): **Assertion style follows the house style.** The task brief
  asks for `googletest` and `pretty_assertions`; neither is used anywhere in
  this workspace, and prior ExecPlans (11.1.1, 11.1.2) resolved the same
  tension by citing the `rust-unit-testing` skill while using `rstest` plus
  plain assertions and `insta`. Introducing two new dev-dependencies for one
  small module would contradict AGENTS.md's dependency discipline and
  Constraint 3's spirit. Date/Author: 2026-08-06, planning session.

## Outcomes & retrospective

To be completed at delivery.

## Context and orientation

You are working in the `ortho_config` workspace (Rust, edition 2024, workspace
version 0.8.0). Workspace members: `ortho_config` (the library),
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

The `ortho_config` crate's `src/lib.rs` currently declares these top-level
modules: `agent_context`, `csv_env`, `declarative`, `discovery`, `docs`,
`error`, `file`, `localizer` (private), `merge`, `post_merge`, `result_ext`,
`subcommand`. This task adds a new public `cargo` module. clap is already a
direct dependency with the `derive` and `string` features (the `string`
feature is what makes `clap::builder::Str` accept owned strings).

Testing infrastructure this plan reuses:

1. Unit tests: `rstest` 0.26.1 (dev-dependency of `ortho_config`); unit tests
   for a module live in a sibling `tests.rs` included with `#[cfg(test)]`
   (for example `ortho_config/src/error/tests.rs`).
2. Integration tests: files under `ortho_config/tests/`.
3. Behavioural tests: a single registered test binary
   (`[[test]] name = "rstest_bdd" path = "tests/rstest_bdd/mod.rs"` in
   `ortho_config/Cargo.toml`), feature files under
   `ortho_config/tests/features/*.feature`, step definitions under
   `ortho_config/tests/rstest_bdd/behaviour/steps/`, scenario-local state via
   `Slot<T>` in `#[derive(ScenarioState)]` structs
   (`ortho_config/tests/rstest_bdd/scenario_state.rs`), scenarios bound with
   `scenarios!(...)`. The `rstest_bdd` group is serialized to one thread by
   `.config/nextest.toml`.
4. Snapshots: `insta` 1, snapshots beside the owning test; review with
   `cargo insta review` (or `INSTA_UPDATE=always` non-interactively, then
   verify no `.snap.new`/`.pending-snap` files remain).
5. Property tests: `proptest` 1.11 is already a dev-dependency.

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
   `rust-unit-testing` (fixtures, table tests, snapshot discipline);
   `proptest` (the equivalence property); `leta` for navigation;
   `commit-message` for commits; `pr-creation` for the PR;
   `comenq-coderabbit` if the CodeRabbit loop needs driving through a PR.

## Plan of work

Work proceeds in stages with go/no-go validation at each boundary. Stages map
to the milestones in `Progress`.

### Stage A — understand and propose (no code changes)

Complete: this document, informed by a four-agent reconnaissance pass (design
docs, current `cargo-orthohelp` implementation, testing and documentation
conventions, and external prior art) and revised by a community-of-experts
design review. The outputs are Decisions D-1 through D-6. Go/no-go: the user
approves this plan (the approval gate). Do not start Stage B without it.

### Stage B — red tests (Milestone 1)

Do not create any production code in this stage. Write the tests against the
planned `ortho_config::cargo` API so they fail to compile because the module
does not exist yet, run the focused test commands, and record the expected
failure as the red evidence. Then proceed.

1. Unit tests in `ortho_config/src/cargo/tests.rs` (included from the module
   with `#[cfg(test)] mod tests;`), using `rstest`:
   - `wraps_command_under_cargo_parent`: the returned command is named
     `cargo`, has `bin_name` `cargo`, requires a subcommand, and contains
     exactly one subcommand named with the injected name.
   - `parses_cargo_dispatch_argv`: `try_get_matches_from(["cargo-demo",
     "demo", "--verbose"])` succeeds and the subcommand matches carry
     `verbose`.
   - `parses_direct_invocation_argv`: the same argv shape with a direct-path
     argv[0] (for example `"./target/debug/cargo-demo"`) succeeds — proving
     argv[0] is irrelevant to the wrapper.
   - `rejects_bare_invocation_without_injected_token`:
     `try_get_matches_from(["cargo-demo", "--verbose"])` fails with
     `ErrorKind::InvalidSubcommand` or `ErrorKind::UnknownArgument`
     (pin the actual kind observed at red time).
   - `renames_inner_command_to_subcommand_name`: an inner command originally
     named `something-else` surfaces as the `demo` subcommand.
   - `preserves_inner_options_and_version`: an inner command with a version
     and two options round-trips them; `--version` parses at the subcommand
     level (`["cargo-demo", "demo", "--version"]` yields
     `ErrorKind::DisplayVersion`).
2. Snapshot tests in `ortho_config/tests/cargo_entry_point.rs` using `insta`,
   asserting the load-bearing lines only:
   - top-level help contains `Usage: cargo <COMMAND>`;
   - subcommand help contains `Usage: cargo demo [OPTIONS]`.
   Render via `Command::render_help` on the wrapped command (no process
   spawning). Two snapshots, stored beside the test.
3. Behavioural coverage: new feature file
   `ortho_config/tests/features/cargo_entry_point.feature` plus a steps
   module `ortho_config/tests/rstest_bdd/behaviour/steps/cargo_steps.rs`
   (registered alongside the existing steps modules), with scenarios:

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

   Scenario state uses a `Slot`-based `ScenarioState` struct following
   `ortho_config/tests/rstest_bdd/scenario_state.rs` conventions.

Go/no-go: the focused runs fail for the expected reason (unresolved
`ortho_config::cargo`). Commit the red tests.

### Stage C — implementation (Milestone 2)

1. Create `ortho_config/src/cargo/mod.rs` starting with a `//!` module doc
   explaining Cargo's injected-token protocol (with the argv example), the
   wrapper shape, what the helper deliberately does not do (D-3, D-4), and a
   runnable doctest. The function body is small:

   ```rust,ignore
   pub fn external_subcommand(
       installed_bin_name: impl Into<clap::builder::Str>,
       subcommand_name: impl Into<clap::builder::Str>,
       command: clap::Command,
   ) -> clap::Command {
       clap::Command::new("cargo")
           .bin_name("cargo")
           .display_name(installed_bin_name.into().to_string())
           .subcommand_required(true)
           .subcommand(command.name(subcommand_name))
   }
   ```

   (Exact conversion plumbing may differ; `Command::display_name` takes
   `impl IntoResettable<String>`, so the `Str` → `String` conversion is
   resolved at implementation time. If a cleaner parameter type emerges —
   for example two `impl Into<clap::builder::Str>` parameters used
   consistently — prefer it, but keep the three-argument order fixed by the
   design doc: installed binary name, injected subcommand name, inner
   command.)
2. Declare `pub mod cargo;` in `ortho_config/src/lib.rs` alongside the other
   public modules. Do not re-export `external_subcommand` at the crate root:
   `ortho_config::cargo::external_subcommand` reads as a sentence and avoids
   crowding the root namespace (the design doc names the module path
   explicitly).
3. Wire `#[cfg(test)] mod tests;` and make the Stage B unit, snapshot, and
   behavioural tests pass.

Go/no-go: focused tests green, then full gates green
(`make check-fmt`, `make typecheck`, `make lint`, `make test`, sequentially).
Commit.

### Stage D — property test (Milestone 3)

Add the equivalence property to `ortho_config/tests/cargo_entry_point.rs`
(or a sibling file if length demands): for generated option values (a
bounded strategy over simple ASCII strings and flag on/off combinations),

```text
matches_of(wrapper, ["cargo-demo", "demo"] + args).subcommand("demo")
    == matches_of(inner, ["demo"] + args)
```

comparing the extracted values for each known option identifier (compare
values, not `ArgMatches` wholesale — `ArgMatches` does not implement
`PartialEq` usefully for this purpose). Keep the case count modest (the
default 256 is fine; the domain is small). Fresh `Command` values are built
per case because `try_get_matches_from` consumes state.

Go/no-go: gates green. Commit.

### Stage E — documentation sweep (Milestone 4)

1. `docs/users-guide.md`: add a new `##` section "Cargo external-subcommand
   entry points" (placed after "Documentation metadata (OrthoConfigDocs)" and
   before "Additional notes"), covering: why Cargo injects the subcommand
   name; the helper with a worked example showing both invocation forms; the
   plain statement that the wrapper is CLI entry-point structure, not
   configuration loading (design.md §4.17's documentation obligation); the
   derive-based alternative in one paragraph with a pointer to
   `cargo-orthohelp`; and the binary-level obligations the helper leaves with
   the caller (version on the inner command, optional invocation hint on
   error, tracing subscriber per ADR-004).
2. `docs/design.md` §4.17: update the code sketch to the shipped shape
   (`bin_name("cargo")` + `display_name`, per D-2) and note that the helper
   shipped in 8.3.1; keep the section's constraint prose intact.
3. `docs/adr-004-cargo-external-subcommand-entry-point.md`: append a short
   amendment note recording D-2 (usage renders the Cargo dispatch form; the
   installed binary name is carried as `display_name`).
4. `docs/developers-guide.md`: extend the "Behavioural test layout" list (or
   the nearest fitting section) with the new feature file and steps module,
   and note the property test's home, so future contributors extend rather
   than duplicate.
5. `docs/contents.md`: no new document is added, so only touch it if a listed
   summary becomes inaccurate.

Go/no-go: `make markdownlint` and `make nixie` green in addition to the code
gates. Commit.

### Stage F — review, roadmap, delivery (Milestone 5)

1. Run the full gate suite sequentially via `scrutineer`; all green.
2. Run `coderabbit review --agent` (log under `/tmp`); clear every concern.
   Deterministic gates must already be green before the review is requested.
   If the review stalls at `preparing_sandbox` twice, record the stall (Risk
   4) and continue.
3. Tick roadmap item 8.3.1 in `docs/roadmap.md` (the four sub-bullets and the
   item; mark "done" per the item's convention).
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
# Green after Stage C:
cargo test -p ortho_config cargo:: 2>&1 | tee /tmp/green-cargo-unit.out
cargo test -p ortho_config --test cargo_entry_point 2>&1 \
  | tee /tmp/green-cargo-entry-point.out
```

Expected red transcript shape (illustrative):

```plaintext
error[E0432]: unresolved import `ortho_config::cargo`
```

Expected green transcript shape (illustrative):

```plaintext
test parses_cargo_dispatch_argv ... ok
test rejects_bare_invocation_without_injected_token ... ok
test result: ok. 6 passed; 0 failed
```

## Validation and acceptance

Acceptance is behavioural:

1. A hand-built `clap::Command` wrapped by
   `ortho_config::cargo::external_subcommand("cargo-demo", "demo", cmd)`
   parses both `["cargo-demo", "demo", OPTIONS...]` (Cargo dispatch) and the
   identical direct-invocation argv, yielding the same option values as the
   unwrapped command — proven by the unit tests and the property test.
2. Bare invocation without the injected token fails with a subcommand error —
   proven by a unit test and a BDD scenario.
3. Help renders the Cargo dispatch shape (`Usage: cargo <COMMAND>`,
   `Usage: cargo demo [OPTIONS]`) — proven by insta snapshots.
4. The inner command's options and version are preserved, not duplicated —
   proven by `preserves_inner_options_and_version` and the property test.
5. No existing behaviour changes: `cargo-orthohelp`'s dispatch tests and
   snapshots are untouched and green — proven by `make test`.

Red-Green-Refactor evidence to record in `Progress`/`Artefacts` as work
proceeds: the Stage B red run (expected compile failure), the Stage C green
run, and the post-refactor green run plus lint/format gates.

Quality criteria ("done"):

1. Tests: `make test` passes across the workspace; all new tests pass.
2. Lint/typecheck: `make lint` and `make typecheck` clean (warnings denied).
3. Formatting: `make check-fmt` clean; docs pass `make markdownlint` and
   `make nixie`.
4. Review: `coderabbit review --agent` reports no outstanding concerns (or a
   recorded environment stall per Risk 4).
5. Roadmap: 8.3.1 marked done.

## Idempotence and recovery

Each milestone is a separate commit; any stage rolls back with `git revert`
or `git reset` to the prior commit. All edits are additive except the
documentation reconciliation in `docs/design.md` §4.17 (a sketch update,
recoverable from git history). Snapshot generation is repeatable
(`INSTA_UPDATE=always` then verify no pending snapshots remain). No data
migrations, no external side effects.

## Artefacts and notes

To be filled with red/green transcripts and the final diff summary as work
proceeds.

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
/// options. The wrapper is command-line entry-point structure only; it
/// performs no configuration loading and reads no environment state.
///
/// The inner command is renamed to `subcommand_name`; its options,
/// version, and help text are preserved verbatim. `installed_bin_name`
/// becomes the wrapper's display name. Version, Cargo-style colouring,
/// invocation-hint error text, and tracing setup remain the caller's
/// responsibility (see the users' guide).
pub fn external_subcommand(
    installed_bin_name: impl Into<clap::builder::Str>,
    subcommand_name: impl Into<clap::builder::Str>,
    command: clap::Command,
) -> clap::Command;
```

Dependencies: clap 4 only (already a dependency with the `derive` and
`string` features). Dev-only additions: none (rstest, rstest-bdd, insta, and
proptest are already dev-dependencies of `ortho_config`). No new crates, no
new features, no changes to `ortho_config/Cargo.toml` expected; if one turns
out to be needed, Tolerance 3 applies.

## Revision note

Initial draft (2026-08-06). Authored after a four-agent reconnaissance pass
(design documents; current `cargo-orthohelp` entry point; testing and
documentation conventions; external prior art via web research) and pending
revision by a community-of-experts design review before being presented for
approval. Status is DRAFT; no implementation may begin until the user
approves the plan.
