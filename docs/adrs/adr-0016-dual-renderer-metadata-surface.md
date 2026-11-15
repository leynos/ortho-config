# Architectural decision record (ADR) 0016: Dual-renderer metadata surface

## Status

Accepted.

Date: 2026-10-28.

## Context and problem statement

`cargo orthohelp` emits agent context that describes a command's invocation
contract, but it cannot say how the command renders its output. Every command
looks the same to an agent: `output_modes` is emitted as an empty vector and no
field describes whether human or machine output exists, whether colour or
progress animation is used, or which output fields may be localized.

`docs/agent-native-cli-design.md` §6.2.1 asks the reusable contract to describe
human and machine renderer support, terminal (TTY) sensitivity, closed-stdin
behaviour, colour, emoji, progress, pager, width, accessibility, and
plain-output policy, the stdout and stderr contracts per renderer, which output
fields are localized, and whether progress or subprocess output may be
inherited. Roadmap item 7.2.2 delivers that metadata.

ADR-0010 confined the `behaviour(...)` group to runtime execution semantics and
recorded that later roadmap items add sibling groups rather than growing it.
Dual-renderer description is a sibling concern: it describes presentation, not
whether a command prompts or mutates state. It therefore needs its own
attribute group and its own IR block.

## Decision drivers

- Keep the no-inference rule (design §8.1): never derive rendering from a
  command name, a `--json` flag, an output-format name, a terminal probe, or
  the host locale. A declaration is transported, not verified.
- Preserve the schema ownership split of ADR-0003: the documentation IR belongs
  to `ortho_config::docs`, agent context to `ortho_config::agent_context`, and
  the conversion to `cargo-orthohelp`. Neither library schema imports the other.
- Keep wire stability: add only optional fields, retain every existing field
  name, enum spelling, default meaning, ordering rule, and the
  omission-versus-`null` convention. Agent-context version `1` and docs IR
  `2.0` already accommodate additive optional blocks (design §8.2).
- Describe rendering independently per command. A parent's declaration must not
  be inherited by a child whose output behaviour differs.
- Exclude JSON-mode enforcement, exit classes, pagination, and new lint rules;
  those remain 7.2.3–7.2.4 work.

## Options considered

### Option A: A sibling `renderer(...)` attribute group with per-command records

Authors declare rendering with a nested struct-level group:

```rust,no_run
#[derive(OrthoConfig)]
#[ortho_config(
    prefix = "APP",
    renderer(
        human(supported = true, colour = "auto", width = 80),
        machine(supported = true, stdout = "structured", colour = "never")
    )
)]
struct ReportArgs {
    /* ... */
}
```

The declared values reach the IR as `DocMetadata::renderer`, cross the bridge as
`AgentCommand::renderer`, and appear in agent-context JSON. Undeclared
properties stay absent rather than becoming an affirmative promise.

This is the accepted option.

### Option B: Extend the existing `behaviour(...)` group

Add renderer keys to `behaviour(...)`. Rejected: ADR-0010 scoped that group to
execution semantics, and mixing presentation into it makes the group's meaning
depend on which key is read. It also forces authors who only care about
rendering to supply unrelated execution metadata, or vice versa.

### Option C: Reuse `output_modes` with structured string tokens

Populate the existing `output_modes: Vec<String>` field with tokens such as
`human`, `machine`, and `colour:auto`. Rejected: a flat string list cannot
carry the typed width object, cannot express per-renderer policy pairs without
inventing a parsing grammar on the wire, and would change the meaning of an
existing field. `output_modes` is left exactly as it was.

### Option D: One flat renderer record with `human_*` and `machine_*` keys

Rejected: it doubles every key, makes the partial-declaration states hard to
read, and produces a schema that cannot grow a third renderer family without
another round of prefixing. Two named records keep the mirror alignment simple.

| Topic                    | A: sibling group | B: behaviour keys | C: string tokens | D: flat prefixes |
| ------------------------ | ---------------- | ----------------- | ---------------- | ---------------- |
| Scope separation         | Clear            | Mixed             | Mixed            | Clear            |
| Typed width object       | Yes              | Yes               | No               | Yes              |
| Existing field untouched | Yes              | Yes               | No               | Yes              |
| Per-renderer partiality  | Native           | Awkward           | Awkward          | Verbose          |

