# OrthoConfig roadmap

This roadmap describes the future product work for OrthoConfig. It replaces the
previous retrospective checklist with forward-looking phases, workstreams, and
concrete tasks. The roadmap is intentionally date-free: sequencing is driven by
dependency order, review size, and product coherence.

The source documents for this roadmap are:

- [Design Document: The `OrthoConfig` Crate](design.md);
- [Agent-native CLI assistance design](agent-native-cli-design.md);
- [OrthoConfig IR documentation design for cargo-orthohelp](cargo-orthohelp-design.md);
- [Improved error message design](improved-error-message-design.md);
- [DDLint gap analysis](ddlint-gap-analysis.md);
- [ADR-001: Replace `serde_yaml` with `serde-saphyr`](adr-001-replace-serde-yaml-with-serde-saphyr.md);
- [ADR-002: Replace `cucumber-rs` with `rstest-bdd`](adr-002-replace-cucumber-with-rstest-bdd.md).

## 1. Reconcile the design baseline

This phase makes the documentation set trustworthy before new agent-native
features are implemented. It removes stale completion claims, preserves
historical context, and records the exact boundary between OrthoConfig's
responsibilities and downstream application responsibilities.

### 1.1. Repair current truth

- [ ] 1.1.1. Reconcile the missing-required-values design with the actual
  error surface.
  - [ ] Verify whether `OrthoError::MissingRequiredValues` exists, whether it
    was renamed, or whether the feature was never implemented.
  - [ ] Update `docs/improved-error-message-design.md`,
    `docs/users-guide.md`, and release notes so they describe the current
    behaviour accurately.
  - [ ] If the implementation is absent, keep the design as proposed work and
    move the build into phase 3.

- [ ] 1.1.2. Retire stale retrospective roadmap items.
  - [ ] Move completed historical milestones out of the active roadmap path or
    reference them as background only.
  - [ ] Confirm DDLint gap-analysis items are either implemented, deliberately
    deferred, or replaced by agent-native policy work.
  - [ ] Update historical design notes so maintainers can tell whether a note
    is active guidance or preserved rationale.

- [ ] 1.1.3. Add an agent-native documentation index.
  - [ ] Link `docs/design.md`,
    `docs/cargo-orthohelp-design.md`, and the user guide to
    `docs/agent-native-cli-design.md`.
  - [ ] State that the documentation IR and agent-context schema are sibling
    outputs with independent versioning.
  - [ ] Document that OrthoConfig models, generates, and lints contracts; it
    does not become every downstream application's command runner.

### 1.2. Establish schema ownership

- [ ] 1.2.1. Define ownership for documentation IR, agent context, and policy
  reports.
  - [ ] Keep localized documentation IR in the existing `OrthoConfigDocs`
    contract.
  - [ ] Specify a compact agent-context schema with its own schema version.
  - [ ] Specify a policy report schema for warnings and hard failures emitted
    by `cargo-orthohelp`.

- [ ] 1.2.2. Record migration rules for existing consumers.
  - [ ] Ensure existing `--format ir`, `--format man`, `--format ps`, and
    `--format all` behaviours remain compatible until a versioned migration is
    explicitly approved.
  - [ ] Document how new metadata fields default when older derives do not
    provide them.
  - [ ] Add compatibility notes for downstream crates that only consume
    human-facing documentation output.

## 2. Deliver whole-CLI introspection

This phase makes the command tree visible. Agent-context output and vocabulary
linting cannot be correct while generated metadata only describes top-level
fields.

### 2.1. Populate subcommand metadata

- [ ] 2.1.1. Generate recursive `DocMetadata.subcommands` values.
  - [ ] Reuse information already parsed by `SelectedSubcommandMerge` where it
    describes selected subcommand enum variants.
  - [ ] Introduce a small companion trait if enum-level documentation cannot be
    represented cleanly through the existing `OrthoConfigDocs` trait.
  - [ ] Preserve deterministic command ordering so generated documentation and
    agent context are stable.

