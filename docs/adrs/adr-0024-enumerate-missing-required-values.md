# Architectural decision record (ADR) 0024: Enumerate missing required configuration values

## Status

Accepted for implementation.

Amended by
[ADR-0025](adr-0025-enumerate-missing-values-for-selected-subcommands.md)
(2026-11-15).

Date: 2026-11-09.

## Context and problem statement

A caller that omits several required configuration values currently learns
about one of them per attempt. The merged value reaches `serde_json` without
any requirement metadata, so the deserializer reports the first absent field it
meets as an `OrthoError::Merge`. An agent driving a command-line tool therefore
cannot build a successful second invocation from the first diagnostic: it must
discover the remaining requirements by retrying.

The information needed to do better already exists in the derive input. The
macro sees each field, its type, its Serde naming attributes, its
`#[ortho_config(default)]` expression, its inferred clap default, whether it is
excluded from the CLI, and its environment prefix. What it does not see is the
syntax of another type's fields, so nested requirement enumeration needs an
explicit composition mechanism rather than reflection or a second
deserialization pass.

Two further constraints bound the solution. First, a false requiredness claim
is worse than no claim: `Option`, defaults, container defaults, skipped fields,
collections, and custom deserializers do not share one rule. Second, an
advertised supply route must be one the real provider consumes. A fabricated
`--database-host` flag or an environment name the provider cannot nest would
send the caller down a dead end.

## Decision drivers

- One typed, inspectable error listing every absent requirement, in schema
  order, without duplicates.
- Truthful supply routes: CLI, environment, and configuration-file spellings
  that the actual loader accepts for that canonical path.
- Requiredness derived from the loader's real semantics, never from
  documentation-only metadata.
- Nested enumeration only where a type explicitly opts in; arbitrary Serde
  implementations stay opaque and keep their existing errors.
- Finite schema construction with bounded recursion and no double evaluation of
  defaults or custom deserializers.
- Preservation of precedences, collection merging, profiles, injected sources,
  and clap help/version behaviour.
- No new production dependency.

## Options considered

### Option A: Pre-flight Figment probes

Inspect the final `Figment` with `find_value` per field. Rejected: the design
works on the pre-merge object model, `find_value` can fail on combined-provider
errors, and the field list would still need the same schema. It also misses
collection overlays that exist only in the declarative finish state.

### Option B: Deserialize field by field

Attempt `serde_json::from_value` for each field and classify "missing field"
failures. Rejected: it classifies English error strings, cannot separate
absence from invalid input, evaluates custom deserializers repeatedly, and
violates the requirement never to treat every deserialization failure as
absence.

### Option C: Explicit schema composition

Generate a requirement schema from the derive input, compose child schemas only
through an opt-in field marker, inspect the final merged `Value` once, and
report the ordered set of activated absent requirements. Accepted.

### Option D: Arbitrary Serde reflection

Recover nested requirements from the deserializer or from documentation IR.
Rejected: no stable reflection exists, documentation IR is descriptive rather
than authoritative, and deriving runtime requiredness from it would be false.

| Topic                       | A: Figment probes | B: field deserialize | C: explicit schema | D: reflection |
| --------------------------- | ----------------- | -------------------- | ------------------ | ------------- |
| Aggregates all requirements | Partly            | No                   | Yes                | Partly        |
| Distinguishes invalid input | No                | No                   | Yes                | No            |
| Single evaluation           | Yes               | No                   | Yes                | Unknown       |
| Nested types without bounds | No                | No                   | Opt-in only        | No            |
| Uses authoritative metadata | No                | No                   | Yes                | No            |

_Table 1: Comparison of missing-requirement discovery strategies._

## Decision outcome

### Runtime module and public surface

Runtime schema and collection code live in `ortho_config/src/required_values/`.
Presentation lives with the error payload under `ortho_config/src/error/`. Both
are reachable from downstream macro expansion through the crate root and the
existing error facade.

```rust
pub trait RequiredConfigSchema {
    fn collect_required_schema(
        stack: &mut RequiredSchemaStack,
    ) -> Result<Vec<SchemaField>, RequiredSchemaError>;

    fn required_route_hints() -> RouteHints {
        RouteHints::default()
    }
}

pub struct RequiredSchemaStack { /* type-name recursion stack */ }

impl RequiredSchemaStack {
    pub fn new() -> Self;
    pub fn enter<T: ?Sized>(&mut self) -> Result<(), RequiredSchemaError>;
    pub fn leave<T: ?Sized>(&mut self);
}

pub struct SchemaField {
    pub key: String,
    pub aliases: Vec<String>,
    pub activation: Activation,
    pub children: SchemaChildren,
}

pub enum Activation {
    Required,
    Optional,
}

pub enum SchemaChildren {
    Leaf,
    Nested(Vec<SchemaField>),
}

pub struct RouteHints {
    pub prefix: Option<String>,
    pub cli_flags: Vec<CliFlagHint>,
}

pub struct CliFlagHint {
    pub key: String,
    pub flag: String,
    pub takes_value: bool,
}

pub fn ensure_no_missing_required_values<T: RequiredConfigSchema>(
    value: &serde_json::Value,
    program: &str,
) -> OrthoResult<()>;

pub fn program_label_from_args(args: &[std::ffi::OsString]) -> Option<String>;
```

