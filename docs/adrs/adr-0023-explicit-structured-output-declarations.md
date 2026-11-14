# Architectural decision record (ADR) 0023: Explicit structured-output declarations

## Status

Accepted.

Date: 2026-11-14.

## Context and problem statement

`docs/agent-native-cli-design.md` §6.2 asks command metadata to describe whether
`--json` is supported, what a command's normal stdout result is, whether
stderr carries diagnostics, and which stable exit classes exist. Agent context
currently answers none of those questions, so an agent cannot tell a data
command from a prose command, or from one that writes nothing at all.

Section 8.1 already reserved flat `supports_json` and `exit_classes` rows as
planned fields. Those rows were a placeholder for an intended capability, not a
decision about its shape. Left in place while a real record is introduced, they
would describe a second, contradictory source of truth.

ADR-0010 confined `behaviour(...)` to runtime execution semantics and recorded
that later roadmap items add sibling groups. ADR-0016 did exactly that for
presentation with `renderer(...)`. Structured output is a third concern: it
describes the capability and the meaning of a command's streams, not how a
renderer draws them and not what the command does to the world.

Roadmap 7.2.3 owns this metadata. Stream framing belongs to 7.2.4 and numeric
exit-code taxonomies to 7.2.5, so both are deliberately absent here.

## Decision drivers

- Preserve the no-inference rule (design §8.1). Never derive JSON support, a
  stdout purpose, or an exit class from a command name, a flag, an
  output-format token, a verb, or an observed stream.
- Keep a refusal distinguishable from a silence. An author who states that JSON
  is unsupported and an author who says nothing are making different claims,
  and an agent that cannot tell them apart will act on the wrong one.
- Preserve schema ownership (ADR-0003): `ortho_config::docs` owns the strict IR,
  `ortho_config::agent_context` owns the tolerant wire, and `cargo-orthohelp`
  owns the mirror and the bridge. No schema imports another.
- Keep both wire versions: docs IR `2.0` and agent-context schema `1`. The
  addition is optional and additive.
- Declare capability per command. A parent's claim must never be inherited by a
  child whose output differs.

## Options considered

### Option A: A sibling `output(...)` group with one per-command record

```rust,no_run
#[derive(OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    output(
        supports_json = true,
        stdout = "data",
        stderr_diagnostics = true,
        exit_class(name = "success", meaning = "The command completed"),
        exit_class(name = "usage", meaning = "The invocation was invalid")
    )
)]
struct ReportArgs {
    /* ... */
}
```

The declaration becomes `DocMetadata::output`, crosses the bridge as
`AgentCommand::output`, and appears in agent-context JSON. Undeclared members
stay absent. This is the accepted option.

### Option B: Extend the existing `renderer(...)` group

Rejected. Renderer metadata describes how output is drawn and already carries a
`stdout` content category for a different question: whether a renderer writes
none, text, structured text, or a mixture. A JSON-capability flag and a list of
exit classes inside that record would make the group's meaning depend on which
key is read, and would force a command with no rendering policy to invent one.
ADR-0016's `stdout` category is left exactly as it was.

### Option C: Populate `output_modes` with structured string tokens

Rejected. It is an existing `Vec<String>` whose entries name selectable modes.
Encoding capability, purpose, and exit classes as ad-hoc tokens would change an
existing field's meaning and invent a parsing grammar on the wire.

### Option D: Realize the flat §8.1 rows as top-level fields

Rejected. Top-level `supports_json`, `stdout`, `stderr_diagnostics`, and
`exit_classes` on `DocMetadata` and `AgentCommand` cannot express "declared an
empty block", scatter one concern across four sibling fields that must agree,
and force every future output concern into the top level. One optional record
carries the same information with a single presence rule.

| Topic                        | A: sibling group | B: renderer keys | C: string tokens | D: flat fields |
| ---------------------------- | ---------------- | ---------------- | ---------------- | -------------- |
| Concern separation           | Clear            | Mixed            | Mixed            | Clear          |
| Refusal vs silence           | Native           | Native           | No               | Native         |
| One presence rule            | Yes              | No               | No               | No             |
| Existing fields untouched    | Yes              | Yes              | No               | Yes            |
| Room for 7.2.4 stream policy | Yes              | No               | No               | No             |

_Table 1: Comparison of structured-output representation options._

## Decision outcome / proposed direction

### Attribute surface

A struct-level `#[ortho_config(output(...))]` group with one optional record.
Every member is a named assignment; a bare boolean is a syntax error because
the key it would set is ambiguous.

| Key                         | Values                                    |
| --------------------------- | ----------------------------------------- |
| `supports_json`             | `true`, `false`                           |
| `stdout`                    | `"data"`, `"path"`, `"summary"`, `"none"` |
| `stderr_diagnostics`        | `true`, `false`                           |
| `exit_class(name, meaning)` | two non-optional string literals          |

_Table 2: Accepted structured-output attribute keys and vocabulary._

- `stdout` describes the semantic purpose of a normal successful result. It
  excludes help, version, and error output, and says nothing about encoding or
  presentation. `"path"` means the result is a filesystem path the command
  produced; `"summary"` means a human-readable account of work done; `"none"`
  means a successful invocation writes nothing.
- `stderr_diagnostics` concerns diagnostic output only and says nothing about
  exit status or about the normal result.
- `exit_class` is repeatable and ordered. `name` must match `[a-z][a-z0-9_]*`
  and `meaning` must be nonblank. Names are unique within one command, compared
  exactly: no case folding or whitespace normalization. A duplicate is an
  error, never a silent merge.
- `meaning` is carried verbatim. It is an explanation, not a Fluent identifier,
  and no localizer may resolve or translate it.
