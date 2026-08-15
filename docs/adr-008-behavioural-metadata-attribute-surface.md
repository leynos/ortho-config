# Architectural decision record (ADR) 008: Behavioural metadata attribute surface

## Status

Accepted.

Date: 2026-08-14.

## Context and problem statement

Agents that drive a command-line tool need to know, before running a command,
whether it will block on a prompt and whether it mutates state. The
agent-context output of `cargo orthohelp` emits `interaction_mode` and
`mutation_effect` for every command, but before 7.2.1 those fields were always
`"unknown"`: no mechanism existed for a project author to declare these facts
at the derive site.

Roadmap item 7.2.1 asks for metadata that represents whether a command is
non-interactive, may prompt, or requires a bypass flag, and whether the command
reads, writes, deletes, or submits asynchronous work, plus a lint for destructive
commands without `--force` or equivalent approved metadata.

The design document (`docs/agent-native-cli-design.md`) §6.1 and §6.4 define
the required semantics, and §8.2 defines the agent-context schema v1
compatibility policy: additive changes within a major version are allowed, and
fields must not be inferred from command names or spelling.

## Decision drivers

- Keep schema v1 wire-stable: §8.2 makes renaming fields, changing enum wire
  strings, changing serialized defaults, or toggling null-versus-omitted
  breaking changes.
- Do not infer interaction or mutation semantics from command names, verbs, or
  flags (design doc §8.1). Absent metadata stays `unknown`.
- Preserve dependency direction `ortho_config_macros` → `ortho_config` →
  `cargo-orthohelp`; the bridge transform is the only place the documentation
  IR and agent-context schemas meet (ADR-003 ownership split).
- Use one nested attribute group scoped to runtime execution semantics; later
  roadmap items add sibling groups (for example `output(...)`) rather than
  growing this one indefinitely.
- No new external dependencies.

## Options considered

### Option A: One nested struct-level `behaviour(...)` group

Authors declare behaviour with a single nested struct-level attribute group
`#[ortho_config(behaviour(...))]` on the command's arguments struct:

```rust,no_run
#[derive(OrthoConfig, OrthoConfigDocs)]
#[ortho_config(
    prefix = "APP",
    behaviour(interaction = "interactive", mutation = "delete", bypass = "--force")
)]
struct PurgeArgs {
    /* ... */
}
```

The group is scoped to runtime execution semantics only. Later roadmap items
add sibling groups rather than growing this one. Struct-level attributes
already flow through the `StructAttrs`/`DocStructAttrs` parse path and reach
subcommand metadata via the ADR-005 companion-trait delegation, which
overwrites only `app_name` and `about_id`.

This is the accepted option.

### Option B: Variant-level attributes on subcommand enums

Authors annotate subcommand enum variants directly. This was rejected because
variant-level attributes have no existing parse path today and would duplicate
state held by the struct-level metadata that already reaches subcommands via
the ADR-005 delegation.

### Option C: A third `InteractionMode` variant for "requires bypass"

Represent §6.1's three states as three interaction modes instead of an
interaction × bypass pair. This was rejected: §6.1 treats the bypass flag as a
property ("which flag bypasses prompting"), not a distinct mode, and adding a
v1 wire-enum variant requires an unknown-variant fallback contract for no
expressive gain. The pair `interaction` × `bypass` maps the three states as:

- `interaction = "non_interactive"` — never prompts;
- `interaction = "interactive"` with no `bypass` — the lint fires;
- `interaction = "interactive"` with a declared `bypass` — prompting is
  explicitly bypassable.

This mirrors the MCP tool-annotation style of orthogonal declared hints.

| Topic                    | Option A           | Option B      | Option C         |
| ------------------------ | ------------------ | ------------- | ---------------- |
| Attribute shape          | Nested group       | Variant-level | Nested group     |
| Existing parse path      | Yes                | No            | Yes              |
| Wire-schema impact       | Additive           | Additive      | New enum variant |
| Three-state model        | interaction×bypass | Same          | Third variant    |

