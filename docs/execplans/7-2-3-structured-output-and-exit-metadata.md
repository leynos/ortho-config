# Model structured output and exit metadata (7.2.3)

## Status

APPROVED — the user approved this plan on 2026-11-14 and directed
implementation to proceed. The M1–M3 milestones below are authorized. The
temporary #664 prerequisite merged on 2026-11-11 as `eaf4c028`, and the branch
was rebased onto `origin/main` at `959dbd7e` before implementation began.

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone.

## Purpose / big picture

A command author will explicitly describe JSON availability, the purpose of
stdout, whether stderr carries diagnostics, and stable exit-class names. Those
declarations will survive derive expansion, documentation intermediate
representation (IR), the bridge, and compact agent-context generation. A
reusable policy check will identify data-returning commands without declared
JSON support. Users of `cargo-orthohelp` will have a tested description of its
existing process statuses.

Success is observable by deriving metadata for a nested fixture command,
generating its agent context, and checking the output contract in both the JSON
file and stdout. Calling the new policy API on that context will produce one
stable finding for each explicitly data-returning command whose JSON support is
false or undeclared, unless enforcement is off.

## Conformance basis

The original design baseline is commit
`ddba3578fae04227e426ea3a61aa2b24aa043232`. There is no separate task-specific
terms-of-reference document. The governing requirements are
[roadmap 7.2.3](../roadmap.md), the user's planning request, and
[agent-native CLI design](../agent-native-cli-design.md) §§6.2, 6.2.2, 8.1 and
8.2. Roadmap 7.2.1 and 7.2.2 are complete at this baseline.

