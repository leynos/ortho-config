# Document the derive-friendly Cargo subcommand template

Status: DRAFT — awaiting explicit approval before implementation.

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current throughout execution.

## Purpose / big picture

A consumer should be able to turn an existing `clap::Args` options struct into
an installed Cargo subcommand by adapting the README or user's guide alone. The
worked example will show the complete parser, dispatch, dependencies, and
invocation forms. Running `cargo demo --verbose` and
`cargo-demo demo --verbose` should reach the same options value. Cargo
intentionally supplies `demo` as the first positional argument after the
executable name; a flat options parser cannot consume that token.

This task delivers roadmap 8.3.2. The current change publishes only this plan
and its documentation-index entry. Approval of the plan is required before
editing examples, tests, runtime code, or roadmap completion state.

## Constraints

Preserve `OrthoConfig::load`, configuration discovery, merge precedence, public
traits, and the existing `cargo-orthohelp` executable. The wrapper only models
the executable entry point. Do not implement the speculative macro from 8.3.3
or the reusable regression-fixture framework from 8.3.4.

Use the requested `#[command(name = "cargo", bin_name = "cargo-demo")]`
annotation, a `Cli` field annotated `#[command(subcommand)]`, and a
`clap::Subcommand` enum containing `Demo(DemoArgs)`, where `DemoArgs` derives
`clap::Args`. Explain that `demo` replaces `<name>` consistently in the
installed binary, enum command name, and invocation examples. Preserve the
options struct's existing fields when adapting the template.

Follow `AGENTS.md`, the documentation style guide, and the live Makefile. Keep
each Rust file below 400 lines. Every new public-guide fence needs a unique
`tested-example` marker and executable validation. Keep
`ortho_config/README.md` identical to the root README. Do not mutate the
process environment in tests; use existing child-process environment setup.

Run all gates sequentially through a scrutineer agent with `tee` logs and
pipeline failure propagation. Preserve compiler admission and the shared Cargo
cache. Do not build under `/tmp`, replace compiler wrappers, install Whitaker,
modify shared services, or kill another agent's processes.

## Tolerances (exception triggers)

Stop and record a proposed deviation if runtime or macro behaviour must change,
if a public API must change, or if a new production dependency is needed.
Existing test dependencies `rstest`, `googletest`, and `pretty_assertions` are
already available. Add `tracing = "0.1"` only to the generated example manifest
and documented dependency recipe; the repository already uses tracing. This
small manifest extension is in scope. Any environment, compiler-admission, or
other shared runner repair must be separately reviewed before execution.

Stop after three unsuccessful repairs of the same gate, on disk exhaustion, or
on a build-limits refusal. Waiting for an admitted compiler slot is normal and
is not a reason to retry. There is no elapsed-time implementation limit. If a
discovery invalidates the design, record affected requirements and options, set
status to BLOCKED, and obtain acceptance of the deviation before proceeding.

## Risks

The highest documentation risk is confusing the requested derive spelling with
the shipped hand-built helper. The helper uses `bin_name("cargo")`, whereas
this template uses `bin_name = "cargo-demo"`. Both accept injected argv, but
their usage strings differ. Assert the selected template's actual output and
explain the distinction beside the example.

A medium-likelihood risk is documentation-test drift: an unregistered or copied
snippet can look correct without being tested. Compile the actual marked
fences, register both IDs, and retain the README parity test.

Implementation validation is currently blocked by the existing runner.
`ExampleWorkspace::new` uses `tempfile::tempdir()` and the child runner places
its Cargo target below that directory. `TMPDIR` is unset on this host, so this
would build under `/tmp`. Its Cargo environment allow-list also drops the live
`RUSTC_WRAPPER` and `RUSTDOC` admission adapters. No nested build, including
`make test`, may run until prerequisite P0 below is discharged. Plan
publication uses documentation gates and is not blocked by this runtime
prerequisite.

## Progress

