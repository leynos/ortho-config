# Agent-native CLI assistance design

## 1. Purpose and scope

This document defines how OrthoConfig will help Rust projects build
agent-native command-line interfaces. It translates the agent-native CLI
principles accepted in the planning conversation into product requirements,
metadata contracts, lint policy, and roadmap boundaries.

The design is intentionally stricter than a documentation guideline. The
project goal is to make agent-native behaviour mechanically visible and
enforceable through derive metadata, generated context, and `cargo-orthohelp`
checks. Maintainers should not have to rely on code review to catch a command
that uses `info` where the rest of the CLI uses `get`, accepts
`--skip-confirmations` instead of `--force`, or emits a broad table where an
agent needs bounded JSON.

This document covers the product shape and the future implementation contract.
It does not claim that all features are already implemented. The implementation
order lives in [roadmap.md](roadmap.md).

## 2. Product rationale

OrthoConfig already asks developers to describe configuration once and derive
the CLI, environment, file, merge, and documentation behaviour from that
description. That makes it the right place to assist with agent-native CLI
contracts, because the hard problem is consistency across a whole command
surface.

The product direction is:

- describe command, option, output, and workflow contracts once;
- generate human documentation from documentation-oriented metadata;
- generate compact agent invocation context from agent-oriented metadata;
- lint command vocabulary and behavioural gaps before release;
- make `cargo-orthohelp` the reference CLI for the first agent-native tier.

The design preserves OrthoConfig's current identity. OrthoConfig is not being
turned into a mandatory application runtime. It should model, generate,
validate, and optionally provide reusable helpers for common surfaces such as
profiles, delivery targets, feedback stores, and job ledgers. Downstream
applications still own their domain side effects.

## 3. Contract surfaces

Agent-native support is split across four surfaces. Each surface has a
different audience and stability requirement.

### 3.1 Human documentation IR

The existing `OrthoConfigDocs` and `DocMetadata` model remains the source for
localized documentation, roff man pages, PowerShell help, and generated
reference material. It is allowed to contain prose, headings, examples, and
localization metadata.

The documentation IR must continue to be versioned and serializable. It should
gain only the command metadata required to keep human documentation accurate.
It must not become the compact agent context by accident.

### 3.2 Agent context

The new agent-context contract is a compact, machine-oriented JSON document
that describes how to invoke the CLI. It is not localized prose and should be
kept small enough for agents to load cheaply.

The planned `cargo-orthohelp` interface is:

```console
cargo orthohelp --format agent-context
```

Equivalent command forms are acceptable if implementation work finds a cleaner
fit, but the output must remain a first-class format, not a scraped help page.

The top-level shape should include:

```json
{
  "schema_version": "1",
  "package": "example-cli",
  "commands": [],
  "profiles": {
    "supported": false
  },
  "feedback": {
    "supported": false
  },
  "policy": {
    "agent_native": "warn"
  }
}
```

Each command entry should include the invocation path, canonical verb, input
flags, value types, required flags, defaults, enum values, output modes,
pagination controls, mutation boundaries, interaction mode, async metadata,
delivery support, and examples that are short enough for an agent to use
directly.

### 3.3 Agent-native lint policy

The lint policy is the enforcement layer. It should be exposed through
`cargo-orthohelp` and should also be reusable by tests or continuous
integration.

The planned command shape is:

```console
cargo orthohelp --check-agent-native
```

The policy should support `off`, `warn`, and `deny` modes. Early adoption
should default to warnings so existing users can see the work required before
turning on hard failures.

### 3.4 Long-form workflow material

Long-form skill manifests, tutorials, or MCP-facing descriptions are useful,
but they are downstream of the compact contracts. They should be generated or
validated against the documentation IR and agent context rather than maintained
as an independent source of truth.

## 4. Whole-CLI introspection

Whole-CLI introspection is the first implementation dependency. The current
documentation IR already has recursive `subcommands`, but generated
`OrthoConfigDocs` implementations currently emit an empty subcommand list. The
future design must close that gap before agent-context can be complete.