_Table 1: Comparison of attribute-surface options._

## Decision outcome / proposed direction

The accepted surface is a single struct-level
`#[ortho_config(behaviour(...))]` group with the keys:

- `interaction = "non_interactive" | "interactive"`;
- `mutation = "read_only" | "write" | "delete" | "submit"`;
- `bypass = "<flag>"` matching `--[a-z0-9]+(-[a-z0-9]+)*`;
- `dry_run = "<flag>"` matching the same grammar.

Undeclared keys stay `None` at the IR layer and `unknown`/`null` at the
agent-context layer. The IR (`ortho_config::docs::ir::BehaviourMetadata`) owns
the declared enums without an `Unknown` variant; agent context has `Unknown`.
`ORTHO_DOCS_IR_VERSION` bumped from `"1.1"` to `"1.2"` for the additive
`DocMetadata::behaviour` block; `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` stays
`"1"` under the §8.2 additive-change policy.

## Goals and non-goals

- Goals:
  - Let authors declare interaction and mutation boundaries at the derive site.
  - Populate the reserved agent-context fields without a schema version bump.
  - Add the optional `bypass_flag` and `dry_run_flag` fields.
  - Provide the `--check-agent-native[=off|warn|deny]` lint.
- Non-goals:
  - Infer any semantics from command names or flags.
  - Grow the `behaviour(...)` group beyond runtime execution semantics.
  - Couple `mutation = "submit"` to the existing `async_submission` contract.
  - Represent dry-run support as a boolean.

## Migration plan

The phases were implemented as milestones B–F of the 7.2.1 execplan:

1. Milestone B: IR and agent-context schema types plus the version bump.
2. Milestone C: derive attribute surface with validation.
3. Milestone D: bridge population and fixture coverage.
4. Milestone E: `--check-agent-native` lint with policy report.
5. Milestone F: documentation and closure.

## Known risks and limitations

- The lint's exit code 3 presently collides with no other `cargo-orthohelp`
  exit class, but the exit-code taxonomy is scheduled for roadmap item 7.2.5,
  which may supersede this provisional code. The developers' guide records the
  decision.
- The executable report has no source locations: `AgentContext` carries no
  source spans, so `PolicyResult.location` is `None`. The finding message is
  the entire operator experience and names the command path plus the exact
  annotation to add.
- Dry-run support declared as a flag-name string (`dry_run_flag`) loses the
  tri-state boolean's "declared absent" state; absence of declaration means
  unknown, and an explicit declared-absent marker is deferred until a consumer
  needs it.
- The hand-maintained IR mirror in `cargo-orthohelp/src/schema/mod.rs` must be
  kept in sync with `ortho_config::docs::ir`; the schema-pin test and the
  golden bridge tests catch drift.

## Outstanding decisions

- None. ADR-007 remains the reference for downstream `context --json` naming.

## Architectural rationale

The accepted surface keeps the no-inference rule (design doc §8.1): the derive
and the bridge transport declarations, they never verify runtime behaviour.
The attribute group stays scoped to execution semantics so later roadmap items
(dual renderer, structured output) can add sibling groups without churn to
existing declarations. The IR version bump follows the conservative precedent
for additive schema changes, and the version-skew contract (documented in the
7.2.1 execplan) lets older readers ignore the new `behaviour` block and newer
readers accept 1.1 IR with `behaviour: None`.

## References

- [Agent-native CLI assistance design](agent-native-cli-design.md) §6.1, §6.4,
  §8.1, §8.2.
- [ADR-003: Define schema ownership for agent-native contracts](adr-003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-005: Subcommand docs companion trait](adr-005-subcommand-docs-companion-trait.md).
- [ADR-007: Downstream context command naming](adr-007-downstream-context-command-naming.md).
- [Model Context Protocol tool annotations](https://modelcontextprotocol.io/specification/2025-06-18/tools).
- [Command Line Interface Guidelines (clig.dev)](https://clig.dev/).
