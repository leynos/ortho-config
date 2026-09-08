# OrthoConfig roadmap

This roadmap describes the future product work for OrthoConfig. The completed
v0.8.0-era phases are retained in
[Archived v0.8.0 roadmap](archive/v0-8-0-roadmap.md), and this active roadmap
continues the numbering from that archive with forward-looking phases,
workstreams, and concrete tasks. The roadmap is intentionally date-free:
sequencing is driven by dependency order, review size, and product coherence.

The source documents for this roadmap are:

- [Design Document: The `OrthoConfig` Crate](design.md);
- [Agent-native CLI assistance design](agent-native-cli-design.md);
- [OrthoConfig IR documentation design for cargo-orthohelp](cargo-orthohelp-design.md);
- [Improved error message design](improved-error-message-design.md);
- [CLI localization surface design](cli-localization-design.md);
- [DDLint gap analysis](ddlint-gap-analysis.md);
- [ADR-001: Replace `serde_yaml` with `serde-saphyr`](adr-001-replace-serde-yaml-with-serde-saphyr.md);
- [ADR-002: Replace `cucumber-rs` with `rstest-bdd`](adr-002-replace-cucumber-with-rstest-bdd.md);
- [ADR-004: Cargo external-subcommand entry-point architecture](adr-004-cargo-external-subcommand-entry-point.md);
- [ADR-005: Subcommand docs companion trait](adr-005-subcommand-docs-companion-trait.md);
- [RFC 0003: Trust-aware policy composition](rfcs/0003-trust-aware-policy-composition.md).

The first downstream consumers for the expanded agent-native contract are
Weaver and Netsuke. Their plans make several generic requirements explicit:
dual human/machine renderers, canonical global flags, strict JSON mode,
exit-code metadata, skill manifest validation, context command naming,
capability provenance, profile redaction, delivery and feedback parsers, and
configurable execution ledgers. OrthoConfig should absorb those reusable
contracts before the applications fossilize divergent local schemas.

## 5. Reconcile the design baseline

This phase makes the documentation set trustworthy before new agent-native
features are implemented. It removes stale completion claims, preserves
historical context, and records the exact boundary between OrthoConfig's
responsibilities and downstream application responsibilities.

### 5.1. Repair current truth

- [x] 5.1.1. Reconcile the missing-required-values design with the actual
  error surface.
  - [x] Verify whether `OrthoError::MissingRequiredValues` exists, whether it
    was renamed, or whether the feature was never implemented.
  - [x] Update `docs/improved-error-message-design.md`,
    `docs/users-guide.md`, and release notes so they describe the current
    behaviour accurately.
  - [x] If the implementation is absent, keep the design as proposed work and
    move the build into phase 7.

- [x] 5.1.2. Retire stale retrospective roadmap items.
  - [x] Move completed historical milestones out of the active roadmap path or
    reference them as background only. The active roadmap should keep the v0.8.0
    completion detail in `docs/archive/v0-8-0-roadmap.md` and use explicit
    archive-status notes where a historical completion claim is corrected,
    superseded, or deferred into active work. See
    `docs/archive/v0-8-0-roadmap.md` §Archived v0.8.0 roadmap and
    `docs/design.md` §6.
  - [x] Confirm DDLint gap-analysis items are either implemented, deliberately
    deferred, or replaced by agent-native policy work. Loading gaps belong to
    the historical analysis once implemented; command-shape ideas such as
    `rules`, `explain`, `--format <compact|json|rich>`, and `--no-ignore` are
    prior art unless a later agent-native roadmap item names reusable
    OrthoConfig policy. See `docs/ddlint-gap-analysis.md` §Observed gaps and
    current status and §Next steps; see also `docs/agent-native-cli-design.md`
    §5 and §9.
  - [x] Update historical design notes so maintainers can tell whether a note
    is active guidance or preserved rationale. Prefer document-level `Status:`
    markers over scattered caveats. See `docs/documentation-style-guide.md`
    §Design document, ADR, and RFC and §Architectural decision records; see
    also `docs/agent-native-cli-design.md` §1.

- [x] 5.1.3. Add an agent-native documentation index.
  - [x] Link `docs/design.md`,
    `docs/cargo-orthohelp-design.md`, and the user guide to
    `docs/agent-native-cli-design.md`.
  - [x] State that the documentation IR and agent-context schema are sibling
    outputs with independent versioning.
  - [x] Document that OrthoConfig models, generates, and lints contracts; it
    does not become every downstream application's command runner.

### 5.2. Establish schema ownership

- [x] 5.2.1. Define ownership for documentation IR, agent context, and policy
  reports.
  - [x] Keep localized documentation IR in the existing `OrthoConfigDocs`
    contract.
  - [x] Specify a compact agent-context schema with its own schema version.
  - [x] Specify a policy report schema for warnings and hard failures emitted
    by `cargo-orthohelp`.

- [x] 5.2.2. Record migration rules for existing consumers.
  - [x] Ensure existing `--format ir`, `--format man`, `--format ps`, and
    `--format all` behaviours remain compatible until a versioned migration is
    explicitly approved.
  - [x] Document how new metadata fields default when older derives do not
    provide them.
  - [x] Add compatibility notes for downstream crates that only consume
    human-facing documentation output.

- [x] 5.2.3. Record consumer dependency boundaries for Weaver and Netsuke.
  - Requires 5.2.1 and 5.2.2.
  - See agent-native-cli-design.md §2.1 and
    adr-003-define-schema-ownership-for-agent-native-contracts.md.
  - [x] Document that OrthoConfig owns reusable command-contract machinery,
    while Weaver owns semantic code-edit execution and Netsuke owns build and
    package execution.
  - [x] Mark whole-CLI introspection, strict vocabulary policy, agent-context
    IR, and localized help generation as hard dependencies for Weaver's
    generated command surface.
  - [x] Mark profiles, delivery, feedback, skill manifests, and execution
    ledgers as soft dependencies where consuming applications may temporarily
    adapt locally if OrthoConfig support is not available in time.

## 6. Deliver whole-CLI introspection

This phase makes the command tree visible. Agent-context output and vocabulary
linting cannot be correct while generated metadata only describes top-level
fields.

### 6.1. Populate subcommand metadata

- [x] 6.1.1. Generate recursive `DocMetadata.subcommands` values.
  - [x] Reuse information already parsed by `SelectedSubcommandMerge` where it
    describes selected subcommand enum variants.
  - [x] Introduce a small companion trait if enum-level documentation cannot be
    represented cleanly through the existing `OrthoConfigDocs` trait.
  - [x] Preserve deterministic command ordering so generated documentation and
    agent context are stable.

- [x] 6.1.2. Cover nested command trees with behavioural fixtures.
  - Requires 6.1.1.
  - See cargo-orthohelp-design.md §§6-7 and agent-native-cli-design.md §4.
  - [x] Add a fixture CLI with at least one nested subcommand and one command
    with no subcommands.
  - [x] Assert that generated IR includes the recursive tree, field metadata,
    command names, examples, and Windows wrapper metadata where applicable.
  - [x] Ensure existing man-page and PowerShell output remains compatible when
    subcommands are present.

### 6.2. Add compact agent-context output

- [x] 6.2.1. Add `--format agent-context` to `cargo-orthohelp`.
  - Requires 6.1.1.
  - See agent-native-cli-design.md §3.2 and §4; cargo-orthohelp-design.md
    §6.3.1.
  - [x] Generate JSON from the same bridge output used by documentation IR.
  - [x] Include command paths, verbs, flags, positional arguments, value types,
    required inputs, defaults, and enum values.
  - [x] Exclude localized long prose unless a concise summary is needed for
    command selection.

- [x] 6.2.2. Version and validate the agent-context schema.
  - Requires 6.2.1.
  - See agent-native-cli-design.md §3.2 and §8;
    adr-003-define-schema-ownership-for-agent-native-contracts.md.
  - [x] Add schema-version tests that fail on accidental shape changes.
  - [x] Add golden fixtures for a simple CLI, a nested CLI, and a CLI with enum
    values.
  - [x] Document the schema and compatibility policy in
    `docs/agent-native-cli-design.md`.

- [x] 6.2.3. Define downstream `context --json` command naming.
  - Requires 6.2.1.
  - See agent-native-cli-design.md §3.2 and §5;
    adr-007-downstream-context-command-naming.md.
  - [x] Prefer `<tool> context --json` for application command surfaces while
    keeping `cargo orthohelp --format agent-context` as the generator format.
  - [x] Include a payload `kind` such as `<tool>.agent_context`.
  - [x] Avoid public `agent-context` aliases before first release unless a
    migration explicitly requires them.

