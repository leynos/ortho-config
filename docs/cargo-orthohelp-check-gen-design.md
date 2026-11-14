# Design amendment: explicit cargo-orthohelp check and gen commands

Status: design amendment for review; implementation pending roadmap item 7.2.8.

Scope: command routing, policy orchestration, compatibility, and the integration
boundary with 7.2.3. This amendment changes no executable behaviour by itself.

Audience: maintainers and the agents implementing 7.2.3 and 7.2.8.

## 1. Problem and governing decisions

The existing `--check-agent-native` operation checks policy configuration
without obtaining command metadata. The behaviour checker is a reusable API;
7.2.3 adds output declarations and another reusable checker. Those APIs need a
first-party command-line consumer. Requiring every adopter to write an
integration leaves the promised enforcement workflow incomplete.

The requested public operations are `cargo orthohelp check` and
`cargo orthohelp gen`. Generation without an operation and
`--check-agent-native` remain supported but deprecated. The existing cheap
configuration check must survive without constraining the new metadata-aware
operation to the same limited scope.

These are developer-tool operations, not additions to every downstream
application's canonical verb vocabulary. Keep any tool-specific vocabulary
exceptions explicit rather than widening the shared defaults.

This amendment governs the target operation surface in
[cargo-orthohelp design](cargo-orthohelp-design.md) section 6 and
[agent-native CLI design](agent-native-cli-design.md) sections 3.2, 3.3, and 7.
It amends the command-surface scope of
[ADR-0008](adrs/adr-0008-agent-native-policy-configuration.md), not its policy
defaults, exception semantics, or process statuses. Existing command examples
continue to describe supported compatibility forms until 7.2.8 lands. The
[Cargo dispatch boundary](adrs/adr-0004-cargo-external-subcommand-entry-point.md)
and [schema ownership](adrs/adr-0003-define-schema-ownership-for-agent-native-contracts.md)
remain unchanged.

The 7.2.3 agent continues its approved work. Neither this amendment nor 7.2.8
adds CLI implementation to that agent's acceptance criteria. The agents meet at
the public metadata and policy API boundary described in section 6.

## 2. Canonical command surface

The following commands describe the target interface, not currently available
commands:

```bash
cargo orthohelp gen --package my-cli --format agent-context
cargo orthohelp gen --package my-cli --format man
cargo orthohelp check --package my-cli --policy-mode deny
cargo orthohelp check --package my-cli --scope config --policy-mode deny
```

### 2.1. Generate documentation and context

`gen` owns the existing generation pipeline: documentation intermediate
representation (IR), localized man pages, PowerShell help and wrappers, and
agent-context output. It preserves existing format defaults, package and target
selection, root-type resolution, locale options, output paths, cache behaviour,
and format-specific options. Moving the operation must not change generated
content or the established stdout/stderr ownership of any format.

Generation does not implicitly run policy checks. `gen` rejects checking-only
options with a usage error. Moving the command surface must not introduce a
second generator, configuration loader, or metadata schema.

### 2.2. Check declared command contracts

`check` defaults to `--scope all`. With enforcement enabled, it evaluates policy
configuration and all implemented, applicable command-metadata rule groups.
The initial complete set includes configuration checks, the existing behaviour
checker, and the output checker delivered by 7.2.3. Later vocabulary and other
rules join when their own roadmap items land; 7.2.8 does not claim to implement
or require unfinished rules.

The command obtains recursive command metadata through the existing bridge and
agent-context conversion. It acquires that metadata once per selected target,
then calls the reusable evaluators. It does not render man pages, PowerShell
help, or agent-context files merely to perform a check. Normal bridge cache
writes and the policy-report artefact remain permitted and documented.

Package, target, root-type, output-directory, and relevant bridge/cache options
remain available where they have a defined meaning. Generation-only options,
including `--format`, are usage errors on canonical `check`; separate commands
express generation and checking. Shared options must have one parser definition
and consistent resolution rather than drifting copies.

A missing root type, unusable target, failed bridge, invalid metadata, or cache
miss under `--no-build` is an operational failure when command metadata is
required. It must never downgrade `all` to configuration-only or manufacture an
empty successful report. Existing cache-validity rules apply; neither a stale
cache nor an older report is evidence that the current check completed.

### 2.3. Keep a genuinely configuration-only check

`check --scope config` evaluates configuration without building or running the
bridge. It requires no root type, library target, or `ortho_config` dependency
in the checked package. Reports explicitly identify the limited scope and the
command-metadata rule groups that were not evaluated.

Both scopes retain the configured enforcement default of `off` and the explicit
`--policy-mode off|warn|deny` override. Invalid configuration still fails. In
`off` mode, no policy rules run and there is no reason to acquire command
metadata solely for checking. The report and stderr summary must say that
nothing was evaluated, rather than describing the target as compliant.