- [ ] 2.1.2. Cover nested command trees with behavioural fixtures.
  - [ ] Add a fixture CLI with at least one nested subcommand and one command
    with no subcommands.
  - [ ] Assert that generated IR includes the recursive tree, field metadata,
    command names, examples, and Windows wrapper metadata where applicable.
  - [ ] Ensure existing man-page and PowerShell output remains compatible when
    subcommands are present.

### 2.2. Add compact agent-context output

- [ ] 2.2.1. Add `--format agent-context` to `cargo-orthohelp`.
  - [ ] Generate JSON from the same bridge output used by documentation IR.
  - [ ] Include command paths, verbs, flags, positional arguments, value types,
    required inputs, defaults, and enum values.
  - [ ] Exclude localized long prose unless a concise summary is needed for
    command selection.

- [ ] 2.2.2. Version and validate the agent-context schema.
  - [ ] Add schema-version tests that fail on accidental shape changes.
  - [ ] Add golden fixtures for a simple CLI, a nested CLI, and a CLI with enum
    values.
  - [ ] Document the schema and compatibility policy in
    `docs/agent-native-cli-design.md`.

## 3. Enforce agent-native policy

This phase turns design rules into checks. The target is mechanical assistance:
projects should learn about inconsistent verbs, flags, output contracts, and
unsafe mutation surfaces before release.

### 3.1. Implement vocabulary policy

- [ ] 3.1.1. Add an opt-in agent-native policy configuration.
  - [ ] Support `off`, `warn`, and `deny` modes.
  - [ ] Provide canonical defaults for verbs and flags: `get`, `list`,
    `create`, `update`, `delete`, `--json`, `--force`, `--dry-run`,
    `--limit`, `--cursor`, `--wait`, `--profile`, and `--deliver`.
  - [ ] Allow explicit project exceptions that are visible in policy output.

- [ ] 3.1.2. Lint off-policy verbs and flags.
  - [ ] Flag `info`, `ls`, `--format=json`, `--output json`, and
    `--skip-confirmations` under strict policy.
  - [ ] Report the canonical replacement in every diagnostic.
  - [ ] Add tests for warning mode, deny mode, and configured exceptions.

### 3.2. Model behavioural semantics

- [ ] 3.2.1. Add metadata for non-interactive execution and mutation
  boundaries.
  - [ ] Represent whether a command is non-interactive, may prompt, or needs a
    bypass flag.
  - [ ] Represent whether a command reads, writes, deletes, or submits work.
  - [ ] Lint destructive commands that lack `--force` or equivalent approved
    metadata.

- [ ] 3.2.2. Add metadata for structured output and exit classes.
  - [ ] Model `--json` support, stdout contracts, stderr diagnostics, and exit
    classifications.
  - [ ] Lint data-returning commands that cannot emit structured output.
  - [ ] Document stable exit classes for `cargo-orthohelp`.

- [ ] 3.2.3. Add metadata for bounded list output.
  - [ ] Model `--limit`, `--cursor`, default limits, maximum limits, and
    truncation hints.
  - [ ] Lint list-shaped commands that lack bounded defaults.
  - [ ] Keep generated agent descriptions within an explicit size budget.

### 3.3. Rebuild improved required-value diagnostics

- [ ] 3.3.1. Implement or restore enumerating missing-required-values errors
  after the phase 1 truth audit.
  - [ ] Aggregate all missing required values before returning an error.
  - [ ] Show valid supply paths through CLI flags, environment variables, and
    file keys.
  - [ ] Add unit, macro, and behavioural tests that prove agents can
    self-correct from one diagnostic.

## 4. Make `cargo-orthohelp` the reference CLI

This phase dogfoods the table-stakes agent-native behaviours before asking
downstream users to adopt them.

### 4.1. Add structured command results

- [ ] 4.1.1. Add `--json` to `cargo-orthohelp`.
  - [ ] Emit a structured success summary containing generated artefact kind,
    locale, and path.
  - [ ] Emit structured errors when JSON mode is requested.
  - [ ] Keep human diagnostics on stderr and machine-readable command results
    on stdout.