- Unknown keys, wrong literal types, repeated assignment to the same leaf, a
  missing class member, and an invalid class name are hard errors with spans.
  Several `output(...)` groups may contribute distinct leaves; a leaf assigned
  twice is rejected even across groups.
- `output()` with no members is accepted and leaves every leaf unset. It records
  that an author considered the question and declared nothing, which is
  representable but claims nothing.

### Wire representation and defaults

- `DocMetadata::output: Option<OutputMetadata>` in both IR owners with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- `AgentCommand::output: Option<OutputContract>` with the same presence rule.
  There is no role-specific legacy default: unlike the renderer records, an
  undeclared command acquires no block at all. A default such as
  `supports_json: false` would fabricate a refusal the author never made.
- Every member of a present block is omitted when undeclared, and
  `exit_classes` is omitted when empty. A present empty block serializes as
  `{}` and claims nothing.
- An IR `null` for the block or any member reads as absence, as elsewhere in the
  IR. A present `null` inside agent context is invalid; `null` for
  `exit_classes` is rejected rather than read as an empty list.
- The agent-context stdout enum carries `#[serde(other)] Unknown` so a future
  token leaves the rest of the document readable. `Unknown` never counts as
  data-returning. The IR has no such variant: authoring and IR input reject an
  unrecognized token instead of discarding an author's declaration.
- `ExitClass` carries no numeric code. Mapping classes to process statuses is
  roadmap 7.2.5 work, and inventing a number here would pre-empt it.
- Roadmap 7.2.3 delivers the declarations and one consumer, the
  `agent-native.output.json-missing` rule, but not a JSON stream contract
  (7.2.4) or numeric exit-code taxonomy metadata (7.2.5). This ADR therefore
  remains the record of the declaration surface; a later ADR can extend it
  without renegotiating the absent-means-undeclared rule.

### Module ownership

The types live in cohesive modules: `ortho_config/src/docs/ir/output.rs`,
`ortho_config/src/agent_context/output.rs`,
`cargo-orthohelp/src/schema/output.rs`, and the bridge mapping in
`cargo-orthohelp/src/agent_context/output.rs`. The bridge maps each IR stdout
variant to its wire counterpart explicitly, one arm per variant, without
localizing, sorting, deduplicating, or inheriting. Only the agent-context
bridge calls that mapping; the schema modules contain no probing or adapter
dependencies.

## Goals and non-goals

- Goals:
  - Let authors declare JSON support, stdout purpose, diagnostic stderr, and
    documented exit classes per command.
  - Transport those declarations through the derive, both schema owners, the
    bridge, and agent-context JSON without acquiring claims never made.
  - Keep refusals, silences, and empty blocks distinguishable at every layer.
- Non-goals:
  - Enforce JSON mode, model stream framing, or reject unsafe combinations of
    declarations; roadmap 7.2.4 and later policy work own those.
  - Assign numeric exit codes or check them against a process; 7.2.5 owns that.
  - Verify that a command actually behaves as declared. A declaration is not a
    certificate.
  - Infer any output property from names, verbs, flags, formats, or locale.

## Migration plan

JSON compatibility is preserved; the Rust source API is not.

- `DocMetadata` and `AgentCommand` each gain a field, so every exhaustive struct
  literal gains `output: None` (IR) or `output: None` (wire).
- All workspace callers, fixtures, examples, and doctests are updated in the
  same atomic change. No compatibility alias is introduced.
- Existing agent-context fixtures are regenerated rather than frozen, because
  the addition is additive and reviewable as a snapshot diff.
- The §8.1 planned flat rows are replaced by the realized `output` rows in the
  same change, so no document describes a shape the tool does not emit.

## Known risks and limitations

- A declaration can be untrue. Metadata cannot establish that a downstream
  process emits valid JSON, writes nothing to stdout, or exits with a
  documented class when it succeeds. Consumers must treat the block as a claim
  to check.
- The mirror in `cargo-orthohelp/src/schema/output.rs` duplicates the IR shape.
  Snapshot tests pin both spellings together, but drift remains possible in
  fields neither side declares.
- Adding fields to public structs breaks downstream exhaustive struct literals.
  This is accepted and documented, consistent with ADR-0016 and roadmap 13.1.

## Outstanding decisions

- Stream framing and the `JsonModeContract`-style shape remain with roadmap
  7.2.4.
- Numeric exit-code mappings and their stability guarantees remain with roadmap
  7.2.5.
- Whether a lint rule should require JSON support for a data-returning command
  is roadmap 7.2.3's policy API. The first rule is delivered as `check_output`
  with rule ID `agent-native.output.json-missing` and code
  `json_support_missing`; expanding the CLI's check to run it is later work.

## Architectural rationale

The accepted surface keeps every driver intact. A sibling group preserves
ADR-0010's scoping and ADR-0016's precedent. One optional record gives the
concern a single presence rule, leaves room for 7.2.4's stream policy inside
the same block, and admits no second source of truth. Declaring nothing never
produces a defaulted claim, so an agent can act on exactly what an author
stated. Strict authoring with tolerant agent reading lets a future stdout token
arrive without stranding older readers, while the IR still refuses to silently
discard an author's declaration.

## References

- [Agent-native CLI assistance design](../agent-native-cli-design.md) §6.2,
  §8.1, §8.2.
- [ADR-0003: Define schema ownership for agent-native contracts](adr-0003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-0010: Behavioural metadata attribute surface](adr-0010-behavioural-metadata-attribute-surface.md).
- [ADR-0016: Dual-renderer metadata surface](adr-0016-dual-renderer-metadata-surface.md).
- [Execplan: Model structured output and exit metadata](../execplans/7-2-3-structured-output-and-exit-metadata.md).