The `OrthoConfig` derive emits `RequiredConfigSchema`. A schema-only derive of
the same trait lets a child type participate without deriving a stand-alone CLI
or implementing `OrthoConfig`. The schema-only derive emits nothing when the
same item also derives `OrthoConfig`, so the two cannot conflict.

### Requiredness matrix

A field is required unless its declared shape or attributes guarantee a value:

| Declared shape or attribute                   | Requiredness  |
| --------------------------------------------- | ------------- |
| `Option<T>` (one outer layer)                 | Optional      |
| `#[ortho_config(default = ...)]`              | Optional      |
| Inferred clap default (`default_value`, `_t`) | Optional      |
| `#[serde(default)]` on the field              | Optional      |
| `#[serde(default)]` on the container          | Optional      |
| `#[serde(skip)]` or `skip_deserializing`      | Not inspected |
| `Vec<T>`, `BTreeMap<K, V>`, `HashMap<K, V>`   | Required      |
| Any other scalar, map, or opaque type         | Required      |

Collections are required unless a default supplies them, because the loader
emits an empty collection only through an explicit default or a merge overlay.
The documentation IR's "collections are non-required" rule is descriptive and
does not drive runtime decisions.

A field marked `#[serde(flatten)]` without the nested marker is not inspected:
its keys do not appear under its own name, so any report would name a key the
loader never reads. With both `#[serde(flatten)]` and
`#[ortho_config(nested)]`, the child's fields compose at the current level,
which is exactly Serde's flatten contract.

### Canonical keys and aliases

A canonical key is the field's deserialization name: an undirected
`#[serde(rename)]`, a `rename(deserialize = ...)` direction, a
`rename_all(deserialize = ...)` rule, or the unrawed identifier. Aliases are
`#[serde(alias = ...)]` values. A requirement is present when the object holds
the canonical key or any alias; an accepted alias is never reported as missing.
Serialize-only renames do not move the canonical key, because deserialization
does not accept them.

The generated CLI and defaults structs copy no Serde naming attributes, so
their layers use the identifier. A CLI hint is therefore emitted only when the
canonical key equals the identifier; otherwise the field has no CLI route and
none is advertised.

### Activation and observation

For each schema node the collector compares one merged `Value` with the
declared requirements, in declaration order, recording only activated and
absent observations.

- A present key is never missing. Explicit null and empty values are present
  and are left to Serde's type rules.
- A present object under a nested node is inspected recursively.
- A present non-object under a nested node is invalid, not a collection of
  missing children. It contributes nothing; other requirements are still
  reported, so the complete absence set precedes any invalid-value error.
- An absent optional node deactivates its subtree.
- An absent required nested node activates its required descendants. If it has
  none, the node itself is reported with an object-shaped file route, so an
  absent required object containing only optional children cannot escape the
  report.

The report is the ordered, deduplicated subsequence of activated absent
requirements. It contains no supplied values. Source, discovery, and parse
failures retain precedence because no trustworthy merged value exists.

### Supply routes

Routes are computed for reported entries only, in CLI, environment,
configuration-file order.

- **CLI** only for a top-level requirement whose canonical key is the
  identifier the generated CLI layer emits. The flag and value arity come from
  the generated clap field.
- **Environment** only when no path segment contains a literal dot, because the
  provider splits on `__` and then nests on `.`. The variable is the prefix
  followed by the uppercased segments joined with `__`, matching
  `CsvEnv::prefixed(prefix).uppercase(true).split("__")`, or the unprefixed
  spelling for a raw provider.
- **Configuration file** always, rendered as `key = value` for a leaf or a
  `[table]` header for an object requirement.

The program label comes from the argument list the loader was given. Direct
`merge_from_layers` calls have no argument list and use the configuration type
name instead. Diagnostics never read the ambient process arguments or
environment.

### Error payload and display

