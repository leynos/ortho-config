# Architectural decision record (ADR) 0026: Derive CLI layer presence from clap value sources

## Status

Accepted.

Date: 2026-11-21.

This record amends the "Flag-equals-default resolution" subsection of
[ADR-0012](adr-0012-profile-selection-and-layering.md), which described the
legacy derive as keeping a whole-object comparison.

## Context and problem statement

The generated declarative loader used to decide whether the CLI layer existed
by comparing the whole sanitized CLI object with the sanitized defaults object.
Equality does not mean the user omitted every argument: an explicit value that
happens to equal its configured default produces an object equal to the
defaults. The comparison could therefore discard the entire CLI layer and let a
lower-precedence file or environment value win over an explicit argument.

The profile-enabled path kept that comparison as one arm of its push condition
alongside a per-field value-source check. That arm made a load with no
configuration argument add an empty CLI layer whenever the defaults object was
non-empty, because the empty sanitized CLI object differs from the defaults.
Layer presence therefore depended on clap's default-filling behaviour rather
than on user input, even though the merged values happened to be unaffected.

The derive already computes `CliFieldInfo` for every option field: the clap
argument ID answers `ArgMatches::value_source`, and the serialized key names
the JSON member that holds the parsed value. Clap reports `CommandLine`,
`EnvVariable`, or `DefaultValue` independently of the parsed value.

## Decision drivers

- An explicit argument equal to its configured default must outrank every
  lower-precedence layer.
- A parser default or an omitted argument must not outrank those layers.
- One rule must cover every generated field, including boolean and collection
  fields, without a per-attribute pruning list that can drift from the CLI
  surface.
- A load that supplies no configuration argument should add no CLI layer.
- The profile path must keep an environment-filled argument above the profile
  layer.

## Requirements

### Functional requirements

- Explicit scalar, boolean, optional, and collection values equal to their
  configured defaults remain in the CLI layer.
- Omitted arguments do not enter the CLI layer, whether or not clap filled a
  default for them.
- The declarative path admits only command-line values.
- The profile-enabled path admits command-line and environment values.

### Technical requirements

- The generated layer is a sparse `serde_json::Map` keyed by each field's
  serialized name.
- Membership is decided per field through `ArgMatches::value_source`, never by
  comparing whole objects.
- Generated selectors such as `profile` and `config_path` never enter the
  layer.
- Generated code remains warning-free under `-D warnings`, including for a
  configuration struct with no option fields.

## Options considered

### Option A: Keep the profile push gate and prune around it

Retain the whole-object arm only for the profile path and keep the existing
pruning lists. Rejected: the equality arm still adds an empty layer for absent
input, and the pruning lists duplicate a rule that the value-source lookup can
apply uniformly.

### Option B: Sparse per-field extraction through shared tokens

Build one shared sparse-layer emitter. Both compose paths pass the clap sources
they accept; the emitter copies only matching fields and pushes only when at
least one field matches. Accepted.

## Decision outcome / proposed direction

`ortho_config_macros/src/derive/load_impl/cli.rs` owns one sparse-layer emitter.
`build_cli_layer_tokens` passes `CommandLine`;
`build_profile_cli_layer_tokens` passes `CommandLine | EnvVariable`. The
emitter computes `has_explicit_cli_value` from the same per-field source checks
and pushes an object containing only the matching fields.

`CliFieldInfo` is threaded through `LoadImplArgs` so generation uses the
argument ID and serialized key already computed for the CLI surface. The
`cli_default_as_absent` field-name list and the boolean-default pruning list
are removed from `DefaultStructInit` and the loader: a `DefaultValue` source is
simply absent from the sparse layer, while an explicit value equal to the
default is present.

A struct with no option fields emits no layer. The generated method still reads
its parse bindings so the expansion does not introduce unused-variable warnings.

## Goals and non-goals

- Goals:
  - Make CLI-layer membership depend only on clap value sources.
  - Cover every generated field with one rule.
  - Keep both compose paths consistent while preserving their accepted source
    policies.
- Non-goals:
  - Changing the subcommand `CliValueExtractor` path, which already consults
    `value_source` and has its own `cli_default_as_absent` contract.
  - Changing the five-tier precedence order.

## Migration plan

1. Thread `CliFieldInfo` through the loader arguments.
2. Replace both CLI-layer builders with the shared sparse emitter.
3. Remove the redundant pruning plumbing from the defaults and profile
   builders.
4. Add regression coverage for explicit and absent scalar, boolean, optional,
   and collection values on both compose paths.

## Known risks and limitations

- A future clap value source would need an explicit decision about whether it
  counts as explicit input; the shared emitter makes that a one-place change.
- No generated configuration-field argument binds a clap `env` today, so the
  profile path's `EnvVariable` arm is latent. It is retained so a future
  binding cannot silently fall below the profile layer.
- Collection merge strategies key on JSON presence, so omitting an absent
  collection is what preserves append and replace semantics. The sparse layer
  satisfies that by construction.

## Architectural rationale

The CLI layer is a projection of parsed input, not a general snapshot of the
parser. Projecting only fields with an explicit origin keeps the merge engine's
presence-based collection rules and makes the precedence guarantee independent
of whether a parsed value happens to coincide with a configured default.

## Amendments

None.
