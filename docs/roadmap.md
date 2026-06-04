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

- [ ] 5.2.3. Record consumer dependency boundaries for Weaver and Netsuke.
  - Requires 5.2.1 and 5.2.2.
  - See agent-native-cli-design.md §2.1 and adr-003-define-schema-ownership-for-agent-native-contracts.md.
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
  - Requires 6.1.1.
  - See cargo-orthohelp-design.md §§6-7 and agent-native-cli-design.md §4.
  - [ ] Add a fixture CLI with at least one nested subcommand and one command
    with no subcommands.
  - [ ] Assert that generated IR includes the recursive tree, field metadata,
    command names, examples, and Windows wrapper metadata where applicable.
  - [ ] Ensure existing man-page and PowerShell output remains compatible when
    subcommands are present.

### 6.2. Add compact agent-context output

- [ ] 6.2.1. Add `--format agent-context` to `cargo-orthohelp`.
  - Requires 6.1.1.
  - See agent-native-cli-design.md §3.2 and §4; cargo-orthohelp-design.md §6.3.1.
  - [ ] Generate JSON from the same bridge output used by documentation IR.
  - [ ] Include command paths, verbs, flags, positional arguments, value types,
    required inputs, defaults, and enum values.
  - [ ] Exclude localized long prose unless a concise summary is needed for
    command selection.

- [ ] 6.2.2. Version and validate the agent-context schema.
  - Requires 6.2.1.
  - See agent-native-cli-design.md §3.2 and §8; adr-003-define-schema-ownership-for-agent-native-contracts.md.
  - [ ] Add schema-version tests that fail on accidental shape changes.
  - [ ] Add golden fixtures for a simple CLI, a nested CLI, and a CLI with enum
    values.
  - [ ] Document the schema and compatibility policy in
    `docs/agent-native-cli-design.md`.

- [ ] 6.2.3. Define downstream `context --json` command naming.
  - Requires 6.2.1.
  - See agent-native-cli-design.md §3.2 and §5.
  - [ ] Prefer `<tool> context --json` for application command surfaces while
    keeping `cargo orthohelp --format agent-context` as the generator format.
  - [ ] Include a payload `kind` such as `<tool>.agent_context`.
  - [ ] Avoid public `agent-context` aliases before first release unless a
    migration explicitly requires them.

### 6.3. Validate skill manifests against real commands

- [ ] 6.3.1. Add skill manifest metadata.
  - Requires 6.2.1.
  - See agent-native-cli-design.md §3.4.
  - [ ] Model skill manifest path, schema version, and command index metadata.
  - [ ] Link skill manifest locations from agent context.
  - [ ] Keep downstream skill prose application-owned.

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
  - See improved-error-message-design.md §§1-3 and agent-native-cli-design.md §6.3.
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
locale-flag chicken-and-egg, bridges OrthoConfig with `i18n-embed`, and
extends the derive so localization identifiers are generated rather than
hand-authored. The design lives in
[cli-localization-design.md](cli-localization-design.md). Sequencing is
quality-of-life-first: §11.1 and §11.2 carry no policy risk, while §11.3
and later progressively add opinion.

### 11.1. Promote example helpers to crate API

- [ ] 11.1.1. Promote `LocalizeCmd` to a public extension trait on
  `clap::Command`.
  - See cli-localization-design.md §4.
  - [ ] Move the example trait into `ortho_config::localizer` and extend it
    to cover per-argument `help`, `long_help`, and `value_name`, plus
    subcommand `about`/`long_about` recursively, optional `version`/
    `long_version`, and the help-template footer.
  - [ ] Expose `LocalizeCmd::with_base("…")` for applications that share a
    catalogue across multiple binaries.
  - [ ] Add the public `ortho_config::message_id_for(&command_path, suffix)`
    function with documented identifier shape, ASCII normalization rules,
    and panic-on-collision behaviour.
  - [ ] Success: the `hello_world` example deletes its local
    `LocalizeCmd` impl and re-exports the crate one for one release.

- [ ] 11.1.2. Promote `try_parse_localized*` to a generic blanket trait.
  - Requires 11.1.1.
  - See cli-localization-design.md §4.2.
  - [ ] Add `LocalizedParse: clap::Parser` with `try_parse_localized`,
    `try_parse_localized_from`, and `try_parse_localized_with_matches`.
  - [ ] Provide a blanket impl for every `clap::Parser`.
  - [ ] Preserve the `*_with_matches` variant for callers that need the
    raw `ArgMatches` for `load_and_merge_with_matches`.
  - [ ] Add identifier-coverage tests that compare derive-emitted
    identifiers with `message_id_for` output across a fixture command tree.

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
  - See cli-localization-design.md §6.1.
  - [ ] Add Fluent strings for `NoEquals`, `ValueValidation`, `TooManyValues`,
    `TooFewValues`, `WrongNumberOfValues`, `ArgumentConflict`,
    `InvalidUtf8`, `Io`, and `Format` (alongside the four existing
    identifiers).
  - [ ] Expose `pub const CLAP_ERROR_IDS: &[(clap::error::ErrorKind,
    &str)]` so consumers can iterate, validate, and write coverage tests.
  - [ ] Implement the mechanical coverage gate (build script plus
    `const_assert_eq!`) as specified in
    [cli-localization-design.md §6.1](cli-localization-design.md). The
    design document owns the mechanism; this task implements it.

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
  - [ ] Add a `ClapErrorCoverage` builder that walks `CLAP_ERROR_IDS` and
    reports identifiers the supplied `Localizer` fails to resolve.

- [ ] 11.2.4. Document and ship the monomorphised `LocalizedFormatter`
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
  - [ ] Use `FluentLanguageLoader::has_message` for presence detection,
    not the `loader.get(id) == id` heuristic; document the three Fluent
    edge cases (attributes-only messages, self-transform values, empty
    string values) the heuristic would have got wrong.
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
  - [ ] Spell out the `BootHandle` two-phase flow with a worked example so
    consumers cannot accidentally skip finalization.

- [ ] 11.7.3. Add a migration note for `spycatcher-harness`.
  - Requires 11.4.1.
  - See cli-localization-design.md §3 and §7.
  - [ ] Document how to migrate from a hand-rolled
    `localize_harness_error` plus `FluentLanguageLoader` to
    `FluentEmbedLocalizer`.
  - [ ] Confirm that the bridge eliminates the duplicate FTL parse pass
    and the duplicate locale-negotiation block.