_Table 1: Comparison of renderer-metadata representation options._

## Decision outcome / proposed direction

### Attribute surface

A struct-level `#[ortho_config(renderer(...))]` group with two optional nested
records, `human(...)` and `machine(...)`. Each record accepts optional
`supported = <bool>` plus the policy keys below. Unknown keys, invalid tokens,
wrong literal types, and repeated assignment to the same leaf are hard errors
with spans. Separate groups may contribute distinct leaves; a repeated leaf is
rejected even across groups.

Every token is `snake_case` on the wire.

| Key                                    | Values                                        |
| -------------------------------------- | --------------------------------------------- |
| `supported`                            | `true`, `false`                               |
| `tty`                                  | `independent`, `adapts`, `requires`           |
| `closed_stdin`                         | `ignored`, `eof`, `error`                     |
| `colour`, `emoji`, `progress`, `pager` | `never`, `auto`, `always`                     |
| `width`                                | `unbounded`, `"terminal"`, or an integer `80` |
| `accessibility`                        | `unspecified`, `text_equivalent`              |
| `plain`                                | `unsupported`, `available`, `always`          |
| `stdout`, `stderr`                     | `none`, `text`, `structured`, `mixed`         |
| `progress_inheritance`                 | `allowed`, `forbidden`                        |
| `subprocess_inheritance`               | `allowed`, `forbidden`                        |
| `field(<id>, localization = "...")`    | `localized`, `non_localized`                  |

_Table 2: Accepted renderer attribute keys and vocabulary._

- A terminal (TTY) means stdout's terminal status; it says nothing about stdin.
- `auto` for colour, emoji, progress, and pager means application-selected, not
  a promise about a particular environment variable or precedence algorithm.
- `accessibility = "text_equivalent"` declares that meaning does not depend
  solely on colour, emoji, animation, or layout. It is an application claim,
  not a certification.
- `plain = "available"` asserts a mode exists; it does not add a flag or resolve
  option precedence.
- `stdout` and `stderr` describe content categories, not framing or document
  counts. Structured output need not be JSON.
- Inheritance means downstream output may pass through directly. A
  human-only inheritance pairing is human `allowed` plus machine `forbidden`;
  neither renderer may promise behaviour for the other.

### Width encoding

Width uses an internally tagged object in both schemas: `{"kind":"terminal"}`,
`{"kind":"unbounded"}`, or `{"kind":"fixed","columns":80}`. The fixed variant
carries `Fixed { columns: NonZeroU16 }`, so a declared positive width cannot
contradict itself. Zero, negative, non-integer, and overflowing columns are
syntax errors in the derive input and invalid known values in the IR. Agent
context normalizes an unrecognized kind to `{"kind":"unknown"}` and ignores its
extra fields; the IR rejects unknown kinds rather than discarding a declaration.

### Output field localization

Each renderer record may carry ordered `fields: Vec<RendererField>` entries
with a stable `id: String` and a `localization` classification. The
classification describes the field's value, never its identifier. An identifier
is an opaque application-defined name, not a JSON pointer or executable lookup.
Non-empty, unique identifiers are derive-input guarantees only; the public
records and the JSON retain permissive `String`/`Vec` shapes, and the bridge
preserves hand-built entries without validating or certifying them. An empty
list makes no claim about undeclared fields, and declaration order is preserved.

### Wire representation and defaults

- `DocMetadata::renderer: Option<RendererMetadata>` in both IR owners, with
  `#[serde(default, skip_serializing_if = "Option::is_none")]`. Absence is
  expressed by `None`; there is no `unknown` variant at this layer.
- `AgentCommand::renderer: RendererContract` with `#[serde(default)]`. The
  missing-field, empty-object, omitted-record, empty-record, and
  missing-`supported` cases all apply role-specific support defaults: human
  `true`, machine `false`. Those are documented legacy compatibility values,
  not observed capability.
- A present `null` renderer, record, or `supported` is invalid in agent context.
  An optional IR renderer accepts `null` as absence, and optional policy leaves
  accept `null` as absence.
- Default serialization is exactly
  `{"human":{"supported":true},"machine":{"supported":false}}`. Missing policy
  fields are omitted, never replaced by `false` or an affirmative promise.
