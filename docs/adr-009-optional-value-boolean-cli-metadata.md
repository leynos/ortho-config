# ADR-009: Optional-value boolean CLI metadata

Status: Accepted.

Date: 2026-09-24.

## Context and problem statement

Generated boolean CLI fields previously advertised a presence-only
`clap::ArgAction::SetTrue`, while the hidden field type stayed `Option<bool>`.
The parser could therefore produce `None` or `Some(true)`, but no generated
surface could produce an explicit `Some(false)`. A `true` supplied by a
lower-precedence configuration file or environment variable could never be
cleared from the command line.

Making `--flag=false` reachable solves the parsing problem, but the
documentation IR describes CLI flags to renderers and agents that have no
knowledge of clap internals. Reporting such flags with `takes_value: false`
would tell those consumers that no value can follow, which is precisely the
information an agent needs to decide whether `--flag=false` is well formed.

The question is whether the existing `CliMetadata` fields can express "accepts
an optional value" or whether the IR needs a new marker.

## Decision drivers

- Preserve absent-versus-present semantics: an omitted flag must still leave
  lower-precedence sources in control.
- Keep the generated IR accurate for renderers and agent-context consumers.
- Follow the ADR-003 schema-ownership rules, including a version bump and
  `#[serde(default)]` for additive fields.
- Avoid introducing a second clap argument or a second argument identifier per
  field.
- Keep `cargo-orthohelp::schema` byte-for-byte aligned with
  `ortho_config::docs`, as the version-alignment test requires.

## Options considered

### Option A: Express the surface using existing fields only

Boolean flags would be reported with `takes_value: true`, a `BOOL` value name,
and `possible_values: ["true", "false"]`, but no dedicated marker.

This reuses the existing shape, so no version bump or ADR is needed. It is
rejected because `takes_value: true` already means "a value is required" for
every other option. Consumers cannot distinguish `--flag <BOOL>` from
`--flag[=<BOOL>]`, and renderers would print a misleading unbracketed
placeholder.

### Option B: Add an optional-value marker to `CliMetadata`

`CliMetadata` gains `value_optional: bool`, `ORTHO_DOCS_IR_VERSION` moves from
"1.1" to "1.2", and the mirrored schema definition changes in step. Boolean
flags are reported with `takes_value: true`, `value_name: Some("BOOL")`,
`possible_values: ["true", "false"]`, and `value_optional: true`. Renderers
print the optional-value form when the marker is set: the roff renderer emits
`--flag[=BOOL]` with an italic placeholder, following clap's `require_equals`
suffix rather than clap's help notation. This is the accepted option.

### Option C: Generate a separate negating flag

Boolean fields would generate an extra `--no-flag` argument alongside `--flag`.

This is rejected. It breaks the one-argument-identifier-per-field model that
the derive relies on for extraction, and it contradicts the preference against
auto-generated `--no-x` pairs recorded in
[Agent-native CLI assistance design](agent-native-cli-design.md) §5.

## Decision outcome

The selected surface is a single value-taking optional-value flag per boolean
field, described in the documentation IR by a new `value_optional` marker.

`CliMetadata::value_optional` defaults to `false`. IR written before the field
existed therefore reads as a plain switch, which preserves the meaning of
existing documents without a migration step, per
[ADR-003](adr-003-define-schema-ownership-for-agent-native-contracts.md).

`ORTHO_DOCS_IR_VERSION` becomes "1.2". The marker is additive, but the
accompanying boolean-field changes (`takes_value`, `value_name`, and
`possible_values`) make the 1.1 rendering of a boolean flag differ from the 1.2
rendering, so the version bump records a real change in what the IR says about
the generated CLI surface.

`cargo-orthohelp::schema::CliMetadata` and `ORTHO_DOCS_IR_VERSION` are updated
in step with the `ortho_config::docs` definitions. The repository's
version-alignment test enforces that pairing, and the schema test's
`sample_cli_metadata` constructor acts as a compile-time tripwire for any
missing field.

Table 1 compares the accepted option with the rejected alternatives.

| Option | IR shape                      | Outcome                                         |
| ------ | ----------------------------- | ----------------------------------------------- |
| A      | Reuse `takes_value` only      | Rejected: ambiguous with required values        |
| B      | New `value_optional` marker   | Accepted                                        |
| C      | Separate `--no-flag` argument | Rejected: breaks one-identifier-per-field model |

_Table 1: Comparison of optional-value boolean metadata options._

## Goals and non-goals

- Goals:
  - Describe optional-value boolean flags accurately in the documentation IR.
  - Keep absent-versus-present semantics intact for every layer.
  - Keep the IR and its mirrored schema definition aligned.
- Non-goals:
  - Change the merge or precedence rules for any other option type.
  - Add negation flags or aliases for non-boolean options.
  - Change the runtime behavioural contract of existing IR consumers beyond the
    version-gated additions.

## Known risks and limitations

- Renderers that ignore `value_optional` will print a bare switch. The
  PowerShell MAML renderer, the roff renderer, and the agent-context bridge are
  updated here; any future renderer must consult the marker.
- Booleans now publish `possible_values: ["true", "false"]`. Consumers that
  classify a non-empty `possible_values` list as an enumeration would
  misclassify booleans as enums. The agent-context bridge guards against this
  by checking `ValueType::Bool` before the possible-values heuristic.
- The marker is a one-way door for the IR: once consumers depend on it,
  removing it would be a breaking change requiring a further version bump.

## Consequences

Generated boolean fields accept `--flag`, `--flag=true`, and `--flag=false`,
with the bare spelling meaning `true` and an omitted flag leaving the value
absent. The flag must use `=` before an explicit value so that `--flag` does
not swallow the following argument.

The documents emitted by `cargo-orthohelp` and the metadata exposed through
`OrthoConfigDocs` both carry the new marker. Man pages render `--flag[=BOOL]`
with an italic placeholder, and PowerShell help gains a paragraph explaining
the two spellings.

The users' guide and the migration guide are the normative prose references for
the command-line surface and for upgrading consumers of the IR.

## References

- [Agent-native CLI assistance design](agent-native-cli-design.md) §5.
- [OrthoConfig IR documentation design for cargo-orthohelp](cargo-orthohelp-design.md).
- [ADR-003: Define schema ownership for agent-native contracts](adr-003-define-schema-ownership-for-agent-native-contracts.md).
- [clap `Arg::num_args` documentation](https://docs.rs/clap/latest/clap/struct.Arg.html#method.num_args).