- [x] (2026-10-14) Read repository guidance and loaded CodeGraph, Rust Router,
  ExecPlans, Firecrawl, and community-of-experts review guidance.
- [x] (2026-10-14) Renamed the local branch to
  `8-3-2-document-derive-friendly-cargo-subcommand-template`.
- [x] (2026-10-14) Confirmed 8.3.1 is complete and inspected the existing
  wrapper and executable documentation harness with a Wyvern team.
- [x] (2026-10-14) Drafted the contracts, verification scope, and milestones.
- [x] (2026-10-14) Reviewed with six expert perspectives and reconciled
  runner prerequisites, tracing dependencies, and helper ownership sequencing.
- [ ] Validate, commit, push, configure the requested upstream, and publish
  the draft plan pull request (PR).
- [ ] Receive explicit implementation approval.
- [ ] M1: publish and exercise both derive examples.
- [ ] M2: reconcile documentation and mark 8.3.2 done after final gates.

## Surprises & discoveries

The 8.3.1 ADR amendment already records the helper's Cargo-style usage choice.
The derive sketch in design §4.17 still deliberately uses the requested
installed-binary name. The new material must distinguish these surfaces rather
than silently rewriting the requirement.

The requested `docs/rtest-bdd-users-guide.md` is named
`docs/rstest-bdd-users-guide.md` in this checkout. The requested
`docs/scripting-standards.md` does not exist. No script is proposed, so this
missing reference does not prevent planning; repository instructions govern
commands, and any later script proposal must first resolve the missing guide.

## Decision log

- 2026-10-14: Retain the literal derive annotation requested by 8.3.2 and
  document its help difference from the helper. This satisfies the current
  roadmap without changing the accepted 8.3.1 implementation.
- 2026-10-14: Reuse the public-document fence registry and `ExampleWorkspace`
  rather than copy snippets into a new fixture crate. This keeps tests tied to
  what consumers read and leaves shared fixture generalization to 8.3.4.
- 2026-10-14: Use executable documentation integration tests for this finite
  dispatch contract. No new algorithm, business rule, or semantic kernel needs
  Verus, Kani, property testing, or output-format snapshots.

## Outcomes & retrospective

Planning is in progress. No implementation is approved or delivered. Record
review findings, actual gate results, commit identifiers, and PR publication
here as they become available. At completion of implementation, reconcile the
upstream documents and acceptance evidence before marking the plan COMPLETE.

## Context and orientation

`docs/roadmap.md` §8.3 records the completed hand-built helper and the four
remaining acceptance bullets for 8.3.2. `docs/design.md` §4.17 and
`docs/adr-004-cargo-external-subcommand-entry-point.md` define the entry-point
boundary. `ortho_config/src/cargo/mod.rs` owns the hand-built helper.
`cargo-orthohelp/src/cli/mod.rs` contains the existing `Cli`,
`CargoSubcommand::Orthohelp(Args)`, and `Args` structures; these are evidence,
not required reading for users of the finished template.

Extend the Cargo external-subcommand section of `docs/users-guide.md` and add a
short standalone section near the helper mention in `README.md`. Mirror the
README into `ortho_config/README.md`. The guide currently tells derive users to
consult internals instead of providing a template.

`ortho_config/tests/documentation_examples/mod.rs` extracts the exact fenced
source. `documentation_examples_tests.rs` in the same tests directory owns
`EXPECTED_EXAMPLE_IDS` and README parity. The existing integration target
`documentation_examples_rust_tests` already exercises Cargo dispatch through
`ExampleWorkspace::add_binary_as`, `build`, `run`, and `run_cargo_subcommand`.
Its source has 369 lines at the planning baseline; place the new test in a
small child module rather than exceeding 400 lines.

## Conformance basis

The repository baseline is `dc979878b5728bbb23866df9c05c2570bdb5629e`. There is
no separate terms-of-reference document for this task. The normative sources
are roadmap 8.3.2, design §4.17, ADR-004 including its 2026-08-09 amendment,
and the user's explicit requirements. Recheck their revisions when
implementation begins.

