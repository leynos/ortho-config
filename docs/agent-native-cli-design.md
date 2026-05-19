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
agent needs JSON to be bounded.

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

This document is the canonical agent-native contract and boundary reference
for OrthoConfig. Other project documents should link here when they need to
describe agent-native scope instead of duplicating the full contract.

## 2.1 Consumer application boundary

Weaver and Netsuke are the first planned consumers that make the boundary
concrete. OrthoConfig owns the reusable command-contract machinery:

- schemas and command metadata;
- documentation IR and compact agent-context IR;
- vocabulary and global-option policy;
- renderer metadata for human and machine output;
- generated help, man pages, completions, and reference artefacts;
- policy linting and drift checks;
- optional primitives for profiles, delivery targets, feedback stores, skill
  manifests, and execution ledgers.

Weaver owns semantic execution: capability routing, Rope, rust-analyzer,
Language Server Protocol (LSP), Tree-sitter, and Sempai providers, sandboxing,
Double-Lock safety, actual edits, job execution, semantic refusal logic, and
provider-specific idempotency.

Netsuke owns build and package semantics: manifest interpretation, subprocess
execution, build graph logic, package-specific run records, and any domain
payloads it sends to delivery or feedback sinks.

This split lets Weaver and Netsuke depend on OrthoConfig for consistent command
contracts without pushing their execution engines into a configuration crate.
OrthoConfig models, generates, serializes, and lints reusable command
contracts. Downstream applications own command execution, side effects,
domain-specific safety policy, and long-running job semantics.

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

The documentation IR and agent-context schema are sibling outputs from the same
metadata spine, not nested versions of one another. The documentation IR stays
localized and human-documentation-oriented. The agent-context schema stays
compact, machine-oriented, and independently versioned so agent-facing changes
do not force documentation IR migrations unless the same data is genuinely
needed by both outputs.

The planned `cargo-orthohelp` generator interface is:

```console
cargo orthohelp --format agent-context
```

Equivalent command forms are acceptable if implementation work finds a cleaner
fit, but the output must remain a first-class format, not a scraped help page.
For downstream application command surfaces, OrthoConfig should prefer the
public command name `context`:

```console
example-cli context --json
```

The payload should identify itself precisely, for example:

```json
{
  "kind": "example-cli.agent_context"
}
```

This keeps the public command approachable while preserving an explicit machine
schema. Hidden aliases such as `agent-context` should be avoided before the
first public release unless a migration requires them.

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

`cargo orthohelp --check-agent-native` must emit a machine-stable policy report
when JSON output is requested. Tests and CI should parse `rule_id` and `code`
for deterministic handling; prose in `message` is explanatory and may improve
without changing the machine contract.

```json
{
  "version": "1",
  "tool": "cargo-orthohelp",
  "mode": "warn",
  "results": [
    {
      "rule_id": "agent-native.vocabulary.canonical-flag",
      "severity": "warn",
      "code": "canonical_flag_missing",
      "message": "Use --json for structured output instead of --format=json.",
      "file": "Cargo.toml",
      "range": {
        "start": {
          "line": 12,
          "column": 1
        },
        "end": {
          "line": 12,
          "column": 20
        }
      }
    }
  ],
  "summary": {
    "off": 0,
    "warn": 1,
    "deny": 0,
    "total": 1
  }
}
```

Each result must contain:

- `rule_id`: stable policy rule identifier;
- `severity`: one of `off`, `warn`, or `deny`;
- `code`: stable machine-readable finding code;
- `message`: human-readable diagnostic text;
- `file`: source file path when available;
- `range` or `span`: optional source location metadata.

Mode handling is direct: `off` suppresses checks, `warn` emits findings without
failing the command, and `deny` exits with a validation-class failure when any
deny-level finding is present.

### 3.4 Long-form workflow material

Long-form skill manifests, tutorials, or MCP-facing descriptions are useful,
but they are downstream of the compact contracts. They should be generated or
validated against the documentation IR and agent context rather than maintained
as an independent source of truth.

Skill manifests are still first-class contracts. OrthoConfig should model the
manifest path, schema version, command index, and validation rules that prove a
manifest mentions real commands and flags. It must not own a downstream skill's
domain prose, such as Weaver's safe Rust rename workflow or Netsuke's build
workflow.

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
- `--no-input` for non-interactive operation;
- `--force` for destructive prompt bypass;
- `--dry-run` for previewing consequential operations;
- `--limit` and `--cursor` for bounded list output;
- `--wait` for blocking until an async submission completes;
- `--profile` for selecting a named persistent identity;
- `--deliver` for routing generated artefacts.

