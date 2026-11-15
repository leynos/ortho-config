# Architectural decision record (ADR) 0021: Enumerate missing required values for selected subcommands

## Status

Accepted.

Date: 2026-11-15.

This record amends [ADR-0020](adr-0020-enumerate-missing-required-values.md),
which excluded selected-subcommand merges from enumeration.

## Context and problem statement

ADR-0020 reported every activated absent requirement in one typed error, but
its coverage contract excluded "selected-subcommand values merged through the
Figment extraction helpers". The exclusion was recorded against an
implementation assumption: that such a merge finishes by extracting directly
from `Figment` and therefore never has a schema to walk.

An agent driving a tool with subcommands hits the same one-value-per-attempt
problem there, and worse. A selected subcommand does not read its own
`PREFIX_key` variables or root file keys. It reads `PREFIX_CMDS_<NAME>_key`
variables and the `[cmds.<name>]` file section, which the loader builds inside
the merge. A diagnostic that named the struct's own prefix would advertise
routes the loader never consults, so the first correction attempt would fail
for a second, spurious reason.

## Decision drivers

- One-diagnostic recovery is the value of ADR-0020; excluding a whole loader
  family leaves the common case uncovered.
- An advertised route must be one the loader really reads, or the diagnostic is
  worse than silence.
- The route namespace is derived twice — once by the provider, once by the
  diagnostic — and duplicated derivations drift.

## Requirements

### Functional requirements

- A selected-subcommand load reports every activated absent requirement of the
  selected variant, and reports nothing for unselected variants.
- Each reported item advertises the environment variable and the file path the
  selected-subcommand loader actually reads.
- The command-line hint set is unchanged.

### Technical requirements

- Route construction accepts a namespace rather than assuming the type's own
  `RouteHints` prefix.
- The environment prefix is produced by one function shared by the provider and
  the diagnostic.
- The raw `DeserializeOwned` subcommand helpers keep their existing boundary;
  they never learn which type's schema to compose.

## Options considered

### Option A: Widen `RouteHints` on the generated implementation

Emit the subcommand namespace from the derive, so `required_route_hints()` is
already correct. Rejected: a variant's prefix depends on the runtime prefix and
command name the selected merge observes, not on the type alone, and widening
the public `RouteHints` struct breaks external struct literals.

### Option B: Carry a loader scope through collection

Keep `RouteHints` as the CLI and prefix source and pass a crate-private
`RouteScope` (environment prefix plus file section) into the collector. The
ordinary load derives its scope from the hints; the subcommand load builds one
from the loader's namespace.

| Topic                 | Option A          | Option B            |
| --------------------- | ----------------- | ------------------- |
| Public API impact     | Breaking          | None                |
| Namespace correctness | Type-level only   | Loader-true         |
| Files touched         | Derive and schema | Collector and merge |

_Table 1: Comparison of the two approaches._

## Decision outcome / proposed direction

Option B. `collect::RouteScope` holds an optional environment prefix and a file
section. `collect_missing` derives the default scope from `RouteHints`;
`collect_missing_scoped` takes the hints and an explicit scope together as one
`RouteContext`, because every reported path consults both. `routes_for`
prepends the scope prefix to the environment variable and the scope section to
the file path.

`subcommand_env_prefix` in `ortho_config/src/subcommand/mod.rs` is the single
source of the `PREFIX_CMDS_<NAME>_` namespace. The provider uses it to read, and
`sources::finish_with_required_values` uses it to describe.

The four `load_and_merge_subcommand_for*` helpers and the matching
`SubcmdConfigMerge` methods gain a `RequiredConfigSchema` bound. Every type
that derives `OrthoConfig` already satisfies it.

## Goals and non-goals

- Goals:
  - Cover the selected-subcommand loaders with the ADR-0020 contract.
  - Keep every advertised route reproducible with the loader that raised it.
  - Leave the raw generic boundary untouched.
- Non-goals:
  - Enumeration across an unselected variant's configuration.
  - Changing the command-line hint grammar.

## Migration plan

1. Thread `RouteScope` through the collector and add the scoped entry point.
2. Extract `subcommand_env_prefix` and use it in both the provider and the
   diagnostic.
3. Lift the `RequiredConfigSchema` bound onto the `_for_*` helpers and record
   the change in the v0.11.0 migration guide.

## Known risks and limitations

- A subcommand whose prefix or command name differs between the parser and the
  loader would still advertise a wrong route; the shared helper is what keeps
  them equal, so it must stay the only producer.
- Enumeration still inherits every ADR-0020 limitation: custom `Deserialize`
  implementations, unmarked `#[serde(flatten)]`, and enum payloads.
- The CLI hint for a selected subcommand comes from the derived schema, so a
  subcommand that renames its fields keeps the ADR-0020 rule of omitting a flag
  the parser cannot populate.

## Architectural rationale

The runtime keeps one collector and one route vocabulary. The namespace a
loader reads is data passed to that collector, not a second code path, so the
truthfulness rule has a single place to be enforced and a single place to
change.

## References

- [ADR-0020: Enumerate missing required configuration values](adr-0020-enumerate-missing-required-values.md).
- [Execplan: Enumerate missing required configuration values](../execplans/7-3-1-enumerating-missing-required-values.md).
