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
- [DDLint gap analysis](ddlint-gap-analysis.md);
- [ADR-001: Replace `serde_yaml` with `serde-saphyr`](adr-001-replace-serde-yaml-with-serde-saphyr.md);
- [ADR-002: Replace `cucumber-rs` with `rstest-bdd`](adr-002-replace-cucumber-with-rstest-bdd.md);
- [ADR-004: Cargo external-subcommand entry-point architecture](adr-004-cargo-external-subcommand-entry-point.md).

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

- [ ] 5.2.1. Define ownership for documentation IR, agent context, and policy
  reports.
  - [ ] Keep localized documentation IR in the existing `OrthoConfigDocs`
    contract.
  - [ ] Specify a compact agent-context schema with its own schema version.
  - [ ] Specify a policy report schema for warnings and hard failures emitted
    by `cargo-orthohelp`.

- [ ] 5.2.2. Record migration rules for existing consumers.
  - [ ] Ensure existing `--format ir`, `--format man`, `--format ps`, and
    `--format all` behaviours remain compatible until a versioned migration is
    explicitly approved.
  - [ ] Document how new metadata fields default when older derives do not
    provide them.
  - [ ] Add compatibility notes for downstream crates that only consume
    human-facing documentation output.

- [ ] 5.2.3. Record consumer dependency boundaries for Weaver and Netsuke.
  - [ ] Document that OrthoConfig owns reusable command-contract machinery,
    while Weaver owns semantic code-edit execution and Netsuke owns build and
    package execution.
  - [ ] Mark whole-CLI introspection, strict vocabulary policy, agent-context
    IR, and localized help generation as hard dependencies for Weaver's
    generated command surface.
  - [ ] Mark profiles, delivery, feedback, skill manifests, and execution
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

- [ ] 6.1.2. Cover nested command trees with behavioural fixtures.
  - [ ] Add a fixture CLI with at least one nested subcommand and one command
    with no subcommands.
  - [ ] Assert that generated IR includes the recursive tree, field metadata,
    command names, examples, and Windows wrapper metadata where applicable.
  - [ ] Ensure existing man-page and PowerShell output remains compatible when
    subcommands are present.

### 6.2. Add compact agent-context output

- [ ] 6.2.1. Add `--format agent-context` to `cargo-orthohelp`.
  - [ ] Generate JSON from the same bridge output used by documentation IR.
  - [ ] Include command paths, verbs, flags, positional arguments, value types,
    required inputs, defaults, and enum values.
  - [ ] Exclude localized long prose unless a concise summary is needed for
    command selection.

- [ ] 6.2.2. Version and validate the agent-context schema.
  - [ ] Add schema-version tests that fail on accidental shape changes.
  - [ ] Add golden fixtures for a simple CLI, a nested CLI, and a CLI with enum
    values.
  - [ ] Document the schema and compatibility policy in
    `docs/agent-native-cli-design.md`.

- [ ] 6.2.3. Define downstream `context --json` command naming.
  - [ ] Prefer `<tool> context --json` for application command surfaces while
    keeping `cargo orthohelp --format agent-context` as the generator format.
  - [ ] Include a payload `kind` such as `<tool>.agent_context`.
  - [ ] Avoid public `agent-context` aliases before first release unless a
    migration explicitly requires them.

### 6.3. Validate skill manifests against real commands

- [ ] 6.3.1. Add skill manifest metadata.
  - [ ] Model skill manifest path, schema version, and command index metadata.
  - [ ] Link skill manifest locations from agent context.
  - [ ] Keep downstream skill prose application-owned.

- [ ] 6.3.2. Add skill manifest validation.
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
  - [ ] Support `off`, `warn`, and `deny` modes.
  - [ ] Provide canonical defaults for verbs and flags: `get`, `list`,
    `create`, `update`, `delete`, `--json`, `--no-input`, `--force`,
    `--dry-run`, `--limit`, `--cursor`, `--wait`, `--profile`, and
    `--deliver`.
  - [ ] Allow explicit project exceptions that are visible in policy output.

- [ ] 7.1.2. Lint off-policy verbs and flags.
  - [ ] Flag `info`, `ls`, `--format=json`, `--output json`, and
    `--skip-confirmations` under strict policy.
  - [ ] Report the canonical replacement in every diagnostic.
  - [ ] Add tests for warning mode, deny mode, and configured exceptions.

- [ ] 7.1.3. Add the canonical human-facing global option glossary.
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
  - [ ] Represent whether a command is non-interactive, may prompt, or needs a
    bypass flag.
  - [ ] Represent whether a command reads, writes, deletes, or submits work.
  - [ ] Lint destructive commands that lack `--force` or equivalent approved
    metadata.

- [ ] 7.2.2. Add dual-renderer metadata.
  - [ ] Model human renderer support and machine renderer support separately.
  - [ ] Model TTY sensitivity, closed-stdin behaviour, colour, emoji,
    progress, pager, width, accessibility, and plain-output policy.
  - [ ] Model localized versus non-localized fields so protocol identifiers do
    not drift with human language.

- [ ] 7.2.3. Add metadata for structured output and exit classes.
  - [ ] Model `--json` support, stdout contracts, stderr diagnostics, and exit
    classifications.
  - [ ] Lint data-returning commands that cannot emit structured output.
  - [ ] Document stable exit classes for `cargo-orthohelp`.

