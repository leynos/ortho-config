# Developers guide

This guide documents how contributors work with tests in this repository. It
focuses on behavioural tests because they span multiple crates and have the
highest maintenance cost when patterns drift.

## Current testing strategy

The workspace runs one unified test workflow via Make targets:

- `make check-fmt`
- `make lint`
- `make test`

These are required quality gates for code changes. Behavioural coverage runs
inside the standard Rust test harness, not a bespoke test runner.

## Clap default inference

The `cli_default_as_absent` default path is split between parse-time metadata
and generated loader code. `ortho_config_macros/src/derive/parse/clap_attrs.rs`
retains the raw `default_value`, the leaf type and field shape, and parser
hints such as `value_parser`, `value_enum`, delimiters, and enum case handling.
Keep this metadata in the parse intermediate representation (IR); do not
reconstruct it from generated tokens later in the pipeline.
The parser-faithful approach is recorded in the
[design decision log](design.md#9-decision-log).

The parse layer accepts scalar, `Option<T>`, and `Vec<T>` fields. It rejects
nested wrappers and map fields when their clap string default cannot be
replayed faithfully. Unsupported shapes should receive a focused compile-time
diagnostic that points to an explicit `#[ortho_config(default = ...)]`
alternative. Parser settings that change tokenization or accepted spelling
must either be retained in the IR and applied to the synthetic argument, or
cause the shape to be rejected during parsing.

`ortho_config_macros/src/derive/build/defaults.rs` owns the replay boundary.
`DefaultStructInit` keeps fallible default resolutions separate from the
infallible defaults struct fields: generated code builds a one-argument clap
command, parses the captured default, and then supplies the resolved value to
the defaults layer. A conversion failure is appended to the existing error
accumulator as `OrthoError::DefaultValueConversion`; it must not be handled by
`unwrap`, `expect`, or a panic in generated code. Explicit
`#[ortho_config(default = ...)]` remains higher precedence than inferred clap
metadata.

When changing this path, update parse-IR tests, generated-loader integration
tests, and compile-fail coverage for unsupported shapes. Include cases where a
file or environment value overrides the inferred default, while an explicit
CLI value still wins. Run the standard quality gates before requesting a
CodeRabbit review.

### Nextest test-group serialization

`.config/nextest.toml` assigns two test binaries to single-threaded groups:

- **`rstest_bdd`** — each BDD scenario runs in its own OS process under nextest,
  so a process-local `Mutex` cannot protect the shared `target/orthohelp` cache
  directory. Setting `max-threads = 1` for this binary ensures scenarios run
  sequentially and do not race on cache reads or `remove_dir_all` calls.
- **`powershell_windows`** — both test cases invoke
  `cargo-orthohelp --format ps` for the same package, which writes to the same
  ephemeral bridge directory (`target/orthohelp/<hash>/`). On Windows,
  `cargo build` holds a read lock on `Cargo.toml`; a concurrent
  truncate-and-rewrite from the second invocation violates that lock.
  Serializing the binary prevents the race.

Do not remove the `max-threads = 1` constraint from either group without first
verifying that the underlying shared-state access has been eliminated.

## Subcommand dispatch changes

Cargo's external-subcommand contract is an entry-point concern, not a
configuration-loading concern. When the way a `cargo-*` binary accepts or
forwards the injected subcommand token is changed, update all of the following
in the same change:

- `docs/design.md` §4.17 and
  [ADR-004](adr-004-cargo-external-subcommand-entry-point.md).
- `docs/roadmap.md` if the work remains tracked there.
- Any user-facing guide or README that shows `cargo <name>` or
  `cargo-<name> <name>` invocation.
- Regression coverage for both `cargo <name> [OPTIONS]` and
  `cargo-<name> <name> [OPTIONS]` once the repository adds or revises those
  tests.

## Schema ownership

Documentation IR, agent context, and policy reports have separate owners. See
[ADR-003](adr-003-define-schema-ownership-for-agent-native-contracts.md) for
the accepted decision.

Add localized human-documentation fields to `ortho_config::docs` only when they
are required by generated documentation, localization, roff, PowerShell help,
or other human-facing reference material. Those fields are versioned by
`ORTHO_DOCS_IR_VERSION` and exposed through `OrthoConfigDocs`.
`OrthoConfigSubcommandDocs` is part of the same human-documentation IR contract
and uses the same versioning boundary for recursive subcommand metadata.

Add compact agent invocation fields to `ortho_config::agent_context` when
downstream applications need a reusable machine-readable command contract. Use
`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` for compatibility. Do not add Fluent
message identifiers, localized long prose, or renderer-specific output
structures to the agent-context schema.

Use `AGENT_CONTEXT_KIND_SUFFIX` and `agent_context_kind` as the single source
for the agent-context `kind` discriminator. Do not hand-format
`"<tool>.agent_context"` at call-sites. `kind` identifies the payload family;
compatibility detection stays on `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`.

Skill manifest descriptors are part of this agent-context contract: keep
`SkillManifest`, `SkillCommandRef`, and `AgentContext.skill_manifests` in
`ortho_config::agent_context`, and keep downstream manifest prose
application-owned.

`localizer::identifier::normalize_segment` is the single source of truth for
strict runtime and derive-time Fluent identifier segments. Reuse it from
command localization, derive output, and future lookup-id generation instead of
duplicating ASCII normalization rules. Keep the tolerant catalogue load path in
`localizer::fluent` separate: it exists only to pre-normalize hand-authored
resource ids such as dotted catalogue keys before Fluent parses them, and must
not be used to validate generated command ids.

Use `LocalizedParse` for default-base localized clap parsing and
`parse_localized_command` when callers need to pass a command that has already
been localized with `LocalizeCmd::with_base`. Keep the two parse-error paths in
that helper distinct: errors from `try_get_matches_from_mut` already have the
command available, while `FromArgMatches::from_arg_matches` errors must be
enriched with `with_cmd(&command)` so missing-subcommand translations retain
`valid_subcommands`.

Compile-time coverage for this API belongs in `trybuild` pass/fail cases that
exercise the public trait bound (`LocalizedParse: clap::Parser`). Fluent-unsafe
command identifiers are still a documented runtime panic contract owned by
`message_id_for` and `LocalizeCmd::localize`, so keep that coverage in ordinary
runtime panic tests until derive-emitted identifiers move validation to compile
time.

Add agent-native warning and hard-failure report fields to
`cargo_orthohelp::policy` while `cargo-orthohelp` is the only emitter. Use
`ORTHO_POLICY_REPORT_SCHEMA_VERSION` for compatibility and keep rule
identifiers, finding codes, severities, and source locations machine-stable.
Extract the report model into `ortho_config` only after a new ADR approves
shared ownership.

Use `rstest` for schema unit tests. Add `rstest-bdd` behavioural scenarios and
end-to-end tests when a change affects observable CLI behaviour, generated
artefacts, persisted output, integration contracts, stdout, stderr, or exit
codes. Do not add Kani, Verus, or property-test tooling unless the change
introduces a substantive invariant across a range of inputs, states, orderings,
or transitions.

When adding metadata fields, record the legacy default beside the field
definition and cover the absent-field case in tests. Defaults must be explicit:
do not infer JSON support, mutation effect, interaction mode, exit classes,
pagination, profile support, capability provenance, delivery support, feedback
support, or execution-ledger support from command names or missing data. Apply
defaults in OrthoConfig readers, generators, or transforms; do not rely on JSON
Schema validation to mutate payloads.

Keep generated human documentation compatible unless a roadmap item approves a
versioned migration. The `cargo-orthohelp` `ir`, `man`, `ps`, `agent-context`,
and `all` formats, their accepted spellings, generated paths, and process
success/failure contract are externally visible behaviours. Add policy and JSON
status surfaces beside those formats rather than changing them.

Keep schema ownership aligned with ADR-003. Localized human-documentation data
belongs in `ortho_config::docs`, compact reusable agent context belongs in
`ortho_config::agent_context`, and policy reports stay in
`cargo_orthohelp::policy` until a later ADR extracts a shared report model. Do
not introduce crate dependency cycles to share convenience helpers; move shared
contracts downward instead.

### Generating agent-context output

`cargo-orthohelp --format agent-context` reads the same bridge `DocMetadata` as
the human documentation generators and writes `<out>/agent-context.json`.
`cargo-orthohelp --format all` writes the same file beside IR, man-page, and
PowerShell artefacts. Keep the transform projective: it may copy or derive
compact command metadata from the bridge IR, but it must not inspect rendered
roff, PowerShell help, or localized IR output.

`--format agent-context` is the generator format. Downstream applications that
emit their own runtime payload expose `context --json` as defined by
[ADR-007](adr-007-downstream-context-command-naming.md).

Agent-context output is not localized. The current transform may use the short
en-US command description as `AgentCommand.summary`, but it must not copy
localized long help, Fluent identifiers, roff fragments, or PowerShell wrapper
structures into `ortho_config::agent_context`.

Represent positional inputs by leaving `AgentInput.long` absent. The adapter
detects a positional input from existing CLI metadata when
`cli.long.is_none() && cli.short.is_none() && cli.takes_value`. Do not add a new
`AgentInput` kind field unless a later ADR or roadmap item changes the schema
ownership decision.

Treat `AgentInput.default` as display-only. It is normalized for stable
goldens, but it is not executable or machine-parseable.

Evolve the schema through the compatibility policy in
[agent-native-cli-design.md](agent-native-cli-design.md) §8.2. Bump
`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` for breaking changes such as field
renames, enum wire-string changes, null-versus-omitted changes, required-field
changes, or `deny_unknown_fields`. Additive optional fields stay within the
same version, and consumers must ignore unknown fields.

Pin every schema change with the `ortho_config` wire snapshot and
variant-exhaustive enum tests. Add end-to-end `cargo-orthohelp` goldens by
using the existing multi-root fixture pattern: add a root type to
`tests/fixtures/orthohelp_fixture` and select it with `--root-type` rather than
creating a new workspace crate. Keep the 6.2.2 nested agent-context assertions
separate from the 6.1.2 roff, PowerShell, and IR nested-renderer assertions. Do
not duplicate a per-field schema table here; use the rustdoc for
`ortho_config::agent_context`, the §3.2 JSON example, and the committed wire
snapshot as the canonical field references.

Run `coderabbit review --agent` after major milestones that change schemas,
documentation contracts, or externally visible behaviour. Clear its concerns
before moving to the next milestone.

### Public API

The following functions form the stable agent-context surface for 6.2.2.

`cargo_orthohelp::agent_context`:

```rust
/// Convert bridge documentation IR into an `AgentContext` payload.
///
/// `package` is used to populate `AgentContext.package`.  Pass `None` for
/// `localizer` to omit command summaries; pass an EN-US `Localizer` to
/// include them.
#[must_use]
pub fn bridge_ir_to_agent_context(
    meta: &DocMetadata,
    package: &str,
    localizer: Option<&dyn Localizer>,
) -> AgentContext
```

`cargo_orthohelp::output`:

```rust
/// Serialise `payload` as pretty-printed JSON and write it atomically to
/// `<out_dir>/agent-context.json`.
///
/// Returns the path of the written file on success.  Fails with
/// `OrthohelpError::Io` for filesystem errors and `OrthohelpError::IrJson`
/// for serialisation failures.
pub fn write_agent_context(
    out_dir: &Utf8Path,
    payload: &AgentContext,
) -> Result<Utf8PathBuf, OrthohelpError>
```

`cargo_orthohelp::cli::OutputFormat`:

```rust
/// Emit a compact, non-localized agent-context JSON manifest.
/// Writes `<out_dir>/agent-context.json`.
AgentContext,
```

### Consumer dependency tiers

[Agent-native CLI assistance design](agent-native-cli-design.md) §2.2 is the
authoritative source for the hard and soft ship-time dependency tiers that
apply to Weaver, Netsuke, and other downstream consumers. When changing a
hard-dependency capability, update §2.2 and the cited roadmap item in the same
change. When changing a soft-dependency capability, also record which roadmap
item any temporary local consumer adapter shadows, so its eventual replacement
can be tracked.

Run `coderabbit review --agent` after major milestones that change schemas,
documentation contracts, or externally visible behaviour. Clear its concerns
before moving to the next milestone.

## Agent-native architecture boundary

Agent-native CLI assistance is contract modelling work inside OrthoConfig, not
a transfer of downstream application execution into this repository. The
canonical contract and boundary document is
[Agent-native CLI assistance design](agent-native-cli-design.md).

Contributors should keep reusable command-contract policy in OrthoConfig:

- command, option, output, and workflow metadata;
- documentation IR, agent-context schema, and related versioning policy;
- generated human documentation and compact agent-facing context;
- vocabulary, structured-output, and bounded-list lint policy; and
- optional shared primitives for profiles, delivery targets, feedback stores,
  skill manifests, and execution-ledger metadata.

Downstream applications own the execution side of those contracts. Weaver,
Netsuke, or another consumer remains responsible for command execution, domain
side effects, sandboxing, safety policy, long-running job semantics, provider
routing, build graph behaviour, and application-specific persistence. If
OrthoConfig executes downstream commands or owns downstream side effects, stop
and revisit the boundary in the agent-native design.

## Behavioural test layout

Behavioural suites live in crate-local integration test targets:

- `ortho_config/tests/rstest_bdd/`
- `cargo-orthohelp/tests/rstest_bdd/`
- `examples/hello_world/tests/rstest_bdd/`

Feature files are in:

- `ortho_config/tests/features/`
- `cargo-orthohelp/tests/features/`
- `examples/hello_world/tests/features/`

Step definitions use `rstest-bdd` macros (`#[given]`, `#[when]`, `#[then]`) and
consume `rstest` fixtures. Scenario-local mutable state is modelled with
fixtures and `Slot<T>` values inside `#[derive(ScenarioState)]` structs.
Cross-scenario mutable sharing is forbidden; use `#[once]` only for expensive,
effectively read-only infrastructure.

Keep richer fixture families isolated. For example, `NestedDocsConfig` and
`NestedDocsContext` back `docs_ir_nested.feature`, and their steps live in a
fixture-specific `tests/rstest_bdd/behaviour/steps/nested_docs_steps.rs` module
rather than expanding unrelated step files.

### Integration test targets

`ortho_config` also carries plain `rstest` integration suites that are
distinct from the behavioural (`rstest-bdd`) suites above. Each directory-style
suite needs its own `[[test]]` stanza in `ortho_config/Cargo.toml`; a suite
without a stanza is not compiled or run by Cargo:

- `ortho_config/tests/subcommand/mod.rs` (cargo target `subcommand`) — tests
  for subcommand configuration helpers, covering `basic`, `cli`, `merge`,
  `nesting`, and `prefix` submodules. It shares `../util.rs` and
  `../support/to_anyhow.rs` via `#[path]` includes.
- `ortho_config/tests/clap_integration/mod.rs` (cargo target
  `clap_integration`) — integration tests for CLI parsing across multiple
  configuration sources, covering `common`, `config_path`, `error_cases`,
  `option_cases`, `parsing`, and (on Unix and Redox only) `xdg`.

Run either suite on its own for focused debugging:

```bash
cargo test -p ortho_config --test subcommand
cargo test -p ortho_config --test clap_integration
```

### Public documentation examples

The root README and `docs/users-guide.md` are executable documentation. Every
fenced block in either file must have a unique `tested-example` marker on the
immediately preceding line. `ortho_config/tests/documentation_examples/mod.rs`
owns parsing and lookup for these examples. It is test infrastructure only;
production code, other crates, and examples must not depend on it.

The loader is shared by the documentation integration-test targets. Each target
loads and parses the documents once, then borrows examples from its cached
registry. The cache is immutable and has no reset operation within an
integration-test process. Tests that need fresh input should call the pure
parser with owned text or use a separate integration-test target. Keep the
loader's scope limited to loading exact fence bodies, rejecting unmarked or
malformed fences, and querying stable identifiers. Put scenario policy in the
test target that consumes it: compile and run Rust, parse data formats, execute
documented commands, and compare observable output. Do not copy a fence into a
fixture because the copied text can pass after the published example has
drifted. Identifiers use a closed filename-safe grammar: start with a lowercase
ASCII letter, then use lowercase ASCII letters, digits, or single hyphens. The
parser and temporary workspace both enforce this boundary before constructing
paths.

`documentation_examples/workspace.rs` owns temporary Cargo package assembly for
Rust examples. Reuse it only from documentation tests that compile an exact
fence body. A workspace has one owner: operations that add, build, run, or
write require an exclusive mutable borrow. Concurrent scenarios must create an
independent workspace per thread rather than share one workspace. The helper
may add the dependencies needed to compile a published example, but it must not
rewrite that source. Keep the expected identifier registry closed so adding an
example without choosing its behavioural contract fails a test.
`documentation_examples/cargo_runner.rs` owns isolated Cargo process setup for
these documentation tests; reuse it only when executing a documented Cargo
workflow with a caller-owned temporary state directory. Run Cargo and child
binaries with cleared environments. Cargo receives only the toolchain and
platform paths it needs; binaries receive only the non-sensitive Windows
runtime variables named by the workspace allow-list and deliberate scenario
overrides. Keep fallible host-tool discovery and environment preparation at the
runner boundary; command construction consumes the prepared values without
starting subprocesses or writing files. The sibling `process_runner.rs` owns
bounded execution for these documentation tests only: route Cargo, host-tool,
and documented-binary commands through it, while keeping command construction
and exit-status policy with their existing owners. Run-file paths must contain
normal relative components only.

### Shared test-support helpers

`ortho_config/tests/support/to_anyhow.rs` owns the `ToAnyhow` trait, which
converts an `OrthoResult<T>` (`Result<T, Arc<OrthoError>>`) into an
`anyhow::Result<T>`, preserving the original error as the `anyhow` source. It
is the single conversion point for integration-test targets that report
failures through `anyhow`; before it existed, each suite grew its own free
function or inline `map_err(|err| anyhow!(err))` call, and those copies agreed
on behaviour by accident rather than by construction. New local
`to_anyhow`-style conversions are not permitted — depend on this module
instead.

`ortho_config/tests/support/discovery_builder.rs` owns `discovery_with`, a
pure builder that returns an independent `ConfigDiscovery` wired to the
caller's `MapEnv` only; it reads no other environment variables and touches
no filesystem, and it resolves its sole project root to `/workspace`. It is
owned by the injected-source discovery suites — candidates, properties,
telemetry, and metrics — which need a `ConfigDiscovery` configured
identically so that their results remain comparable. Any test needing a
deterministic discovery over an injected environment may reuse it.

`ortho_config/tests/rstest_bdd/behaviour/steps/common.rs` owns
`set_scalar_once`, `set_nonblank_scalar_once`, and the `SlotTakeOrExt`
extension trait's `take_or`. The first two centralize the repeated "guard,
validate, and populate a scalar slot" shape used to fill scenario `Slot`
state exactly once; `take_or` centralizes the repeated "take the slot's
value or fail with a descriptive error" shape. Both are scoped to step
modules under `behaviour/steps/`; step modules must use these helpers rather
than re-implementing slot-guard boilerplate.

## Snapshot tests

Use `insta` for renderer golden coverage that would be noisy as handwritten
string assertions. Place snapshots beside the integration test that owns them,
as `cargo-orthohelp/tests/golden/nested_subcommand_snapshots.rs` does, and
redact dates, absolute paths, and other environment-specific substrings with
`insta::with_settings!` filters before committing baselines.

Review snapshot changes with `cargo insta review`. For non-interactive baseline
creation in a controlled milestone, use `INSTA_UPDATE=always` and then verify
that no `.pending-snap` or `.snap.new` files remain before running the normal
quality gates.

## `rstest-bdd` v0.5.0 migration strategy

Status: adopted. See `docs/execplans/adopt-rstest-bdd-v0-5-0.md` for execution
history and rationale.

Migration guidance for contributors:

- Upgrade workspace pins to `rstest-bdd = "0.5.0"` and
  `rstest-bdd-macros = "0.5.0"`.
- Scenario functions must return `()` or explicit unit results
  (`Result<(), E>` / `rstest_bdd::StepResult<(), E>`). Avoid return type
  aliases in scenario signatures.
- Prefer `scenarios!(..., fixtures = [...], tags = ...)` for large feature
  bindings to reduce handwritten wrapper boilerplate.
- Prefer descriptive placeholder names over generic `{string}` placeholders so
  step signatures remain explicit and compile-time checked.
- Prefer underscore-prefixed fixture names only when no step resolves that
  fixture by name.
- Remove file-wide lint suppressions used only for historical generated-fixture
  warnings; retain only narrow, item-level `#[expect(...)]` annotations when
  still required.
- Keep scenario isolation as the default and reserve `#[once]` for shared
  infrastructure only.
- If a sync step needs async bridging, use
  `rstest_bdd::async_step::sync_to_async`.
- Keep tag names filter-friendly (`@name_part` style). Avoid dots in tag names
  used with `tags = "..."` expressions.

## Adding or changing behavioural tests

When adding scenarios or steps:

1. Add or edit the `.feature` file first.
2. Implement or update step definitions under the matching `tests/rstest_bdd`
   module.
3. Bind scenarios using `scenarios!` where possible; use explicit `#[scenario]`
   only when a feature needs bespoke fixtures or per-scenario control.
4. Keep assertions user-observable (`Then` steps) and avoid asserting private
   internals unless the behaviour cannot be observed externally.
5. Run the full required quality gates before finalizing.

## Observability

OrthoConfig and `cargo-orthohelp` follow a single observability convention so
that downstream applications can attach the subscribers and exporters they
prefer without contending with this workspace for global state.

- Use the `tracing` crate for all diagnostic output. Prefer structured
  `tracing::{trace, debug, info, warn, error}` events and spans over `println!`,
  `eprintln!`, or direct `log` macros. Attach fields for identifiers, state,
  and error context so subscribers can filter and correlate events without
  parsing message text.
- Wrap meaningful units of work in spans. Use `#[tracing::instrument]` or
  explicit spans around request handling, command execution, retries, and
  background jobs. Do not hold a `Span::enter()` guard across `.await`; use
  `Instrument::instrument` or scoped synchronous spans instead.
- Use the `metrics` crate where usage, uptake, failure, or mitigation metrics
  are required. Choose `counter!` for cumulative events, `gauge!` for values
  that rise and fall, and `histogram!` for distributions such as latency or
  payload size.
- Describe emitted metrics with `describe_counter!`, `describe_gauge!`, or
  `describe_histogram!` whenever the unit or purpose is not obvious from the
  metric name. Keep metric names stable and labels low-cardinality. Do not put
  user input, request identifiers, unbounded path parameters, or raw error
  strings into labels.
- Respect the library and application boundary. Libraries in this workspace,
  including `ortho_config` and `cargo-orthohelp`'s reusable modules, may emit
  `tracing` events and `metrics` instrumentation, but must not install global
  subscribers or recorders. Applications and binaries should initialize their
  chosen exporters and subscribers once, as early as practical in startup.

Use `tracing` and `metrics` together where it aids diagnosis: spans give the
contextual envelope, events describe what happened inside, and metrics
aggregate the same activity for monitoring. New observability primitives, such
as additional metric families or span fields used across crates, should be
mentioned in the relevant design or component architecture document, so the
contract stays discoverable.

## Digest rendering

`cargo-orthohelp` hashes cache inputs with SHA-256 and renders the digest as 64
lowercase hexadecimal digits. From `sha2` 0.11 the `finalize` and `digest`
methods return `hybrid_array::Array<u8, _>`, which dereferences to `[u8]` but
does not implement `core::fmt::LowerHex`, so the `{:x}` formatting the cache
previously used no longer compiles. `sha2` 0.11 also no longer implements
`std::io::Write`, so `std::io::copy` cannot stream into a hasher.

- Render digest bytes through the crate-internal `to_lower_hex` helper in
  `cargo-orthohelp/src/hex.rs`. It is scoped to `cargo-orthohelp`, owns no
  state, and may be called from any crate-internal site that needs lowercase
  hex. Do not reintroduce `{:x}` or per-byte `format!` calls.
- The helper maps each nibble arithmetically rather than indexing a digit
  table, because the workspace denies `clippy::indexing_slicing`. Keep it that
  way instead of adding a scoped suppression.
- Keep the helper private. The crate exposes no hex-encoding contract to
  downstream consumers, and publishing one would commit it to supporting that
  contract. Add a dedicated dependency such as `hex` only if a public surface
  genuinely needs one.
- Pass the digest by reference, as in `to_lower_hex(&hasher.finalize())`, so
  `Array` coerces to `&[u8]` through `Deref`.
- Feed a hasher with `update` from a bounded buffered read loop, or through a
  small `std::io::Write` adapter newtype that forwards to `update`. Do not
  reach for `std::io::copy` with a hasher as the sink.
- The sibling RustCrypto crates move in lockstep and carry the same break:
  `sha1`, `sha3`, and `md-5` go to 0.11, and `hmac`, `hkdf`, and `pbkdf2` go to
  0.13. Apply the same rules when introducing any of them.

The rendered form is byte-for-byte identical to the previous `{:x}` output, so
the change does not invalidate cache directories. A bounded exhaustive unit test
covers the whole `u8` range, and `cache.rs` pins the canonical SHA-256 digest of
`b"abc"` so a self-consistent but wrongly ordered or zero-truncated encoder
cannot pass.

### Discovery telemetry

`discovery/telemetry.rs` is the only place configuration discovery emits
events, and it is written so that a leak is a compile-time impossibility rather
than a review responsibility.

| Event                       | Fields                                                                                                                                                                                                                                           | Emitted from                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------- |
| `discovery.source_selected` | `source`: `process`, `injected`                                                                                                                                                                                                                  | `builder::build`            |
| `discovery.selector`        | `state`: `not_configured`, `unset`, `empty`, `accepted`                                                                                                                                                                                          | `candidates::push_selector` |
| `discovery.xdg`             | `config_home` and `dirs`: `absent`, `empty`, `present`; `resolution`: `default`, `list`                                                                                                                                                          | `candidates::push_xdg`      |
| `discovery.home`            | `source`: `home`, `userprofile`, `fallback`, `none`                                                                                                                                                                                              | `candidates::push_home`     |
| `discovery.attempt`         | `operation`: `discover_first`, `compose_layers`                                                                                                                                                                                                  | `load`                      |
| `discovery.candidate`       | `operation`; `outcome`: `optional_failure`, `required_failure`; `required`; `source`: `required_explicit`, `explicit`, `selector`, `xdg`, `windows`, `home`, `project`; `category`: `file`, `cyclic_extends`, `gathering`, `validation`, `other` | `load`                      |
| `discovery.project_root`    | `state`: `cwd_unavailable`                                                                                                                                                                                                                       | `builder::build`            |
| `discovery.load`            | `operation`; `outcome`: `success`, `not_found`; `source` (success only): `required_explicit`, `explicit`, `selector`, `xdg`, `windows`, `home`, `project`                                                                                        | `load`                      |

Table: Structured events emitted during configuration discovery, with the
closed field vocabulary each carries and the module that emits it.

Behind the optional `metrics` feature, discovery also increments three counter
families using the same bounded labels:

| Metric                                      | Labels                            |
| ------------------------------------------- | --------------------------------- |
| `ortho_config.discovery.attempts`           | `operation`                       |
| `ortho_config.discovery.outcomes`           | `operation`, `outcome`            |
| `ortho_config.discovery.candidate_failures` | `operation`, `source`, `category` |

Table: `metrics`-feature counter families and their bounded labels, mirroring
the `tracing` event fields above.

Rules for extending this set:

- **Every field is a `&'static str` from a constant declared in
  `telemetry.rs`, or a `bool`.** Discovery's inputs and outputs — variable
  values, resolved paths, file contents — are exactly what must not reach a
  log, and callers cannot pass one through a function that accepts only the
  closed vocabulary. The same property keeps the `metrics` labels bounded; an
  unbounded label here becomes an unbounded time-series in a consumer's
  monitoring system.
- **`tests/discovery_telemetry.rs` enforces it.** The suite feeds discovery
  distinctive values and asserts no captured field contains any of them. Adding
  a field that formats a path fails that test rather than shipping.
- **Counters go behind the optional `metrics` feature**, per the library
  and application boundary above. The `tracing` events are unconditional.

One trap worth recording: `tracing`'s global maximum level is recomputed as
dispatchers register and drop, so concurrent tests each installing a
thread-local subscriber can drop the maximum out from under one another and
silently lose events. `pin_global_max_level` in that test file holds one
dispatcher for the lifetime of the binary. This was an observed intermittent
failure, not a precaution.

## Environment access boundary

`EnvSource` (`ortho_config/src/env_source.rs`) is the crate's injectable
environment. `ProcessEnv` is the default and preserves the historical behaviour;
`MapEnv` models a closed set of variables for tests.

### Ownership and permitted call sites

- `EnvSource` is owned by the discovery subsystem. `ConfigDiscovery` holds one
  as `Arc<dyn EnvSource>`, supplied through
  `ConfigDiscoveryBuilder::env_source` and defaulting to `ProcessEnv`.
- `discovery/candidates.rs` is the only module that reads through it, and holds
  no `std::env::var_os` call of its own.
- It is **not** a general environment service. Adding readers elsewhere in the
  crate requires a decision about scope, not a call site.

### Composition rules

- **Lookup by name only.** There is deliberately no enumeration method. RFC 0001
  makes "the crate never scans the whole process environment" a safety property
  of environment access: a process holding unrelated secrets must never have
  them enumerated, copied, or logged. An enumeration method here would void
  that guarantee for every holder of an `EnvSource`, however careful individual
  callers were.
- **The merge layer needs a different abstraction.** `CsvEnv` legitimately scans
  a prefix because `figment::providers::Env` does. When it gains an injectable
  source ([#412](https://github.com/leynos/ortho-config/issues/412)) it must
  take a separate, explicitly named type, so the scanning capability is visible
  in the signature rather than latent in a trait whose other users must not
  scan. Until then, `CsvEnv` injection is out of scope and a consumer needing
  to control `APP_*` configuration-value variables must still set them in the
  process.
- **Owned returns, deliberately.** An `impl Iterator` return would be
  return-position `impl Trait` in traits (RPITIT) and make the trait unusable
  behind a trait object, forcing `ConfigDiscovery<S>` generics to leak through
  the derive-generated `load()` and every call site. Configuration resolves
  once per process, so the allocation is immaterial.
- **`home_fallback` defaults to `None`.** `ProcessEnv` overrides it by
  default, and custom sources may too. See the users' guide for why an injected
  source must be able to suppress the platform lookup.

## Dependency management

Cargo dependencies in this workspace follow strict version pinning rules so
that builds remain stable and reproducible across contributors and continuous
integration (CI) environments.

- Use SemVer-compatible caret requirements for every dependency declared in
  `Cargo.toml`, for example, `some-crate = "1.2.3"`. This is Cargo's default
  and accepts non-breaking minor and patch updates while rejecting breaking
  changes from a new major version.
- Do not use wildcard (`*`) or open-ended inequality (`>=`) version
  requirements. They admit unpredictable upstream changes into the build and
  are forbidden in this workspace.
- Reserve tilde (`~`) requirements for the narrow case where a dependency must
  be locked to patch-level updates for a specific, documented reason. Record
  the rationale alongside the dependency entry or in the related design
  document, so a later reader can re-evaluate the constraint.
- Keep dependencies current. When upgrading a crate, run the full quality
  gates (`make check-fmt`, `make lint`, `make test`) and, where the upgrade
  changes behaviour or public API, update the relevant design document, ADR, or
  migration guide.
- Capture substantive dependency choices, such as adopting or replacing a
  crate, in an ADR following the documentation style guide. Reference the ADR
  from the design document and from this guide where future contributors should
  be aware of the decision.

### Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows, including
calls into `leynos/shared-actions`. Contract tests that assert a caller's exact
commit SHA create a lockstep dependency: every time Dependabot opens a bump PR,
the test fails until a human edits the pinned constant to match. That defeats
the purpose of automated dependency updates and turns a routine bump into a
manual chore.

Contract tests may still verify the *shape* of a reusable-workflow caller. They
must not verify the specific SHA value.

- Do assert the workflow references the correct reusable workflow path.
- Do assert the ref is pinned to a full 40-character commit SHA, not a
  mutable branch such as `main` or `rolling`.
- Do assert the expected `on:` triggers, least-privilege `permissions:`, and
  the inputs the caller relies on.
- Do not hard-code the current SHA value as an expected string. Match it with
  a pattern instead.
- Do not fail a test purely because Dependabot bumped the pinned SHA.

```python
import re

SHA_RE = re.compile(r"^[0-9a-f]{40}$")

def test_uses_pinned_full_sha(caller_step):
    ref = caller_step["uses"].split("@")[-1]
    assert SHA_RE.match(ref), f"expected a 40-hex commit SHA, got {ref!r}"
```

If a workflow's behaviour genuinely depends on a feature only present from a
particular commit onwards, express that as a comment or a changelog note, not
as a test assertion on the SHA string.

## Spelling gate

`make markdownlint` enforces en-GB-oxendict (Oxford) spelling over the
repository's Markdown prose with [`typos`](https://github.com/crate-ci/typos),
as required by the [documentation style guide](documentation-style-guide.md).
Run the gate on its own with `make spellcheck`. The generated configuration
lives in the repository-root `typos.toml` and works in three layers:

1. The `en-gb` locale corrects American spellings (`color` to `colour`,
   `behavior` to `behaviour`, `analyzed` to `analysed`).
2. The shared estate dictionary supplies generated `extend-words` entries that
   restore Oxford spelling, which the locale alone would not enforce: identity
   entries accept `-ize` inflections that the locale would otherwise "correct"
   to `-ise`, and `-ise` entries are corrected to `-ize`. Stems taking `-yse`
   (`analyse`, `paralyse`) are left to the locale, which already enforces them.
3. `typos.local.toml` adds only repository-specific names, quotations,
   deliberate fixtures, and exclusions that do not belong in the shared base.

`typos.toml` is a generated file. Never edit its entries by hand. The generator
refreshes the shared dictionary into untracked `.typos-oxendict-base.toml` only
when its configured authority is newer, merges `typos.local.toml`, and writes
deterministic output:

```bash
uv run scripts/generate_typos_config.py
```

Generic Oxford stems and corrections belong in the shared dictionary maintained
by `leynos/agent-helper-scripts`. Keep local entries narrow: this repository's
overlay preserves its library names, non-English fixtures, tool and standards
names, and ExecPlan headings. Quoted APIs keep US spelling per the
documentation style guide. Inline code is spellchecked, so add a narrowly
backtick-bound pattern to `typos.local.toml` for an upstream API or identifier
rather than adding a word-level exception. Fenced code blocks remain ignored.
The helper tests cover dictionary validation, source-scoped HTTP validators,
freshness decisions, offline fallback, deterministic rendering, and generated
configuration drift.

`scripts/typos_rollout_http.py` owns shared-cache freshness, HTTPS transport
security and persistence coordination. Only `scripts/typos_rollout.py` may
compose it with dictionary validation. The established
`scripts/generate_typos_config.py` adapter retains its no-argument
`render_config()` and positional `main(output)` interfaces for operator
automation; application and release code must not reuse these spelling-policy
internals.

The gate runs over the `MD_FILES_FIND` Markdown file list with
`--force-exclude` so the `typos.toml` excludes also apply to explicitly passed
paths (for example, Markdown that appears inside `target` build output). To fix
findings mechanically, rerun the gate's `typos` command with `--write-changes`
appended, substituting the version from the Makefile `TYPOS_VERSION` variable:

```bash
uv tool run typos@<TYPOS_VERSION> --config typos.toml --force-exclude \
  --write-changes <files>
```

Review automated rewrites before committing; spelling corrections must not
touch code samples, API names, or quoted material.

`typos` is a Rust binary rather than a locked Python dependency, so its version
is pinned once in the Makefile `TYPOS_VERSION` variable and run through
`uv tool run typos@$(TYPOS_VERSION)`. CI inherits the pin by calling
`make spellcheck`. The target first runs the isolated helper tests, refreshes
and regenerates the configuration, and fails when the tracked output drifts.
When bumping the version, update `TYPOS_VERSION` and rerun the gate.

## Command checklist

Run from repository root:

```bash
set -o pipefail; make check-fmt 2>&1 | tee /tmp/make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/make-lint.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/make-markdownlint.log
set -o pipefail; make test 2>&1 | tee /tmp/make-test.log
```

For targeted behavioural debugging:

```bash
cargo test -p ortho_config --tests
cargo test -p hello_world --tests --all-features
```

For a single `ortho_config` integration target, for example the subcommand or
clap-parsing suites described in
[Integration test targets](#integration-test-targets):

```bash
cargo test -p ortho_config --test subcommand
cargo test -p ortho_config --test clap_integration
```