The target is a command tree where each command node can describe:

- its command path, such as `profile save` or `jobs get`;
- its canonical verb, such as `get`, `list`, `create`, `update`, or `delete`;
- accepted flags and positional arguments;
- whether it reads, writes, deletes, or submits work;
- whether it can prompt and which flag bypasses prompting;
- output contracts and stable exit classes;
- pagination, async, delivery, profile, and feedback support.

`SelectedSubcommandMerge` already parses subcommand enum information. Future
implementation should reuse that knowledge or introduce a small companion trait
so command trees are generated from Rust types rather than manually copied into
documentation.

## 5. Vocabulary and flag policy

Agent-native CLIs should use vocabulary that neighbouring CLIs already teach
agents to expect. OrthoConfig should therefore support a strict vocabulary
policy that is opt-in at first and capable of becoming a project default later.

The canonical command verbs are:

- `get` for one resource;
- `list` for collections;
- `create` for creation;
- `update` for mutation;
- `delete` for destructive removal;
- `jobs` for durable async job inspection;
- `profile` for persistent identity configuration;
- `feedback` for reporting friction to maintainers.

The canonical flags are:

- `--json` for structured output;
- `--force` for destructive prompt bypass;
- `--dry-run` for previewing consequential operations;
- `--limit` and `--cursor` for bounded list output;
- `--wait` for blocking until an async submission completes;
- `--profile` for selecting a named persistent identity;
- `--deliver` for routing generated artefacts.

The policy should flag or reject off-convention aliases such as `info`, `ls`,
`--format=json`, `--output json`, and `--skip-confirmations` when strict mode
is enabled. Projects may still opt out or configure exceptions, but exceptions
must be explicit and visible in generated context.

## 6. Required command semantics

Agent-native metadata must describe behaviour, not only syntax. A flag list is
not enough for an agent to decide whether an invocation is safe, bounded, or
recoverable.

### 6.1 Non-interactive execution

Commands should run without prompts when invoked by an agent. Metadata should
state whether a command is non-interactive, may prompt, or requires a bypass
flag. A command that may prompt without declaring `--force`, `--yes`,
`--no-input`, or an equivalent project-approved bypass should fail strict
agent-native lint.

The preferred destructive bypass flag is `--force`. If a project chooses a
different convention, it must configure that convention once and expose it in
agent context.

### 6.2 Structured output

Data-returning commands should support `--json`. Structured data belongs on
stdout, diagnostics belong on stderr, and exit codes should be stable enough to
document.

Agent-context output metadata should describe:

- whether `--json` is supported;
- whether stdout contains data, a path, a summary, or no output;
- whether diagnostics can appear on stderr;
- stable exit classes and their meanings;
- the response schema when it is known.

`cargo-orthohelp` should dogfood this by returning structured success summaries
for generated artefacts when `--json` is provided.

### 6.3 Errors that enumerate choices

When a command rejects a value because it is outside a known set, the error
should name the valid set. This applies to enum values, output formats,
delivery schemes, locale identifiers, package names, binary targets, profile
names, and policy modes.

The documentation set currently contains stale claims that
`OrthoError::MissingRequiredValues` is implemented. Future work must reconcile
the missing-required-values design with the actual error enum before treating
that feature as complete.

### 6.4 Mutation boundaries and retries

Mutating commands should declare whether they are read-only, write, delete, or
submit asynchronous work. Destructive commands should declare their
confirmation bypass flag. Consequential commands should declare whether
`--dry-run` exists.

Create-like commands should prefer idempotency tokens or natural keys where the
application domain supports them. OrthoConfig should model and lint the
contract; the downstream application owns the domain-specific idempotency store.

### 6.5 Bounded responses

List-shaped commands should declare bounded defaults. The canonical flags are
`--limit` and `--cursor`. Metadata should include the default limit, maximum
limit where applicable, cursor support, and whether truncation hints are
returned.