### 6.3. Validate skill manifests against real commands

- [x] 6.3.1. Add skill manifest metadata.
  - Requires 6.2.1.
  - See agent-native-cli-design.md §3.4.
  - Modelling can land before 6.2.1; only generator output depends on 6.2.1.
  - [x] Model skill manifest path, schema version, and command index metadata.
  - [x] Link skill manifest locations from agent context.
  - [x] Keep downstream skill prose application-owned.

- [ ] 6.3.2. Add skill manifest validation.
  - Requires 6.3.1 and step 7.1.
  - See agent-native-cli-design.md §3.4 and §5.
  - [ ] Validate that skills mention real command paths and flags.
  - [ ] Validate that examples honour canonical vocabulary and global options.
  - [ ] Add fixtures for Weaver-style operation skills and Netsuke-style build
    workflow skills without embedding either application's domain semantics.

## 7. Enforce agent-native policy

This phase turns design rules into checks. The target is mechanical assistance:
projects should learn about inconsistent verbs, flags, output contracts, and
unsafe mutation surfaces before release.

### 7.1. Implement vocabulary policy

- [ ] 7.1.1. Add an opt-in agent-native policy configuration.
  - Requires step 6.2.
  - See agent-native-cli-design.md §3.3 and §5.
  - [ ] Support `off`, `warn`, and `deny` modes.
  - [ ] Provide canonical defaults for verbs and flags: `get`, `list`,
    `create`, `update`, `delete`, `--json`, `--no-input`, `--force`,
    `--dry-run`, `--limit`, `--cursor`, `--wait`, `--profile`, and
    `--deliver`.
  - [ ] Allow explicit project exceptions that are visible in policy output.

- [ ] 7.1.2. Lint off-policy verbs and flags.
  - Requires 7.1.1.
  - See agent-native-cli-design.md §5; ddlint-gap-analysis.md §Next steps.
  - [ ] Flag `info`, `ls`, `--format=json`, `--output json`, and
    `--skip-confirmations` under strict policy.
  - [ ] Report the canonical replacement in every diagnostic.
  - [ ] Add tests for warning mode, deny mode, and configured exceptions.

- [ ] 7.1.3. Add the canonical human-facing global option glossary.
  - Requires 7.1.1.
  - See agent-native-cli-design.md §5 and §6.2.1.
  - [ ] Standardize names for colour, emoji, progress, accessibility, plain
    output, pager control, width, locale, quiet, and verbose options when those
    concepts are present.
  - [ ] Lint near-miss names such as `--output-format`, `--colour-policy`,
    `--diag-json`, boolean `--progress`, `--no-emoji`, and boolean
    `--accessible`.
  - [ ] Permit projects to omit unsupported concepts without forcing every CLI
    to implement every global flag.

### 7.2. Model behavioural semantics

- [ ] 7.2.1. Add metadata for non-interactive execution and mutation
  boundaries.
  - Requires step 6.2.
  - See agent-native-cli-design.md §6.1 and §6.4.
  - [ ] Represent whether a command is non-interactive, may prompt, or needs a
    bypass flag.
  - [ ] Represent whether a command reads, writes, deletes, or submits work.
  - [ ] Lint destructive commands that lack `--force` or equivalent approved
    metadata.

- [ ] 7.2.2. Add dual-renderer metadata.
  - Requires 7.2.1.
  - See agent-native-cli-design.md §6.2 and §6.2.1.
  - [ ] Model human renderer support and machine renderer support separately.
  - [ ] Model TTY sensitivity, closed-stdin behaviour, colour, emoji,
    progress, pager, width, accessibility, and plain-output policy.
  - [ ] Model localized versus non-localized fields so protocol identifiers do
    not drift with human language.

- [ ] 7.2.3. Add metadata for structured output and exit classes.
  - Requires 7.2.1.
  - See agent-native-cli-design.md §6.2 and §6.2.2.
  - [ ] Model `--json` support, stdout contracts, stderr diagnostics, and exit
    classifications.
  - [ ] Lint data-returning commands that cannot emit structured output.
  - [ ] Document stable exit classes for `cargo-orthohelp`.

- [ ] 7.2.4. Add a JSON mode stream contract.
  - Requires 7.2.3.
  - See agent-native-cli-design.md §6.2.
  - [ ] Model success stdout as a single JSON result document.
  - [ ] Model failure stderr as a single JSON diagnostic document.
  - [ ] Model subprocess output policy so child process output never leaks to
    stdout in JSON mode except via an agreed artefact path.

- [ ] 7.2.5. Add exit-code taxonomy metadata.
  - Requires 7.2.3.
  - See agent-native-cli-design.md §6.2.2.
  - [ ] Model code-to-class mappings in documentation IR and agent context.
  - [ ] Lint that every documented error class has an exit code.
  - [ ] Lint that JSON diagnostics report the same class and code.

- [ ] 7.2.6. Add metadata for bounded list output.
  - Requires 7.2.1.
  - See agent-native-cli-design.md §6.5.
  - [ ] Model `--limit`, `--cursor`, default limits, maximum limits, and
    truncation hints.
  - [ ] Lint list-shaped commands that lack bounded defaults.
  - [ ] Keep generated agent descriptions within an explicit size budget.

- [ ] 7.2.7. Add generic capability and provenance metadata.
  - Requires 7.2.1.
  - See agent-native-cli-design.md §6.9.
  - [ ] Model capability identifiers, command mapping, provider visibility,
    provider override policy, and whether provider provenance appears in JSON.
  - [ ] Lint that ordinary public commands do not require backend provider
    names when a stable capability command would suffice.
  - [ ] Keep provider registries, selection, execution, and safety harnesses
    application-owned.

### 7.3. Rebuild improved required-value diagnostics

- [ ] 7.3.1. Implement or restore enumerating missing-required-values errors
  after the phase 5 truth audit.
  - Requires 5.1.1.
  - See improved-error-message-design.md §§1-3 and agent-native-cli-design.md
    §6.3.
  - [ ] Aggregate all missing required values before returning an error.
  - [ ] Show valid supply paths through CLI flags, environment variables, and
    file keys.
  - [ ] Add unit, macro, and behavioural tests that prove agents can
    self-correct from one diagnostic.

## 8. Make `cargo-orthohelp` the reference CLI

This phase dogfoods the table-stakes agent-native behaviours before asking
downstream users to adopt them.

### 8.1. Add structured command results

- [ ] 8.1.1. Add `--json` to `cargo-orthohelp`.
  - Requires 7.2.3 and 7.2.4.
  - See agent-native-cli-design.md §7 and cargo-orthohelp-design.md §6.
  - [ ] Emit a structured success summary containing generated artefact kind,
    locale, and path.
  - [ ] Emit structured errors when JSON mode is requested.
  - [ ] Keep human diagnostics on stderr and machine-readable command results
    on stdout.

- [ ] 8.1.2. Enumerate valid choices in errors.
  - Requires 8.1.1.
  - See agent-native-cli-design.md §6.3 and §7.
  - [ ] Invalid formats should list every supported format.
  - [ ] Unknown packages should list candidate packages from Cargo metadata.
  - [ ] Unknown binaries should list candidate binary targets.
  - [ ] Invalid locales should list configured locales and the fallback
    `en-US` behaviour.

### 8.2. Make generated artefacts robust

- [ ] 8.2.1. Write generated files atomically.
  - See cargo-orthohelp-design.md §6.2 and §10.
  - [ ] Write to a sibling temporary file, flush it, and rename into place.
  - [ ] Preserve existing output paths and cache semantics.
  - [ ] Add failure-path tests that prevent partial generated artefacts from
    replacing valid files.

- [ ] 8.2.2. Document the reference CLI contract.
  - Requires 8.1.1, 8.1.2, and 8.2.1.
  - See cargo-orthohelp-design.md §§6 and 12; agent-native-cli-design.md §7.
  - [ ] Update `cargo-orthohelp/README.md` with stdout/stderr behaviour,
    `--json`, JSON mode stream contracts, exit classes, and agent-native lint
    usage.
  - [ ] Include examples for human documentation output and agent-context
    output.
  - [ ] Explain which behaviours are already implemented and which require
    future phases.

### 8.3. Standardize Cargo external-subcommand entry points

This step answers whether OrthoConfig can make Cargo subcommand binaries
straightforward without moving entry-point shape into the core configuration
trait. The outcome informs future `cargo-*` tools and keeps `cargo-orthohelp`
from carrying a bespoke pattern that other crates copy by hand. See
`docs/design.md` §4.17.