- [ ] 4.1.2. Enumerate valid choices in errors.
  - [ ] Invalid formats should list every supported format.
  - [ ] Unknown packages should list candidate packages from Cargo metadata.
  - [ ] Unknown binaries should list candidate binary targets.
  - [ ] Invalid locales should list configured locales and the fallback
    `en-US` behaviour.

### 4.2. Make generated artefacts robust

- [ ] 4.2.1. Write generated files atomically.
  - [ ] Write to a sibling temporary file, flush it, and rename into place.
  - [ ] Preserve existing output paths and cache semantics.
  - [ ] Add failure-path tests that prevent partial generated artefacts from
    replacing valid files.

- [ ] 4.2.2. Document the reference CLI contract.
  - [ ] Update `cargo-orthohelp/README.md` with stdout/stderr behaviour,
    `--json`, exit classes, and agent-native lint usage.
  - [ ] Include examples for human documentation output and agent-context
    output.
  - [ ] Explain which behaviours are already implemented and which require
    future phases.

## 5. Add compounding primitives

This phase adds optional helpers and metadata for repeated agent workflows. It
must preserve the boundary that OrthoConfig provides reusable contracts and
helpers, while downstream applications own domain behaviour.

### 5.1. Profile contracts

- [ ] 5.1.1. Design and implement optional profile metadata.
  - [ ] Standardize `--profile <name>` as the root selection flag.
  - [ ] Document the precedence
    `explicit CLI > environment > selected profile > config file > default`.
  - [ ] Expose profile support, profile listing commands, and selected-profile
    semantics in agent context.

- [ ] 5.1.2. Decide whether OrthoConfig ships a profile store helper.
  - [ ] Evaluate a JSON-backed helper against applications that already manage
    their own profile storage.
  - [ ] If implemented, provide list, show, save, and delete helpers without
    forcing downstream CLIs to use a specific command framework.

### 5.2. Delivery and feedback contracts

- [ ] 5.2.1. Design reusable delivery target parsing.
  - [ ] Support `stdout`, `file:<path>`, and `webhook:<url>` schemes.
  - [ ] Enumerate supported schemes when parsing fails.
  - [ ] Require atomic file writes and visible webhook HTTP status reporting.

- [ ] 5.2.2. Design reusable feedback storage.
  - [ ] Store local feedback as JSONL by default.
  - [ ] Optionally send feedback upstream when an endpoint is configured.
  - [ ] Expose local and upstream feedback capability in agent context.

### 5.3. Async job contracts

- [ ] 5.3.1. Model application-level async jobs.
  - [ ] Represent `--wait`, job identifier fields, status commands, and job
    ledger support in metadata.
  - [ ] Lint async submit commands that force agents to write their own polling
    loops.
  - [ ] Keep this separate from asynchronous configuration loading in
    `OrthoConfig::load`.

- [ ] 5.3.2. Evaluate a reusable job ledger helper.
  - [ ] Decide whether a local JSONL ledger belongs in OrthoConfig or should
    remain application-owned.
  - [ ] If implemented, provide list, get, and prune primitives that downstream
    CLIs can expose under a `jobs` command.

## 6. Deferred extensions

These items are useful but should wait until whole-CLI introspection,
agent-context output, policy linting, and the `cargo-orthohelp` reference CLI
are working.

### 6.1. Integration extensions

- [ ] 6.1.1. Generate Model Context Protocol (MCP) descriptions from
  agent-context output.
- [ ] 6.1.2. Explore OpenAPI-shaped runtime explorer endpoints for downstream
  applications.
- [ ] 6.1.3. Generate long-form skill manifests from documentation IR and
  agent context.

### 6.2. Configuration extensions

- [ ] 6.2.1. Explore asynchronous loading of configuration files and
  environment variables.
- [ ] 6.2.2. Provide an API for registering custom `figment` providers, such as
  secrets managers or remote key-value stores.
- [ ] 6.2.3. Investigate live reloading of configuration when files change.
