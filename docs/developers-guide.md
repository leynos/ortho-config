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
reconstruct it from generated tokens later in the pipeline. The parser-faithful
approach is recorded in the [design decision log](design.md#9-decision-log).

The parse layer accepts scalar, `Option<T>`, and `Vec<T>` fields. It rejects
nested wrappers and map fields when their clap string default cannot be
replayed faithfully. Unsupported shapes should receive a focused compile-time
diagnostic that points to an explicit `#[ortho_config(default = ...)]`
alternative. Parser settings that change tokenization or accepted spelling must
either be retained in the IR and applied to the synthetic argument, or cause
the shape to be rejected during parsing.

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
file or environment value overrides the inferred default, while an explicit CLI
value still wins. Run the standard quality gates before requesting a CodeRabbit
review.

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

`#[derive(OrthoConfig)]` emits `OrthoConfigLocalization` for the deriving
configuration. Its `LOCALIZATION_BASE` is the dotted catalogue root from
`#[ortho_config(localization_base = "…")]` (or the normalized application-name
default), while the command-level constants and `ARG_IDS` use the normalized,
hyphen-joined Fluent ids. Each `ArgLocalizationIds` entry records the Clap
argument name and its help, long-help, and value-name ids. Argument ids are
validated for normalized collisions during expansion; flattened, subcommand, and
`skip_cli` fields are excluded from `ARG_IDS`. Set
`ORTHO_CONFIG_EMIT_IDENTIFIERS=1` for an opt-in standalone inventory at
`${OUT_DIR}/ortho-config/cli-identifiers.json`; the derive writes this file
through per-expansion fragments and merges them deterministically. Mounted
command-tree identifiers remain owned by the path-aware documentation IR.

Normalized argument-ID collisions within one derived struct fail at compile
time. Hand-built and dynamic command trees retain the runtime panic contract.
Cargo does not include proc-macro environment reads in its rebuild fingerprint;
after changing the opt-in variable or source, refresh the artefact with
`cargo clean -p <package> && ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build -p <package>`.
Do not rely on a warm build to observe changed artefact output.

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

The runtime and macro normalizers are deliberate twins, locked by their shared
version marker and property tests against `message_id_for`. Generated docs IR
receives mounted paths through path-aware trait methods; handwritten trait
implementations use their provided fallback. The optional identifier artefact
uses per-expansion fragments and a deterministic merge below `OUT_DIR`; its
schema has standalone scope until a downstream consumer joins it to docs IR.

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

Behavioural suites live in crate-local integration test targets. The repository
layout guide records the current target and feature-file inventory.

Step definitions use `rstest-bdd` macros (`#[given]`, `#[when]`, `#[then]`) and
consume `rstest` fixtures. Scenario-local mutable state is modelled with
fixtures and `Slot<T>` values inside `#[derive(ScenarioState)]` structs.
Cross-scenario mutable sharing is forbidden; use `#[once]` only for expensive,
effectively read-only infrastructure.

Keep richer fixture families isolated. For example, `NestedDocsConfig` and
`NestedDocsContext` back `docs_ir_nested.feature`, and their steps live in a
fixture-specific module rather than expanding unrelated step files. Likewise,
`CargoContext` and its `#[fixture]` provider plus the `cargo` steps live in
`tests/rstest_bdd/behaviour/steps/cargo_steps.rs`, backing
`cargo_entry_point.feature` (scenarios: Cargo dispatch parses the inner
options; bare invocation without the injected token is rejected), also isolated
in a fixture-specific steps module. Future contributors should extend the
existing fixture families rather than duplicate them.

### Integration test targets

`ortho_config` also carries plain `rstest` integration suites that are distinct
from the behavioural (`rstest-bdd`) suites above. Each directory-style suite
needs its own `[[test]]` stanza in the crate manifest; a suite without a stanza
is not compiled or run by Cargo. The repository-layout guide records the
current suite and module inventory.

Run either suite on its own for focused debugging:

```bash
cargo test -p ortho_config --test <target>
```

### Public documentation examples

The root README and `docs/users-guide.md` are executable documentation. Every
fenced block in either file must have a unique `tested-example` marker on the
immediately preceding line. The documentation-example test support owns parsing
and lookup for these examples. It is test infrastructure only; production code,
other crates, and examples must not depend on it.

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

The documentation-example workspace helper owns temporary Cargo package
assembly for Rust examples. Reuse it only from documentation tests that compile
an exact fence body. A workspace has one owner: operations that add, build,
run, or write require an exclusive mutable borrow. Concurrent scenarios must
create an independent workspace per thread rather than share one workspace. The
helper may add the dependencies needed to compile a published example, but it
must not rewrite that source. Keep the expected identifier registry closed so
adding an example without choosing its behavioural contract fails a test. The
documentation-example Cargo runner owns isolated Cargo process setup for these
documentation tests; reuse it only when executing a documented Cargo workflow
with a caller-owned temporary state directory. Run Cargo and child binaries
with cleared environments. Cargo receives only the toolchain and platform paths
it needs; binaries receive only the non-sensitive Windows runtime variables
named by the workspace allow-list and deliberate scenario overrides. Keep
fallible host-tool discovery and environment preparation at the runner
boundary; command construction consumes the prepared values without starting
subprocesses or writing files. The process-runner helper owns bounded execution
for these documentation tests only: route Cargo, host-tool, and
documented-binary commands through it, while keeping command construction and
exit-status policy with their existing owners. Run-file paths must contain
normal relative components only.

### Python docstring examples

The helper scripts under `scripts/` carry NumPy-style `Examples` sections, and
those examples are executed rather than read. `PYTEST_FLAGS` in the Makefile
hands `--doctest-modules` the modules to collect, and `make test` runs them
beside the script tests.

The list is written by hand, which is the mechanism that fails, and it fails in
both directions. A module that gains an example is collected only if somebody
remembers to name it, and the spelling helper had gained one that nothing ran,
so it could have gone untrue without a gate noticing. A module the list still
names after a deletion is the other direction, and the same helper supplied it:
it was removed with the legacy spelling generator while the list still named
it, which ends the whole lane rather than quietly collecting less.

`scripts/tests/test_doctest_collection.py` is the guard. It sweeps `scripts/`
for modules containing `>>>` and fails when one is neither named by
`PYTEST_FLAGS` nor covered by a directory the list names, which is how
`scripts/tests` covers the test modules. A second contract fails when the list
names a path that no longer exists, because that ends the whole lane rather
than quietly collecting less. A third pins the sweep itself: both of the others
are satisfied by a discovery that returns nothing, so the sweep must find
`scripts/bump_version.py`, which carries fifty-odd examples.

Three mutations are caught: a module dropped from the list, a named path
misspelled, and a sweep narrowed to a suffix no file uses.

### Shared test-support helpers

The shared test-support modules own the `ToAnyhow` trait, which converts an
`OrthoResult<T>` (`Result<T, Arc<OrthoError>>`) into an `anyhow::Result<T>`,
preserving the original error as the `anyhow` source. It is the single
conversion point for integration-test targets that report failures through
`anyhow`; before it existed, each suite grew its own free function or inline
`map_err(|err| anyhow!(err))` call, and those copies agreed on behaviour by
accident rather than by construction. New local `to_anyhow`-style conversions
are not permitted — depend on this module instead.

The shared discovery-builder helper owns `discovery_with`, a pure builder that
returns an independent `ConfigDiscovery` wired to the caller's `MapEnv` only;
it reads no other environment variables and touches no filesystem, and it
resolves its sole project root to a fixed test root. It is owned by the
injected-source discovery suites — candidates, properties, telemetry, and
metrics — which need a `ConfigDiscovery` configured identically so that their
results remain comparable. Any test needing a deterministic discovery over an
injected environment may reuse it.

The common behavioural-step helper module owns `set_scalar_once`,
`set_nonblank_scalar_once`, and the `SlotTakeOrExt` extension trait's
`take_or`. The first two centralize the repeated "guard, validate, and populate
a scalar slot" shape used to fill scenario `Slot` state exactly once; `take_or`
centralizes the repeated "take the slot's value or fail with a descriptive
error" shape. Both are scoped to behavioural step modules; step modules must
use these helpers rather than re-implementing slot-guard boilerplate.

## Snapshot tests

Use `insta` for renderer golden coverage that would be noisy as handwritten
string assertions. Place snapshots beside the integration test that owns them,
and redact dates, absolute paths, and other environment-specific substrings with
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

### Environment merge telemetry

The environment merge boundary emits a `merge.layer` tracing event at the
decision and terminal points of source-aware work. Events use only these
bounded fields:

- `operation`: `csv_env`, `derived_load`, or `subcommand_load`;
- `source`: `process` or `injected`;
- `outcome`: `attempt`, `success`, or `failure`; and
- `category`: `none`, `opaque_key_transform`, `invalid_nesting`, `cli`,
  `file`, `cyclic_extends`, `gathering`, `merge`, `validation`, or `aggregate`.

`CsvEnv` emits process-backed and injected events. Derive-generated loads and
subcommand loads emit events when their source-aware entry points are used. The
events never contain environment values, keys, paths, configuration data,
caller-supplied prefixes, or raw error text. Error categories are reduced to
the closed vocabulary before emission so subscribers can aggregate failures
without receiving sensitive input.

Capture tests must cover successful and failing paths for each emitting
operation. They assert the operation, source, outcome, and category fields, and
verify that captured events contain neither injected values nor keys or paths
from the test inputs. The library does not install a global subscriber;
applications attach their own capture or export layer at the boundary.

With the optional `metrics` feature enabled, the same merge boundaries also
increment `ortho_config.merge.attempts` and `ortho_config.merge.outcomes`. Both
counter families use only the bounded `operation`, `source`, `outcome`, and
`category` labels described above; the attempt counter uses `attempt` and
`none` for its outcome and category. No metrics recorder is installed by the
library.

## Digest rendering

`cargo-orthohelp` hashes cache inputs with SHA-256 and renders the digest as 64
lowercase hexadecimal digits. From `sha2` 0.11 the `finalize` and `digest`
methods return `hybrid_array::Array<u8, _>`, which dereferences to `[u8]` but
does not implement `core::fmt::LowerHex`, so the `{:x}` formatting the cache
previously used no longer compiles. `sha2` 0.11 also no longer implements
`std::io::Write`, so `std::io::copy` cannot stream into a hasher.

- Render digest bytes through the crate-internal `to_lower_hex` helper. It is
  scoped to `cargo-orthohelp`, owns no state, and may be called from any
  crate-internal site that needs lowercase hex. Do not reintroduce `{:x}` or
  per-byte `format!` calls.
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
the change does not invalidate cache directories. A bounded exhaustive unit
test covers the whole `u8` range, and `cache.rs` pins the canonical SHA-256
digest of `b"abc"` so a self-consistent but wrongly ordered or zero-truncated
encoder cannot pass.

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
- **The discovery telemetry suite enforces it.** The suite feeds discovery
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

`EnvSource` is the crate's injectable environment. `ProcessEnv` is the default
and preserves the historical behaviour; `MapEnv` models a closed set of
variables for tests.

### Ownership and permitted call sites

- `EnvSource` is owned by the discovery subsystem. `ConfigDiscovery` holds one
  as `Arc<dyn EnvSource>`, supplied through
  `ConfigDiscoveryBuilder::env_source` and defaulting to `ProcessEnv`.
- `ScanEnvSource` is owned by the merge boundary. `CsvEnv` accepts one through
  `with_source`, and the derived loader receives it through
  `OrthoConfig::load_from_iter_with_sources`.
- The discovery candidate reader is the only production reader, and holds no
  `std::env::var_os` call of its own.
- It is **not** a general environment service. Adding readers elsewhere in the
  crate requires a decision about scope, not a call site.

### Composition rules

- **Lookup by name only.** There is deliberately no enumeration method. RFC 0001
  makes "the crate never scans the whole process environment" a safety property
  of environment access: a process holding unrelated secrets must never have
  them enumerated, copied, or logged. An enumeration method here would void
  that guarantee for every holder of an `EnvSource`, however careful individual
  callers were.
- **The merge layer uses a different abstraction.** `CsvEnv` legitimately scans
  a prefix because `figment::providers::Env` does, so its injectable path takes
  `ScanEnvSource`, a separate explicitly named type. The scanning capability is
  visible in the signature rather than latent in a trait whose other users must
  not scan. Callers can use one `Arc<MapEnv>` by coercing cloned `Arc`s to
  `SharedEnvSource` and `SharedScanEnvSource`;
  [#412](https://github.com/leynos/ortho-config/issues/412) completed this path.
- **Injected merge providers are declarative.** Injected `CsvEnv` loading
  replays only the declarative prefix, case, split, and CSV options. Calling
  `CsvEnv::with_source` after `map()` or `filter_map()` is rejected because
  arbitrary closures cannot be replayed against an injected `ScanEnvSource`.
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

Dependabot owns scheduled dependency updates through `.github/dependabot.yml`:

- Root GitHub Actions updates run weekly and use the `dependencies` and
  `github-actions` labels.
- Root Cargo workspace updates run daily and use the `dependencies` and
  `cargo` labels.
- Python test requirements in `scripts/` update daily and use the
  `dependencies` and `python` labels.
- The root `rust-toolchain.toml` updates weekly through the independent
  `rust-toolchain` route, using the `dependencies` and `rust-toolchain` labels.

GitHub Actions updates include calls into `leynos/shared-actions`. Contract
tests that assert a caller's exact commit SHA create a lockstep dependency:
every time Dependabot opens a bump PR, the test fails until a human edits the
pinned constant to match. That defeats the purpose of automated dependency
updates and turns a routine bump into a manual chore.

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

One narrow exception exists, and it is worth stating because it looks like a
violation. `sccache_wiring_test.py` names `32c8ea64` in a set of pins that
install sccache while exporting neither half of its wiring, and fails if the
workflows return to one of them. That is not a lockstep on the current value:
it never fails a forward bump, it hard-codes nothing that Dependabot will
change, and it exists because reverting the pin is the one edit that would undo
this wiring while leaving every other assertion here green.

### sccache: the wrapper and the backend

sccache needs two halves to do anything. The rustc wrapper makes the compiler
run through it; the backend gives the cache somewhere to live. Either half
alone is worse than neither, because the job pays the install and reports
nothing amiss.

This repository ran with neither. The shared Rust setup action installs sccache
whenever `use-sccache` is true, which is its default, and at pin `32c8ea64` it
exported no wrapper and selected no backend. Every Rust job installed sccache
and compiled uncached, which is visible only as a slow build.

The two halves are now set in different places, deliberately:

| Half                  | Where it is set                        | Why there                                                                                         |
| --------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `RUSTC_WRAPPER`       | the shared action, not this repository | it must be the absolute path of the sccache the action installed                                  |
| `SCCACHE_GHA_ENABLED` | job-level `env:` in this repository    | it is a choice about where the cache lives, and the action stands aside when a caller has made it |

*Table 1: Which half of the sccache wiring is set where, and why.*

`RUSTC_WRAPPER` is deliberately absent from every workflow here. A bare
`RUSTC_WRAPPER: sccache` resolves through `PATH`, and an invocation that
resolves nothing compiles uncached without entering the hit-rate denominator,
so the statistic a reader would check cannot show the failure; whitaker #409
measured 69 such invocations after switching to the action's absolute path. The
action also stands aside when the caller has already set the variable, so a
value here would silently replace the path with the bare name.

Three jobs run the shared Rust setup and select the backend: `build-test` and
`binstall-packaging` in `ci.yml`, and `coverage-upload` in `coverage-main.yml`.

Two Rust lanes are outside this, each for its own reason:

- `mutation-testing.yml` calls a reusable workflow. Caller job `env:` does not
  propagate into one, and that workflow exposes no sccache input, so there is
  nothing to set from here.
- `verify-published-assets` in `release.yml` runs the setup but compiles
  nothing: it dry-runs `cargo binstall` against already published archives.
  Note that it is *not* excluded by the action's own
  `github.event_name != 'release'` guard, which a reader might assume. This
  workflow triggers on a tag push and on `workflow_dispatch`, never on the
  `release` event, so that guard never fires here.

`rust-build-release` and `mutation-cargo` are exempt from the one-SHA
assertion. Every other reference into the shared-actions tree sits at
`a5765019` today, and those two remain at `0e3c4d24`, where the CodeScene
coverage adoption's repin left them. The exemption is an allowance for them to
diverge without the contract reporting a partial repin: neither serves a lane
this wiring governs, `rust-build-release` is the release build the action
excludes from sccache, and `mutation-cargo` is a reusable workflow, which
caller job environments cannot reach. Moving either is a separate change that
needs its own evidence.

The allowance is proved in both directions rather than assumed. Moving
`rust-build-release` to a SHA of its own leaves the contract passing, which is
the exemption working; adding a sibling whose path merely begins with an exempt
one, such as a `rust-build-release-extra`, fails it, because the exemption is
keyed by the whole path inside the shared-actions tree and matched by equality
rather than by prefix.

`sccache_wiring_test.py` holds four things: that every reference into the
shared-actions tree sits at one SHA, so a partial repin cannot pass while the
guide claims otherwise; that the pin is not one of the revisions known to
export neither half; that no workflow sets `RUSTC_WRAPPER`; and that every job
discovered to run the shared Rust setup selects a backend, with the exclusions
named individually and with their reasons rather than as a blanket allowance.
The discovery is itself pinned, because the other assertions are all satisfied
by a sweep that finds no jobs.

Two of its sweeps are open rather than enumerated, and both had to be widened
after review found them closed. References are matched by the
`leynos/shared-actions/` prefix with `rust-build-release` and `mutation-cargo`
named as the exceptions, rather than by listing the four paths in use today: a
governed reference added later under an unlisted path would otherwise sit at
any SHA it liked while the contract reported agreement. And workflows are read
from both `*.yml` and `*.yaml`, because GitHub runs either, so a sweep over one
suffix claims repository-wide coverage while ignoring half the places a Rust
job can be declared.

Each caching job also ends with a `Report sccache statistics` step, and the
contract requires it. The wiring is invisible from the outside: a job with a
wrapper and a job without one both succeed, and only sccache's own compile
request and hit counts tell them apart. The step invokes the binary through
`SCCACHE_PATH` rather than by name, for the same reason the wrapper is not set
by name, and it runs on failure too, because the statistics of a failed build
are often what explain it.

Read the second run of a branch, not the first. The first stores into an empty
cache, so its hit rate says nothing about whether the wiring works. Compare
compile requests as well as hit rates: an invocation that resolved no wrapper
never enters the denominator, so a wrapper defect raises the request count
rather than lowering the rate.

Nine mutations are caught: one reference left at the old pin, the backend
removed from a job, `RUSTC_WRAPPER` set by name, the sweep narrowed so it finds
nothing, an excluded job quietly gaining a backend, the statistics step
removed, the statistics reported through a bare `sccache` rather than
`SCCACHE_PATH`, a governed reference added under a new path at a different SHA,
and a Rust job declared in a `.yaml` workflow without a backend. The last two
passed before the sweeps were widened, which is how they were shown to be real
rather than theoretical.

### Workflow contract gate

`make test-workflow-contracts` runs the contracts in `tests/workflow_contracts`
against the checked-in workflow and Make sources. It is a separate target from
`make test` because it needs neither a Rust toolchain nor the workspace's
Python test requirements:

```bash
uv run --with 'pytest>=8,<10' --with 'pyyaml>=6,<7' \
    --with 'hypothesis>=6,<7' pytest \
    tests/workflow_contracts --doctest-modules -q
```

Three things about that command are deliberate. The requirements are named
inline rather than taken from `scripts/requirements-test.txt`, so the gate
stays runnable on a checkout with no virtual environment. Each carries an upper
bound, because the target has no lockfile and a future major release of any of
them could change collection or doctest behaviour with no edit to this
repository. And `--doctest-modules` collects the examples in the support
modules, so an example that stops matching the helper it documents fails the
gate rather than ageing quietly.

`tests/workflow_contracts/makefile_support.py` holds the helpers the contracts
share for reading a Makefile: recognizing a recipe line, and extracting the
subcommand a target hands to a tool. It is support code, not a contract, so it
contains no test functions; its own executable examples are what
`--doctest-modules` collects. Contracts that read a workflow parse the YAML
document rather than matching its text, so a re-indentation cannot change a
verdict.

## Publish dry run

`make publish-check` runs `lading publish` over the workspace. lading copies
the workspace, then packages and dry-run publishes each crate in the order
`lading.toml` declares. That per-crate packaging is the point: `cargo package`
builds each crate from its own packaged sources, so it is the only thing here
that sees what a published crate exports. A symbol that is public within the
workspace but missing from a crate root compiles under the workspace test run
and under Clippy, and fails only in this step. Issue #414 is this repository's
own instance: `OrthoConfigSubcommandDocs` is exported by the workspace
`ortho_config_macros` but not by its published release of the same version, so
tarball verification resolves the re-export against the published crate and
fails with "no `OrthoConfigSubcommandDocs` in the root". Nothing else in CI
sees that.

Before it packages, lading runs a pre-flight:
`cargo check --workspace --all-targets`, then `cargo test`, both into a
throwaway target directory. `lading.toml` sets `preflight.unit_tests_only`,
which narrows the second of those to the library and binary unit tests.

In CI the pre-flight is skipped outright. The `Publish dry run` step sets
`LADING_SKIP_PREFLIGHT`, because both `Test and Measure Coverage` steps are
unconditional and run ahead of it, so the pre-flight would be a second
execution of a workspace this job has already passed and failed its lane on.
The skip drops the auxiliary builds and the cargo check and test pair, and
nothing else. The `Cargo.lock` freshness guard still runs, and the working-tree
guard is unaffected: it is opt-in through `--forbid-dirty` either way, and
`PUBLISH_CHECK_FLAGS` is empty, so neither a skipped nor an executed pre-flight
enforces a clean tree here. The packaging still runs.

The variable is set on the step rather than in `lading.toml`, because a
configuration file cannot tell a CI run from a local one. On a workstation
nothing has run the tests first, so `make publish-check` still runs the full
pre-flight. `tests/workflow_contracts/publish_preflight_scope_test.py` pins
both halves of that arrangement, and pins the packaging from both ends: the
step's command as tokens, and the Make target's recipe handing lading the
`publish` subcommand. Either half alone is defeatable.

lading itself is pinned. `LADING_REF` in the Makefile holds the full commit SHA
of the v0.3.1 release commit, which is the first release carrying the skip. It
names a SHA rather than the tag because a tag can be repointed. Without any pin
at all, `uvx --from git+...` tracks lading's default branch and would change
what the release gate runs with no edit to this repository.

## Releasing `cargo-orthohelp` binaries

`cargo-orthohelp` is installed by downstream continuous integration (CI) under
a no-source-build policy, so every release must carry prebuilt archives that
`cargo binstall` can resolve. `.github/workflows/release.yml` publishes them
for five targets, each built on a runner of its own architecture and operating
system rather than cross-compiled:

Release targets for `cargo-orthohelp`:

| Target                      | Runner             |
| --------------------------- | ------------------ |
| `x86_64-unknown-linux-gnu`  | `ubuntu-24.04`     |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` |
| `x86_64-apple-darwin`       | `macos-15-intel`   |
| `aarch64-apple-darwin`      | `macos-latest`     |
| `x86_64-pc-windows-msvc`    | `windows-latest`   |

Table: Release targets for `cargo-orthohelp` and the runner each archive is
built on, one runner per target architecture and operating system.

### Archive layout

`scripts/release_archive.py` builds one target and writes
`dist/cargo-orthohelp-<target>-v<version>.tgz`, holding exactly one member,
`cargo-orthohelp-<target>-v<version>/cargo-orthohelp` (with `.exe` on Windows),
plus a `sha256sum`-compatible `.sha256` sidecar. Member metadata and the gzip
timestamp are fixed, so rebuilding a tag reproduces the same bytes.

That layout is a contract with the `[package.metadata.binstall]` templates in
`cargo-orthohelp/Cargo.toml`: `pkg-url` renders the archive name and `bin-dir`
renders the member path. Change one and the other must change with it.
`scripts/tests/test_release_archive.py` renders the real templates against the
staged archive, and `tests/workflow_contracts/release_workflow_test.py` pins
the workflow shape, so a mismatch fails on the pull request.
`scripts/verify_release_archives.py` applies the same checks to a directory of
archives; the workflow runs it on the staged output and again on the downloaded
draft assets.

### Release flow

The workflow creates a draft release, builds and uploads every target's archive
and sidecar, audits the draft, publishes it, then resolves the real asset URLs
with `cargo binstall --dry-run`. Two details are load-bearing and have broken
releases elsewhere in the estate:

- The jobs that call `gh` without an `actions/checkout` step set `GH_REPO`.
  Otherwise `gh` infers the repository from a git remote and fails with
  `not a git repository`, skipping every downstream job.
- The auditing job requests `contents: write` even though it only reads.
  Draft releases are invisible to read-scoped tokens.

A tag with a suffix, such as `v1.2.3-beta.1`, is published as a prerelease. The
workflow serializes on the effective tag, so a tag push and a manual dispatch
for the same tag cannot both pass the "does this release exist" check before
either creates it.

Build jobs check the release tag out, so the packaging script comes from the
tagged tree. Tags cut before this workflow landed carry no packaging script and
cannot be republished; cut a new tag instead.

`cargo binstall` reads `[package.metadata.binstall]` from the crates.io
release, not from this repository, so a version must be published to crates.io
with that metadata before `cargo binstall cargo-orthohelp@<version>` resolves
the archives. 0.9.1 is the first such version.

`cargo-orthohelp` carries its own version rather than inheriting the workspace
one, so a tooling-only release is a patch bump of that crate alone. Edit
`cargo-orthohelp/Cargo.toml`, refresh the lock with
`cargo update -p cargo-orthohelp`, and tag `v<version>`. Use
`scripts/bump_version.py` only when the whole workspace is being released.

### Verifying the packaging locally

`make test` covers the packager and the auditor; `make test-workflow-contracts`
covers the workflow shape. To exercise the real build:

```bash
uv run --script scripts/release_archive.py "$(rustc -vV | sed -n 's|host: ||p')"
uv run --script scripts/verify_release_archives.py --dist-dir dist \
  --target "$(rustc -vV | sed -n 's|host: ||p')"
```

The pull-request job `binstall-packaging` runs those two commands plus a smoke
test of the extracted binary on Linux, macOS, and Windows.

## Spelling gate

`make markdownlint` enforces en-GB-oxendict (Oxford) spelling over the
repository's Markdown prose with [`typos`](https://github.com/crate-ci/typos),
as required by the [documentation style guide](documentation-style-guide.md).
Run the gate on its own with `make spellcheck`. The whole gate is the shared
[`typos-config-builder`](https://github.com/leynos/typos-config-builder), run
through `uv tool run` and pinned by the Makefile `TYPOS_CONFIG_BUILDER_VERSION`
variable, so local runs and CI use the same version of both the builder and the
`typos` binary it pins:

```bash
make spellcheck
```

The gate renders `typos.toml`, runs `typos` over the tracked Markdown files,
and then enforces the shared `[phrases.corrections]` policy, which rejects the
hyphenated form of "handwritten". Any stage that reports findings fails the
gate.

The generated configuration lives in the repository-root `typos.toml` and works
in three layers:

1. The `en-gb` locale corrects American spellings (`color` to `colour`,
   `behavior` to `behaviour`, `analyzed` to `analysed`).
2. The shared estate dictionary supplies generated `extend-words` entries that
   restore Oxford spelling, which the locale alone would not enforce: identity
   entries accept `-ize` inflections that the locale would otherwise "correct"
   to `-ise`, and `-ise` entries are corrected to `-ize`. Stems taking `-yse`
   (`analyse`, `paralyse`) are left to the locale, which already enforces them.
3. `typos.local.toml` adds only repository-specific names, quotations,
   deliberate fixtures, and exclusions that do not belong in the shared base.

`typos.toml` is a generated file. Never edit its entries by hand. The gate
regenerates it on every run from the live shared dictionary, which it refreshes
into untracked `.typos-oxendict-base.toml`, merged with the `typos.local.toml`
overlay. A word added to the shared dictionary therefore reaches this
repository on its next run, with no change here. Because the dictionary is live,
`typos.toml` must never be drift checked in continuous integration; any hand
edit is overwritten on the next run.

Generic Oxford stems and corrections belong in the shared dictionary maintained
by `leynos/agent-helper-scripts`. Keep local entries narrow: this repository's
overlay preserves its library names, non-English fixtures, tool and standards
names, and ExecPlan headings. Quoted APIs keep US spelling per the
documentation style guide. Inline code is spellchecked, so add a narrowly
backtick-bound pattern to `typos.local.toml` for an upstream API or identifier
rather than adding a word-level exception. Fenced code blocks remain ignored.

The gate passes `--force-exclude` so the `typos.toml` excludes also apply to
explicitly passed paths, for example Markdown that appears inside `target`
build output. To fix findings mechanically, run `typos` directly against the
generated configuration with `--write-changes`:

```bash
uv tool run typos --config typos.toml --force-exclude --write-changes <files>
```

Review automated rewrites before committing; spelling corrections must not
touch code samples, API names, or quoted material.

Bumping `TYPOS_CONFIG_BUILDER_VERSION` is only needed for builder code changes;
dictionary changes need no bump.

## Cancelling superseded pull-request runs

Every push to a pull request starts a fresh run of each gate. The run already
in flight is answering a question about a commit nobody will merge, and left
alone it holds a runner until it finishes, so the branch pays twice for one
answer. Every workflow a pull request can start therefore carries a concurrency
block:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

Two halves matter, and each fails in a way nothing else would notice.

- **The group keys on the pull request.** A group built from
  `github.run_id` is unique to one run, so it matches no predecessor and
  cancels nothing while reading exactly like a concurrency control. A constant
  group is the opposite failure: every open pull request shares one queue, and
  the first push anywhere cancels the gates running everywhere else.
- **Cancellation is conditioned on the event.** A literal
  `cancel-in-progress: true` reads as the stricter setting and is a regression.
  A push to `main`, a schedule, and a dispatch have no successor waiting, and
  the run on `main` writes the warm cache and records the coverage that no
  later run repeats.

`pull_request_target` workflows are out of scope. They run against the base
repository to carry a token, and the ones here automate pull-request
housekeeping rather than building, so cancelling one mid-flight is a hazard
with no minutes to win.

### The cancellation contract

`tests/workflow_contracts/concurrency_test.py` reads `.github/workflows` and
asserts, for every workflow declaring a `pull_request` trigger, that it
declares a concurrency group, that the group names no per-run expression, that
the group names something that varies per pull request, and that
`cancel-in-progress` is exactly the expression above. A further test asserts
that discovery still finds the workflows it is expected to, so a broken read
cannot empty the list and turn the rest into a vacuous pass. Run it with
`make test-workflow-contracts`.

## Test timeouts: the tiers this repository sets

Four independent timers can end a test run, and the canonical statement of how
they must be ordered lives in the `generate-coverage` README in
[`leynos/shared-actions`][shared-actions-coverage]. All four are set here now;
until this branch, tier one covered two trybuild binaries and nothing else, and
tier two did not exist at all.

| Tier                     | What it bounds                     | Where it is set                            | Current value                                          |
| ------------------------ | ---------------------------------- | ------------------------------------------ | ------------------------------------------------------ |
| Per-test `slow-timeout`  | one test                           | `.config/nextest.toml`                     | 600 s (60 s x 10); the trybuild override is also 600 s |
| nextest `global-timeout` | the whole test run                 | `.config/nextest.toml`                     | 1,800 s (30 m)                                         |
| Cargo watchdog           | one `cargo` invocation, wall clock | `RUN_RUST_CARGO_WAIT_TIMEOUT` at job level | 2,700 s (45 m)                                         |
| Job `timeout-minutes`    | the whole job                      | job level                                  | 165 m in `ci.yml`, 120 m in `coverage-main.yml`        |

*Table: the timers that can end a run, innermost first.*

### The outermost tier was missing

Neither coverage job declared `timeout-minutes` before this was written, so
both inherited GitHub's six-hour default. That is not a budget anyone chose,
and the Windows leg of `ci.yml` already runs for 86 minutes, so a hang there
cost six hours of a paid runner before anything stopped it.

### Two watchdogs per job, not one

Each coverage job runs the action twice, once with `serde_saphyr` and once
without, and each invocation gets its own watchdog. So the job must be able to
contain both budgets before it contains anything else, and the requirement is
the watchdog multiplied by the number of coverage steps in that job, plus the
work outside them. The canonical rule does not spell this out because most
callers invoke the action once.

### The per-test budget is a product, not a period

`terminate-after` counts warning periods, so the budget a test gets is `period`
multiplied by it. The longest override here is 120 s with a multiplier of five,
so reading the period alone would report 120 s where the real figure is 600 s.
The contract asserts that reading outright rather than leaving it implied.

### Tier one covered two binaries, and tier two did not exist

The only `slow-timeout` in `.config/nextest.toml` was the trybuild override.
Two binaries were bounded at 600 s and every other test in the suite ran with
nothing under the job ceiling to stop it, because nextest's built-in 60 s
`slow-timeout` warns and never terminates: it names no `terminate-after`. The
sampled Windows run below reported 22 tests as slow and terminated none of
them. No `global-timeout` was set either, so a hung run was bounded only by the
cargo watchdog, which names `cargo` rather than the test still running.

Both are set now, and both were sized from this repository's own run history
rather than chosen. Three `build-test` jobs were read line by line, across
successful and failed runs and both platforms:

| Sample                                | Slowest single test | Longest nextest run | Run         |
| ------------------------------------- | ------------------- | ------------------- | ----------- |
| `build-test (windows-latest)`         | 364.8 s             | 1,023.6 s           | 34119577952 |
| `build-test (ubuntu-latest)`          | 301.8 s             | 620.0 s             | 34120845807 |
| `build-test (windows-latest)`, failed | no test output      | none                | 34124529877 |

*Table: per-test and whole-run durations, read from the nextest output of each
job. Each job runs the suite twice, so the two successful samples carry four
runs between them: 1,023.6 s and 648.2 s on Windows, 620.0 s and 417.7 s on
Linux, over 2,228 and 2,244 timed test results.*

The slowest test outside the trybuild override was
`cargo-orthohelp::compile_time must_use_compile_tests` at 364.8 s. The base
allowance is 600 s, ten warning periods of 60 s, about 1.6 times that worst
case: enough that a legitimately slow test finishes, small enough that a hang
is caught well inside the whole-run budget.

The whole-run budget is 30 minutes, about 1.76 times the 1,023.6 s worst run.
It has to fit inside the watchdog with nextest's termination procedure and a
cold build counted, which is 1,800 s plus 70 s plus 600 s, or 2,470 s. The
watchdog was 1,800 s and could not have carried it, so it rises to 2,700 s.
That is what moves the ceilings: the requirement is the watchdogs a job
contains plus the work outside them plus the margin, and each job runs the
action twice.

[Issue 483](https://github.com/leynos/ortho-config/issues/483) asked for these
measurements and named the constraint they had to satisfy, that each coverage
job invokes the action twice so raising the watchdog costs twice as much
ceiling here. It is answered above.

### What the ceilings are sized against

The allowance for work outside the watchdogs is per lane, because the two
differ by an order of magnitude and holding the trunk lane to the pull-request
lane's figure would demand a ceiling its own runs cannot justify.

| Lane                                   | Coverage steps  | Worst whole job | Outside those steps | Run         |
| -------------------------------------- | --------------- | --------------- | ------------------- | ----------- |
| `ci.yml` `build-test` (windows-latest) | 1,323 s + 950 s | 5,530 s         | 3,257 s             | 33447440225 |
| `coverage-main.yml` `coverage-upload`  | 563 s + 495 s   | 1,342 s         | 284 s               | 31908409573 |

*Table: measured coverage-step and whole-job durations. The last column is the
job's duration less its two watchdog-bounded coverage steps, so it is the work
the job timer covers and the watchdogs do not.*

The sample is 103 `ci.yml` coverage jobs, 100 successful and the rest failed or
cancelled, and 31 runs of `coverage-main.yml`, 29 successful and 2 failed. Runs
of every conclusion are read, not only successful ones: a run cancelled at its
ceiling is the case the sizing exists to prevent. No run in either sample was
ended by any of the four timers, the worst `ci.yml` job reaching 5,530 s.

On the Windows leg the work outside the coverage steps is dominated by cache
saving. So `ci.yml` is allowed 60 minutes and `coverage-main.yml` 15. With the
watchdog at 2,700 s and two coverage steps per job, the requirements are 165
and 120 minutes: 5,400 s of watchdog, plus the lane's allowance, plus a 900 s
margin. The margin is a term of the requirement rather than slack above it,
because a ceiling equal to the sum it contains cancels the job at the moment
the watchdog would have reported the overrun, and the report is the only thing
that makes an overrun actionable. The ceilings equal their requirements.

A lane in a workflow the contract has not measured is held to the larger
allowance until someone measures it and records a run id.

The first version of this section recorded 2,717 s for `ci.yml` and two
different figures for `coverage-main.yml`, 68 s here and 138 s in the contract.
Both came from counting the coverage steps differently: the numbers above
subtract the two `generate-coverage` steps and nothing else, which is exactly
what the watchdogs bound.

None of those runs was genuinely cold. One run is the coldest seen so far, not
a measurement of the cold case.

### The contract

`tests/workflow_contracts/timeout_ordering_test.py` asserts this by value over
every job invoking the coverage action, in both the `.yml` and `.yaml`
extensions. Jobs are its unit rather than steps, because the ceiling belongs to
a job and has to contain every watchdog inside it; counting the steps is what
makes the two invocations visible to the arithmetic. It reads a step's own
environment before the job's, as GitHub resolves it, and it fails on a
coverage-invoking job that declares no ceiling at all. The readings it rests on
live in `nextest_budgets.py`, `nextest_durations.py`, `nextest_errors.py`,
`timeout_budgets.py` and `coverage_lanes.py`, and are driven with controlled
values in `timeout_reading_test.py`.

Run them with `make test-workflow-contracts`. The target provisions `pytest`,
`pyyaml` and `hypothesis` through `uv run --with` rather than from the
project's own dependencies, so the contracts need no virtual environment of
their own and nothing they need reaches the published package. They are Python
because what they read is YAML and TOML; `make test` does not run them.

Each reading takes what it reads rather than fetching it. `coverage_jobs_of`
queries supplied workflow documents and reaches no filesystem and no parser,
`workflow_documents` is the acquisition that reads a directory, and
`coverage_jobs_in` is the two together, defaulting its directory to the
repository's own so the contract can call it with no argument at all while the
query below it can never reach a file. The pair is named for what each takes,
because a call site has to say which it is doing. The budget readings are
driven with Hypothesis as well as with named cases, in
`timeout_budget_properties_test.py`: the unit table and the duration grammar
are where a single wrong entry would leave every comparison downstream an
inequality between two plausible numbers, which a fixed case only catches when
it happens to be the case somebody wrote.

The nextest configuration is parsed with `tomllib` rather than matched as text.
A text match finds a key inside a comment, inside a `filter` string, or in a
table nextest never consults, and reports a budget the runner does not use.
`.config/nextest.toml` sets `global-timeout = "30m"`, and the ordering
assertion requires that value to sit above the largest per-test allowance and
inside the watchdog; it skips only when no whole-run budget is set at all. A
scraping reader would read a commented-out or filtered budget as one in force,
and comparing against a budget nobody had written is the failure this avoids.
`terminate-after` is optional, and a `slow-timeout` without it marks a test
slow and never stops it, so the reading refuses that form rather than reporting
one period as the budget. Every table in `.config/nextest.toml` sets it
explicitly, so no value here changes.

Durations are read with the grammar `humantime` accepts, which is what nextest
deserializes them with: a sequence of components each carrying a unit, written
`60s`, `2h 37m` or `2h37m`, with the long unit spellings. The reader used to
take one value and one of four short units, so `2h 37m`, `300 sec` and `30d`
were each refused as malformed while nextest loads all three, and two of them
sat in the contract's refusal list asserting the reader's own limitation as
though it were the file's fault. The grammar was measured against humantime
2.3.0, which is what the lockfile of the pinned cargo-nextest release resolves,
by compiling that parser and running the cases through it. Naming the version
matters: an earlier note here cited 2.4.0, which is the newest release rather
than the one the shared coverage action installs, `cargo-nextest@0.9.120`. A
value may carry a fractional part, and whitespace is tolerated around the
point, so `1.5m` and `1 . 5 m` are both ninety seconds. Whitespace inside the
number is ignored too, so `1 0s` is ten seconds and `1 2 . 3 4 s` is 12.34. The
short spellings `wk`, `wks`, `yr` and `yrs` are units alongside the longer
ones. The bare `0` is the one duration humantime reads without a unit, and it
is the exact text: its parser special-cases `0` before reading a character, so
`" 0 "` is refused and a reader that stripped whitespace first would accept a
duration nextest rejects. Case is significant, so `m` is minutes and `M` is
months.

The watchdog is resolved at the innermost scope that declares it, blank
included. GitHub takes the most specific declaration of an environment
variable, and an empty string is a declaration: a step setting
`RUN_RUST_CARGO_WAIT_TIMEOUT` to `""` hands that step's process an empty value,
not the job's. A reader that skips blanks and carries on outward credits the
lane with a budget nothing enforces, and the ordering assertion then passes
over a ceiling that does not exist. A case covers it, and restoring the
skip-and-continue reading fails that case alone.

Whitespace is Rust's, not Python's, and the class is written out for the same
reason the digit class below is. Rust's `char::is_whitespace` is the Unicode
White_Space property; Python's `\s` is that property plus U+001C to U+001F, the
file, group, record and unit separators, and `str.strip` and `str.split` carry
the same excess. Measured over the whole of Unicode, that is the only
disagreement, and it runs one way: Rust matches nothing Python does not. So a
reader spelling its whitespace `\s` skips a separator wherever it skips a
space, and reads `1\x1cs` as one second from a configuration nextest refuses at
startup.

Five sites carry the class, not one: the digit run, the fraction, the unit, the
outer trim, and the join that collapses a spaced number's digits. The join
cannot change an answer while the pattern refuses a separator, so no input
through the reader distinguishes a correct join from `str.split`; it is written
correctly anyway and tested through the widest whitespace the class allows,
because a later widening of the pattern would otherwise turn a refusal into a
silently different number.

Two contracts hold it. Four refusal cases name the separators, including one
between a digit and its unit, which is the shape nobody would notice in a file.
The other pins the class in both directions: Python's whitespace must exceed
the reader's by exactly those four, and the reader's must exceed Python's by
nothing. The second direction is the one the refusal cases cannot see, because
a class that had lost a genuine space would make the reader refuse
configurations nextest loads.

A digit is `0` to `9` and nothing else. Python's `\d` matches every Unicode
decimal digit and `int` reads them, so a reader written with it returns three
hundred seconds for `\u0663\u0660\u0660s` and for the mixed `3\u0660\u0660s`,
both of which humantime refuses: its parser compares against `'0'..='9'`,
reporting "expected number at 0" for the run that opens with such a digit and
"invalid character at 1" for the run that does not. The mixed spelling is the
sharper case because a reader that checked only its first character would still
accept it. That is the wrong direction for a contract, which would then certify
a configuration nextest cannot load.

The arithmetic is exact and in integers, because humantime's is: its parser
works in checked `u64` throughout and reports every failure as an overflow.
Reading a value through a float instead rounds what humantime refuses into
something plausible and certifies a configuration nextest cannot load.

Which integer depends on the unit, and this is the part a reader working in
nanoseconds alone gets wrong. A fraction of an hour or anything longer is
converted into whole *seconds*, so `0.000001h` is refused although its value is
a whole 3,600,000 ns, while `0.25h` is fifteen minutes. A fraction of a minute
or anything shorter is converted into whole nanoseconds, so `1.999999999s` is
accepted and `0.0000000015s` is not. A fraction of a nanosecond is refused
outright, whatever it spells, so even `1.0ns` will not load. The unit tables
are therefore split by which of the two a unit is measured in.

Four ceilings come with it, and they are different. A numeric literal must fit
the `u64` humantime reads it into, so `1000000000000000000000ns` is refused
even though its value in seconds is small. A fraction's own arithmetic is
checked, so `0.1000000000000000000s` overflows on the multiplication and
`1.00000000000000000000s` on the denominator, although both would fit as
durations. The accumulated seconds must fit the `u64` they are summed into, so
`18446744073709551615s` loads and one second more does not.

And the nanosecond remainder has a ceiling of its own, which is the one that
catches a reader summing into an unbounded integer. `add_current` opens with
`(out.subsec_nanos() as u64).add(nsec)?`, before any carry, so the remainder
held so far plus the component's nanoseconds must fit a `u64` by themselves.
Two values of `u64::MAX` nanoseconds carry the first to 18,446,744,073 seconds
and then overflow on the second. The duration they name, about 36.9 billion
seconds or some 1,169 years, is nowhere near the seconds ceiling; it is the
remainder that overflows, and it overflows first. That is exactly why checking
only the accumulated seconds afterwards reports a duration for text nextest
will not start under.

The reading was checked against the parser rather than against its
documentation: 4,016 generated durations, spanning every unit spelling,
fractions of up to twenty-one digits, values around the `u64` boundary and
humantime's tolerated whitespace, were run through both this reader and
humantime 2.3.0 compiled from the pinned release, and the two agreed on every
one.

It pins the condition each lane carries, which is none today. A skipped step
runs no `cargo`, so its watchdog never arms and the tiers say nothing about it:
`if: false` on the step or on its job would leave a lane that looks bounded and
is not. Adding a condition has to change the contract and this section with it,
and the lane coordinates are compared both ways, so a coverage lane appearing
without an entry fails rather than passing unexamined.

A document whose shape the reading does not expect fails on the assertion it
belongs to rather than with a Python fault several frames away. Each malformed
shape had its own way of raising during derivation: a `jobs` value that is a
scalar reaches `.items()`, and a non-mapping `env` or an unreadable
`timeout-minutes` reached arithmetic they could not survive. Each would have
failed the contract on a workflow that has nothing to do with coverage.

The two outcomes are not the same. A `jobs` value that is not a mapping yields
no lane at all: nothing in that document is a coverage job, so the document
contributes nothing to the assertions. A malformed value inside a job that does
run coverage keeps the lane because the lane is real. It reads the affected
budget as unset: an `env` that is not a mapping leaves the watchdog unset, and a
`timeout-minutes` that is not a positive whole number of minutes leaves the
ceiling unset. Both then fail the assertion that a coverage lane must declare
the tier in question, which is what a maintainer can act on.

It also pins how many coverage steps each job runs. The ceiling's requirement
is the sum of the watchdogs found, so deleting one of a job's two coverage
steps lowers that requirement by 2,700 s and every timing assertion still
passes while the lane measures half of what it did.

`tests/workflow_contracts/timeout_budget_properties_test.py` holds the readings
themselves, driven with synthetic workflows and synthetic nextest
configurations rather than the repository's own. Every ceiling here sits well
above its requirement, so a missing term in the derivation changes nothing
observable in this tree; against controlled numbers it does not. The lane
reading requires its documents, so `coverage_jobs_of(documents)` reaches no
filesystem and no parser; `workflow_documents(directory)` is where the
filesystem access and the YAML parsing happen, defaulting its directory to the
repository's own; and `coverage_jobs_in(directory)` composes the two. The
boundary is one named function rather than a default inside the derivations.

The `binstall-packaging` job also declares no ceiling. It invokes no coverage
step, so it is outside this contract, and bounding it is separate work.

[shared-actions-coverage]: https://github.com/leynos/shared-actions/blob/main/.github/actions/generate-coverage/README.md

## CodeScene coverage belongs to main

`coverage-main.yml` is the only workflow in this repository that runs a
CodeScene action. It runs on pushes to main, generates ratcheted coverage, and
uploads with `mode: upload`. No workflow serving pull requests names a
CodeScene action, invokes `cs-coverage`, or puts `CS_ACCESS_TOKEN` in reach of
any process.

This is the estate rule `main-owned-codescene-coverage`, and it is a policy
rather than a gap. A pull request from a fork cannot read the repository's
secrets, so the changed-line check on that lane was a silent skip for exactly
the contributions least likely to have been measured already. On a branch it
put a second tool on the critical path, and when this CodeScene project stopped
returning a gates configuration that tool failed every pull request here over a
defect in none of them.

What a pull-request lane keeps is the ratchet. `ci.yml` runs
`generate-coverage` with `with-ratchet` on the ubuntu leg, comparing against
the baseline `coverage-main.yml` writes, which applies the same "do not go
backwards" gate from this repository's own history with no token and no second
tool.

**The two lanes must be built the same way.** Both run two feature legs, paired
by the report each writes. The ratchet compares this commit's report against
that baseline, so the inputs deciding *what* is measured must agree across the
pair: the output path, the format and the feature selection.

That was wrong before this adoption. The pull-request leg compiled `metrics`
and the publisher's did not, so every line of the metrics facade read as newly
uncovered against a baseline that had never compiled it. The failure is silent,
because the ratchet reports a number either way: nothing distinguishes a fall
in coverage from two runs having built different code. The fix is on the
publisher rather than the lane, since the comment on that leg records why
`metrics` is there at all, namely that it is off by default and the facade
would otherwise never be compiled in CI.

**The publisher is guarded on the ref, and runs one at a time.** It declares no
`workflow_dispatch` today, so `github.ref` is always main and the guard changes
nothing now; it is there because a dispatch can be aimed at any branch and the
upload carries no ref, so adding one later would let a run from a feature
branch publish that branch's coverage as the trunk's. The contract holds the
guard as one `&&` term and refuses any `||`, because a substring match passes
`... && ref == main || dispatch`, which makes every conjunct optional. The
concurrency group queues a superseded run rather than cancelling it: two runs
racing would decide the baseline by which finished last, and a cancelled run
abandons both its upload and its baseline write, while a queued one publishes
later and the later push still wins.

**No checksum input.** `installer-checksum` is rejected outright when non-empty
from the pinned uploader, and `archive-checksum` is not a rename of it: it
digests the action's CLI manifest archive, while the `CODESCENE_CLI_SHA256`
repository variable holds the installer script's digest. Carrying the old value
across under the new name fails every run. The action pins the CLI through its
own manifest now, which is what that variable stood in for; the variable is
unreferenced and can be removed from the repository's settings.

`tests/workflow_contracts/codescene_coverage_test.py` holds the shape, reading
through `codescene_coverage.py` (which workflows a rule applies to) and
`codescene_reach.py` (what a selected workflow must not do). The generic
parsing is in `workflow_reading.py`. `codescene_reader_test.py` drives those
readings on documents this repository does not contain, and
`codescene_reader_properties_test.py` drives the step and token readings with
Hypothesis over generated workflows of any number of jobs and steps.

**The pull-request lane is a closure, not a trigger list.** A workflow
declaring only `workflow_call` runs on a pull request when a pull-request
workflow calls it, and `secrets: inherit` hands it the token. Every
pull-request clause (the action, the command, the token and the `codescene.io`
host) runs over the pull-request workflows and everything they call,
transitively. A call is recognized by shape rather than by a list of prefixes:
a leading `./` is stripped, and the remainder must be a file directly under
`.github/workflows/`. `pull_request_closure_test.py` holds a `workflow_call`
probe that curls the CodeScene API with an inherited token and asserts that
both the token clause and the host clause catch it. The host clause reads every
value in each parsed workflow rather than a list of expected places, because a
URL reaches a step through the workflow's, the job's or the step's `env`, a
step's inputs, or a reusable-workflow call's `with`; comments are not read,
because the parser discards them.

**The ratchet has to stay switched on.** Pairing the legs' selections proves
the comparison is fair, not that it happens, so the contract also holds each
pull-request leg to the publisher's choice: a leg whose baseline is written
ratchets on the Linux leg, and a leg whose baseline is not written does not
ratchet at all.

**Workflows are loaded strictly.** The loader refuses a mapping that declares
one key twice, since PyYAML otherwise keeps the last value silently and a job
declaring `runs-on` twice would read as the half GitHub may not run. A file
that is not YAML at all is reported as a `WorkflowReadingError` naming the
file, not as a parser error naming none.

Two properties of that reading are worth knowing before changing it. The
publisher is "pushes to main **and serves no pull request**": a repository's
main workflow often declares both, so a predicate reading only the push makes
one file simultaneously required to upload and forbidden from uploading. And
the trigger reader looks under both `"on"` and the boolean `True`, because YAML
1.1 resolves an unquoted `on:` to a boolean; a reader finding nothing makes
every rule above pass over an empty set, which reports compliance rather than
an error. `load_workflow` uses `yaml.BaseLoader` and keeps the string, which is
precisely why narrowing the reader to the string key alone fails nothing
against the real files, and why the constructed case in
`codescene_reader_test.py` exists.

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