- [ ] 8.3.1. Add a small `ortho_config::cargo` helper for hand-built clap
  commands.
  - See design.md §4.17 and adr-004-cargo-external-subcommand-entry-point.md.
  - [ ] Provide an `external_subcommand` helper that accepts the installed
    binary name, injected Cargo subcommand name, and an existing
    `clap::Command`.
  - [ ] Return the standard `Command::new("cargo")` shape with
    `bin_name("cargo-<name>")` and a `<name>` subcommand.
  - [ ] Preserve the existing options on the inner command rather than
    introducing another configuration-loading pathway.
  - [ ] Success: a hand-built `clap::Command` can support both
    `cargo <name> [OPTIONS]` and `cargo-<name> <name> [OPTIONS]` without
    duplicating parser setup.

- [ ] 8.3.2. Document the derive-friendly Cargo subcommand template.
  - Requires 8.3.1.
  - See design.md §4.17 and adr-004-cargo-external-subcommand-entry-point.md.
  - [ ] Add user-guide and README examples showing a `Cli` wrapper with
    `#[command(name = "cargo", bin_name = "cargo-<name>")]`, a
    `#[command(subcommand)]` field, and an enum variant wrapping the existing
    `#[derive(clap::Args)]` option struct.
  - [ ] Explain that Cargo intentionally injects the subcommand name as the
    first positional argument when dispatching `cargo <name>`.
  - [ ] State that the wrapper is entry-point structure, not a change to
    OrthoConfig's merge precedence or `OrthoConfig::load`.
  - [ ] Success: users can adapt the documented template without reading
    Cargo's external-subcommand reference or `cargo-orthohelp` internals.

- [ ] 8.3.3. Evaluate an optional macro attribute for Cargo subcommand
  wrappers.
  - Requires 8.3.2.
  - See design.md §4.17 and adr-004-cargo-external-subcommand-entry-point.md.
  - [ ] Prototype the candidate `cargo_subcommand` and `cargo_bin` attribute
    syntax from `docs/design.md` §4.17.
  - [ ] Decide whether the macro should generate a companion wrapper parser,
    a helper function, or only metadata consumed by documentation tooling.
  - [ ] Reject the attribute unless it removes real repeated boilerplate
    across multiple OrthoConfig-powered Cargo tools without hiding the Cargo
    dispatch contract.
  - [ ] Success: the design records either a narrow accepted macro surface or
    a clear reason to keep the helper and documentation as the only supported
    abstraction.

- [ ] 8.3.4. Add regression fixtures for Cargo-dispatched binaries.
  - Requires 8.3.1.
  - See design.md §4.17 and adr-004-cargo-external-subcommand-entry-point.md.
  - [ ] Add a small workspace fixture or shared test helper that runs
    `cargo-<name> <name> --help`.
  - [ ] Add a companion assertion for `cargo <name> --help` with the fixture
    binary on `PATH`.
  - [ ] Reuse the fixture for `cargo-orthohelp` and any future `cargo-*`
    tools.
  - [ ] Success: tests fail if a Cargo subcommand binary accepts direct flat
    invocation but rejects Cargo's injected subcommand argument.

## 9. Add compounding primitives

This phase adds optional helpers and metadata for repeated agent workflows. It
must preserve the boundary that OrthoConfig provides reusable contracts and
helpers, while downstream applications own domain behaviour.

### 9.1. Profile contracts

- [ ] 9.1.1. Design and implement optional profile metadata.
  - Requires step 6.2.
  - See agent-native-cli-design.md §6.7.
  - [ ] Standardize `--profile <name>` as the root selection flag.
  - [ ] Document the precedence
    `built-in defaults < config files < selected profile < environment <
    flags`.
  - [ ] Expose profile support, profile listing commands, and selected-profile
    semantics in agent context.

- [ ] 9.1.2. Add profile redaction metadata.
  - Requires 9.1.1.
  - See agent-native-cli-design.md §6.7.
  - [ ] Mark secret and reference-only profile fields.
  - [ ] Redact sensitive profile values from context output and generated
    documentation examples.
  - [ ] Validate that profile names can be exposed without leaking profile
    contents.

- [ ] 9.1.3. Decide whether OrthoConfig ships a profile store helper.
  - Requires 9.1.1.
  - See agent-native-cli-design.md §6.7.
  - [ ] Evaluate a JSON-backed helper against applications that already manage
    their own profile storage.
  - [ ] If implemented, provide list, show, save, and delete helpers without
    forcing downstream CLIs to use a specific command framework.

### 9.2. Delivery and feedback contracts

- [ ] 9.2.1. Design reusable delivery target parsing.
  - Requires step 6.2.
  - See agent-native-cli-design.md §6.8.
  - [ ] Support `stdout`, `file:<path>`, and `webhook:<url>` schemes.
  - [ ] Enumerate supported schemes when parsing fails.
  - [ ] Require atomic file writes and visible webhook HTTP status reporting.
  - [ ] Keep application-specific webhook payload semantics outside
    OrthoConfig.

- [ ] 9.2.2. Design reusable feedback storage.
  - Requires 9.2.1.
  - See agent-native-cli-design.md §6.8.
  - [ ] Store local feedback as JSONL by default.
  - [ ] Optionally send feedback upstream when an endpoint is configured.
  - [ ] Expose local and upstream feedback capability in agent context.

### 9.3. Execution ledger contracts

- [ ] 9.3.1. Model application-level execution ledgers.
  - Requires step 6.2 and 7.2.1.
  - See agent-native-cli-design.md §6.6.
  - [ ] Represent `--wait`, job identifier fields, status commands, and job
    ledger support in metadata.
  - [ ] Lint async submit commands that force agents to write their own polling
    loops.
  - [ ] Keep this separate from asynchronous configuration loading in
    `OrthoConfig::load`.

- [ ] 9.3.2. Support configurable public ledger nouns.
  - Requires 9.3.1.
  - See agent-native-cli-design.md §6.6.
  - [ ] Allow applications to expose `jobs`, `runs`, `tasks`, or `operations`
    while sharing one metadata model.
  - [ ] Include record identifiers, status enums, timestamps, command paths,
    input hashes, idempotency keys, log references, result references, prune
    commands, and bounded list behaviour.

- [ ] 9.3.3. Evaluate a reusable execution ledger helper.
  - Requires 9.3.1.
  - See agent-native-cli-design.md §6.6.
  - [ ] Decide whether a local JSONL ledger belongs in OrthoConfig or should
    remain application-owned.
  - [ ] If implemented, provide list, get, and prune primitives that downstream
    CLIs can expose under their configured ledger noun.

## 10. Deferred extensions

These items are useful but should wait until whole-CLI introspection,
agent-context output, policy linting, and the `cargo-orthohelp` reference CLI
are working.

### 10.1. Integration extensions

- [ ] 10.1.1. Generate Model Context Protocol (MCP) descriptions from
  agent-context output.
  - Requires phase 6.
  - See agent-native-cli-design.md §10.
- [ ] 10.1.2. Explore OpenAPI-shaped runtime explorer endpoints for downstream
  applications.
  - Requires phase 6.
  - See agent-native-cli-design.md §10.
- [ ] 10.1.3. Generate optional long-form skill prose from documentation IR and
  agent context after validation exists.
  - Requires phases 6 and 7.
  - See agent-native-cli-design.md §3.4 and §10.

### 10.2. Configuration extensions

- [ ] 10.2.1. Explore asynchronous loading of configuration files and
  environment variables.
  - See design.md §4.1 and §8.
- [ ] 10.2.2. Provide an API for registering custom `figment` providers, such as
  secrets managers or remote key-value stores.
  - See design.md §5 and §8.
- [ ] 10.2.3. Investigate live reloading of configuration when files change.
  - See design.md §8.

## 11. Promote and widen the CLI localization surface

This phase promotes the load-bearing localization helpers from the
`hello_world` example to first-class crate surface, widens clap-error
translation coverage, names a locale-resolution lifecycle that survives the
locale-flag chicken-and-egg, bridges OrthoConfig with `i18n-embed`, and extends
the derive, so localization identifiers are generated rather than
hand-authored. The design lives in
[cli-localization-design.md](cli-localization-design.md). Sequencing is
quality-of-life-first: §11.1 and §11.2 carry no policy risk, while §11.3 and
later progressively add opinion.

### 11.1. Promote example helpers to crate API