The implementation prerequisite is temporarily
`issue-644-nested-cargo-timeouts` at
`8309e3b41287f2fb9b70016194b8bfe2235db4fd`, tracked by
[PR #664](https://github.com/leynos/ortho-config/pull/664). The user identified
its nested Cargo timeout fix as necessary to implement this plan on current
`main`. On 2026-11-09 the plan branch was rebased onto that exact parent and
[PR #661](https://github.com/leynos/ortho-config/pull/661) was prepared for
retargeting to it. This changes the execution prerequisite, not the feature
scope or approval status.

Once #664 merges, the implementing agent may rebase this branch back onto
freshly fetched `origin/main` and retarget #661 to `main`. Use the `rebase`
skill: first verify #664's merged state and landing commit, preserve the
current head, and identify the exclusive inherited parent boundary. The
recorded parent above is the initial boundary receipt, not permission to ignore
later parent changes. Replay only this branch's plan/implementation commits; do
not replay the timeout-fix commits after a squash merge. Audit with
`git range-diff`, rerun the applicable gates, lease-push against the observed
remote head, refresh tracking, and verify the PR base and head.

That procedure completed on 2026-11-14. #664 merged as `eaf4c028`; its final
head was `91750238`, which advanced linearly past the recorded `8309e3b4`
boundary, so `91750238` was not inherited and `git merge-base --all` returned
`8309e3b4` alone. Recovery refs
`refs/recovery/7-2-3-{old-base,old-head,target}` retain the pre-rebase
identities. Only the two planning commits were replayed onto `959dbd7e`;
`git range-diff` reports both unchanged.

The traceability chain is:

- R1: explicit JSON, stdout, stderr, and exit classifications → M1 → E1,
  derive/IR/mirror/context round trips and legacy fixtures.
- R2: lint data-returning commands without structured output → M2 → E2,
  complete rule truth table, generated-input properties, and BDD scenarios.
- R3: document stable `cargo-orthohelp` exit classes → M3 → E3, actual-process
  status/stream tests and the users' guide.
- R4: preserve schema ownership, no inference, and stream ownership → all
  milestones → E4, schema parity, nested command isolation, and existing CLI
  contract regression suites.

[ADR-0003](../adrs/adr-0003-define-schema-ownership-for-agent-native-contracts.md)
assigns independent schema ownership;
[ADR-0010](../adrs/adr-0010-behavioural-metadata-attribute-surface.md) requires
sibling attribute groups;
[ADR-0016](../adrs/adr-0016-dual-renderer-metadata-surface.md) defines renderer
semantics and additive wire evolution;
[ADR-0008](../adrs/adr-0008-opt-in-agent-native-policy-configuration.md)
preserves policy defaults and the bridge-free CLI check.
[ADR-0017](../adrs/adr-0017-consumer-side-prover-tooling.md) defines the
available proof tooling without claiming that its fixture proves application
logic.

Use [documentation contents](../contents.md) and
[repository layout](../repository-layout.md) to locate owned interfaces.
Required supporting references and their roles are:

- [Core design](../design.md) and
  [cargo-orthohelp design](../cargo-orthohelp-design.md): metadata ownership,
  bridge mirror, generation and independent schema versions.
- [CLI localization](../cli-localization-design.md) and
  [Fluent library design](../localizable-rust-libraries-with-fluent.md):
  application-owned locale selection; protocol identifiers never localize.
- [Clap dispatch integration](../clap-dispatch-and-ortho-config-integration.md):
  preserve the Cargo external-subcommand invocation boundary.
- [Improved errors](../improved-error-message-design.md): proposed diagnostics
  are not implemented runtime guarantees and belong to later roadmap work.
- [rstest fixtures](../rust-testing-with-rstest-fixtures.md),
  [doctest guide](../rust-doctest-dry-guide.md), [dependency injection](../reliable-testing-in-rust-via-dependency-injection.md),
  and [rstest-bdd guide](../rstest-bdd-users-guide.md): test ownership,
  fallible fixtures and executable consumer contracts. The requested
  `rtest-bdd-users-guide.md` does not exist; the linked filename is correct.
- [Complexity guidance](../complexity-antipatterns-and-refactoring-strategies.md)
  and [scripting standards](../scripting-standards.md): small focused modules
  and maintainable tooling. No new script is planned.
- [Documentation style](../documentation-style-guide.md): ADR format,
  British English with Oxford spelling, wrapping and cross-references.

Load `codegraph-mcp`, `rust-router`, and `leta` before implementation
navigation; route to `rust-types-and-apis` for the public declarations and
`rust-unit-testing` / `proptest` for verification. Use `rust-verification` when
the assurance boundary changes, and `verus` only if a substantive proof is
selected. Use `firecrawl-mcp` for external research, `commit-message` for
commits, and `pr-creation` for delivery. The planning review uses
`logisphere-experts`; full gate runs belong to `scrutineer`.

## Constraints

Implement only after explicit plan approval. Keep 7.2.3 unchecked until the
feature is complete. Do not implement general `cargo orthohelp --json`, JSON
framing, subprocess capture, code-to-class mappings, or diagnostic/code
consistency checks here. These are subsequent 7.2.4/7.2.5 work. Do not assign
status 5 to external-tool failures merely because the design gives that
illustrative example.

Preserve the existing `--check-agent-native` bridge-free configuration check.
The new command-metadata lint is a reusable API like `check_behaviour`; it is
not silently wired into a CLI path which currently has no command metadata.
Document this limitation prominently. A CLI integration expansion requires a
separate accepted design decision, not an incidental bridge build.

Do not infer output from command names, mutation effects, flags, locale,
renderer defaults, or actual terminal state. Do not inherit a parent's output
contract into children. Keep `output_modes` unchanged. Metadata is a declared
capability, not proof that a downstream executable implements it.

Preserve existing stdout ownership: policy checks emit the report, even when an
explicit generation format is also requested. Agent context alone emits its
JSON document; policy plus generation suppresses that second stdout document.
Human policy summaries remain on stderr. Preserve all existing exit statuses
and do not introduce a universal application taxonomy.

Keep schemas independent and the local documentation IR mirror synchronized.
Keep code files below 400 lines, use shared Cargo caches, preserve compiler
admission, and never build in `/tmp`. Do not alter Whitaker installation or
host services. Run formatting, lint and tests sequentially. Commit only green
logical units; do not commit deliberately failing red-stage tests.

## Tolerances (exception triggers)

Stop implementation, record the affected R/E identifiers and alternatives, set
status BLOCKED, and obtain explicit direction if work needs a numeric exit
taxonomy, runtime JSON mode, a bridge build in policy-only execution, changed
existing wire fields/defaults, or a changed exit status. Likewise escalate if a
sound implementation needs an unplanned dependency beyond the authorized
testing dependencies, or a schema-breaking change. Optional field additions and
all required source call-site updates are already in scope.

There is no time limit. No arbitrary source-line or file-count budget applies;
ExecPlan files would be excluded from any subsequently agreed size limit.
Repeated failures require diagnosis and recorded evidence, not lint suppression
or weakened acceptance criteria.

## Risks

The highest design risk is confusing output purpose with renderer content. Keep
those axes independent: stdout `data` describes the semantic result; renderer
stdout `structured` describes presentation. Neither fills the other.
Contradictory declarations remain descriptive claims in this milestone;
claim-consistency and byte-level enforcement are explicitly deferred.

A false positive is possible when a command emits another structured format but
has no JSON option. The approved product requirement specifically asks for
`--json`; the rule name and message must say JSON rather than suggesting that
every other machine format is invalid. No inference from a literal `--json`
input is permitted.

Public struct additions require all literal constructors to be updated in one
commit. Search semantic references first and keep mirror parity tests. Do not
create source compatibility aliases or transitional wrappers for these pre-1.0
APIs. Older persisted IR/context JSON is the concrete compatibility case:
omission must deserialize without inventing support.

Exit class identifiers are application-defined strings. Derive validation must
reject malformed declarations, but direct construction/deserialization is not a
runtime verification service. Document this distinction rather than claiming
all possible public values are validated.

## Progress

- [x] (2026-11-08) Rename branch and inspect baseline instructions and roadmap.
- [x] (2026-11-08) Gather Wyvern metadata and CLI evidence in context pack
  `pk_rdwlccj6`; load requested skills and check primary upstream references.
- [x] (2026-11-08) Complete six-perspective expert review and reconcile
  schema defaults, enum fallback, grammar, stdout purpose and doctest findings.
- [x] (2026-11-08) Validate the revised planning document: formatting,
  format checking, Markdown/spelling and Mermaid gates all passed.
- [x] (2026-11-08) Commit, push, configure the named upstream, and create
  draft PR #661.
- [x] (2026-11-09) Rebase the sole plan commit onto the #664 prerequisite;
  `git range-diff` confirms an unchanged patch and no conflicts.
- [x] (2026-11-11) #664 merged as `eaf4c028`; the timeout-fix prerequisite
  landed on `main`, so no retarget of PR #661 to the prerequisite branch is
  required.
- [x] (2026-11-14) Rebase onto `origin/main` at `959dbd7e`, replaying only this
  branch's two planning commits from the exclusive boundary `8309e3b4` (the
  recovered #664 fork point, a unique merge base and an ancestor of the merged
  #664 head). `git range-diff` reports both patches unchanged and
  `git diff 959dbd7e..HEAD` is limited to the two planning documents.
- [x] (2026-11-14) Obtain explicit plan approval.
- [ ] M1: metadata, derive transport, documentation and integration coverage.
- [ ] M2: structured-output lint and executable consumer examples.
- [ ] M3: exit documentation, process regression evidence and completion.

## Surprises & discoveries

The reference design discusses a broader output feature than roadmap 7.2.3. The
roadmap allocates stream framing to 7.2.4 and numeric taxonomy to 7.2.5.
ADR-0016's older scope sentence mentions 7.2.3–7.2.4; the current roadmap is
the authoritative task boundary.

`cargo-orthohelp/src/policy/check/mod.rs::run_policy_check` currently evaluates
configuration with `PolicyInputs::default()`. The command-behaviour API in
`cargo-orthohelp/src/policy/rules/behaviour.rs` is separate. A new output rule
must not promise automatic command linting through that CLI flag.

The existing `main() -> Result<(), OrthohelpError>` maps ordinary runtime
errors to failure status 1. Clap uses its own exit path. Policy denial shares
status 1 with other failures; `summary.deny` distinguishes it in the report.

## Decision log

2026-11-08, planning proposal: add an optional sibling `output(...)` group, not
more behaviour or renderer keys. Keep semantic stdout purpose separate from
renderer presentation and preserve unknown versus false. Approval of this plan
accepts the proposed contract below; implementation records it in a new ADR
using the next available four-digit number.

2026-11-08, planning proposal: publish the new lint beside `check_behaviour`.
Preserve the no-bridge policy CLI contract rather than silently expanding its
execution cost and failure surface. The public API and doctest are the
acceptance surface for R2.

2026-11-08, verification decision: complete finite tables and property tests
are proportionate for this metadata transport and one finite predicate. No
unbounded theorem or new stateful business algorithm is introduced. Do not
manufacture a Verus wrapper around an equivalent boolean assumption. Reassess
if implementation introduces such an algorithm or a stronger claim.

## Context and orientation

`ortho_config/src/docs/ir/mod.rs::DocMetadata` owns the documentation IR. Add
its output types in `ortho_config/src/docs/ir/output.rs`, re-export through the
existing public docs surface, and mirror them in
`cargo-orthohelp/src/schema/output.rs` and `schema/mod.rs`.
`ortho_config/src/agent_context/mod.rs::AgentCommand` owns the compact command
contract; add separate sibling types in `agent_context/output.rs`.

Macro declarations pass through
`ortho_config_macros/src/derive/parse/doc_types.rs::DocStructAttrs` and the
struct attribute dispatcher. Follow `parse/renderer_attrs.rs` and
`generate/docs/renderer.rs` for new `output_attrs.rs` and `docs/output.rs`
modules. Update the struct dispatcher and docs metadata assembly together. The
public conversion entry point is
`cargo-orthohelp/src/agent_context/mod.rs::bridge_ir_to_agent_context`. Its
`walk` traversal calls mappings such as `map_renderer`, defined in
`cargo-orthohelp/src/agent_context/renderer.rs`. Add a focused sibling output
mapping module and keep the traversal within its size limit.

The reusable lint belongs in `cargo-orthohelp/src/policy/rules/output.rs`,
exported by `rules/mod.rs`. Reuse report constructors and canonical vocabulary.
Sweep existing helpers before extracting any abstraction; document new helper
ownership and allowed callers in the component design. Do not refactor
unrelated behaviour rules.

## Behavioural contract before implementation

For a command, the abstract declaration is a partial record O with JSON support
J in {undeclared, false, true}, stdout purpose S in {undeclared, data, path,
summary, none}, stderr diagnostics D in {undeclared, false, true}, and an
ordered sequence of exit-class descriptions X. Absence of O and an empty O make
no capability claim. X being empty means no exit classes have been documented,
not that errors cannot occur.

Proposed derive syntax is:

```rust,no_run
#[ortho_config(output(
    supports_json = true,
    stdout = "data",
    stderr_diagnostics = true,
    exit_class(name = "success", meaning = "The command completed"),
    exit_class(name = "usage", meaning = "The invocation was invalid")
))]
```

The `output` field is optional on both metadata types. Its scalar members are
optional and omitted when absent; its `exit_classes` vector defaults to empty
and is omitted when empty. A present empty record remains representable as `{}`
and has the same policy meaning as absence. Keep docs IR version `2.0` and
agent-context version `1` under the existing additive-field policy. This
deliberately refines the design §8.1 planned flat `supports_json = false` row:
the new nested field preserves declaration provenance, while its effective
capability remains false unless explicitly true. Approval accepts that
refinement. Update §8.1 with `output.supports_json`, `output.exit_classes`, and
their precise defaults, and §8.2 with the new omission exceptions, unknown
fallback and additive history. Do not leave the planned flat rows suggesting a
second source of truth. The mirror must serialize exactly the same shape as the
core IR.

`OutputMetadata` and `OutputContract` carry `supports_json: Option<bool>`,
`stdout: Option<StdoutPurpose>`, `stderr_diagnostics: Option<bool>`, and
`exit_classes: Vec<ExitClass>`. `StdoutPurpose` serializes as `data`, `path`,
`summary`, or `none`; `ExitClass` contains `name: String` and
`meaning: String`. Each schema owns its types. Names are stable, non-localized
ASCII tokens matching `[a-z][a-z0-9_]*`; meanings are nonblank explanations
transported verbatim, not Fluent keys. Duplicate class names are derive errors.
Preserve author order, reject unknown keys, invalid literal kinds, blank
meanings, and repeated scalar assignments across groups. Each `exit_class(...)`
requires exactly one `name` and one `meaning`; missing or repeated members are
errors. Accept `output()` as an explicitly empty declaration. Distinct
declarations may compose across groups as renderer metadata does. Optional
blocks and scalars accept JSON `null` and omission as `None`; serialization
canonicalizes `None` to omission. An empty class vector is accepted and
omitted, but a `null` class vector is rejected. These rules apply only to the
new fields, never existing serialized fields. The documentation IR rejects
unknown stdout tokens. The agent-context enum adds an `Unknown` fallback,
serialized as `unknown`, for unknown wire tokens; it never establishes
data-returning status. Test this fifth explicit variant and future-token
fallback independently of undeclared stdout. Do not fetch or validate a
response schema; schema references remain future work because 7.2.3's
acceptance list does not require them.

The policy specification is independent of the implementation:

```plaintext
missing_json(command) = (S = data) and (J is not true)
results(off, commands) = []
results(warn, commands) = one warn finding per missing_json command
results(deny, commands) = one deny finding per missing_json command
```

Stdout purpose describes the normal successful command result, independently of
renderer and encoding. It excludes help/version display and error output; it
does not promise identical output on every invocation.

Only an explicit `stdout = "data"` establishes data-returning status. An absent
output block, undeclared stdout, `path`, `summary`, and `none` produce no
finding. Both false and undeclared JSON support fail for declared data.
`supports_json = true` is the author's assertion that the `--json` capability
exists; verifying the actual flag or runtime is out of scope.

Expose `check_output(context: &AgentContext, mode: PolicyMode) -> PolicyReport`
beside the existing behaviour checker. Emit stable rule ID
`agent-native.output.json-missing` and code `json_support_missing`, with
`location: None`, command path in the explanation, and an actionable
`output(supports_json = true)` annotation only if the application actually
implements JSON. Recommend implementing JSON or correcting the data claim. Use
one command traversal and one stable ordering operation; no new benchmark or
instrumentation is justified. Use command-path order and preserve encounter
order for equal paths. Preserve canonical vocabulary and mode in empty reports.
One finding is emitted per command occurrence, including equal paths; do not
silently deduplicate.

For `cargo-orthohelp`, document status 0 as `success` (including help/version),
1 as `failure` (metadata, bridge, serialization, I/O and policy denial), and 2
as `usage` (Clap argument/dispatch rejection). These names describe current
behaviour, not newly emitted diagnostic fields. A policy report with a positive
deny count identifies policy rejection even though its process code is shared.
Signals, forced termination and launcher failures are outside this
normal-completion contract.

## Verification plan

Establish these obligations before executable changes. The intended semantic
facts below are review hypotheses, not accepted axioms or speculative proof
functions.

E1, declaration preservation: for each command independently, mapping copies
all declared values and class order, preserves unknown/false distinctions, and
never draws information from a parent, flags, locale or renderer. The
representation relation matches each named field across core IR, local mirror
and context. Preconditions are well-formed derive declarations; a separate
legacy test covers absent blocks. Postconditions are exact serialized values
and no new support claims. Use `rstest` tables for literal grammar and
`proptest` for round trips and nested command independence, with generated
valid identifiers, arbitrary nonblank meanings and bounded trees (depth 0–4, up
to 32 commands). Include old JSON without `output`, explicit false, empty
records and each enum variant. Negative control: map false to true or inherit a
parent's record; at least one named regression must fail.

E2, rule soundness and completeness: each command occurrence receives exactly
one output finding if and only if the specified predicate holds and mode is not
off. Test the complete 3 × 6 × 3 table for J, S and enforcement mode, including
the agent-context unknown stdout fallback as the sixth S state. Vary D and X
independently to demonstrate that unrelated fields cannot affect the rule. Use
an independent expected table, not a call to the production predicate.
Severity, stable IDs, location, vocabulary and summary counts are asserted
explicitly. Generated command vectors of length 0–64 check input permutation
invariance for distinct paths, stable ties for equal paths, additivity of
counts and unchanged input. These are sampled integration properties, not an
unbounded proof. Include empty and nonempty witnesses and both
matching/nonmatching classes without heavy rejection filters. Negative controls
omit undeclared J from the failing cases or duplicate a finding; tests must
reject both faults, then restore the code.

E3, executable boundary: spawn the real binary and capture status, stdout and
stderr for success, help/version, usage failure, generic failure, and policy
warn/deny. Parse complete stdout as JSON where the current command promises
JSON, compare file contents independently, and ensure a combined policy and
generation request never emits two JSON documents. Use existing supervised
fixture helpers and case-private build output when the helper requires it;
never isolate `CARGO_HOME` or mutate process-global environment directly.
Include a policy-only fixture with neither `root_type` nor a library target.
Assert successful report generation and absence of bridge build artefacts; this
makes the preserved no-bridge boundary observable. This verifies integration
with real Clap and Rust process termination, not third-party internals. A
changed exit status or an extra stdout line must fail a named test; snapshots
alone are insufficient.

E4, presentation stability: use `insta` for partial/full/absent output metadata
and off/warn/deny reports, alongside structural assertions using `googletest`
and `pretty_assertions`. Review snapshots rather than accepting them blindly.
Run legacy renderer, behaviour, policy-report and locale fixtures unchanged.
Use at least two locales to show identifiers and class meanings remain exact.

Use `rstest` fixtures and parameterization, `rstest-bdd` for the feature below,
`googletest` matchers for semantic assertions, and `pretty_assertions` for
readable serialized-record diffs. Check current manifest versions before adding
missing dev-dependencies; use caret requirements and the workspace's existing
versions where present. The upstream GoogleTest documentation shows `#[rstest]`
followed by `#[googletest::test]` and a fallible result.[^1]

Assumptions are Serde's documented field/default behaviour, Clap's documented
exit-code routing,[^2] Rust's `Result` termination behaviour, and the
repository's bridge faithfully transporting JSON. Actual-process and
mirror-parity tests exercise those boundaries. Metadata cannot establish that
an arbitrary downstream process emits truthful JSON; that residual gap must
remain visible in documentation.

Verus is not warranted for the finite rule, field transport, diagnostics
formatting or integration plumbing. No proof milestone is selected. If a new
unbounded semantic invariant becomes necessary, stop and first revise this plan
with an independent model, satisfiable preconditions, representation relation
and candidate semantic lemmas. Load `verus`; use the existing
`make verus VERUS_PROOF_FILES='<production-proof-path>'` acceptance command in
addition to ordinary gates. The proof must connect to production logic and
reject a seeded fault. Never use `assume`, `admit`, external/trusted proof
bodies or a copied predicate assumed equivalent to production. Do not count
`make test-prover-tools-integration` as a production proof.

## Plan of work and milestones

### M1: carry explicit output declarations end to end

After approval, first add the minimal parser and serialization tests and
observe the expected red failure. Implement the named types, parser, generator,
schema mirror and mapping together, updating all struct literals. Run focused
tests to green, then refactor only within this feature. Add nested fixtures and
an actual generated agent-context integration test analogous to
`cargo-orthohelp/tests/agent_context_renderer_end_to_end.rs`.

Record the accepted output surface and no-inference relation in a new ADR; link
it from `docs/agent-native-cli-design.md`, `docs/cargo-orthohelp-design.md`, and
`docs/contents.md`. Update the users' guide with the derive syntax and the
difference between declarations and runtime checks. Document helper/module
ownership in the component design.

The plateau is usable metadata without the new policy rule. E1/E4 must pass,
including legacy fixtures, mirror parity and real bridge output. Run all
ordinary gates below and commit the coherent change. Confirm R1 and R4,
unchanged defaults and versions, and no unexpected API/dependency change. On
failure, fix from captured evidence; a deliberate rollback reverts this whole
milestone. M2's rule and M3's exit documentation remain outstanding.

### M2: lint declared data commands without JSON support

Add the failing complete rule table and BDD specification before the rule.
Implement `check_output` and wire only its public module export. Use report
constructors rather than parallel reporting infrastructure. Add the generated
input properties and negative controls described in E2. Add an executable
Rustdoc consumer example on `check_output` showing how to call both command
checkers on a context and inspect each report's deny count. Mirror or link that
example in `docs/cargo-orthohelp-api-guide.md`, and run
`cargo test -p cargo-orthohelp --doc` explicitly: a Markdown fence alone is not
a runnable doctest and the ordinary test target does not run this Rustdoc
suite; do not add an unnecessary report-combining API.

The feature file is `cargo-orthohelp/tests/features/structured_output.feature`,
bound from the existing `tests/rstest_bdd.rs` target with feature-local support
modules:

```gherkin
Feature: Declared structured output capability
  Scenario: A data command has no JSON declaration
    Given a command with stdout purpose "data" and undeclared JSON support
    When the output policy is checked in "deny" mode
    Then exactly one "json_support_missing" finding has severity "deny"

  Scenario: A data command supports JSON
    Given a command with stdout purpose "data" and JSON support true
    When the output policy is checked in "warn" mode
    Then the output policy report has no findings

  Scenario: Enforcement is disabled
    Given a command with stdout purpose "data" and JSON support false
    When the output policy is checked in "off" mode
    Then the output policy report has no findings

  Scenario: Unknown output is not inferred from a verb
    Given a command named "get" with no output declaration
    When the output policy is checked in "deny" mode
    Then the output policy report has no findings
```

Use fixtures to supply the compiled context, invoke the public rule, and assert
stable machine fields. Table tests additionally cover non-data purposes,
partial declarations and false JSON support in warn/deny modes.

The plateau is a usable metadata lint API with executable documentation. E2/E4,
the BDD scenarios and ordinary gates must pass before commit. Confirm R2/R4 and
the unchanged bridge-free CLI boundary. Reverting this milestone leaves M1
coherent. No source compatibility shim is necessary. M3 remains.

### M3: pin documented process classes and complete delivery

Add or strengthen named process assertions in
`cargo-orthohelp/tests/cli_dispatch.rs`, `policy_report_contract.rs`, and
`documented_command.rs`. Existing behaviour should pass; prove sensitivity with
an isolated executable mutation that changes a runtime failure status or emits
an extra stdout line, then restore it before the green gates. A wrong expected
assertion alone does not establish implementation sensitivity. If
implementation contradicts the proposed status table, revise the plan rather
than changing process behaviour to fit prose. Do not refactor `main` merely to
name a documentation table.

Document the 0/1/2 contract and policy-denial discriminator in
`docs/users-guide.md` and `docs/cargo-orthohelp-design.md`. Update
`docs/developers-guide.md` with fixtures, policy API testing and the separation
between finite/property evidence and future proof work. Reconcile the
agent-native design and ADR, documenting the deferred 7.2.4/7.2.5 work and
which capability this release actually provides.

The plateau discharges E3 and all R1–R4 acceptance evidence. Run all gates,
review the whole diff, and mark 7.2.3 and its three bullets done in
`docs/roadmap.md` only after the feature is complete. Use the `mapsplice` skill
for roadmap structural edits if required by that skill's trigger. Leave later
items unchecked. Re-run documentation gates after the completion edit and
commit. Reverting M3 leaves metadata/lint usable but exit-documentation
completion outstanding. Set this plan COMPLETE only with all evidence and
accepted decisions reconciled; publish the implementation for review using the
repository's authorized PR workflow.

## Concrete commands and acceptance

Run from the repository root on `7-2-3-structured-output-and-exit-metadata`.
Start with `git status --short` and check the baseline and approved plan before
modifying files. The focused commands below are subordinate to sequential
candidate gate ownership:

```sh
cargo test -p ortho_config_macros output
cargo test -p ortho_config output
cargo test -p cargo-orthohelp output
cargo test -p cargo-orthohelp --doc
cargo test -p cargo-orthohelp --test rstest_bdd
cargo test -p cargo-orthohelp --test cli_dispatch
cargo test -p cargo-orthohelp --test policy_report_contract
cargo test -p cargo-orthohelp --test documented_command
cargo test -p cargo-orthohelp --test agent_context_output_end_to_end
```

The last target is new in M1; others exist. Run the explicit doctest command
after M2 and again at final acceptance. A filter returning zero relevant tests
is not acceptance: record the named cases and result counts. Red-stage commands
must fail for the missing capability, not a fixture/tool failure. At every
major milestone, delegate sequential full gates to `scrutineer`, logging each
with `tee` and retaining pipeline exit status:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-ortho-723.out
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-723.out
make lint 2>&1 | tee /tmp/lint-ortho-723.out
make test 2>&1 | tee /tmp/test-ortho-723.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-723.out
make nixie 2>&1 | tee /tmp/nixie-ortho-723.out
```

Execute each command separately and proceed only on exit zero. Inspect
formatter changes before accepting them. Add milestone suffixes to log names
when executing successive milestones. If spelling regeneration changes
unrelated dictionary content, follow repository policy for a separate base PR;
never hand-edit generated `typos.toml` or hide the change.

For this documentation-only planning PR, run `make fmt`, `make check-fmt`,
`make markdownlint`, and `make nixie` sequentially and review the diff. Runtime
lint/test gates above are mandatory for each implementation milestone; this
plan does not claim they ran for a feature not yet implemented.

## Idempotence and recovery

Focused tests and gates are repeatable. Preserve fixture-specific output
isolation and shared download caches. Read a failed gate's captured log before
rerunning, fix its cause, and rerun only after a relevant change. A healthy
compiler queue is not a failure. Follow AGENTS.md for supervisor faults; never
bypass admission. Stop and report disk exhaustion.

Commit each coherent green milestone with `git commit -F` using an inspected
temporary message file. Roll back with a reviewed revert of the milestone, not
a destructive reset of shared work. Preserve unrelated edits and inspect
post-commit code for refactoring needs; any necessary refactor is separately
gated. Refresh remote tracking refs after explicit SSH-URL pushes and verify
the remote head before reporting publication.

## Outcomes & retrospective

Planning only. No feature implementation or proof has run. Pandalump and
Wafflecat accepted the layering with explicit default/fallback and stdout
purpose refinements. Telefono and Dinolump required complete grammar and real
Rustdoc execution; those revisions are included. Doggylump and Buzzy Bee
confirmed exit and stream guarantees and required an explicit
no-root-type/no-library policy fixture. That fixture is now part of E3; the
rule needs no new instrumentation. Follow-up architecture and contract reviews
confirmed that their findings are resolved.

Planning gates passed on 2026-11-08: `make fmt`, `make check-fmt`,
`make markdownlint` (including spelling), and `make nixie`. Evidence is in
`/tmp/fmt-ortho-723-plan.out`, `/tmp/check-fmt-ortho-723-plan.out`,
`/tmp/markdownlint-ortho-723-plan.out`, and `/tmp/nixie-ortho-723-plan.out`.
Generated spelling configuration is unchanged. Runtime lint/test gates have not
been run for this documentation-only change. Publication evidence belongs in
the draft PR; implementation remains unapproved. At implementation completion,
replace this paragraph with the discharged evidence, remaining limitations, and
reconciled decisions.

## Research references

Firecrawl was used on 2026-11-08 to check the primary upstream interfaces.
These references inform tests, not a universal exit taxonomy.

[^1]: [GoogleTest attribute documentation](https://docs.rs/googletest/latest/googletest/attr.test.html)
    documents integration with `rstest`.

[^2]: [Clap error documentation](https://docs.rs/clap/latest/clap/type.Error.html)
    documents status 2 for stderr errors and 0 for display exits.

## Revision note

2026-11-08: initial plan derived from current source and two Wyvern
reconnaissance reports. It separates 7.2.3 from later stream/taxonomy work,
exposes the existing CLI lint boundary, and specifies verification before
implementation. Expert review is now reconciled; explicit user approval remains
outstanding.

2026-11-08: expert review clarified forward-compatible enum handling, planned
legacy-default refinement, exact derive/null grammar and the executable Rustdoc
acceptance command. It replaced a weak assertion-only negative control with an
executable mutation. These revisions strengthen acceptance without expanding
the implementation scope.

2026-11-08: recorded successful planning gates and resolved expert findings.
The final prose update receives the documentation gates again before commit.
Implementation milestones remain untouched and await approval.

2026-11-09: record the user-requested temporary #664 prerequisite and the safe
return-to-main procedure after it merges. The original plan commit rebased
unchanged; implementation remains subject to explicit approval.
