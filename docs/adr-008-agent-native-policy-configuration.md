# ADR-008: Opt-in agent-native policy configuration

Status: Accepted.

Date: 2026-08-12.

## Context and problem statement

`cargo-orthohelp` generates documentation and agent-facing metadata for
configurations built with OrthoConfig. Roadmap item 7.1.1 introduces the first
enforcement surface for the agent-native command-line interface (CLI) design:
projects need a way to declare which command verbs and flags they are held to,
so agents and continuous integration (CI) can rely on a stable, canonical
vocabulary.

Two defaults collide unless separated. The shipped
`ortho_config::agent_context::AgentPolicy::default()` is `warn`, and the three
published golden agent-context snapshots advertise `"agent_native": "warn"`. A
new policy *check* is opt-in by design, so its enforcement default is `off`.
Reusing one default for both surfaces would either fail every unconfigured
project or silently advertise a mode the project never chose.

The question is where the policy lives, how it is enforced, how findings are
reported, and what the two defaults mean for each surface.

## Decision drivers

- Keep the feature opt-in: a package with no policy table checks nothing.
- Follow ADR-003 schema ownership: the policy-report schema stays in
  `cargo_orthohelp::policy`; reusable agent-context types stay in
  `ortho_config::agent_context`.
- Keep machine-readable policy output separate from human diagnostics.
- Preserve published schema values (`ORTHO_POLICY_REPORT_SCHEMA_VERSION` and
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION` stay `"1"`; additions are additive and
  serde-defaulted).
- Leave roadmap 7.1.2 a clean seam for per-rule lint rules.
- Avoid building the bridge crate for a policy check, so packages still
  adopting the toolchain can be checked.

## Options considered

### Option A: A dedicated policy file

A standalone `ortho-policy.toml` beside `Cargo.toml`.

Dedicated files only become necessary once configuration outgrows a metadata
table (prior art: cargo-dist). The 7.1.1 configuration — one mode plus an
exception list — is small, and `package.metadata.ortho_config` is already
parsed by the tool. Rejected.

### Option B: Reuse the advertised `warn` default for enforcement

Making `warn` the enforcement default would fail every project that has not
opted in. Rejected in favour of opt-in `off` enforcement (D9).

### Option C: Advertise the configured or overridden mode everywhere

Propagating the transient `--policy-mode` override into agent context would
misrepresent a one-off CI override as a project commitment. Rejected (D9).

### Option D: Free-form exception strings

Opaque strings lose the verb-versus-flag distinction, the mandatory reason, and
the optional command scope. Rejected in favour of structured exception tables
(D3).

## Decision outcome

### Configuration surface (D1)

The opt-in configuration surface is `[package.metadata.ortho_config.policy]` in
the target package's `Cargo.toml`:

```toml
[package.metadata.ortho_config.policy]
mode = "warn" # "off" (default), "warn", or "deny"

[[package.metadata.ortho_config.policy.exceptions]]
kind = "verb" # or "flag"
name = "get"
reason = "redundant but part of the migration surface"
command_path = "fixture" # optional; space-separated command path scope
```

Unknown keys inside the policy table are rejected (strict
`deny_unknown_fields`, D7), so a misspelt option fails in all modes instead of
silently disabling policy. The `rules` key is reserved for roadmap 7.1.2 and is
therefore also rejected until then; this is a documented version-skew
consequence (older pinned tools hard-fail on newer policy keys).

Exceptions are structured allowlists (D3): `kind` distinguishes verbs from
flags, `reason` is mandatory so exceptions stay honest and reviewable, and
`command_path` optionally scopes an exception to one command.

### Command surface (D11)

`cargo orthohelp --check-agent-native [--policy-mode <off|warn|deny>]` resolves
the package and parses the metadata table only, then evaluates and writes the
report — without building the bridge crate or requiring `root_type`, a library
target, or an `ortho_config` dependency. When a generator `--format` is
explicitly requested in the same invocation, the check runs first and the
generator pipeline follows; the default `--format ir` is treated as "not
explicitly requested" when `--check-agent-native` is present (clap
`ValueSource` detection). `--policy-mode` declares
`requires = "check_agent_native"` and overrides the configured mode for the
report only.

### Report channel and exit behaviour (D5, D6)

`--check-agent-native` always writes `policy-report.json` atomically to the
output directory (same channel as other generator artefacts), then prints a
one-line human summary to standard error. In `deny` mode with deny-level
findings, the command returns a `PolicyViolation` error after the report has
been written, so the artefact exists even when CI gates on the exit code. The
exit code (1) is shared with generic tool failure; the report's `summary.deny`
is documented as the authoritative CI signal.

### Two defaults (D9)

Enforcement default and advertisement default are distinct. The enforcement
default is `off`: a package with no policy table checks nothing and the feature
is opt-in. The agent context continues to advertise `AgentPolicy::default()`
(`warn`) when no policy table is present, preserving the shipped version-1 wire
values and the early-adoption-defaults-to-warnings intent. Only a configured
policy table changes the advertised mode to the configured value. The transient
`--policy-mode` override affects only the policy report's effective mode, never
the generated agent context, which records what the project has committed to
rather than what one invocation used.

### Exception visibility (D12)

The full exception shape including `reason` is published in
`policy-report.json`. The agent-context mirror type
(`ortho_config::agent_context::PolicyException`) carries only string-typed
`kind`, `name`, and optional `command_path` — never `reason` — so internal
context written for maintainers is not distributed to third-party agents.
Single-point `From` conversions in `cargo-orthohelp` map between the policy and
agent-context mirrors so they cannot drift.

### Evaluator seam and vocabulary (D14)

The evaluator is
`evaluate(config: &PolicyConfig, inputs: &PolicyInputs) -> PolicyReport`, where
`PolicyInputs` is `#[non_exhaustive]` and empty in 7.1.1. The report gains
additive, serde-defaulted `exceptions` and `vocabulary` fields (the latter
populated from the canonical `vocabulary` constants, D2), plus
`PolicyReport::with_details`. Roadmap 7.1.2 passes the bridge IR through
`PolicyInputs` and adds an additive `replacement: Option<String>` to
`PolicyResult` without a schema bump.

### Off-mode loudness (D13)

When the effective mode is `off`, the stderr summary is deliberately loud, for
example:
`policy mode off (no [package.metadata.ortho_config.policy] table
found); nothing was checked; report: …`.
Strict unknown-key handling applies *inside* the policy table only; a misspelt
table name still resolves to `off` (the residual typo gap), so the users' guide
documents a CI recipe asserting the mode
(`jq -e '.mode != "off"' policy-report.json`) and the `--policy-mode warn|deny`
"fail if unconfigured" pattern.

## Goals and non-goals

- Goals:
  - Deliver the configuration and reporting machinery only; off-policy
    vocabulary diagnostics are roadmap 7.1.2.
  - Expose configured exceptions and canonical vocabulary in policy output.
  - Make the check usable for packages without a buildable bridge.
- Non-goals:
  - Implement vocabulary lint rules (`info`, `ls`, `--format=json`, and so
    on); those are 7.1.2.
  - Change the shipped agent-context default value or rewrite published
    snapshots beyond the additive empty `exceptions` field.
  - Bump either schema version constant.

## Known risks and limitations

- A misspelt table *name* resolves to `off`, producing a never-gating gate.
  Mitigated by the loud off-mode summary and the CI mode-assertion recipe.
- The deny exit shares code 1 with generic tool failure. Mitigated by the
  report artefact as the authoritative signal (D6).
- The agent-context `exceptions` field changes existing golden snapshots by
  one additive field; that diff was reviewed deliberately in Milestone 4.

## Consequences

- Projects opting in declare a mode and optional exceptions in
  `[package.metadata.ortho_config.policy]`.
- `policy-report.json` is machine-stable and versioned, carrying `mode`,
  `results`, `summary`, `exceptions`, and `vocabulary`.
- Agent context advertises the configured mode and exceptions (without
  reasons) once a policy table exists, and keeps the shipped `warn` default
  otherwise.
- Roadmap 7.1.2 can plug per-rule levels into the reserved `rules` key and
  bridge IR into `PolicyInputs` additively.

## References

- [Agent-native CLI assistance design](agent-native-cli-design.md) §3.3 and
  §5.
- [OrthoConfig IR documentation design for cargo-orthohelp](cargo-orthohelp-design.md)
  §6.3.2.
- [Execution plan for roadmap 7.1.1](execplans/7-1-1-opt-in-agent-native-policy-configuration.md).
- [ADR-003: Define schema ownership for agent-native contracts](adr-003-define-schema-ownership-for-agent-native-contracts.md).