- [x] 11.1.1. Promote `LocalizeCmd` to a public extension trait on
  `clap::Command`.
  - See cli-localization-design.md §4.
  - [x] Move the example trait into `ortho_config::localizer` and extend it
    to cover per-argument `help`, `long_help`, and `value_name`, plus
    subcommand `about`/`long_about` recursively, optional `version`/
    `long_version`, and the help-template footer.
  - [x] Expose `LocalizeCmd::with_base("…")` for applications that share a
    catalogue across multiple binaries.
  - [x] Add the public `ortho_config::message_id_for(&command_path, suffix)`
    function with documented identifier shape, ASCII normalization rules,
    and panic-on-collision behaviour.
  - [x] Success: the `hello_world` example deletes its local
    `LocalizeCmd` impl and re-exports the crate one for one release.

- [x] 11.1.2. Promote `try_parse_localized*` to a generic blanket trait.
  - Requires 11.1.1.
  - See cli-localization-design.md §4.2.
  - [x] Add `LocalizedParse: clap::Parser` with `try_parse_localized`,
    `try_parse_localized_from`, and `try_parse_localized_with_matches`.
  - [x] Provide a blanket impl for every `clap::Parser`.
  - [x] Preserve the `*_with_matches` variant for callers that need the
    raw `ArgMatches` for `load_and_merge_with_matches`.
  - [x] Add identifier-coverage tests that compare derive-emitted
    identifiers with `message_id_for` output across a fixture command tree.
  - Decision: expose `parse_localized_command` as the base-agnostic primitive
    and keep `LocalizedParse` as the default-base convenience wrapper.
  - Decision: keep the `hello_world` example on the `hello_world.cli` base so
    the multi-segment catalogue example remains available for 11.1.3.
  - Finding: identifier coverage is locked by a recording localizer that
    compares every runtime lookup with `message_id_for` over a fixture
    `#[derive(clap::Parser)]` command tree.
  - Finding: the promoted parser helpers inherit `LocalizeCmd::localize`
    panics for invalid Fluent roots or colliding identifiers; ADR-006 records
    that widened panic surface until 11.1.3 adds derive-time guards.
  - Progress: `examples/hello_world` now calls `parse_localized_command` and
    no longer carries `ParsedCommandLine` or inherent
    `CommandLine::try_parse_localized*` methods.
  - Validation: `make check-fmt`, `make typecheck`, `make lint`, `make test`,
    `make markdownlint`, and `make nixie` passed on 2026-06-15.
  - Observation: repeated `coderabbit review --agent` attempts stalled at
    `preparing_sandbox` with no findings or rate-limit message; this is
    recorded in the 11.1.2 execplan.

- [ ] 11.1.3. Add the `OrthoConfigLocalization` trait and derive emission.
  - Requires 11.1.2.
  - See cli-localization-design.md §8.1 and §8.2.
  - [ ] Define `OrthoConfigLocalization` with `ABOUT_ID`, `LONG_ABOUT_ID`,
    `USAGE_ID`, and per-argument `ARG_IDS` constants.
  - [ ] Extend the `OrthoConfig` derive to emit `OrthoConfigLocalization`
    impls. Generate identifiers from command path and field `id` (or
    kebab-cased field name).
  - [ ] Add a blanket `OrthoConfigDocs` impl that delegates to
    `OrthoConfigLocalization` so the docs IR picks up the same identifiers.
  - [ ] Emit `${OUT_DIR}/ortho-config/cli-identifiers.json` with a 1 MiB
    cap and split-file behaviour for larger trees.
  - [ ] Add a compile-time `compile_error!` for fields whose normalized
    identifiers collide.

### 11.2. Widen clap-error coverage and preserve clap's rich context

- [ ] 11.2.1. Ship en-US translations for the complete clap stable error
  matrix.
  - Requires 11.1.1.
  - See cli-localization-design.md §6.1 and §6.2.
  - [ ] Add Fluent strings for `NoEquals`, `ValueValidation`, `TooManyValues`,
    `TooFewValues`, `WrongNumberOfValues`, `ArgumentConflict`,
    `InvalidUtf8`, `Io`, and `Format` (alongside the four existing
    identifiers).
  - [ ] Define `pub enum ClapErrorTranslation { Translated(&'static str),
    DisplayOnly }` and expose `pub const CLAP_ERROR_IDS:
    &[(clap::error::ErrorKind, ClapErrorTranslation)]`. The matrix is
    **exhaustive** over `ErrorKind`: display-only variants
    (`DisplayHelp`, `DisplayHelpOnMissingArgumentOrSubcommand`,
    `DisplayVersion`) appear with the `DisplayOnly` sentinel, so the gate
    reduces to a simple length comparison.
  - [ ] Implement the mechanical coverage gate (build script plus
    `const_assert_eq!`) as specified in
    [cli-localization-design.md §6.1.1](cli-localization-design.md). The
    design document owns the mechanism; this task implements it. The
    build script emits the **total** `ErrorKind` variant count without
    classifying display-only variants, because the exhaustive matrix
    pairs each variant with either a translated identifier or the
    sentinel.

- [ ] 11.2.2. Switch error localization to clap's mutation surface.
  - Requires 11.2.1.
  - See cli-localization-design.md §6.4.
  - [ ] Rewrite `localize_clap_error_with_command` to call
    `clap::error::Error::insert(ContextKind::Custom, ...)` plus
    `Error::format(cmd)` rather than `Error::raw`, so the usage tail,
    suggestion list, and styling survive.
  - [ ] Run the localization eagerly inside `try_parse_localized*` so the
    error is fully rendered before it escapes the helper's stack frame.
  - [ ] Add behavioural tests that prove the suggestion list survives
    localization on at least `UnknownArgument` and `InvalidSubcommand`.
  - [ ] Deprecate the old `Error::raw` path with a removal note for the
    next minor release.

- [ ] 11.2.3. Add observable fallback for missing translations.
  - Requires 11.2.1.
  - See cli-localization-design.md §6.3 and §9.
  - [ ] Emit a `tracing` event at `warn` severity when the missing
    identifier originates from a `ClapError`, and at `debug` severity for
    application messages.
  - [ ] Introduce the `MissingTranslationReporter` trait and wire it into
    `FluentLocalizer`, `FluentEmbedLocalizer` (deferred to 11.4.1), and the
    clap-error pipeline.
  - [ ] Add a `ClapErrorCoverage` builder that walks `CLAP_ERROR_IDS`,
    filters to `ClapErrorTranslation::Translated(id)` entries (skipping
    the display-only sentinels), and reports identifiers that the supplied
    `Localizer` fails to resolve.

- [ ] 11.2.4. Document and ship the monomorphized `LocalizedFormatter`
  escape hatch.
  - Requires 11.2.2.
  - See cli-localization-design.md §6.4.1.
  - [ ] Implement `LocalizedFormatter<L: Localizer + Default + 'static>`
    that swaps clap's formatter at the type level via `Error::apply`.
  - [ ] Document the formatter as an advanced opt-in; recommend the eager
    path for almost every adopter. Explicitly state that the crate does
    **not** ship a thread-local-backed dynamic formatter.

### 11.3. Define the locale-resolution lifecycle

- [ ] 11.3.1. Add the `LocaleResolver` trait and shipped implementations.
  - Requires 11.1.1.
  - See cli-localization-design.md §5.1.
  - [ ] Define `LocaleResolver` with `boot_locale()` and
    `merged_locale(explicit)`.
  - [ ] Ship `EnvLocaleResolver` (LC_ALL → LC_MESSAGES → LANG, with POSIX
    normalization and `C`/`POSIX` special-cases), `FixedLocaleResolver`,
    and `ConfigLocaleResolver`.
  - [ ] Document `EnvLocaleResolver` as opt-in: daemons and embedded
    interfaces are entitled to write their own resolver.

- [ ] 11.3.2. Add the `BootLocalizer` factory and the `BootHandle`
  typestate.
  - Requires 11.3.1.
  - See cli-localization-design.md §5.2 and §5.3.
  - [ ] Implement `BootLocalizer::build` returning `BootHandle<Boot>`.
  - [ ] Implement `BootHandle::finalize` and `BootHandle::finalize_with`
    so the merge-phase locale, and optionally a fresh resolver, can be
    applied without rebuilding the factory.
  - [ ] Implement `Drop` for `BootHandle<Boot>` that emits a `warn`-level
    tracing event when finalization was missed.
  - [ ] Implement `BootHandle::build_failed()` on both `BootHandle<Boot>`
    and `BootHandle<Final>` (see cli-localization-design.md §5.2) so
    degraded-mode banners can be surfaced before parsing and again after
    merge. Re-emit the build-failure event from `finalize` with
    exponential backoff.

