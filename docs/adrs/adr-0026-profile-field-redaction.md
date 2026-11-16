# Architectural decision record (ADR) 0026: Profile field redaction

## Status

Accepted.

Date: 2026-11-15.

## Context and problem statement

The documentation IR describes a field's default, its field-level examples, its
CLI possible values, and the members of an enum value type. Those slots are
derived from Rust default expressions, examples, and value enumerations, and
they can contain credentials, private locators, or other sensitive material.

Roadmap 9.1.2 lets an author mark a field as `secret` or `reference_only` (see
[agent-native CLI design](../agent-native-cli-design.md) §6.7). Both labels
mean the field's value-bearing metadata must never reach a consumer artefact;
only the field's public descriptor and its label survive. The label records
*why* the value is withheld: a secret holds confidential material, while a
reference-only field names material the application resolves elsewhere, and the
locator itself is treated as sensitive.

Without a single owner, the same rule would be reimplemented by the derive, the
`cargo-orthohelp` pipeline, the generated bridge, and each renderer. Any one of
them could retain a default or an enum member, and a cache could serve metadata
produced under the old policy.

## Decision drivers

- One policy implementation, applied at every public conversion boundary.
- Structural redaction of the modelled value slots, not a substring scanner over
  arbitrary text.
- A conservative rule for command-level examples: an unstructured example
  cannot be mapped back to the fields it exercises, so a parent or sibling
  example could disclose a protected child's value.
- Wire stability: additive optional fields and labels, with legacy documents
  reading as ordinary fields.
- Deterministic context output: protected-field records in identity order, and
  a names catalogue independent of profile bodies.
- A cache identity that changes when the disclosure policy changes.

## Options considered

### Option A: One structural projection in the shared schema (accepted)

`ortho_config::docs` owns `DocMetadata`, `FieldMetadata`, `ValueType`, and the
new `ProfileFieldExposure`. `DocMetadata::redacted_for_output()` returns a copy
with protected values removed, and every boundary applies it.

### Option B: Filter in each renderer

Rejected. Four renderers and two serializers would each need the rule, a new
renderer could omit it, and the agent-context bridge and cache would still see
unredacted metadata.

### Option C: Omit protected fields entirely

Rejected. The field's name, help identifiers, CLI spelling, requiredness, and
value-type shape are public invocation-contract data. Removing the field would
make an invocable flag disappear from the contract and would hide the fact that
it exists.

### Option D: Scrub known secret strings from every string slot

Rejected. Metadata has no trustworthy mapping from an example or help sentence
back to a field, substring replacement corrupts public text that happens to
match, and the set of secret strings is not available at projection time.

| Topic                   | A: shared projection | B: per renderer | C: omit field | D: scrub strings |
| ----------------------- | -------------------- | --------------- | ------------- | ---------------- |
| Single policy owner     | Yes                  | No              | Yes           | No               |
| Keeps public descriptor | Yes                  | Yes             | No            | Yes              |
| No false positives      | Yes                  | Yes             | Yes           | No               |
| Covers cache and bridge | Yes                  | No              | Yes           | No               |
| Bounded traversal       | Yes                  | Per renderer    | Yes           | Unbounded        |

*Table 1: Comparison of disclosure-policy placement options.*

## Decision outcome / proposed direction

### Attribute surface

A field-level `#[ortho_config(profile_field = "secret")]` or
`#[ortho_config(profile_field = "reference_only")]`. The labels are
case-sensitive `snake_case` strings. An unknown label, a duplicate annotation,
or an annotation on a flattened container is a span-local compile error. A
flattened container is rejected because its emitted child fields have their own
names, so a label on the container cannot be mapped to them; authors mark the
leaf fields on the child type instead.

### Projection contract

On a protected field, `redacted_for_output` clears the default, the field-level
examples, the CLI `possible_values`, and every nested `ValueType::Enum` member,
recursively through `List` and `Map`. It preserves the field name, help
identifiers, long-help identifier, CLI spelling and flags, environment
variable, file key path, requiredness, deprecation metadata, links, notes, the
structural value type, and the exposure label itself.

When any field anywhere in the tree is protected, every command-level example
in the tree is removed as well. When no field is protected, command examples
and ordinary field slots are preserved unchanged. The projection is idempotent
and performs exactly two linear traversals: one to decide whether any protected
field exists, then one projection pass over the whole tree.

### Boundaries

- The derive emits metadata that has already been projected.
- The `cargo-orthohelp` pipeline projects deserialized IR before rendering, so a
  handwritten or legacy JSON document cannot bypass the policy.