R1 is the complete derive wrapper in both public guides. R2 is Cargo's injected
argument and both invocation forms. R3 is the unchanged configuration-loading
boundary. R4 is self-contained adaptation without external reference reading.
M1 discharges R1–R4 through exact-fence compilation, process tests, and a
reader walkthrough. M2 discharges documentation consistency and completion
recording after all acceptance evidence passes.

Consult these repository references for the following specific boundaries:

- [Design](../design.md) §4.17 and
  [ADR-004](../adr-004-cargo-external-subcommand-entry-point.md) govern wrapper
  shape and entry-point ownership.
- [CLI localization](../cli-localization-design.md) and
  [Fluent library design](../localizable-rust-libraries-with-fluent.md) keep
  localization and subscriber setup under application authority.
- [cargo-orthohelp design](../cargo-orthohelp-design.md) §6 and
  [agent-native CLI design](../agent-native-cli-design.md) §7 define the
  reference executable boundary; no IR or policy schema changes are needed.
- [Clap dispatch integration](../clap-dispatch-and-ortho-config-integration.md)
  provides background, not a requirement to adopt another dispatch dependency.
- [Error design](../improved-error-message-design.md) is proposed; examples
  must use current clap errors rather than invent proposed error variants.
- [rstest fixtures](../rust-testing-with-rstest-fixtures.md),
  [doctest guidance](../rust-doctest-dry-guide.md), and
  [dependency injection](../reliable-testing-in-rust-via-dependency-injection.md)
  govern fixture reuse, testing published source, and process isolation.
- [BDD guidance](../rstest-bdd-users-guide.md) applies if scenarios change;
  [complexity guidance](../complexity-antipatterns-and-refactoring-strategies.md)
  supports a small test module and reuse of existing process helpers.