The invocation override changes enforcement for this run, not the policy
advertised in a generated agent context. Existing exception semantics remain
owned by the corresponding evaluators. A rule must not gain exceptions merely
because the orchestrator combines its results with another rule group.

## 3. Deprecated aliases and migration

An omitted operation, with or without the existing generation options, routes
to `gen`. Thus `cargo orthohelp --format man` retains its generation behaviour,
and `cargo orthohelp` retains the existing default-generation behaviour,
including errors caused by missing required package metadata.

The old root-level `--check-agent-native` spelling maps to
`check --scope config`, not the new full check. Expanding its scope would add
build requirements and new denials to previously valid automation. Its
`--policy-mode` override and configuration-only support remain intact.

Preserve the existing combined legacy invocation too. A root-level
`--check-agent-native` plus an explicitly supplied generator `--format` first
checks configuration and then generates only if the check allows continuation.
Policy denial prevents generation. The policy report remains the sole stdout
JSON document; generation retains its artefacts without emitting a second
agent-context document. An implicit default format does not request generation
in this legacy mode. This is a compatibility dispatch path, not another public
canonical operation or a second policy implementation.

Explicit `check` or `gen` combined with the legacy check flag is a usage error.
Do not guess a precedence between contradictory operation selectors. Both
canonical operations use the established Cargo external-subcommand entry point
and its supported direct-binary spelling. Preserve ordinary root and operation
help/version display without running a check or generation pipeline.

Emit at most one actionable deprecation warning on stderr per legacy operation
invocation. Never put warnings in machine stdout, reports, or generated
artefacts, and never change the operation's exit status because it used an
alias. Help/version-only requests need not emit a deprecation warning. Describe
both aliases as deprecated in help and migration guidance; use canonical forms
in new examples, completions, and generated reference documentation. No alias
removal date or release is authorized by this item.

Migration examples, to become executable documentation during implementation:

```bash
# Generation: old form, then its replacement.
cargo orthohelp --format agent-context
cargo orthohelp gen --format agent-context

# Configuration-only checking: old form, then its exact replacement.
cargo orthohelp --check-agent-native --policy-mode deny
cargo orthohelp check --scope config --policy-mode deny

# Deliberate adoption of full command-metadata checking.
cargo orthohelp check --policy-mode deny
```

## 4. Reports, coverage, and process behaviour

Compose evaluator results into one deterministic `PolicyReport`; do not print a
report per checker. Preserve rule identifiers, codes, severities, command
locations, exception data, canonical vocabulary, and correctly recomputed
summary counts. Preserve distinct command occurrences even when their paths are
equal. Existing report constructors and ordering contracts remain the basis of
composition; the CLI must not duplicate the output-rule predicate.

A check writes the report atomically to the existing policy-report location and
emits the same single JSON document to stdout, with the human summary on stderr.
Bridge progress and child-process diagnostics must not contaminate stdout.
Publish deny findings before returning failure. If metadata acquisition fails,
return an operational error; any report that can be produced must identify the
failed acquisition rather than assert a completed clean check.

The report needs additive, machine-readable execution coverage. Record the
requested scope, effective mode, and whether each implemented rule group was
not evaluated, evaluated without findings, evaluated with findings, or failed.
Record a stable reason for non-evaluation, such as disabled policy or excluded
scope. For command rules, expose visited and applicable command counts so zero
eligible declarations cannot masquerade as comprehensive validation. An older
report without coverage means unknown coverage, never an implicit pass.

The 7.2.8 agent owns the precise additive report shape and compatibility tests.
It must preserve schema ownership and existing field meanings, update the
policy-report contract documentation, and obtain an explicit schema-evolution
decision if additive evolution cannot represent the required facts. Coverage
records execution, not a proof that declarations match runtime behaviour.

Preserve the normal-completion statuses documented by 7.2.3: zero for success
or display-only exits, one for policy denial or operational failure, and two
for usage errors. Warning findings do not cause a policy failure. A positive
`summary.deny` distinguishes a reported denial from other failures. Off mode
retains a successful status with explicit non-evaluation; continuous integration
(CI) must select an active policy and inspect coverage rather than treating
status zero alone as a compliance certificate. No numeric exit-taxonomy
redesign belongs here.

## 5. Architecture and scope boundaries

Keep three responsibilities separate: declarations and metadata transport,
reusable policy evaluation, and CLI orchestration. `gen` and `check` share the
existing metadata acquisition/conversion implementation; only generation owns
renderers. The check orchestrator resolves scope and policy, acquires required
inputs, invokes evaluators, composes one report, and applies exit policy.

Legacy spellings normalize into explicit operation requests at the CLI
boundary. Do not put compatibility routing into derive macros, the core
configuration loader, or individual rules. Reuse existing adapters and process
supervision rather than adding a plugin registry or generic task framework.