- The generated bridge projects before writing IR JSON.
- The IR-to-agent-context bridge projects again at its public boundary and then
  derives `ProfileFieldRedaction` records from the projected tree.
- The agent-context cache identity includes the literal salt
  `profile-redaction-v1`, so entries produced under a different policy are not
  reused.

### Schema ownership

`cargo-orthohelp::schema` re-exports the types from `ortho_config::docs` rather
than maintaining a handwritten mirror. This removes the drift risk recorded in
ADR-0016 and gives the projection and its consumers one type identity.

### Agent context

`ProfilesDeclaration` keeps its `redaction` and `available_names` metadata
private and validates it through constructors and deserialization:

- `ProfileFieldRedaction` carries `command_path`, `field_name`, the optional
  file `key_path`, and `exposure`. Its identity is
  `(command_path, field_name)`; duplicate identities are rejected rather than
  silently choosing one label.
- An unsupported declaration cannot carry redaction records or names, and
  serialization refuses a declaration whose public `supported` flag was flipped
  to `false` after metadata was attached.
- `available_names` accepts validated `ProfileName` values only. It is sorted,
  deduplicated, and carries no body data, so the names catalogue cannot leak
  profile contents by construction.
- Metadata is omitted from the wire when absent. The legacy unsupported shape
  remains exactly `{"supported": false}`.

### Wire representation

`FieldMetadata::profile_field` is an optional `snake_case` label, omitted when
absent. Deserializing an unknown label fails; it never degrades to an ordinary
field. `ProfileFieldRedaction` uses the same labels. Defaults on existing
fields make legacy IR and agent context read as ordinary metadata.

## Goals and non-goals

- Goals:
  - Let authors classify a field as secret or reference-only.
  - Remove protected value slots from every generated artefact, the agent
    context, the bridge output, and cached metadata.
  - Keep protected-field identity and public descriptors available to consumers.
  - Expose validated profile names independently of profile bodies.
- Non-goals:
  - Scan arbitrary text for confidential strings.
  - Protect values an author embeds deliberately in help text, names, links,
    notes, or custom type names.
  - Implement a profile store or resolve profile bodies.
  - Add a renderer badge or a `--show-sensitive` bypass.

## Migration plan

This is a deliberate pre-1.0 Rust source break, consistent with ADR-0016.

- Every `FieldMetadata` literal gains `profile_field: None`. Derive emission,
  fixtures, tests, and doctests are updated in the same atomic change.
- Existing IR and agent-context JSON deserialize unchanged because the new field
  and records default to absent.
- The cache salt invalidates entries produced before the policy existed; no
  migration of cached artefacts is attempted.
- Milestone M1 lands the schema consolidation, projection, parser, emission,
  bridge records, and both output paths together. Reverting M1 means reverting
  the whole commit, never only one boundary.

## Known risks and limitations

- The contract is structural. Authors must not put sensitive material into
  public descriptors such as help identifiers or custom type names.
- Global command-example suppression is conservative and may remove useful
  public examples from a tree that contains one protected field.
- The projection removes the modelled slots only. Unmodelled future value slots
  need an explicit projection change and a corresponding model update.
- Formal evidence covers a model of the recursive projection. Correspondence
  between the model and the production traversal is established by differential
  property tests and output integration tests, not by a verified binary. The
  developers' guide records that residual testing boundary.

## Outstanding decisions

- None. Proof obligations and CI integration are carried by the 9.1.2 ExecPlan;
  profile-store helpers remain roadmap 9.1.3 work.

## Architectural rationale

A single projection in the schema owner, applied at every public boundary,
makes the disclosure rule impossible to forget in a new renderer and keeps the
cache, bridge, and agent context consistent. The structural rule is auditable:
each modelled value slot is either cleared or preserved, and the global example
rule has one decision point per document. Retaining public descriptors keeps
the invocation contract complete while withholding the values that motivated
the classification.

## References

- [Agent-native CLI assistance design](../agent-native-cli-design.md) §6.7.
- [cargo-orthohelp design](../cargo-orthohelp-design.md) §2.2, §6.3.
- [ADR-0003: Define schema ownership for agent-native contracts](adr-0003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-0012: Profile selection and layering](adr-0012-profile-selection-and-layering.md).
- [ADR-0016: Dual-renderer metadata surface](adr-0016-dual-renderer-metadata-surface.md).
- [Execplan: Profile redaction metadata](../execplans/9-1-2-profile-redaction-metadata.md).