The canonical human-facing global option glossary is:

- `--color auto|always|never` for colour policy;
- `--emoji auto|always|never` for emoji policy when a project supports emoji;
- `--progress auto|always|never` for progress and spinner policy;
- `--accessibility auto|on|off` for accessibility-oriented rendering;
- `--plain` for plain text fallback;
- `--no-pager` for pager suppression;
- `--width <columns>` for terminal-width-sensitive rendering;
- `--locale <locale>` for localized human output;
- `--quiet` and `--verbose` for diagnostic verbosity.

The policy should flag or reject off-convention aliases such as `info`, `ls`,
`--format=json`, `--output json`, and `--skip-confirmations` when strict mode
is enabled. Projects may still opt out or configure exceptions, but exceptions
must be explicit and visible in generated context. It should also flag
near-miss global options when a project uses legacy names for a concept in the
glossary, such as `--output-format`, `--colour-policy`, `--diag-json`, boolean
`--progress`, `--no-emoji`, or boolean `--accessible`.

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

The preferred non-interactive flag is `--no-input`. The preferred destructive
bypass flag is `--force`. If a project chooses a different convention, it must
configure that convention once and expose it in agent context.

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

JSON mode must be stricter than "mostly JSON". The default contract is:

- success writes exactly one JSON result document to stdout and nothing to
  stderr;
- failure writes no stdout unless an explicit artefact has already been
  delivered, and exactly one JSON diagnostic document to stderr;
- subprocess output never leaks beside the result document on stdout;
- protocol identifiers and error classes are not localized.

Some commands legitimately need streams. OrthoConfig should therefore model a
`JsonModeContract`-style shape with these choices:

- `success_stdout`: `one_json_document`, `jsonl_stream`, or `artifact_path`;
- `failure_stderr`: `one_json_diagnostic` or `jsonl_diagnostics`;
- `subprocess_output_policy`: `capture_preview_and_log`,
  `inherit_only_in_human_mode`, or `forbidden`.

The lint policy should reject commands that claim JSON mode but cannot state
where every byte of stdout and stderr goes.

### 6.2.1 Renderer metadata

OrthoConfig should model renderer metadata without becoming every downstream
renderer. The reusable contract should describe:

- human renderer support and machine renderer support;
- TTY sensitivity and closed-stdin behaviour;
- colour, emoji, progress, pager, width, accessibility, and plain-output
  policy;
- stdout and stderr contracts per renderer;
- localized versus non-localized fields;
- whether progress or subprocess output may be inherited in human mode only.

This supports Weaver's dual human/machine output and Netsuke's human-first,
agent-consistent presentation from the same metadata vocabulary.

### 6.2.2 Exit-code taxonomy metadata

OrthoConfig should model exit codes in documentation IR and agent context, but
it should not impose a universal taxonomy. A command contract should be able to
describe:

```json
{
  "exit_codes": {
    "0": { "class": "success" },
    "2": { "class": "usage" },
    "5": { "class": "external_tool_failure" }
  }
}
```

Strict policy should lint that every documented error class has an exit code,
and that JSON diagnostics report the class and code consistently.

### 6.3 Errors that enumerate choices

When a command rejects a value because it is outside a known set, the error
should name the valid set. This applies to enum values, output formats,
delivery schemes, locale identifiers, package names, binary targets, profile
names, and policy modes.

Roadmap item 5.1.1 reconciled the missing-required-values design with the
actual error enum. The current public surface does not expose
`OrthoError::MissingRequiredValues`; missing required values still route through
the existing command-line parsing, merge, gathering/deserialization, or
aggregate error channels. The improved aggregate diagnostic remains phase 7.3.1
implementation work.

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
- status commands, using the public noun configured by the project;
- whether a durable execution ledger exists;
- whether retries can recover an in-flight job.

The generic concept is an execution ledger. The public noun is configurable, so
Weaver can expose `jobs` while Netsuke can expose `runs` without forking the
metadata model. Ledger metadata should cover record identifiers, status enums,
timestamps, command paths, input hashes, idempotency keys, log references,
result references, prune commands, and bounded list behaviour.

OrthoConfig may later provide reusable helper types for execution-ledger
metadata or storage, but the initial design requirement is to model and lint
the command contract.

### 6.7 Persistent profiles

Profiles let agents reuse named bundles of configuration across invocations.
Profile support should be optional, but when present it should use a canonical
root flag:

```console
example-cli render --profile weekly-recap --json
```

The recommended precedence is:

```text
built-in defaults < config files < selected profile < environment < flags
```