This item does not implement runtime JSON verification, subprocess execution of
downstream application commands, stream framing, numeric exit mappings, new
metadata-completeness rules, or unpublished vocabulary rules. Items 7.2.4,
7.2.5, and 7.1.2 retain their own responsibilities. A standalone context-file
input mode is not required to close this delivery gap and may be designed
separately. There is no new dependency on those later features.

## 6. Parallel delivery and cross-over contract

7.2.3 remains the producer. It owns output declarations, derive parsing, docs
IR and mirror types, conversion into `AgentContext`, the reusable
`check_output(context: &AgentContext, mode: PolicyMode) -> PolicyReport` API,
and its rule semantics, stable identifiers, and public consumer examples.
It also owns evidence for the existing exit statuses. Its current scope and
completion criteria remain unchanged; it does not wait for 7.2.8.

7.2.8 is the consumer. It owns `check`/`gen` routing, deprecated aliases,
metadata acquisition orchestration, composition of the behaviour and output
reports, execution coverage, process tests, and command migration guidance.
It may implement routing, generator extraction, configuration-only checking,
and tests against the established interfaces while 7.2.3 proceeds.

The cross-over is the real public 7.2.3 API and its agent-context metadata, not
a copied rule or speculative alternate schema. Record the producer commit and
agreed interface in the 7.2.8 ExecPlan before final integration. If producer
signatures change, coordinate the adapter update; do not stabilize a duplicate
API merely to avoid the integration. Test doubles may support isolated tests,
but cannot satisfy the final acceptance gate.

At the cross-over, run the actual nested-fixture pipeline through the bridge,
7.2.3 conversion, the real output checker, report composition, and the real
`cargo orthohelp check` process. A command declaring data output with JSON
support false or undeclared must produce the 7.2.3 `json_support_missing`
finding under `agent-native.output.json-missing`; deny mode must exit one.
A supported-data command must pass that rule. A command with no output
declaration retains 7.2.3's no-inference behaviour, with applicability visible
in coverage.

7.2.8 cannot be marked complete or advertised as full checking until this
integration passes. 7.2.3 may land first without modification; 7.2.8 then
integrates that landed implementation. Planning and independent implementation
are parallel, while completion depends on 7.2.3. Neither item waits for 7.2.4
or 7.2.5. Preserve each agent's changes when reconciling shared design, index,
roadmap, and report files; do not rewrite the active 7.2.3 ExecPlan in this PR.

## 7. Acceptance and implementation hand-off

The implementing agent prepares a separate ExecPlan for 7.2.8, citing this
amendment and the producer's public contracts. Required evidence includes:

- Parser tests for canonical operations, all legacy forms, argument conflicts,
  help/version, and both supported Cargo and direct-binary invocation forms.
- Generator parity tests across existing formats, defaults, locales, paths,
  and cache options. Alias warnings appear only on stderr, once per invocation.
- A real no-root-type/no-library fixture for configuration-only checking and
  its alias, proving that neither builds or runs the bridge.
- Real-process full-check fixtures for behaviour and output findings, off,
  warn, and deny modes, missing metadata, bridge failure, and `--no-build`
  cache misses. Enabled full checking never silently skips metadata rules.
- The producer/consumer cross-over test in section 6, not just direct calls to
  the checker or mocks of a successful evaluator.
- Complete stdout JSON parsing, atomic report/file parity, correct merged
  counts, stable ordering, coverage states, and old-report deserialization.
  A legacy combined check/generation retains its single-document stdout rule.
- Negative controls showing that disconnecting the output checker, treating a
  failed bridge as a pass, or leaking a warning to stdout fails named tests.
- Updated help, completions, README, users' guide, API guide, design examples,
  and release-specific migration guidance. Current documentation remains
  truthful until implementation; new canonical examples become tested examples
  under the documentation style guide when they ship.

Run the repository's applicable formatting, lint, unit, behavioural,
integration, documentation, and explicit Rustdoc gates sequentially. Select
verification proportionately: routing tables, report properties, and real
process tests establish this orchestration contract; no artificial proof layer
is required. Keep 7.2.8 unchecked until its integration and migration evidence
passes. This design-only PR does not claim those implementation gates ran.

## 8. Alternatives and consequences

Keeping only library APIs leaves each consumer to implement enforcement.
Adding another root `--check` flag retains the growing mixture of generation
and validation options. Broadening `--check-agent-native` silently breaks its
no-bridge contract. Removing the old spellings breaks existing automation.
The selected operation split avoids those failures while preserving a small,
explicit compatibility path.

Full checking can incur a bridge build, so its cost and failure modes must be
visible; configuration-only checking remains the cheap alternative. Optional
metadata still limits what rules can conclude. Reports must expose that limit
rather than treating empty findings as proof of runtime correctness. The
compatibility aliases remain maintenance obligations until a separately
approved removal, but no second generator or evaluator implementation is
justified.