- [ ] 7.2.4. Add a JSON mode stream contract.
  - [ ] Model success stdout as a single JSON result document.
  - [ ] Model failure stderr as a single JSON diagnostic document.
  - [ ] Model subprocess output policy so child process output never leaks to
    stdout in JSON mode except via an agreed artefact path.

- [ ] 7.2.5. Add exit-code taxonomy metadata.
  - [ ] Model code-to-class mappings in documentation IR and agent context.
  - [ ] Lint that every documented error class has an exit code.
  - [ ] Lint that JSON diagnostics report the same class and code.

- [ ] 7.2.6. Add metadata for bounded list output.
  - [ ] Model `--limit`, `--cursor`, default limits, maximum limits, and
    truncation hints.
  - [ ] Lint list-shaped commands that lack bounded defaults.
  - [ ] Keep generated agent descriptions within an explicit size budget.

- [ ] 7.2.7. Add generic capability and provenance metadata.
  - [ ] Model capability identifiers, command mapping, provider visibility,
    provider override policy, and whether provider provenance appears in JSON.
  - [ ] Lint that ordinary public commands do not require backend provider
    names when a stable capability command would suffice.
  - [ ] Keep provider registries, selection, execution, and safety harnesses
    application-owned.

### 7.3. Rebuild improved required-value diagnostics

- [ ] 7.3.1. Implement or restore enumerating missing-required-values errors
  after the phase 5 truth audit.
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
  - [ ] Emit a structured success summary containing generated artefact kind,
    locale, and path.
  - [ ] Emit structured errors when JSON mode is requested.
  - [ ] Keep human diagnostics on stderr and machine-readable command results
    on stdout.

- [ ] 8.1.2. Enumerate valid choices in errors.
  - [ ] Invalid formats should list every supported format.
  - [ ] Unknown packages should list candidate packages from Cargo metadata.
  - [ ] Unknown binaries should list candidate binary targets.
  - [ ] Invalid locales should list configured locales and the fallback
    `en-US` behaviour.

### 8.2. Make generated artefacts robust

- [ ] 8.2.1. Write generated files atomically.
  - [ ] Write to a sibling temporary file, flush it, and rename into place.
  - [ ] Preserve existing output paths and cache semantics.
  - [ ] Add failure-path tests that prevent partial generated artefacts from
    replacing valid files.

- [ ] 8.2.2. Document the reference CLI contract.
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
  - [ ] Standardize `--profile <name>` as the root selection flag.
  - [ ] Document the precedence
    `built-in defaults < config files < selected profile < environment <
    flags`.
  - [ ] Expose profile support, profile listing commands, and selected-profile
    semantics in agent context.

- [ ] 9.1.2. Add profile redaction metadata.
  - [ ] Mark secret and reference-only profile fields.
  - [ ] Redact sensitive profile values from context output and generated
    documentation examples.
  - [ ] Validate that profile names can be exposed without leaking profile
    contents.

- [ ] 9.1.3. Decide whether OrthoConfig ships a profile store helper.
  - [ ] Evaluate a JSON-backed helper against applications that already manage
    their own profile storage.
  - [ ] If implemented, provide list, show, save, and delete helpers without
    forcing downstream CLIs to use a specific command framework.

### 9.2. Delivery and feedback contracts

- [ ] 9.2.1. Design reusable delivery target parsing.
  - [ ] Support `stdout`, `file:<path>`, and `webhook:<url>` schemes.
  - [ ] Enumerate supported schemes when parsing fails.
  - [ ] Require atomic file writes and visible webhook HTTP status reporting.
  - [ ] Keep application-specific webhook payload semantics outside
    OrthoConfig.

- [ ] 9.2.2. Design reusable feedback storage.
  - [ ] Store local feedback as JSONL by default.
  - [ ] Optionally send feedback upstream when an endpoint is configured.
  - [ ] Expose local and upstream feedback capability in agent context.

### 9.3. Execution ledger contracts

- [ ] 9.3.1. Model application-level execution ledgers.
  - [ ] Represent `--wait`, job identifier fields, status commands, and job
    ledger support in metadata.
  - [ ] Lint async submit commands that force agents to write their own polling
    loops.
  - [ ] Keep this separate from asynchronous configuration loading in
    `OrthoConfig::load`.

- [ ] 9.3.2. Support configurable public ledger nouns.
  - [ ] Allow applications to expose `jobs`, `runs`, `tasks`, or `operations`
    while sharing one metadata model.
  - [ ] Include record identifiers, status enums, timestamps, command paths,
    input hashes, idempotency keys, log references, result references, prune
    commands, and bounded list behaviour.

- [ ] 9.3.3. Evaluate a reusable execution ledger helper.
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
- [ ] 10.1.2. Explore OpenAPI-shaped runtime explorer endpoints for downstream
  applications.
- [ ] 10.1.3. Generate optional long-form skill prose from documentation IR and
  agent context after validation exists.

### 10.2. Configuration extensions

- [ ] 10.2.1. Explore asynchronous loading of configuration files and
  environment variables.
- [ ] 10.2.2. Provide an API for registering custom `figment` providers, such as
  secrets managers or remote key-value stores.
- [ ] 10.2.3. Investigate live reloading of configuration when files change.