`OrthoError::MissingRequiredValues(Box<MissingRequiredValues>)` joins the
`#[non_exhaustive]` enum. `MissingRequiredValues` owns a non-empty ordered list
of
`MissingRequiredValue { path: ConfigPath, routes: Vec<SupplyRoute>,
expects_object: bool }`,
built through a fallible constructor. `ConfigPath` keeps its segments
separately, so a literal dotted key is distinct from a nested path; rendering
quotes a segment that contains a dot or whitespace. Display renders the program
label, the ordered missing paths, and the grouped routes. The initial contract
is English and does not consult a localizer.

A cyclic opt-in schema is a declaration defect, not a user input error. The
collector detects it with a type-identity stack and returns
`RequiredSchemaError::CyclicSchema`; the runtime entry point maps that to
`OrthoError::Validation` under the `required_values_schema` key. Reusing a type
in sibling fields is allowed and does not trip the cycle check.

### Coverage contract

Full enumeration covers top-level fields and nested types that opt in with
`#[ortho_config(nested)]`. It does not cover: fields whose requirement lives
inside an arbitrary custom `Deserialize` implementation, `#[serde(flatten)]`
without the nested marker, enum payloads, or selected-subcommand values merged
through the Figment extraction helpers. Those paths keep their existing error
behaviour. An opaque field can still be reported absent as a direct value;
absent leaves inside a supplied opaque value remain Serde errors.

## Goals and non-goals

- Goals:
  - Report every activated absent requirement in one typed error.
  - Advertise only routes the actual providers accept.
  - Let an agent correct a load in one round when absence is the sole defect.
  - Keep schema ownership explicit and the recursion finite.
- Non-goals:
  - Reflecting over arbitrary Serde types.
  - Replaying defaults or custom deserializers to discover requirements.
  - Translating the diagnostic in this phase.
  - Changing documentation IR, agent context, or policy schemas.

## Consequences

- The derive grows a companion trait and a field marker. Types that opt into
  nested diagnostics must implement or derive the schema trait.
- `finish` performs one linear schema walk before deserialization. The cost is
  proportional to the declared requirements, not to the configuration size.
- The merged `Value` is inspected once, so defaults and custom deserializers
  are not evaluated twice.
- Routes are conservative: when a spelling cannot be shown to work, it is
  omitted rather than guessed.

## Known risks and limitations

- A new field on a nested type that is not marked `nested` silently stays
  outside enumeration; the coverage test pins the supported set.
- A `#[serde(rename)]` field has no CLI route today because the generated CLI
  struct does not reproduce the rename. The diagnostic omits the route rather
  than advertising a broken flag.
- Selected-subcommand merges that finish through Figment extraction cannot
  carry the schema; their missing values keep the existing error shape.
- A cyclic schema aborts diagnostics with a validation error instead of
  enumerating requirements; the alternative would be unbounded traversal.

## Amendments

### 2026-11-15 — selected-subcommand enumeration

[ADR-0025](adr-0025-enumerate-missing-values-for-selected-subcommands.md)
removes the coverage contract's exclusion of selected-subcommand merges. A
selected subcommand now collects through a loader-specific `RouteScope` before
deserialization, so its report names `PREFIX_CMDS_<NAME>_key` variables and
`[cmds.<name>]` file paths — the spellings that loader really reads — instead
of the type's own prefix and file root.

The consequence is a widened bound: the four `load_and_merge_subcommand_for*`
helpers and the matching `SubcmdConfigMerge` methods now require
`RequiredConfigSchema`, which every type deriving `OrthoConfig` already
satisfies.

The coverage contract above and the matching risk bullet are retained as
history. Read both as superseded on this point only; every other limitation
still applies.

### 2026-11-15 — flatten defaults and field rename rules

Two refinements narrow how a field's own attributes are read. Both follow
Serde's source rather than an intuitive reading of its documentation.

**A field default never rescues a flatten.** `#[serde(default)]` on a flattened
field is inert: Serde deserializes the field from the unmatched keys through
`FlatMapDeserializer`, and `extract_collected` applies no default wrapper, so a
child that cannot deserialize still fails. The earlier requiredness rule that a
field default makes a field optional therefore does not extend to a flatten. A
flattened child resolves to absence exactly when its type is `Option`, which
keeps the `optional_subtree` demotion on that case alone.

**Field rename rules are not case conversions.** A `rename_all` entry is
applied to a field by `apply_to_field`, whose table leaves a field name
unchanged for `lowercase` and `snake_case` and, for `PascalCase`, `camelCase`,
`kebab-case` and `SCREAMING-KEBAB-CASE`, transforms only the separators and the
first letter. General-purpose case conversion would rename an identifier Serde
still reads under its declared spelling, so the schema would name a key no
loader consumes. The canonical key follows that table exactly.

## References

- [Execplan: Enumerate missing required configuration values](../execplans/7-3-1-enumerating-missing-required-values.md).
- [Improved error message design](../improved-error-message-design.md).
- [Design document](../design.md).
