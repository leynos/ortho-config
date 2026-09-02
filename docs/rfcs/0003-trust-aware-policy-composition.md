# RFC 0003: Trust-aware policy composition

## Preamble

- **RFC number:** 0003
- **Status:** Proposed
- **Created:** 2026-09-02

## 1. Summary

OrthoConfig needs to distinguish ordinary configuration preference from the
authority to grant a capability. The existing merge order answers which value
wins when defaults, files, environment variables, and command-line interface
(CLI) arguments disagree. It cannot answer whether the source of that value is
authorized to widen a security or resource-policy boundary.

This RFC proposes an opt-in, trust-aware policy composer alongside the existing
declarative merge engine. Applications define authority classes, the partial
order between them, source classifications, authoritative field anchors, and
explicit delegation grants. OrthoConfig supplies reusable monotonic policy
families, enforcement, provenance, and bounded diagnostics.

The governing invariant is:

> A less-authoritative configuration source may narrow an
> operator-established policy envelope, but it may not widen that envelope
> without explicit delegation from a more-authoritative source.

Attempted unauthorized widening fails closed by default. Applications may opt
into clamping at the composition boundary, but every clamp produces a mandatory
structured diagnostic. Silent widening and silent clamping are invalid.

The runtime API lands before derive support. Fields that do not opt into policy
composition retain their existing `append`, `replace`, and `keyed` behaviour.

## 2. Problem

Configuration precedence and policy authority answer different questions. A
later source may reasonably override an earlier preference while remaining
untrusted to grant network, filesystem, process, interpreter, or resource
capabilities.

The distinction matters whenever an application loads a trusted operator
configuration before an automatically discovered project configuration. A
project file may be later in preference order because it should customize the
project. That ordering does not mean the project may add an internal network
endpoint, nominate a workspace executable as a trusted shell, or raise an
operator's resource ceiling.

The existing field strategies cannot express the required invariant:

- `append` accumulates values, so a later allowlist grants new values;
- `replace` permits a later scalar to disable a restriction or raise a limit;
- `keyed` permits a later layer to add capability-bearing keys; and
- ordinary precedence does not distinguish a request from authorization.

Downstream applications can build a second policy engine after loading, but
that duplicates layer semantics, loses precise source provenance, and makes
interactions with generated loaders and derives difficult to reason about.
Netsuke's fetch-policy boundary is the immediate canary, but the defect is
generic.

## 3. Current state and boundaries

### 3.1. Preference composition

`MergeComposer` preserves the established order of defaults, file layers,
environment variables, and CLI input. `MergeLayer` records the broad
`MergeProvenance` category and an optional file path. Generated
`DeclarativeMerge` implementations apply scalar replacement, optional-value
rules, vector accumulation or replacement, and keyed or wholesale map
replacement.

These are preference operations. Their names describe collection mechanics, not
security semantics. A `Vec<T>` can represent an ordered search path, a set of
requested features, a set of trusted interpreters, or a blocklist. Its Rust
shape alone cannot determine safe composition.

### 3.2. RFC 0002 scope and origin

[RFC 0002](0002-config-layer-resolution-policy.md) proposes ordered explicit
selectors, fail-closed file selection, automatic scope stacking, and replayable
file-layer outcomes. Its scope and origin information is necessary input to
this proposal, but its ordering remains preference ordering. In particular, RFC
0002 deliberately permits project-scope layers to follow user-scope layers and
deliberately excludes changes to declarative merge strategies.

Path scope does not confer authority. An explicitly selected file, an
automatically discovered project file, and an inherited parent file may share a
path or a `DiscoveryScope` while having different authority classifications.
The application must classify the resolved source deliberately, and the
classification must remain visible in policy provenance.

### 3.3. Agent-context capability metadata

