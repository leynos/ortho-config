# ADR-007: Downstream context command naming

Status: Accepted.

Date: 2026-06-14.

## Context and problem statement

OrthoConfig defines a compact agent-context schema in
`ortho_config::agent_context`, and `cargo-orthohelp` can emit that schema as a
build-time file with `--format agent-context`. Downstream applications need a
runtime command surface for the same contract so agents can discover invocation
metadata directly from an installed tool.

The command name has two competing pressures. Prior art for agent-native CLIs
uses an explicit `agent-context` introspection layer, while OrthoConfig's
application-facing command should be short, stable, and approachable. The JSON
payload still needs an unambiguous discriminator so consumers can recognise the
document without relying on command names alone.

The question is whether downstream applications expose `agent-context`, expose
`context --json` with a payload discriminator, or rely only on JSON shape.

## Decision drivers

- Keep the downstream application command short and suitable for human and
  agent invocation.
- Preserve a clear distinction between the `cargo-orthohelp` generator format
  and application runtime command surfaces.
- Align structured output with the existing canonical `--json` vocabulary.
- Avoid shipping hidden aliases before the first public release unless a
  migration explicitly requires them.
- Keep compatibility detection tied to `schema_version`, not string parsing of
  `kind`.

## Options considered

### Option A: Follow prior art with `agent-context`

Downstream applications would expose `<tool> agent-context`, optionally with a
JSON flag.

This follows Trevin Chow's agent-native CLI prior art most directly, but it
adds a longer command name to every application and risks conflating the
application runtime command with `cargo-orthohelp --format agent-context`.

### Option B: Expose `context --json` with a `kind` discriminator

Downstream applications expose `<tool> context --json`. The emitted JSON uses
`kind: "<tool>.agent_context"` and
`schema_version: ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`. `cargo-orthohelp` keeps
`--format agent-context` as its generator format.

This keeps the public command concise, preserves the generator/runtime
boundary, and still gives machine consumers an explicit payload discriminator.
It is the accepted option.

### Option C: Omit `kind` and rely on shape

Downstream applications expose `context --json`, but consumers infer document
type from fields such as `commands`, `profiles`, and `policy`.

This is rejected. Shape-only detection makes future additive fields harder to
interpret and forces consumers to inspect too much structure before deciding
which schema they are reading.

## Decision outcome

In the context of downstream application agent-context discovery, facing the
need to choose between prior-art explicitness and a concise public command, we
decided for `<tool> context --json` plus `kind: "<tool>.agent_context"` and
against a public `agent-context` command or shape-only detection, to achieve a
stable and approachable runtime surface, accepting deliberate divergence from
the `agent-context` command name used in some prior art.

Compatibility detection uses `schema_version`. The `kind` value identifies the
payload family and is governed by `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` and
`AGENT_CONTEXT_KIND_SUFFIX`; consumers must not parse `kind` to infer schema
compatibility.

The `AGENT_CONTEXT_COMMAND`, `AGENT_CONTEXT_JSON_FLAG`,
`AGENT_CONTEXT_KIND_SUFFIX`, and `agent_context_kind` API in
`ortho_config::agent_context` is the source of truth for this convention.
Downstream applications should not hand-format `kind`.

Table 1 compares the accepted option with the rejected alternatives.

| Option | Command surface         | Discriminator                | Outcome                                             |
| ------ | ----------------------- | ---------------------------- | --------------------------------------------------- |
| A      | `<tool> agent-context`  | Optional                     | Rejected: clear but too coupled to generator naming |
| B      | `<tool> context --json` | `kind` plus `schema_version` | Accepted                                            |
| C      | `<tool> context --json` | Shape only                   | Rejected: brittle for consumers                     |

_Table 1: Comparison of downstream agent-context naming options._

## Goals and non-goals

- Goals:
  - Define the downstream application command name.
  - Define the payload `kind` construction rule.
  - Keep `cargo-orthohelp --format agent-context` unchanged.
  - Document that `schema_version` is the compatibility marker.
- Non-goals:
  - Replace the `cargo-orthohelp` generator format.
  - Generate complete runtime command trees automatically.
  - Introduce a migration alias before a released surface requires one.

## Known risks and limitations

- Agents may be tempted to parse `kind` as a version. Documentation and tests
  must keep the `schema_version` rule explicit.
- The command name diverges from prior art that uses `agent-context`. The
  shorter command is intentional; a future migration can add an alias only if a
  real compatibility need appears.
- Hand-authored example payloads can drift from real commands. Such examples
  must be labelled as illustrative unless they are generated from live metadata.

## Consequences

Downstream applications that expose the agent-context contract use
`context --json` and write compact JSON to stdout. Bare `context` may provide a
human pointer to the JSON form.

`cargo-orthohelp` keeps `--format agent-context`, writes
`<out>/agent-context.json`, and must not gain a public `context` or
`agent-context` subcommand or alias as part of this decision.

The users' guide, developer's guide, and agent-native CLI design are the
normative prose references for this command convention.

## References

- [Agent-native CLI assistance design](agent-native-cli-design.md) §3.2 and
  §5.
- [OrthoConfig IR documentation design for cargo-orthohelp](cargo-orthohelp-design.md)
  §6.3.1.
- [ADR-003: Define schema ownership for agent-native contracts](adr-003-define-schema-ownership-for-agent-native-contracts.md).
- [Trevin Chow, "10 Principles for Agent-Native CLIs"](https://trevinsays.com/p/10-principles-for-agent-native-clis).
- [Cloudflare Wrangler configuration documentation](https://developers.cloudflare.com/workers/wrangler/configuration/).
- [Kubernetes object required fields](https://kubernetes.io/docs/concepts/overview/working-with-objects/).
- [Dapr component specification](https://docs.dapr.io/reference/resource-specs/component-schema/).