Generated agent descriptions should also be bounded. The agent-context schema
should include concise summaries rather than long prose copied from man pages.

### 6.6 Async-aware execution

OrthoConfig should distinguish asynchronous configuration loading from
application-level async jobs. The agent-native requirement concerns
submit-poll-collect workflows in downstream CLIs.

Async submit commands should declare:

- a `--wait` flag;
- the response field that contains the job identifier;
- status commands, usually under `jobs`;
- whether a durable job ledger exists;
- whether retries can recover an in-flight job.

OrthoConfig may later provide reusable helper types for job ledger metadata or
storage, but the initial design requirement is to model and lint the command
contract.

### 6.7 Persistent profiles

Profiles let agents reuse named bundles of configuration across invocations.
Profile support should be optional, but when present it should use a canonical
root flag:

```console
example-cli render --profile weekly-recap --json
```

The recommended precedence is:

```text
explicit CLI > environment > selected profile > config file > default
```

If implementation work decides that profiles are named config overlays, the
roadmap must document the exact merge order and migration impact before code is
changed.

Agent context should expose whether profiles are supported, how to list them,
and which flag selects one.

### 6.8 Delivery and feedback

Two-way I/O has two separate contracts:

- `--deliver` routes generated artefacts to `stdout`, `file:<path>`, or
  `webhook:<url>`;
- `feedback <text>` records local friction reports and optionally sends them
  upstream when an endpoint is configured.

Unknown delivery schemes should produce a structured refusal that enumerates
the supported schemes. File delivery should use atomic writes. Webhook delivery
should surface HTTP status and retryability.

Feedback should write a local JSONL record by default. If an upstream endpoint
is configured, the command should report whether the local record was also sent
successfully.

## 7. `cargo-orthohelp` as reference CLI

`cargo-orthohelp` should be the first CLI in the workspace to satisfy the
table-stakes agent-native behaviours:

- non-interactive operation by default;
- `--json` for structured command results;
- generated artefact summaries on stdout in JSON mode;
- diagnostics on stderr;
- enumerating errors for invalid formats, packages, binaries, locales, and
  policy modes;
- stable exit classes documented in its README;
- atomic writes for generated files;
- agent-native lint and agent-context output once the metadata exists.

This gives downstream users an executable reference rather than only a design
document.

## 8. Versioning and compatibility

The documentation IR and agent-context schema must version independently. A
change that affects man-page generation may not affect agents, and a compact
agent-context addition should not force a documentation IR migration unless the
same data is genuinely needed by human documentation.

The first implementation phase should introduce agent-native support behind
explicit formats, commands, or metadata attributes. Existing generated
documentation output should remain compatible unless the roadmap names a
migration step.

Strict policy should begin as opt-in. Projects should be able to run the check
in warning mode before enforcing it in CI.

## 9. Current gaps to resolve

The design and roadmap updates must address these known gaps:

- generated `OrthoConfigDocs` subcommand metadata is currently empty;
- no compact agent-context format exists;
- no agent-native lint command exists;
- stale documentation claims that `MissingRequiredValues` is complete;
- `cargo-orthohelp` has no structured `--json` result mode;
- `cargo-orthohelp` does not yet enumerate all valid choices in errors;
- generated file writes are not specified as atomic;
- profile, delivery, feedback, async job, mutation, and pagination metadata are
  not yet modelled as first-class contracts.

## 10. Deferred extensions

The following ideas are useful but must not block the core agent-native work:

- MCP server generation from agent context;
- OpenAPI-shaped runtime explorer endpoints for downstream applications;
- full skill manifest generation;
- remote configuration providers;
- live reload of configuration;
- managed application job ledgers;
- asynchronous configuration file loading.

These extensions should be revisited only after whole-CLI introspection,
agent-context output, strict lint policy, and the `cargo-orthohelp` reference
CLI are working.