Roadmap task 7.2.7 and
[the agent-native design](../agent-native-cli-design.md#69-capability-and-provenance-metadata)
describe capability and provider-provenance metadata for documentation, agent
context, and policy linting. They leave provider selection, execution, and
safety harnesses application-owned.

This RFC does not move execution into OrthoConfig. It adds a runtime
composition contract for configuration fields that constrain execution. The
metadata surface and the runtime policy surface are complementary, not
substitutes.

## 4. Goals and non-goals

- Goals:
  - Model preference order and policy authority as orthogonal dimensions.
  - Let applications define authority classes and relationships without
    hard-coding trust meanings for project, user, environment, or CLI layers.
  - Provide reusable monotonic policy families for capabilities, denials,
    resource limits, required protections, permissions, and keyed capability
    maps.
  - Allow a lower-authority source to narrow policy normally.
  - Require an explicit, typed, bounded grant before a lower-authority source
    can widen policy.
  - Preserve source, authority, constraint, and delegation provenance for
    safe, actionable diagnostics.
  - Provide a runtime API before stabilizing derive syntax.
  - Preserve existing merge behaviour for fields that do not opt in.
- Non-goals:
  - Assigning universal trust levels to defaults, files, environment
    variables, CLI arguments, or discovery scopes.
  - Replacing application authorization, sandboxing, provider selection,
    process supervision, network enforcement, or filesystem enforcement.
  - Inferring security semantics from `Vec`, map, number, or boolean types.
  - Treating agent-context capability metadata as runtime authorization.
  - Parsing delegation grants from arbitrary ordinary configuration keys.
  - Changing RFC 0002's file-selection or scope-stacking rules.
  - Changing `append`, `replace`, or `keyed` semantics for unprotected fields.

## 5. Terminology and invariants

### 5.1. Terms

**Preference order** is the existing order in which configuration layers are
presented. It resolves ordinary values and resolves competing contributions
within the same authority class.

**Authority class** is an application-defined stable identifier for a group of
sources with equivalent widening rights.

**Authority order** is an application-defined partial order over authority
classes. A class may dominate, equal, be subordinate to, or be incomparable
with another class.

**Policy family** defines a typed permissiveness relation and a restrictive
composition operation for one semantic kind of policy.

**Policy envelope** is the effective set of capabilities or limits permitted
after active policy constraints are composed.

**Authority anchor** is the explicit initial envelope for a protected field,
associated with an authority class and provenance. The first ordinary layer
never becomes an anchor merely because it was encountered first.

**Constraint** is an accepted policy contribution from a source. A constraint
can narrow an envelope and can later be superseded by an equally or more
authoritative source.

**Delegation grant** is a typed, field-scoped authorization issued by a source
that may widen a particular boundary. It identifies the delegate, the maximum
delegated envelope, and the issuer provenance.

### 5.2. Core invariants

The implementation must preserve these invariants:

1. Every protected field has an explicit authority anchor.
2. No source acquires authority from its position in preference order.
3. A source may always contribute an equal or more restrictive value.
4. A source may supersede constraints from an equal or subordinate authority
   class.
5. A source may not supersede a dominant or incomparable constraint without a
   matching delegation grant.
6. A delegation grant cannot authorize a value wider than its own bound.
7. A delegated source cannot manufacture, broaden, or retarget its grant
   through an ordinary configuration key.
8. Every rejection or clamp is observable and identifies the decisive policy
   relationship without revealing protected values.
9. Composition is deterministic for the same ordered inputs, authority graph,
   anchors, grants, and enforcement mode.

The partial order matters. Two operator domains may be intentionally
incomparable; neither silently overrides the other. Their active constraints
compose restrictively until a class that dominates both contributes a new
policy.

## 6. Proposed design

### 6.1. Application-defined authority graph

The application constructs an immutable `AuthorityPolicy` before protected
fields are composed. The names below illustrate the intended surface rather
than freezing the public API.

```rust,no_run
use ortho_config::policy::{AuthorityClass, AuthorityPolicy};

let operator = AuthorityClass::new("operator")?;
let delegated_project = AuthorityClass::new("delegated-project")?;
let project = AuthorityClass::new("project")?;

let authority = AuthorityPolicy::builder()
    .class(operator.clone())
    .class(delegated_project.clone())
    .class(project.clone())
    .dominates(&operator, &delegated_project)
    .dominates(&delegated_project, &project)
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`AuthorityPolicy::build` validates unique stable identifiers, referenced
classes, and an acyclic graph. Reflexive equality and transitive dominance are
derived. The graph is data that can be summarized and inspected; it is not an
opaque comparison callback.

No built-in class names carry semantics. An application may classify a CLI
layer as operator-authoritative, subordinate, or incomparable with file
configuration. Another application may make the opposite choice.

### 6.2. Source identity and deliberate classification

Trust-aware composition adds a sidecar `PolicySource` to a `MergeLayer` rather
than changing the meaning of `MergeProvenance`. The sidecar contains:

- the resolved authority class;
- a stable, non-secret source identifier;
- the broad `MergeProvenance` kind;
- RFC 0002 selection kind and scope when available;
- a disclosure-controlled source label for diagnostics; and
- provider-specific provenance for explicitly registered custom sources.

Applications classify known source shapes through inspectable rules. The
matching vocabulary must distinguish at least:

- defaults;
- explicitly selected files;
- automatically discovered files by scope;
- inherited files and their resolved child selection;
- environment layers;
- CLI layers; and
- named custom providers.

A missing or ambiguous classification is a `PolicySourceUnclassified` error
when the layer contributes to a protected field. Unprotected fields continue to
merge normally. There is no fallback from a path, scope, or `MergeProvenance`
value to an implied authority class.

An application may attach a class directly when pushing a custom layer. This
escape hatch remains inspectable because the resolved class and source
identifier are retained in the resulting provenance.

### 6.3. Authority anchors

Every protected field requires an anchor value, authority class, and source
provenance. An anchor may come from an application-owned safe default or from a
classified operator source, but the application must nominate it explicitly.

This prevents an automatically discovered project file from becoming the
granting authority merely because no operator file was present. If no anchor
can be established, composition fails with `PolicyAnchorMissing` before an
ordinary contribution is accepted.

An anchor is a policy value, not necessarily the configuration type's Rust
`Default`. Applications can therefore use a conservative security baseline
without changing ergonomic defaults for unrelated fields.

### 6.4. Policy-family contract

A policy family defines a partial order called “no more permissive than” and a
restrictive reducer. A custom family must expose stable descriptive metadata
and implement an interface equivalent to:

```rust,no_run
pub trait PolicyFamily {
    type Value;
    type Summary;

    const FAMILY_ID: &'static str;

    fn relation(requested: &Self::Value, envelope: &Self::Value)
        -> PolicyRelation;
    fn restrict(left: &Self::Value, right: &Self::Value) -> Self::Value;
    fn summarize(value: &Self::Value, exposure: ValueExposure)
        -> Self::Summary;
    fn descriptor() -> PolicyDescriptor;
}
```

`PolicyRelation` distinguishes `Equal`, `Narrower`, `Wider`, and `Mixed`.
`Mixed` means that one request simultaneously narrows one part and widens
another, such as replacing `{a}` with `{b}` in an allowset. A mixed request is
treated as a widening attempt because accepting it would grant something
outside the current envelope.

`restrict` must be associative, commutative, and idempotent. It must return a
value no more permissive than either input. Implementations must be
deterministic and must not receive source provenance, clocks, randomness,
environment access, or other authority-bearing context. Property tests are
required for these laws when a custom family is implemented.

`PolicyDescriptor` exposes the family identifier, relation vocabulary, summary
schema, and reducer kind. Custom families are therefore discoverable by human
and JSON consumers rather than opaque callbacks.

### 6.5. Built-in monotonic policy families

The first release supplies the following semantic families. Public names must
describe the policy meaning rather than only the collection shape.

| Policy family        | Restrictive composition                        | Typical fields                                            |
| -------------------- | ---------------------------------------------- | --------------------------------------------------------- |
| Allowed capabilities | intersection                                   | hosts, schemes, interpreters, executable roots, providers |
| Denied capabilities  | union                                          | blocked hosts, forbidden paths, disabled providers        |
| Maximum limit        | minimum                                        | bytes, timeouts, redirects, recursion, process counts     |
| Minimum requirement  | maximum                                        | protocol versions, protection levels, validation strength |
| Permission           | logical AND                                    | network access, process spawning, filesystem writes       |
| Required protection  | logical OR                                     | HTTPS required, sandbox required, signature required      |
| Keyed capabilities   | key intersection plus a declared value reducer | provider or capability parameter maps                     |

_Table 1: Minimum built-in monotonic policy families._

The boolean families remain separate despite using boolean values. `false`
means “more restrictive” for a permission, while `true` means “more
restrictive” for a required protection. A generic boolean merge name would hide
that distinction.

Allowed and denied capabilities use mathematical set semantics. Their runtime
representation may accept `BTreeSet`, a deduplicated vector, or a domain
newtype, but ordering and duplicate behaviour do not alter the policy algebra.

### 6.6. Keyed capability maps

A keyed capability map keeps only keys present in both constraints. Each shared
key is composed through a declared value reducer that itself satisfies the
policy-family contract. A lower-authority source cannot create a new key, and
omitting a key narrows the effective capability set.

For example, an operator may authorize two providers with separate maximum
budgets. A project may retain one provider and reduce its budget. It may not
add a third provider or raise a retained provider's budget. A map without a
declared value reducer is not eligible for keyed policy composition.

### 6.7. Composition algorithm

Protected fields are evaluated from the same ordered layers used for ordinary
merging, but they maintain an active set of authority-bearing constraints. For
each incoming contribution:

1. Resolve its `PolicySource` and authority class.
2. Compare the requested value with the current effective envelope.
3. If the request is equal or narrower, accept it as an additional active
   constraint and recompute the restrictive result.
4. If the request is wider or mixed, identify each active constraint it would
   relax.
5. Mark a relaxed constraint as authorized when the incoming class equals or
   dominates its class, or when a matching delegation grant authorizes the
   requested value.
6. If every relaxed constraint is authorized, supersede those constraints,
   retain dominant or incomparable constraints not being relaxed, accept the
   incoming contribution, and recompute the result.
7. If any relaxed constraint is unauthorized, apply the configured widening
   response from section 6.9.

Preference order therefore retains a narrow role: a later contribution from the
same authority class supersedes the earlier contribution. A dominant later
source can also widen a subordinate constraint. A later subordinate source can
only narrow unless it holds a valid delegation.

Tracking active constraints rather than only the last effective value is
necessary for diagnostics and incomparable authorities. It also lets a later
dominant source supersede a project restriction deliberately without erasing an
independent operator domain's constraint.

### 6.8. Explicit delegation

A delegation grant is separate from ordinary field data. A shape equivalent to
`DelegationGrant<P>` contains:

- a stable delegation identifier;
- the protected field identifier and policy-family identifier;
- issuer source and authority provenance;
- the delegate authority class or a narrower source matcher;
- a typed maximum delegated envelope; and
- optional application-defined scope metadata that does not affect unrelated
  fields.

The composer validates that the issuer strictly dominates the delegate, equals
or dominates every constraint the grant may relax, and does not grant a bound
wider than the issuer's own envelope. A request may use a grant only when its
source matches the delegate, its field and family match exactly, and its
requested value is no more permissive than the grant bound.

Grants enter through the application-constructed policy plan or a dedicated
issuance API tied to an already classified authoritative source. The composer
never discovers a grant by deserializing another key from the delegated layer.
An application may define an operator-facing file syntax for grants, but it
must parse that syntax from the authoritative source and pass typed grants to
the policy plan before composing delegated layers.

Delegation use is single-field unless the issuer creates separate grants for a
declared capability family. Wildcard field paths and unbounded “same authority
as issuer” grants are excluded from the initial design.

### 6.9. Rejection and clamping

`WideningResponse::Reject` is the default and the recommended security posture.
An unauthorized wider or mixed request contributes a semantic policy error,
does not produce a configuration value, and retains enough provenance to
identify the boundary that blocked it.

An application may construct its policy plan with `WideningResponse::Clamp`. In
that mode the unauthorized portion is reduced through the family's `restrict`
operation, the effective value remains within the established envelope, and
composition returns a mandatory diagnostic with the successful result. The
clamping choice belongs to the application policy plan; it cannot be selected
by an ordinary configuration key in the source being clamped.

Clamp mode is useful for compatibility migrations where hard failure would be
disruptive, but it cannot be silent. A caller must explicitly drain or return
the accompanying `PolicyReport`; APIs that discard a non-empty report are not
provided.

### 6.10. Provenance and bounded diagnostics

Every decision record retains:

- the stable protected-field identifier and policy family;
- the decision: accepted narrowing, accepted by authority, accepted by
  delegation, rejected, or clamped;
- the requesting source identifier and authority class;
- the decisive boundary source and authority class;
- the relationship between requested, boundary, and effective values;
- the delegation identifier and issuer when used; and
- disclosure-safe summaries of the requested and effective policy.

Human and JSON diagnostics use this same typed record. Diagnostics do not log
raw allowset members, denyset members, paths, provider parameters, interpreter
names, or custom-policy values by default. Built-in summaries expose relation
kind and bounded cardinalities. Public scalar limits may be included only when
the field's `ValueExposure` permits them. Secret fields force opaque summaries
regardless of application preference.

Composition limits detailed violations per field and per run, records the
number omitted, and uses low-cardinality decision and family identifiers for
metrics. Source path disclosure follows an application setting; the default
uses a stable source label such as “automatic project file” rather than an
absolute path.

A rejected diagnostic should read conceptually as:

```plaintext
policy widening rejected for max_render_bytes: automatic project file
(authority project) requested a wider maximum than operator configuration
(authority operator); effective policy remains bounded by the operator source
```

The diagnostic communicates the relationship without printing the requested or
operator values.

### 6.11. Runtime API

The runtime surface composes classified layers, field policies, anchors, and
grants into a value plus a mandatory report channel. One possible shape is:

```rust,no_run
let plan = PolicyPlan::builder(authority)
    .field(
        PolicyField::new("allowed_shells", AllowedCapabilities::<ShellName>::new())
            .anchor(operator_source, operator_shells),
    )
    .field(
        PolicyField::new("max_render_bytes", MaximumLimit::<u64>::new())
            .anchor(operator_source, 8 * 1024 * 1024),
    )
    .widening_response(WideningResponse::Reject)
    .build()?;

let outcome = PolicyComposer::new(&plan).compose(layers)?;
let (config, report) = outcome.into_parts();
# Ok::<(), Box<dyn std::error::Error>>(())
```

The exact ownership split between `PolicyPlan`, `PolicyComposer`, and the
existing `MergeComposer` remains an implementation-level naming decision. The
required boundary is stable: ordinary and protected fields may share input
layers, protected fields receive sidecar authority metadata, and policy
outcomes cannot discard diagnostics accidentally.

The runtime API must also permit applications to compose one domain policy
outside a full derived configuration. This is necessary for downstream canary
tests and for applications that adopt the policy engine before derive support.

### 6.12. Derive surface

Derive support follows only after the runtime algebra, provenance records, and
diagnostics have downstream experience. Candidate syntax includes:

```rust,no_run
#[ortho_config(policy = "allowed_capabilities")]
allowed_shells: Vec<ShellName>,

#[ortho_config(policy = "maximum_limit")]
max_render_bytes: u64,

#[ortho_config(policy = "required_protection")]
require_https: bool,
```

The spellings are not approved by this RFC. Before stabilization, the derive
design must decide how anchors, domain newtypes, keyed value reducers, source
classification requirements, and custom family identifiers are expressed
without hiding runtime semantics.

The derive must reject contradictory `merge_strategy` and `policy` attributes
on the same field. Policy families own protected-field composition; ordinary
collection strategies do not run first or afterwards on that field.

Generated metadata records the semantic family identifier, not only the Rust
collection type. Existing derives with no policy attributes generate byte-for-
byte equivalent merge behaviour.

### 6.13. Custom policies

Applications may implement a custom policy family for domain values when the
built-ins are insufficient. A custom family must:

- publish a stable family identifier and descriptor;
- define its permissiveness relation and restrictive reducer;
- satisfy deterministic, associative, commutative, and idempotent laws;
- provide disclosure-safe bounded summaries;
- use the common decision and provenance pipeline; and
- pass the same authority and delegation checks as a built-in family.

The interface does not receive a callback that can inspect paths, authority
classes, or unrelated configuration. Those inputs belong to the generic
composer. This keeps custom policy algebra reviewable and prevents a callback
from silently creating its own trust model.

## 7. Proof cases

### 7.1. Network access

An operator anchor permits only `https` access to `downloads.example.org`. An
automatic project file may intersect the allowset, add denied hosts, lower
redirect and timeout ceilings, or require stronger transport protection. It
cannot add another host, scheme, metadata endpoint, or internal service without
a matching delegation grant.

This is the generic boundary needed by
[leynos/netsuke#644](https://github.com/leynos/netsuke/issues/644).

### 7.2. Trusted interpreters and shells

An operator anchor permits the built-in `sh` and `bash` providers. Project
configuration may request only `sh`. It cannot add a workspace executable, an
arbitrary absolute path, or a newly configured interpreter to the trusted set
merely because the project layer follows the operator layer.

The policy uses semantic shell or interpreter newtypes rather than relying on a
plain vector. It composes with Netsuke's structured-command shell selection from
[leynos/netsuke#638](https://github.com/leynos/netsuke/issues/638):
OrthoConfig determines the effective trusted envelope, while Netsuke owns
command parsing, provider selection, process execution, and sandboxing.

### 7.3. Executable roots and provider allowlists

An operator may authorize executables beneath fixed roots and a bounded set of
providers. Project configuration can remove roots or providers. A keyed
provider map may reduce parameters through its declared value reducer. The
project cannot introduce a provider key, add an executable root, or relax a
per-provider restriction.

### 7.4. Filesystem and network capabilities

Permission booleans compose with logical AND, while required-protection
booleans compose with logical OR. A project can disable network access or file
writes and can require HTTPS or sandboxing. It cannot enable a permission the
operator disabled or remove a protection the operator required.

### 7.5. Resource ceilings

An operator sets an 8 MiB render-output maximum. Project configuration may
reduce it to 1 MiB but may not raise it to 64 MiB. The same maximum-limit
family covers timeouts, recursion depth, redirect counts, file-read budgets,
process counts, and other upper bounds. Minimum requirements use the inverse
ordering and the maximum reducer.

## 8. Requirements

### 8.1. Functional requirements

- Applications define authority classes, their partial order, and source
  classifications.
- Every protected field has an explicit authoritative anchor.
- Lower-authority sources can narrow every built-in policy family.
- Unauthorized widening is rejected by default.
- Explicit clamp mode produces a mandatory diagnostic and a restrictive value.
- Delegation is typed, bounded, field- or family-scoped, and source-scoped.
- The delegated layer cannot obtain a grant from an ordinary configuration
  key.
- Runtime composition supports all policy families in table 1.
- Trusted shells and interpreters are covered as a first-class canary.
- Human and JSON consumers can inspect safe policy decisions and summaries.

### 8.2. Technical requirements

- Authority validation rejects cycles, unknown classes, invalid issuers, and
  ambiguous source classifications.
- Incomparable authorities compose restrictively.
- Custom reducers are deterministic and expose inspectable descriptors.
- Diagnostics are bounded and secret-safe by default.
- The runtime API precedes derive support.
- Existing merge APIs remain source-compatible for unprotected fields.
- Existing `append`, `replace`, and `keyed` fixtures remain unchanged when no
  policy metadata is present.
- Property tests cover policy-family algebra and authority-graph invariants.
- End-to-end tests cover runtime layers, generated layers, explicit selectors,
  automatic scopes, environment, CLI, custom providers, and delegation.

## 9. Compatibility and migration

### 9.1. Opt-in compatibility

This proposal is additive. A field without policy metadata continues through
the existing declarative merge path. No existing source category receives an
authority class unless an application adopts a `PolicyPlan` for protected
fields.

`MergeProvenance` retains its current broad categories. Policy source metadata
is a sidecar so current constructors, matches, and diagnostics do not acquire a
new trust meaning. RFC 0002 origin records feed the sidecar when available.

### 9.2. Adoption sequence

Applications migrate one protected domain at a time:

1. Define domain newtypes and identify protected fields.
2. Define authority classes, relationships, source classifications, and
   explicit anchors.
3. Use the runtime API with rejection mode and inspect policy reports.
4. Add explicit delegation only for demonstrated widening use cases.
5. Remove equivalent downstream post-merge policy code after canary parity is
   proven.
6. Adopt derive metadata after the runtime contract stabilizes.

Applications that need a non-disruptive observation period may use explicit
clamp mode. They must surface the report and must not describe clamp mode as
enforcement parity with rejection mode.

### 9.3. Versioning surface

The following names and wire contracts must be settled before the first public
release:

- stable identifiers for authority classes, policy fields, policy families,
  sources, and delegations;
- authority graph validation and comparison outcomes;
- policy decision and relation enums;
- diagnostic summary and redaction rules;
- default rejection semantics; and
- built-in family descriptors.

Derive attribute strings are deliberately excluded from that first stability
surface. They are staged after runtime experience so an illustrative spelling
does not become a permanent grammar accidentally.

## 10. Alternatives considered

### 10.1. Treat later precedence as authority

This is the current failure mode. It is convenient, but it grants capabilities
to any later source and cannot represent an operator ceiling. It is rejected.

### 10.2. Always apply the restrictive reducer silently

Unconditional intersection, union, minimum, maximum, AND, or OR preserves the
envelope, but silent clamping hides configuration mistakes and attacks. It also
cannot explain deliberate authoritative widening or delegation. It is rejected.

### 10.3. Always reject any syntactic widening

This keeps lower-authority sources safe but also prevents an equal or dominant
source from changing policy deliberately and prevents bounded delegation. It
collapses authority back into one fixed global ceiling. It is rejected.

### 10.4. Hard-code system, user, project, environment, and CLI trust

Trust meanings vary by application and deployment. A CLI may represent a local
operator in one application and untrusted automation in another. Explicit file
selection may or may not elevate a file. Hard-coded levels are rejected.

### 10.5. Leave all policy composition downstream

Applications can enforce policy after ordinary loading, but they must recreate
layer traversal, source classification, delegation, provenance, and redaction.
That makes downstream engines inconsistent and prevents generated merge logic
from participating safely. It is rejected as the default architecture; domain
execution and enforcement still remain downstream.

### 10.6. Use opaque custom callbacks

A callback can encode any policy, but it hides semantics from diagnostics,
metadata, tests, and reviewers. The restricted custom-family trait retains
domain extensibility while keeping policy descriptors and provenance
inspectable. Opaque callbacks are rejected.

| Topic                        | Ordinary precedence | Silent reducer           | Proposed model                        |
| ---------------------------- | ------------------- | ------------------------ | ------------------------------------- |
| Lower source can narrow      | sometimes           | yes                      | yes                                   |
| Lower source can widen       | yes                 | no                       | only by delegation                    |
| Dominant source can widen    | by position only    | no                       | yes                                   |
| Incomparable authorities     | unsupported         | over-restricted silently | restrictive and observable            |
| Delegation                   | unsupported         | unsupported              | typed and bounded                     |
| Diagnostics                  | value conflict only | none                     | source, authority, relation, decision |
| Existing-field compatibility | current             | breaking if universal    | opt-in                                |

_Table 2: Comparison of composition models._

## 11. Risks and limitations

- Authority graphs and source rules add configuration complexity. Builders,
  validation, and safe summaries must make the resolved model inspectable.
- Incorrect application classification can still assign too much authority.
  OrthoConfig can reject omissions and ambiguity, but it cannot infer an
  application's real-world trust boundary.
- Partial orders are more complex than numeric trust levels. They avoid false
  equivalence between independent operator domains and are worth the explicit
  graph.
- Clamp mode can normalize attempted widening into a successful load. Mandatory
  reports and an explicit application-level setting mitigate, but do not erase,
  that operational risk.
- A policy composer constrains configuration; it does not enforce the resulting
  policy against system calls. Downstream applications must still apply the
  effective policy at their execution boundaries.
- Custom policy algebra cannot be proven pure by the Rust type system. A narrow
  trait, law-based property tests, and no authority context reduce the review
  surface.

## 12. Open questions

No question blocks acceptance of the authority model or runtime sequencing. The
following API details remain intentionally deferred until the runtime
implementation has downstream experience:

- the final public names and module layout for policy-plan and composer types;
- the derive attribute grammar;
- whether public scalar limits default to opaque or cardinality-free numeric
  summaries; and
- whether delegation grants need optional expiry metadata in a later release.

Expiry does not belong in the first release because wall-clock evaluation would
complicate deterministic replay. Applications that require expiry can omit an
expired grant when constructing the immutable policy plan.

## 13. Recommendation

Accept the application-defined authority graph, explicit field anchors,
built-in monotonic families, typed bounded delegation, and provenance model in
this RFC. Implement runtime composition first with fail-closed rejection as the
default. Add observable clamp mode as an explicit application choice, validate
the Netsuke proof cases, and only then stabilize derive metadata.

This design preserves OrthoConfig's existing preference semantics while adding
the missing authorization dimension. It lets downstream applications share one
policy algebra without moving their provider selection, sandboxing, or domain
execution into the configuration library.

## 14. Delivery plan

1. **Accept the authority contract.** Validate application-defined partial
   orders, source classification, anchors, and typed delegation through public
   runtime types.
2. **Implement the runtime algebra.** Add the built-in families, policy
   composer, rejection mode, and explicit clamp mode with property and
   behavioural coverage.
3. **Make decisions observable.** Preserve active-constraint and delegation
   provenance and expose bounded human and JSON reports.
4. **Validate downstream canaries.** Prove that Netsuke project configuration
   cannot widen fetch policy, trusted shells or interpreters, or resource
   ceilings, while narrower policy composes normally.
5. **Add derive metadata.** Stabilize semantic attributes only after the
   runtime contract and downstream canaries settle.

The review-sized implementation tasks and their dependencies live in
[roadmap phase 14](../roadmap.md#14-separate-configuration-precedence-from-policy-authority).

## 15. Related work

- [RFC 0002: Customizable configuration layering policy](0002-config-layer-resolution-policy.md)
- [ConfigDiscovery: support multi-scope file loading (all found files, not just first) #318](https://github.com/leynos/ortho-config/issues/318)
- [Enable scoped configuration discovery (#318) #465](https://github.com/leynos/ortho-config/pull/465)
- [Fail closed for explicit configuration selectors #437](https://github.com/leynos/ortho-config/issues/437)
- [Prevent project configuration from widening trusted fetch policy netsuke#644](https://github.com/leynos/netsuke/issues/644)
- [Define structured-command shell selection netsuke#638](https://github.com/leynos/netsuke/issues/638)
- [Separate configuration precedence from policy authority #475](https://github.com/leynos/ortho-config/issues/475)