- [ ] 11.3.3. Document the snapshot-per-parse contract.
  - Requires 11.3.2.
  - See cli-localization-design.md §1.2 and §12.
  - [ ] Add a users'-guide section naming the snapshot semantics
    explicitly, recommending `arc_swap::ArcSwap<dyn Localizer>` as the
    swap primitive for long-lived services, and showing the daemon
    rebuild pattern.
  - [ ] Add an integration test that exercises a locale swap and asserts
    requests started before the swap continue rendering in the original
    locale.

### 11.4. Bridge with `i18n-embed`

- [ ] 11.4.1. Add the `FluentEmbedLocalizer` adapter behind a cargo
  feature.
  - Requires 11.3.2.
  - See cli-localization-design.md §7.
  - [ ] Add the `i18n-embed-bridge` cargo feature and gate the optional
    `i18n-embed` dependency behind it.
  - [ ] Implement `FluentEmbedLocalizer::new(Arc<FluentLanguageLoader>)`.
  - [ ] Use `FluentLanguageLoader::has` (the public `i18n-embed` 0.16
    presence API; it wraps the underlying bundle's `has_message`) for
    presence detection, not the `loader.get(id) == id` heuristic.
    Document the three Fluent edge cases (attributes-only messages,
    self-transform values, empty string values) the heuristic would have
    got wrong. Add a build-time symbol check so a rename in a future
    `i18n-embed` release fails compilation rather than degrading
    silently.
  - [ ] Wire `MissingTranslationReporter` so the adapter participates in
    the §11.2.3 reporting pipeline.

- [ ] 11.4.2. Coordinate parity between `FluentLocalizer` and
  `FluentEmbedLocalizer`.
  - Requires 11.4.1.
  - See cli-localization-design.md §7.
  - [ ] Add a parity test suite that asserts the two implementations
    return identical results for a shared fixture catalogue.
  - [ ] Document the no-loader-constructor decision: the crate does not
    build a `FluentLanguageLoader` from `I18nAssets` on the consumer's
    behalf because that would obscure bundle ownership.

### 11.5. Derive support for per-field embedded defaults

- [ ] 11.5.1. Add per-field `localized_default` attribute support.
  - Requires 11.1.3.
  - See cli-localization-design.md §8.2.
  - [ ] Accept values `none`, `help`, `long_help`, `value_name`,
    `help+long_help`, and `all` on field-level
    `#[ortho_config(localized_default = "...")]`.
  - [ ] Accept a struct-level default that fields inherit unless they
    override.
  - [ ] When the Fluent catalogue is empty for a given identifier and the
    field opted in, return the embedded default rather than the bare clap
    string.

- [ ] 11.5.2. Surface the build-time identifier artefact through
  `cargo-orthohelp`.
  - Requires 11.1.3 and step 6.2.
  - See cli-localization-design.md §11.
  - [ ] Add `cargo orthohelp i18n list-ids` with human, JSON, and Fluent
    stub output formats. The Fluent stub seeds a translator-ready
    catalogue.
  - [ ] Add `cargo orthohelp i18n coverage --locale <tag>` that walks the
    consumer's `Localizer` and reports identifiers the locale fails to
    resolve. Exit non-zero when coverage is below a configurable
    threshold.
  - [ ] Honour the agent-context output contracts from
    agent-native-cli-design.md §6.2.

### 11.6. Translator diagnostics

- [ ] 11.6.1. Ship the `MissingTranslationReporter` trait and aggregation
  pipeline.
  - Requires 11.2.3, 11.4.1, and 11.5.2.
  - See cli-localization-design.md §9.
  - [ ] Define `MissingTranslationReporter`, `MissingTranslationEvent`,
    and `TranslationOrigin`.
  - [ ] Provide a `cargo-orthohelp` reporter implementation that
    aggregates events into
    `target/orthohelp/missing-translations/<locale>.json`.
  - [ ] Document the reporter API in the developers' guide alongside the
    existing `FormattingIssueReporter`.

### 11.7. Migrate the example and downstream guidance

- [ ] 11.7.1. Collapse the `hello_world` example onto the promoted
  surface.
  - Requires 11.1.1 through 11.5.1.
  - See cli-localization-design.md §10.
  - [ ] Replace the example's `LocalizeCmd` impl and
    `try_parse_localized*` helpers with re-exports of the crate types.
  - [ ] Replace `DemoLocalizer` with a thin wrapper that composes
    `EnvLocaleResolver`, `BootLocalizer`, and `FluentLocalizer`.
  - [ ] Add documentation pointing users at §1.3 of the design as the
    adopter quick-start.

- [ ] 11.7.2. Update Weaver and Netsuke migration guidance.
  - Requires 11.7.1.
  - See cli-localization-design.md §3, §6.4, and §10.
  - [ ] Document the migration from local `LocalizeCmd`-style helpers and
    `LayeredLocalizer` wrappers to the promoted crate surface.
  - [ ] Note that `localize_clap_error_with_command` is deprecated in 0.9
    and removed in 0.10; consumers move to `LocalizedParse` for parse-time
    localization.
  - [ ] Spell out the `BootHandle` two-phase flow with a worked example, so
    consumers cannot accidentally skip finalization.

- [ ] 11.7.3. Add a migration note for `spycatcher-harness`.
  - Requires 11.4.1.
  - See cli-localization-design.md §3 and §7.
  - [ ] Document how to migrate from a hand-rolled
    `localize_harness_error` plus `FluentLanguageLoader` to
    `FluentEmbedLocalizer`.
  - [ ] Confirm that the bridge eliminates the duplicate FTL parse pass
    and the duplicate locale-negotiation block.

## 12. Document any ordinary clap CLI

Idea: if the documentation derive covers the whole ordinary clap vocabulary —
unit variants, named-field variants, and positional arguments — and can be
applied to a type without also making that type loadable from configuration
layers, then any existing clap CLI can be documented as written. If it cannot,
generated documentation stays a privilege of command surfaces that happen to be
config-shaped, and every agent-native contract built on top of the IR inherits
the same blind spots.

These are prerequisites rather than enhancements. Whole-CLI introspection
(phase 6) and agent-native policy (phase 7) both assume the IR can describe the
command tree, yet three of the most common shapes in the clap vocabulary are
currently unrepresentable. Every task in this phase is additive to the IR
envelope and leaves existing `--format ir`, `--format man`, and `--format ps`
output unchanged for command surfaces already covered, apart from `ir_version`,
which the minor bump in 12.2.1 changes.

### 12.1. Cover the clap variant vocabulary the docs derive rejects

This step answers whether `OrthoConfigSubcommandDocs` can describe unit and
named-field variants without changing the trait contract or the IR envelope.
The outcome decides whether service-style command sets and three-level command
trees can be documented at all, and it establishes how far the field-metadata
pipeline generalizes beyond struct fields. See cargo-orthohelp-design.md §3.1
and adr-005-subcommand-docs-companion-trait.md §Subsequent amendments.

- [ ] 12.1.1. Support unit subcommand variants in `OrthoConfigSubcommandDocs`.
  - See cargo-orthohelp-design.md §3.1;
    adr-005-subcommand-docs-companion-trait.md.
  - [ ] Emit a minimal `DocMetadata` node per unit variant: clap command label,
    generated `about_id`, default heading identifiers, and empty `fields` and
    `subcommands`.
  - [ ] Replace the `subcommand_docs_unit_variant` compile-fail fixture with
    passing coverage, retaining compile-fail cases for genuinely unsupported
    shapes.
  - [ ] Success: `enum Cmd { Start, Stop, Status }` derives, emits three child
    nodes in declaration order, and the man and PowerShell renderers list all
    three commands.

- [ ] 12.1.2. Support named-field variants that nest a subcommand selector.
  - Requires 12.1.1.
  - See cargo-orthohelp-design.md §3.1.
  - [ ] Recurse through the `#[command(subcommand)]` field's enum to populate
    the variant node's `subcommands`.
  - [ ] Reject more than one `#[command(subcommand)]` field per variant at
    macro time, matching clap's own constraint.
  - [ ] Success: `Cmd::Remote { #[command(subcommand)] action: RemoteAction }`
    produces a three-level tree whose grandchild labels and ordering match
    clap's own command resolution.

- [ ] 12.1.3. Generate field metadata from named-variant argument fields.
  - Requires 12.1.2.
  - See cargo-orthohelp-design.md §3.1 and §3.2.
  - [ ] Run variant fields through the same field-metadata pipeline as struct
    fields, covering value typing, defaults, `env`, `file`, and extras.
  - [ ] Support variants that mix ordinary argument fields with a nested
    selector, populating `fields` and `subcommands` on the same node.
  - [ ] Success: duplicate-identifier and illegal-name diagnostics fire on
    variant fields exactly as they do on struct fields, with spans on the
    offending variant field.

- [ ] 12.1.4. Add a combinatorial variant-matrix fixture across every output
  format.
  - Requires 12.1.3 and 12.2.3.
  - See cargo-orthohelp-design.md §3.1 and §10.
  - [ ] Cover unit, tuple, and named-field variants at three nesting levels,
    including a named-field variant that mixes arguments with a nested
    selector and a command that carries positionals.
  - [ ] Assert the matrix through `--format ir`, `--format man`, `--format ps`,
    and `--format agent-context`.
  - [ ] Success: the fixture fails when any single variant shape is dropped
    from any one of the four outputs, so a shape cannot regress in one renderer
    while passing in another.

### 12.2. Make positional arguments representable end to end

This step answers whether the IR can describe the argument shape that
`git clone <url>` and `cp <src> <dst>` use. The derive currently emits
`long: Some(..)` for every documented field, so positionals are unrepresentable
— probably the single largest gap in ordinary CLI coverage. The outcome gates
any claim that agent context describes a command's invocation surface. See
cargo-orthohelp-design.md §2.2.

- [ ] 12.2.1. Add positional metadata to the documentation IR and derive.
  - See cargo-orthohelp-design.md §2.2 and §12.
  - [ ] Add `CliMetadata.positional: Option<PositionalMetadata>` carrying
    `index`, `variadic`, `var_arg`, and `last`.
  - [ ] Stop emitting `long` unconditionally; derive positional metadata from
    `#[arg(index = …)]`, from the absence of `long` and `short`, from
    `num_args` for `variadic`, from `Arg::trailing_var_arg` for `var_arg`, and
    from `Arg::last` for `last`.
  - [ ] Bump the IR minor version and record the additive change in the design
    document's versioning section.
  - [ ] Skip-serialize `CliMetadata.positional` when absent, using
    `#[serde(skip_serializing_if = "Option::is_none")]` in both
    hand-maintained copies of the type, `ortho_config/src/docs/ir.rs` and
    `cargo-orthohelp/src/schema/mod.rs`, so flags-only IR omits the
    `positional` key rather than emitting `null`.
  - [ ] Success: `git clone <url>` and `cp <src> <dst>` shapes round-trip
    through `--format ir` with correct indices, while a flags-only command
    surface produces IR byte-identical to the previous version apart from the
    version string, with no `positional` key emitted.

- [ ] 12.2.2. Render positionals in man and PowerShell output.
  - Requires 12.2.1.
  - See cargo-orthohelp-design.md §7.1 and §7.2.
  - [ ] Order positionals by index in SYNOPSIS ahead of the options summary and
    render `value_name` rather than a flag spelling.
  - [ ] Emit MAML `position` attributes and preserve `variadic`, `var_arg`, and
    `last` as independent attributes. Reserve “accepting remaining input” for
    `var_arg`; `last` requires `--` and does not consume trailing input.
  - [ ] Success: golden snapshots show `cp <src> <dst>` synopsis ordering, and
    `Get-Help` reports the arguments as positional parameters rather than named
    ones.

- [ ] 12.2.3. Carry positionals into agent context.
  - Requires 12.2.1.
  - See agent-native-cli-design.md §3.2 and §8.2;
    cargo-orthohelp-design.md §6.3.1.
  - [ ] Extend `AgentInput` with positional metadata and stop discarding
    flagless inputs that carry it.
  - [ ] Keep discarding flagless, non-positional inputs; those are part of the
    configuration surface rather than the invocation surface, and the
    existing warning stays for that case.
  - [ ] Confirm the change is additive under the agent-context compatibility
    policy and update the frozen wire snapshot deliberately.
  - [ ] Success: an agent reading context for a `cp`-shaped command can
    reconstruct argument order without consulting the man page.

### 12.3. Separate documenting a type from loading it

This step answers whether a clap-only argument struct can be documented without
being made loadable from configuration layers. Docs currently arrive bundled
with `OrthoConfig`, which asserts `DeserializeOwned`, so documenting a type
also forces a `Default` implementation that lies about required fields. Most
subcommand argument structs in a real CLI are clap-only, so the outcome decides
how much of a command surface the pipeline can reach. See
cargo-orthohelp-design.md §3.1.

- [ ] 12.3.1. Add a standalone `OrthoConfigDocs` derive.
  - See cargo-orthohelp-design.md §3.1.
  - [ ] Export `#[derive(OrthoConfigDocs)]` from `ortho_config_macros`, sharing
    the existing field-metadata pipeline rather than forking it.
  - [ ] Add a docs-only mode to that pipeline which reads clap's declared
    argument attributes — `long`, `short`, `index`, `value_name`, and the
    positional settings — rather than synthesizing a kebab-cased long flag from
    the field identifier in `resolve_cli_field`, and which never invents the
    short flag that `resolve_short_flag` derives from the field name.
  - [ ] Emit environment and file metadata only where explicitly declared,
    replacing the unconditional `env: Some(..), file: Some(..)` that
    `render_io_block` produces today.
  - [ ] Emit no runtime loaders, no `DeserializeOwned` bound assertion, and no
    merge machinery.
  - [ ] Success: a `#[derive(clap::Args)]` struct with required fields, no
    `Default`, and no `Deserialize` derives documentation metadata and appears
    in generated IR, and a field declared `#[arg(long = "endpoint")]` documents
    that flag with no short flag and no environment or file source.
  - [ ] Success: a standalone `#[derive(clap::Parser, OrthoConfigDocs)]` root
    with a `#[command(subcommand)]` selector and nested variants preserves
    nested command labels and declaration order in recursively populated
    `DocMetadata.subcommands`.

- [ ] 12.3.2. Route `OrthoConfig` docs generation through the standalone path.
  - Requires 12.3.1.
  - See cargo-orthohelp-design.md §3.1.
  - [ ] Guarantee a single implementation strategy so the two derives cannot
    drift.
  - [ ] Detect both derives on one type at macro time and emit an error naming
    the duplicate, rather than letting the compiler report conflicting trait
    implementations.
  - [ ] Success: existing `OrthoConfig` IR output is unchanged, and the
    double-derive case has a compile-fail fixture with a readable message.

- [ ] 12.3.3. Document the docs-only adoption path.
  - Requires 12.3.2.
  - See cargo-orthohelp-design.md §3.1; users-guide.md.
  - [ ] Add a users' guide section showing a clap-only subcommand argument
    struct documented without `OrthoConfig`.
  - [ ] State explicitly that `Default` is no longer required on structs with
    required fields merely to obtain documentation.
  - [ ] Add migration guidance for consumers currently deriving `OrthoConfig`
    solely to obtain documentation metadata.

## 13. Keep the published documentation contracts evolvable

Idea: if the IR and agent-context types are non-exhaustive, reachable through
constructors that stamp the schema version, tolerant of unknown enum values,
and backed by a shipped heading catalogue, then upstream can add metadata in a
minor release and a new consumer renders correct output before authoring a
single translation. If they are not, every metadata addition planned in phases
7 and 9 costs a breaking release, and the compatibility policy already written
in agent-native-cli-design.md §8.2 cannot be exercised.

This phase compounds with phase 12. Closing the coverage gaps removes much of
the need for hand-assembled IR; these changes make hand-assembled IR survivable
for the consumers who still build it directly.

### 13.1. Make the IR types safe to extend

This step answers whether an optional metadata field can be added without a
breaking release. `docs/` currently has no `#[non_exhaustive]` while six
sibling modules in the same crate use it, and `ir.rs` has no `impl` blocks at
all, so every hand-assembling consumer breaks on every field addition. The
outcome determines the release cost of all later metadata work. See
cargo-orthohelp-design.md §3.6 and §12.1.

- [ ] 13.1.1. Add constructors for every public IR and agent-context type.
  - Requires 12.2.1.
  - See cargo-orthohelp-design.md §3.6.
  - [ ] Provide `DocMetadata::new`, `FieldMetadata::new`, `CliMetadata::flag`,
    `CliMetadata::positional`, `HeadingIds::defaults`, and equivalents for the
    remaining published types, with optional metadata applied through `with_*`
    methods or field assignment; field assignment applies only to optional
    metadata, never to `ir_version`.
  - [ ] Make `DocMetadata.ir_version` a private field, initialized only by
    `DocMetadata::new` from `ORTHO_DOCS_IR_VERSION` and exposed through a
    read-only accessor, so no consumer writes the version by hand and none can
    claim conformance to a schema version it has not implemented.
  - [ ] Keep `ORTHO_DOCS_IR_VERSION` public for comparison while documenting
    the constructor as the only supported way to populate the field.
  - [ ] Success: the derive and the `cargo-orthohelp` fixtures build IR through
    constructors, and a test asserts that constructor-built metadata reports the
    current schema version.

- [ ] 13.1.2. Apply `#[non_exhaustive]` to the published IR and agent-context
  types.
  - Requires 13.1.1.
  - See cargo-orthohelp-design.md §3.6 and §12.1.
  - [ ] Match the position already taken by `subcommand::selected`,
    `declarative::layer`, and `error::types`.
  - [ ] Ship in the same release as the constructors; the annotation alone
    would leave consumers unable to construct the types at all.
  - [ ] Success: an out-of-crate struct-literal construction fails to compile
    while the constructor path compiles, and adding an optional field to
    `FieldMetadata` requires no consumer change.

- [ ] 13.1.3. Publish the migration note for hand-assembled IR.
  - Requires 13.1.2.
  - See cargo-orthohelp-design.md §12.1.
  - [ ] Record the breaking change, the constructor equivalents, and why the
    two changes ship together.
  - [ ] Add a `CHANGELOG.md` entry and migration-guide coverage for Weaver and
    Netsuke, which assemble metadata directly.

### 13.2. Honour the unknown-variant compatibility promise

This step answers whether a reader built against the current schema survives a
later payload. The agent-context policy already permits additive enum variants
on condition that an unknown-value fallback exists and is tested;
`MutationEffect`, `InteractionMode`, and their siblings declare `Unknown`
variants for exactly that purpose, but nothing routes unrecognized wire strings
to them, so the allowance is unusable. See agent-native-cli-design.md §8.2 and
cargo-orthohelp-design.md §12.2.

- [ ] 13.2.1. Wire unknown-value fallbacks to the forward-compatibility
  variants.
  - See cargo-orthohelp-design.md §12.2; agent-native-cli-design.md §8.2.
  - [ ] Annotate `InteractionMode::Unknown` and `MutationEffect::Unknown` with
    `#[serde(other)]`.
  - [ ] Audit the remaining agent-context and policy enums and record, per
    enum, whether it models forward-compatible metadata or closed operator
    input. `PolicyMode` is closed input: a misspelled mode must fail loudly, so
    it keeps strict deserialization and gains no `Unknown` variant.
  - [ ] Document that an unrecognized value round-trips as `"unknown"`, and
    that the lossiness is the intended trade against a hard error.
  - [ ] Success: a payload carrying an enum string introduced after the current
    schema version deserializes to the documented legacy default instead of
    failing.

- [ ] 13.2.2. Add forward-compatibility fixtures for the schema contract.
  - Requires 13.2.1.
  - See agent-native-cli-design.md §8.1 and §8.2.
  - [ ] Add a frozen fixture representing a future payload, carrying unknown
    enum strings and unknown object fields on every type that promises
    tolerance.
  - [ ] Assert that tolerant types — those that promise forward compatibility,
    such as `InteractionMode` and `MutationEffect`, which ship an `Unknown`
    variant — accept the unknown fields and unknown enum values and resolve
    them per the §8.1 defaulting table.
  - [ ] Assert that closed types, such as `PolicyMode` (`Off | Warn | Deny`,
    with no `Unknown` variant, by deliberate design for strict operator
    input), reject unsupported input.
  - [ ] Success: the compatibility policy is enforced by a test rather than by
    prose, so CI fails on a regression in either direction — a tolerant type
    losing its fallback, or a closed type silently accepting unknown input.

### 13.3. Remove the heading-catalogue onboarding tax

This step answers what a consumer sees on its first generator run with no
Fluent catalogue of its own. The library ships no `ortho.headings.*` entries,
and `cargo-orthohelp` carries a hardcoded English table instead, so headings
are the one part of a generated man page a consumer cannot translate without
reimplementing all eleven identifiers. See cargo-orthohelp-design.md §4.2.1.

- [ ] 13.3.1. Ship the default heading catalogue in the library locales.
  - See cargo-orthohelp-design.md §4.2.1.
  - [ ] Add the full `ortho.headings.*` set — name, synopsis, description,
    options, environment, files, precedence, exit status, examples, see also,
    and commands — to the canonical `ortho_config/locales/en-US/messages.ftl`
    resource and the other currently shipped resource,
    `ortho_config/locales/ja/messages.ftl`.
  - [ ] Keep `en-US/messages.ftl` as the library's canonical English resource;
    there is no library `en-GB` catalogue to alias. Language-only matching
    makes English tags including `en-GB` reuse the embedded `en-US` resources.
  - [ ] Success: every `HeadingIds::defaults()` identifier resolves against the
    library catalogue in each shipped locale, gated by a test that fails when
    an identifier is added without a translation.

- [ ] 13.3.2. Resolve headings through the library catalogue instead of the
  generator's hardcoded table.
  - Requires 13.3.1 and 13.1.1.
  - See cargo-orthohelp-design.md §4.1 and §4.2.1.
  - [ ] Remove `standard_heading_fallback` from `cargo-orthohelp` and let the
    documented layering — consumer bundle, then library defaults, then English
    — supply the value.
  - [ ] Success: a fixture crate with no Fluent catalogue renders a complete man
    page in English and a complete man page in a second shipped locale, and
    neither output contains a raw identifier.

## 14. Separate configuration precedence from policy authority

Idea: if OrthoConfig treats source authority as an application-defined
dimension separate from preference order, lower-authority configuration can
strengthen operator policy without gaining an accidental capability-granting
path. If it cannot, every downstream application must rebuild the same trust
engine and later project layers remain able to widen security boundaries.

This phase depends on the scope and origin foundation from RFC 0002, but may
proceed independently of phases 11-13. It delivers an opt-in runtime contract
before derive syntax, preserves existing field strategies for unprotected
fields, and uses Netsuke's fetch, shell, interpreter, and resource boundaries
as downstream proof cases. See RFC 0003.

### 14.1. Define the authority model before stabilizing policy APIs

This step answers whether applications can describe authority, source identity,
anchors, and bounded delegation without path scope or preference order silently
conferring trust. Its outcome fixes the contracts that the runtime algebra and
diagnostics consume. See RFC 0003 §§5-6.3 and §6.8.

- [ ] 14.1.1. Review and accept RFC 0003's authority and enforcement
  decisions.
  - Confirm the application-defined partial order, explicit anchors,
    incomparable-authority behaviour, typed delegation, and fail-closed default.
  - Resolve the deferred public naming and disclosure-default questions needed
    by the runtime surface; leave derive spelling to 14.2.7.
  - Success: RFC 0003 is accepted with no unresolved question that blocks the
    runtime implementation.

- [ ] 14.1.2. Split the accepted implementation into review-sized child
  issues.
  - Requires 14.1.1.
  - Create one issue for each independently reviewable task in 14.1.3-14.4.4,
    preserving dotted task references and cross-issue dependencies.
  - Link every child issue back to RFC 0003 and issue #475.
  - Success: no implementation issue combines runtime algebra, derive code,
    diagnostics, and downstream adoption into one review.

- [ ] 14.1.3. Implement validated application-defined authority classes and
  partial orders.
  - Requires 14.1.1.
  - Add stable class identifiers, graph construction, dominance comparison,
    and validation for duplicate identifiers, unknown classes, and cycles.
  - Keep the graph inspectable data rather than an opaque comparison callback.
  - Success: property tests cover reflexivity, transitivity, incomparability,
    cycle rejection, and deterministic comparison. See RFC 0003 §§5.2 and 6.1.

- [ ] 14.1.4. Attach deliberate authority metadata to every protected source
  shape.
  - Requires 14.1.3 and the scope/origin foundation from RFC 0002.
  - Model defaults, explicit selectors, automatic scopes, inherited files,
    environment, CLI, and named custom providers without deriving authority
    from their path or preference position.
  - Preserve the resolved class, stable source identifier, selection kind, and
    disclosure-controlled source label in a sidecar to `MergeLayer`.
  - Success: missing and ambiguous classifications fail only protected fields,
    while unprotected fields retain existing merges. See RFC 0003 §6.2.

- [ ] 14.1.5. Implement explicit policy anchors and typed bounded delegation
  grants.
  - Requires 14.1.3 and 14.1.4.
  - Reject protected fields without a nominated anchor and validate delegation
    issuer authority, delegate scope, exact field or family, and maximum bound.
  - Keep grants out of ordinary delegated-layer keys and retain issuer and use
    provenance.
  - Success: a project source cannot create, broaden, or retarget its own grant,
    and a valid grant cannot exceed its issuer's envelope. See RFC 0003 §§6.3
    and 6.8.

### 14.2. Deliver monotonic composition through the runtime API

This step answers whether the built-in algebra and authority algorithm can
compose real protected fields before macro syntax freezes the wrong
abstraction. Its outcome supplies a standalone runtime path that derive support
can reuse. See RFC 0003 §§6.4-6.9 and §6.11.

- [ ] 14.2.1. Define the inspectable policy-family contract and capability-set
  families.
  - Requires 14.1.3.
  - Add stable family descriptors, `Equal`/`Narrower`/`Wider`/`Mixed`
    relationships, disclosure-safe summaries, and restrictive reduction.
  - Implement allowed-capability intersection and denied-capability union with
    set semantics over domain values.
  - Success: law-based property tests prove deterministic, associative,
    commutative, idempotent, and restrictive reduction. See RFC 0003 §§6.4-6.5.

- [ ] 14.2.2. Implement resource-limit and boolean-protection families.
  - Requires 14.2.1.
  - Add maximum limits by minimum, minimum requirements by maximum, permissions
    by logical AND, and required protections by logical OR.
  - Keep permission and required-protection booleans semantically distinct in
    public metadata and diagnostics.
  - Success: lower-authority inputs can lower maxima, raise minima, remove
    permissions, and add protections, but cannot perform the inverse. See
    RFC 0003 §6.5.

- [ ] 14.2.3. Implement keyed capability maps with declared value reducers.
  - Requires 14.2.1.
  - Intersect keys, reduce shared values through another policy family, and
    reject maps without a declared reducer.
  - Success: a lower-authority source cannot add a key or relax a retained
    key's value, including in mixed add/remove requests. See RFC 0003 §6.6.

- [ ] 14.2.4. Implement authority-aware runtime field composition.
  - Requires 14.1.4, 14.1.5, and tasks 14.2.1-14.2.3.
  - Track active constraints, let equal or dominant classes supersede the
    constraints they are authorized to relax, and restrict incomparable
    authorities.
  - Accept delegated widening only within a matching grant and its typed bound.
  - Success: ordered-layer and property tests cover narrowing, same-class
    replacement, dominant widening, incomparable constraints, mixed requests,
    and bounded delegation. See RFC 0003 §6.7.

- [ ] 14.2.5. Add fail-closed rejection and explicit observable clamp modes.
  - Requires 14.2.4.
  - Make rejection the default semantic error and allow clamp mode only through
    the application-owned policy plan.
  - Return every clamped result with a mandatory non-empty policy report; do
    not provide an API that silently discards it.
  - Success: unauthorized widening never changes the effective envelope in
    either mode, and no ordinary source can select clamp mode. See RFC 0003
    §6.9.

- [ ] 14.2.6. Integrate protected-field runtime composition with existing
  ordered layers.
  - Requires 14.2.5.
  - Feed protected and ordinary fields from the same `MergeLayer` sequence,
    while routing protected fields only through their semantic policy family.
  - Support standalone domain-policy composition so adopters can migrate before
    deriving a complete configuration.
  - Success: existing `append`, `replace`, and `keyed` fixtures remain unchanged
    for unprotected fields, and contradictory policy/ordinary strategies cannot
    run on one field. See RFC 0003 §§6.11 and 9.1.

- [ ] 14.2.7. Add declarative policy metadata after the runtime model settles.
  - Requires 14.2.6 and step 14.3.
  - Select semantic field attributes for built-in families, custom family
    identifiers, keyed value reducers, and authority-plan requirements.
  - Generate calls into the runtime implementation rather than a second macro
    policy engine, and reject contradictory `merge_strategy` and `policy`
    attributes at compile time.
  - Success: derive and runtime paths produce equivalent decisions and
    provenance, while a derive with no policy fields generates unchanged merge
    behaviour. See RFC 0003 §6.12.

### 14.3. Make every policy decision observable without exposing values

This step answers whether operators and machine consumers can explain a
rejection, clamp, authoritative widening, or delegated widening from bounded
records rather than raw configuration. Its output makes enforcement auditable
before downstream canaries adopt it. See RFC 0003 §6.10.

- [ ] 14.3.1. Preserve active-constraint and delegation decision provenance.
  - Requires 14.2.4.
  - Record field and family identifiers, decision kind, requesting source and
    authority, decisive boundary, relationship, and delegation issuer and
    identifier when used.
  - Preserve enough active constraints to explain incomparable authorities and
    later supersession.
  - Success: each runtime decision names its authority reason without
    reconstructing it from the final value. See RFC 0003 §§6.7-6.10.

- [ ] 14.3.2. Add bounded secret-safe summaries and structured diagnostics.
  - Requires 14.2.5 and 14.3.1.
  - Default set members, paths, interpreter names, provider parameters, and
    custom values to opaque or cardinality-only summaries; force secret fields
    opaque.
  - Cap detailed violations per field and run, report omitted counts, and emit
    human and JSON forms from the same typed record.
  - Success: rejected and clamped requests identify the requesting and ceiling
    sources and their relationship without logging protected values. See
    RFC 0003 §6.10.

- [ ] 14.3.3. Expose safe policy reports, tracing, and low-cardinality metrics.
  - Requires 14.3.2.
  - Add human and JSON report consumers plus structured events for accepted
    delegation, rejection, and clamping.
  - Keep field-family, decision, and authority labels bounded; exclude raw
    paths, values, request identifiers, and error strings from metric labels.
  - Success: applications can audit policy uptake and violations without
    installing a recorder or subscriber in the library. See RFC 0003 §6.10.

- [ ] 14.3.4. Document runtime adoption, migration, and custom-family laws.
  - Requires 14.2.6 and 14.3.3.
  - Update the users' and developers' guides with authority classification,
    anchor selection, rejection versus clamp mode, delegation issuance,
    redaction, and custom-family property-test requirements.
  - Distinguish runtime policy composition from roadmap task 7.2.7's
    agent-context metadata and from downstream enforcement.
  - Success: an adopter can protect one runtime field without inferring trust
    from RFC 0002 scope or rewriting an application policy engine. See RFC 0003
    §§3.2-3.3, 6.13, and 9.2.

### 14.4. Validate downstream canaries and layer combinations

This step answers whether the generic model closes the Netsuke boundaries that
motivated it and remains monotonic across interacting sources and families. The
canaries decide whether derive support and downstream migration are safe to
recommend. See RFC 0003 §7.

- [ ] 14.4.1. Prove Netsuke project configuration cannot widen operator fetch
  policy.
  - Requires 14.2.6 and 14.3.2.
  - Exercise explicit and automatic project sources against host, scheme,
    redirect, timeout, network-permission, and transport-protection policies.
  - Success: project configuration can narrow allowed endpoints and add blocks,
    but cannot add metadata, internal, or alternative-scheme endpoints without
    a bounded delegation. See RFC 0003 §7.1 and netsuke#644.

- [ ] 14.4.2. Prove trusted shells, interpreters, executable roots, and
  providers remain authority-bounded.
  - Requires 14.2.6 and 14.3.2.
  - Compose the policy envelope with Netsuke's structured-command shell
    selection while keeping provider selection and execution downstream.
  - Success: a project can remove trusted choices but cannot add a workspace
    executable, arbitrary absolute path, interpreter, root, or provider key
    without explicit delegation. See RFC 0003 §§7.2-7.3 and netsuke#638.

- [ ] 14.4.3. Prove resource ceilings and required protections remain
  monotonic.
  - Requires 14.2.6 and 14.3.2.
  - Cover render bytes, timeouts, recursion, redirects, file-read budgets,
    process counts, permission booleans, and required-protection booleans.
  - Success: project sources can reduce ceilings and permissions or strengthen
    requirements, but cannot raise, enable, or weaken them without delegation.
    See RFC 0003 §§7.4-7.5.

- [ ] 14.4.4. Add an end-to-end combinatorial policy-layer matrix.
  - Requires 14.4.1, 14.4.2, and 14.4.3.
  - Cover defaults, RFC 0002 explicit selectors and automatic scopes,
    environment, CLI, custom providers, equal, dominant, subordinate and
    incomparable classes, rejection, clamping, and delegation.
  - Assert blocklists and required protections across every supported layer
    order, including mixed wider-and-narrower requests.
  - Success: the matrix fails on silent widening, silent clamping, missing
    provenance, secret disclosure, or changed unprotected-field semantics. See
    RFC 0003 §§5.2, 6.7-6.10, and 8.2.