- Agent policy enums carry an `unknown` fallback for unrecognized future tokens,
  with a tested Serde fallback. Authoring and IR input reject those tokens.
  Round trips preserve known values; future-token tests assert normalization to
  `unknown` rather than equality with the original token.

The new types live in cohesive modules: `ortho_config/src/docs/renderer.rs`,
`ortho_config/src/agent_context/renderer.rs`,
`cargo-orthohelp/src/schema/renderer.rs`, and the bridge mapping in
`cargo-orthohelp/src/agent_context/renderer.rs`. The bridge explicitly maps
each IR enum to its wire counterpart; it does not localize, infer, sort field
declarations, or populate `output_modes`. Only the agent-context bridge calls
that mapping; the public schema modules contain no terminal probing or adapter
dependencies.

## Goals and non-goals

- Goals:
  - Let authors declare human and machine rendering independently per command.
  - Transport those declarations through the derive, both IR owners, the bridge,
    and agent-context JSON without acquiring claims the author never made.
  - Keep protocol identifiers stable when human language changes.
  - Apply documented legacy support defaults to undeclared commands.
- Non-goals:
  - Implement terminal output, a rendering engine, or environment probing.
  - Enforce JSON mode, model exit classes, add pagination, or add lint rules.
  - Infer any rendering property from names, flags, formats, or locale.
  - Resolve policy precedence or reject semantically unsafe combinations; those
    remain representable for future lint policy.

## Migration plan

This is a deliberate pre-1.0 Rust source break, approved on 2026-10-28. JSON
compatibility is preserved, but struct-literal construction is not.

- `DocMetadata` gains a field, so every exhaustive literal gains
  `renderer: None` (IR) or `renderer: Default::default()` (wire).
- All workspace callers, fixtures, examples, and doctests are updated in the
  same atomic change; no compatibility alias or parallel API is introduced.
- The next breaking-compatible pre-1.0 release publishes a migration note for
  downstream struct-literal consumers, consistent with roadmap 13.1's
  extensibility gap.
- The frozen pre-renderer agent-context fixture is retained as historical
  evidence; a separate renderer-contract fixture is added rather than
  overwriting it.
- Milestone M1 lands types, parser, token emission, both IR schemas, public
  exports, the bridge, and the contract tests together. Reverting M1 means
  reverting the whole commit, never one side of the schema mirror.

## Known risks and limitations

- Adding fields to public structs breaks downstream exhaustive struct literals.
  This is accepted and documented rather than worked around; roadmap 13.1 is
  the longer-term remedy.
- The hand-maintained IR mirror in `cargo-orthohelp/src/schema/` must be kept
  aligned with `ortho_config::docs::ir`. Each file is kept in step with its
  namesake, and mirrored tests catch drift.
- A declaration is not a certificate: an unsupported renderer may retain
  declared policies describing an unavailable implementation, and consumers
  must check support before acting.
- Truthfulness of downstream behaviour and actual terminal accessibility remain
  outside the guarantee, as does any claim about pager or TTY behaviour at
  runtime.

## Outstanding decisions

- None. Concrete JSON mode is deferred to 7.2.3–7.2.4, and exit-code taxonomy to
  7.2.5, per the roadmap.

## Architectural rationale

The accepted surface keeps every driver intact. A separate group preserves
ADR-0010's scoping. Per-command declarations avoid inheriting a parent's
promises into a child with different output behaviour. Optional IR fields and
role-specific wire defaults keep the addition inside the existing schema
versions. Strict authoring with tolerant agent reading lets a future token be
added without stranding older readers, while the IR still refuses to silently
drop an author's declaration.

## References

- [Agent-native CLI assistance design](../agent-native-cli-design.md) §6.2,
  §6.2.1, §8.1, §8.2, §9.
- [ADR-0003: Define schema ownership for agent-native contracts](adr-0003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-0010: Behavioural metadata attribute surface](adr-0010-behavioural-metadata-attribute-surface.md).
- [ADR-0012: Profile selection and layering](adr-0012-profile-selection-and-layering.md).
- [Execplan: Add dual-renderer metadata](../execplans/7-2-2-dual-renderer-metadata.md).