- [Developer's guide](../developers-guide.md),
  [repository layout](../repository-layout.md), and
  [documentation style](../documentation-style-guide.md) govern maintenance.

Use the `codegraph-mcp` skill for structural navigation, `rust-router` to select
`rust-unit-testing` for new tests, `execplans` to maintain this document, and
`firecrawl-mcp` for upstream gaps. Use `logisphere-experts` for design review,
`commit-message` for commits, and `pr-creation` for the draft PR.

## Verification plan

The abstract input model is an argument vector `[executable, token, options…]`.
Preconditions are that an executable named `cargo-demo` is discoverable and the
wrapper has one command named `demo`. Cargo dispatch supplies `token = demo`;
direct invocation must supply the same token. Successful parsing yields the
existing options value. The representation relationship is
`Cli.command = CargoSubcommand::Demo(options)`. Invalid tokens or options are
rejected by clap before application dispatch.

External assumptions are Cargo's documented argument forwarding, clap's
`Parser`/`Args`/`Subcommand` derive contracts, and the operating system's child
process and executable lookup behaviour. Test against real Cargo and clap;
there is no attempt to prove their internals.

V1 (R1, R4): both `readme-cargo-derive-subcommand` and
`guide-cargo-derive-subcommand` must exist, contain complete compilable Rust,
and be registered. Add an `rstest` case per fence to
`ortho_config/tests/documentation_examples/cargo_derive.rs`, included by the
existing Rust-example target. Build each exact fence as `cargo-demo` once and
reuse that binary for every option, help, and failure probe. Serialize the two
expensive cases with the existing `serial_test` support, using a common lock
name on the parameterized test. Ensure both fences execute. The red stage fails
because those IDs do not exist; the green stage succeeds only after both
published examples exist and compile. Keep README parity covered by
`published_crate_readme_matches_repository_readme`.

V2 (R2): for each compiled example, execute both direct and Cargo-dispatched
forms with `--verbose` and without that option; assert the observable option
value is respectively true and false. Use
`tracing::debug!(verbose = args.verbose, "Cargo subcommand accepted")` after
dispatch. Before parsing, initialize a `tracing_subscriber::fmt()` subscriber
with `.with_max_level(tracing::Level::DEBUG)`, `.without_time()`,
`.with_ansi(false)`, `.with_target(false)`, and
`.with_writer(std::io::stderr)`. This example deliberately enables debug output
without relying on `RUST_LOG`, which the Cargo-dispatch runner does not
forward. Explain that production applications own subscriber configuration. Add
`tracing = "0.1"` to `render_manifest` in `documentation_examples/workspace.rs`
and show it with `tracing-subscriber = "0.3"` and clap derive in both
dependency recipes. Do not accept exit status alone as evidence that the
argument was consumed.

V3 (R2, R4): check root help, inner help through direct invocation, and inner
help through Cargo dispatch. The selected naming contract should render
`Usage: cargo-demo <COMMAND>` and `Usage: cargo-demo demo [OPTIONS]`. Confirm
those strings against executable output. Check missing subcommand, wrong
subcommand, flat `--verbose`, and an unknown inner option fail; help must
succeed. Assert stable usage fragments and exit success/failure rather than
entire version-sensitive diagnostics. Use `googletest` for status and substring
assertions and `pretty_assertions` for equality of the extracted `verbose=true`
or `verbose=false` field. Do not compare formatter padding or an entire log
line.

V4 (R3): manually review both guide explanations and the branch diff to confirm
that no loader or merge implementation changes. State ordinary precedence as
defaults → files → environment → explicit CLI arguments, and preserve the
optional selected-profile tier between files and environment for opted-in
structs. Never call `OrthoConfig::load()` after parsing as a way to discard
Cargo's injected token: that would reparse the same process arguments. The
existing configuration-loading tests remain regression coverage.

Non-vacuity: both option values and real Cargo dispatch are positive witnesses.
The missing-ID red stage proves the registry is exercised. During development,
rename the `Demo` variant or add `#[command(name = "other")]` in a disposable
copy of the example. Confirm it still compiles and the dispatch assertion fails
because `demo` is rejected, then restore it before final gates. The flat
invocation case independently demonstrates the original failure mode. Record
the negative-control result, not just a passing suite. A whole-program
exhaustive claim is neither made nor required.

No new business logic or non-trivial invariant is introduced. The argument
relationship is integration plumbing over documented third-party interfaces.
Verus is not warranted; no proof milestone or speculative lemma is proposed. If
a future design introduces a pure semantic kernel, stop and revise the abstract
specification, preconditions, postconditions, and candidate lemmas before
implementation. Any selected formal proof must use Verus substantively, without
`assume`, `admit`, or trusted proof bodies, alongside executable tests.

`rstest-bdd` is not needed for an additional duplicate scenario: these are
executable documentation contracts in an existing integration target. No
multivariant output format is added, so `insta` is unnecessary. No new
open-ended transformation or state machine warrants `proptest` or Kani.

## Plan of work

P0 is a mandatory pre-M1 prerequisite, distinct from approval of this plan. The
current runner violates the two host constraints recorded in Risks. Accept an
independently reviewed upstream repair, or obtain separate approval for a
bounded runner repair. Its evidence must show that nested Cargo retains the
active `RUSTC_WRAPPER` and `RUSTDOC`, uses a writable build target outside
`/tmp`, and retains the shared Cargo cache. A command-scoped `TMPDIR` outside
`/tmp` can solve the location issue but cannot repair the wrapper allow-list.
Do not substitute a parallel fixture runner or weaken isolation. Record the
repair revision and checks here before any nested baseline or `make test`. Also
inspect `documentation_examples/process_runner.rs`: its 600-second subprocess
deadline includes compiler-admission waiting time. The prerequisite must
establish an admission-compatible timeout policy so queued builds are not
terminated as hung; any deadline repair belongs to the separately reviewed
runner change. Until then, implementation validation remains BLOCKED; the plan
can still be reviewed and published. After P0 and explicit plan approval,
recheck the live baseline and establish the existing documentation tests as
passing.

For M1, add the two expected IDs and a new small `cargo_derive` test module. Use
`#[path = "documentation_examples/cargo_derive.rs"] mod cargo_derive;` in
`documentation_examples_rust_tests.rs`; reuse the sibling `workspace` and
`documentation_examples` modules. Keep any local case runner private to this
module and document its allowed call sites in the developer's guide in M1,
before introducing it; do not create a second shared workspace or command
runner. Run the red test before adding the examples.

Then add complete examples with imports, `Cli`, enum, existing-style options
struct, and `main`. Enable clap's `derive` feature in the shown dependency
instructions and give the executable its `cargo-demo` name through package or
`[[bin]]` configuration. Use inline commands for invocation explanations where
a separate executable console fence adds no value. Describe the first
positional argument as `argv[1]`, with `argv[0]` being the executable. Show
extraction of `DemoArgs` by matching `Cli.command`, and explain where existing
application logic receives it. Link existing configuration-merging guidance
rather than inventing a new loader. Include both help forms and the required
direct token.

Add the user's-guide subsection and replace the existing instruction to read
`cargo-orthohelp` internals. Keep README material compact but independently
adaptable. Mirror the packaged README. Document the installed-binary help
spelling beside the derive example and distinguish the helper's Cargo usage
spelling. Keep existing binary-level tracing and diagnostic obligations visible.

For M2, record the template naming distinction in design §4.17 and add a
cross-reference to the guide. The accepted ADR needs no new architectural
decision; if the text implies the amendment covers all derive templates, add a
narrow clarification rather than changing the accepted helper. Document the new
fence IDs and test module ownership in the developer's guide. Reconcile links
and record the implementation evidence. Mark roadmap 8.3.2 and all four
acceptance bullets done using checked boxes only after R1–R4 pass. Do not mark
8.3.3 or 8.3.4 done.

## Milestones and plateaus

M1 delivers both complete documented templates and their exact-source tests.
Its acceptance is V1–V4, the negative control, and all required gates. Review
scope against R1–R4 and the accepted entry-point design before committing. The
repository should remain usable with no runtime or dependency changes.
Remaining work is final design/developer cross-references and completion
recording. Recovery is a focused revert of the milestone commit if needed; no
compatibility layer or persisted-data migration is needed.

M2 delivers coherent design and maintenance guidance plus completed roadmap
status. Its acceptance is a repeated focused documentation suite, all required
gates after the final edits, and a review that every requirement has evidence.
Confirm no unapproved interface, dependency, trust boundary, or format change,
and update all trace links. Commit this milestone separately. Recovery is a
focused revert that also restores incomplete roadmap status. No implementation
milestone may be reported complete while a gate or prerequisite is blocked.

## Concrete steps

Run commands from the repository root. Set a log prefix under `/tmp`, but keep
all build artefacts on the normal workspace storage. Before nested builds,
resolve the runner prerequisite described above. The focused commands are:

```sh
cargo test -p ortho_config --test documentation_examples_tests
cargo test -p ortho_config --test documentation_examples_rust_tests documented_cargo_external_subcommand
cargo test -p ortho_config --test documentation_examples_rust_tests documented_derive_cargo_subcommand
```

Name the new parameterized test `documented_derive_cargo_subcommand`. The last
command must exercise both document cases, fail for missing IDs in red, and
pass in green. Capture output and assert that the filtered run actually
executed cases rather than reporting zero tests. There is no focused Make
target; full validation uses the canonical Make targets.

A scrutineer runs these sequentially, each with `set -o pipefail` and a separate
`tee` log, after every major implementation milestone:

```sh
make fmt
make check-fmt
make lint
make test
make markdownlint
make nixie
```

For example, use
`make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-8-3-2.out` with pipefail
enabled. Inspect formatting changes and retain only intended edits. Never run
these commands concurrently. `make test` runs all-target/all-feature Rust tests
and Python tests in the live Makefile; do not substitute the older simplified
command quoted in generic guidance. `make lint` includes rustdoc, Clippy, and
Whitaker. A missing Whitaker installation is a reported blocker. Generated
spelling changes require the repository's separate-base-change workflow if
unrelated; never hand-edit `typos.toml`.

For this plan-only PR, run documentation formatting, `make check-fmt`,
`make markdownlint`, and `make nixie`. Runtime tests are not implementation
approval and need not execute speculative examples. Record any blocked gates
explicitly rather than calling them passed.

## Validation and acceptance

A reviewer can copy either documented program and its dependency instructions
into a package producing `cargo-demo`, install it on `PATH`, and run
`cargo demo --verbose`, `cargo-demo demo --verbose`, and `cargo demo --help`.
The first two select identical true option values; without `--verbose` the
value is false. Help identifies the inner command and its option. Flat
`cargo-demo --verbose` fails because it omits the command token. The docs must
explain that outcome without requiring a trip to Cargo's reference manual.
Perform the walkthrough independently for each document: check every directly
used crate and feature, package or binary naming, installation command, and
`PATH` step against only its published instructions. The richer generated test
manifest must not mask an omitted dependency.

Acceptance also requires exact-fence tests for both documents, README parity,
unchanged merge semantics, recorded successful gates, and a checked roadmap
entry only at implementation completion. The implementor records red, green,
negative-control, and final gate log paths in this plan. Reviewers must be able
to distinguish passed, not run, and blocked evidence.

## Idempotence and recovery

Document edits and checks can be repeated. Reuse stable marker IDs, avoid
adding duplicate index entries, and inspect `git diff` after repository-wide
formatting. Do not delete other work or caches. A failing gate is read from its
log and rerun only after a relevant repair. Keep the last validated milestone
commit as the rollback point. Record upstream design deviations before work
continues, not after a workaround has landed.

## Interfaces and dependencies

Only documentation and test-local interfaces are added. Reuse clap derive, the
existing `ExampleWorkspace` process methods, and the existing assertion
dependencies. The generated example manifest gains tracing, an already-used
library; no workspace dependency is introduced. No core trait, public helper,
localization schema, policy schema, or output generator changes. If a local
fixture needs a reusable helper, first inspect existing equivalents and record
its ownership and permitted call sites in the developer's guide.

## Artefacts and notes

Firecrawl research on 2026-10-14 confirmed the argument-forwarding contract in
[Cargo's external-tools reference](https://doc.rust-lang.org/cargo/reference/external-tools.html).
The
[clap Cargo cookbook](https://docs.rs/clap/latest/clap/_derive/_cookbook/cargo_example/index.html)
currently displays a builder example with Cargo-style usage; it is evidence
for dispatch shape, not proof of the exact derive annotation requested here.
The executable document tests remain the acceptance evidence for that spelling.

Wyvern evidence is available in context pack `pk_hevsvpzl`; the final plan is
self-contained and does not require that temporary pack to execute.

Expert review (2026-10-14): Pandalump and Wafflecat confirmed the entry-point
boundary and requested an explicit runner prerequisite and earlier helper
ownership documentation. Telefono and Doggylump identified that the generated
manifest lacks tracing and Cargo dispatch cannot receive a test `RUST_LOG`
override. Dinolump and Buzzy Bee checked adaptation and nested-build costs. The
plan now selects deterministic subscriber configuration, a narrow generated
manifest addition, and P0 before any nested build, including
admission-compatible deadlines. Both document builds are serialized and their
binaries reused for all probes; the independent walkthrough also audits the
dependency instructions. Existing source-fence tests provide stronger drift
protection than copied fixtures, at the cost of a separately resolved runner
prerequisite. No expert review constitutes user approval to implement.

Revision note (2026-10-14): the reviewed draft ties the four roadmap
requirements to published-source tests, records the help-name distinction and
runner prerequisite, and reserves all implementation for explicit approval.