If implementation work decides that profiles are named config overlays, the
roadmap must document the exact merge order and migration impact before code is
changed.

Agent context should expose whether profiles are supported, how to list them,
which flag selects one, and which profile fields are redacted or
reference-only. Profile metadata should support secret redaction, profile names
in context output, and generated profile documentation.

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

OrthoConfig owns parsing, validation, schema metadata, and enumerating errors.
It does not own the domain payload sent by Weaver or Netsuke.

### 6.9 Capability and provenance metadata

Capability-hidden providers are a reusable command-contract shape. OrthoConfig
should model generic capability and provenance metadata such as:

```json
{
  "capability_id": "symbol.rename",
  "command": "symbols rename",
  "kind": "actuator",
  "provider_visibility": "provenance",
  "provider_override": "advanced",
  "provenance_in_json": true
}
```

The generic metadata belongs in OrthoConfig because many CLIs hide backend
providers behind stable user intent. Provider registries, provider selection,
semantic execution, and safety harnesses remain application-owned. Strict
policy should be able to warn when provider names are required in ordinary
public commands instead of being exposed as provenance or advanced override
metadata.

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

### 8.1 Defaulting for legacy derives

Older derives will not emit every new metadata field immediately. The
agent-context schema, documentation IR, and man-page generation must therefore
apply explicit defaults instead of guessing from absent data.

| Field                  | Default                  | Rationale                                                                  |
| ---------------------- | ------------------------ | -------------------------------------------------------------------------- |
| `canonical_verb`       | `null`                   | Legacy command metadata did not classify verbs.                            |
| `supports_json`        | `false`                  | Structured output must be declared before tools rely on it.                |
| `json_stdout_contract` | `null`                   | No JSON stream invariant exists until the command opts in.                 |
| `json_stderr_contract` | `null`                   | Diagnostics remain unspecified for legacy commands.                        |
| `exit_classes`         | `[]`                     | Exit-code semantics are unavailable unless documented.                     |
| `interaction_mode`     | `"unknown"`              | Legacy derives cannot prove whether a command prompts.                     |
| `mutation_effect`      | `"unknown"`              | Read/write/delete boundaries must not be inferred from names.              |
| `pagination`           | `null`                   | List bounds and cursors require explicit command metadata.                 |
| `profile_support`      | `{ "supported": false }` | Profiles are opt-in persistent state.                                      |
| `delivery_support`     | `{ "supported": false }` | Delivery sinks change artefact routing and must be explicit.               |
| `feedback_support`     | `{ "supported": false }` | Feedback storage or upload must be explicitly available.                   |
| `execution_ledger`     | `{ "supported": false }` | Jobs, runs, or tasks require application-owned execution state.            |
| `skill_manifest_paths` | `[]`                     | Skills are absent until declared and validated.                            |
| `capability_id`        | `null`                   | Capability routing is optional downstream metadata.                        |
| `provider_provenance`  | `{ "reported": false }`  | Provider names are not emitted unless the application declares provenance. |
| `renderer.human`       | `{ "supported": true }`  | Existing documentation IR already supports human help material.            |
| `renderer.machine`     | `{ "supported": false }` | Machine renderer support must be declared before agents depend on it.      |

Lint behaviour for omitted metadata follows the selected mode. In `off` mode,
the check is not run. In `warn` mode, omitted fields that block an agent-native
guarantee emit warnings but do not fail the command. In `deny` mode, the same
omitted fields fail CI with validation-class diagnostics. Projects should opt
into warning mode first, fix emitted findings, then move to deny mode once the
documentation IR, agent-context schema, and man-page generation are complete
enough for their command surface.

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
- the improved `MissingRequiredValues` diagnostic is reconciled as proposed
  phase 7 work, but is not yet implemented;
- `cargo-orthohelp` has no structured `--json` result mode;
- `cargo-orthohelp` does not yet enumerate all valid choices in errors;
- generated file writes are not specified as atomic;
- renderer metadata, JSON-mode stream contracts, exit-code taxonomy metadata,
  skill manifest validation, capability/provenance metadata, profile redaction,
  delivery, feedback, execution ledger, mutation, and pagination metadata are
  not yet modelled as first-class contracts.

## 10. Deferred extensions

The following ideas are useful but must not block the core agent-native work:

- MCP server generation from agent context;
- OpenAPI-shaped runtime explorer endpoints for downstream applications;
- remote configuration providers;
- live reload of configuration;
- managed application execution ledgers;
- asynchronous configuration file loading.

These extensions should be revisited only after whole-CLI introspection,
agent-context output, strict lint policy, and the `cargo-orthohelp` reference
CLI are working.
