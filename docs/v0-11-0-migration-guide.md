# Migration guide: v0.10.0 to v0.11.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.10.0 to v0.11.0. Applications that derive their configuration and read agent
context need no code changes: every new capability is opt-in, and the wire
contract stays additive.

Two areas deserve a review before release:

- Rust code that builds documentation IR or an `AgentCommand` with a struct
  literal must name the new `renderer` field; and
- authors who want to describe how a command renders can now adopt a
  `renderer(...)` declaration.

JSON consumers need no change, and neither do Fluent catalogues. The
documentation IR version stays at 2.0, because the new field is optional and
omitted when undeclared.

## Impact at a glance

| Priority             | Area                       | What to do                                                                   |
| -------------------- | -------------------------- | ---------------------------------------------------------------------------- |
| **Required**         | Dependency versions        | Update every OrthoConfig crate requirement to v0.11.0 together.              |
| **Required if used** | Documentation-IR literals  | Name `renderer: None`, or the declared records, when building `DocMetadata`. |
| **Required if used** | `AgentCommand` literals    | Supply `renderer: RendererContract::default()`.                              |
| **Optional**         | Renderer declarations      | Add `renderer(human(...), machine(...))` to a command that needs it.         |
| **Optional**         | Agent-native policy checks | Read the projected `renderer` block from agent context.                      |
| **Optional**         | Bridge-cache metrics       | Enable the `metrics` feature; the CLI exports the counters as `tracing`.     |
| **Optional**         | Skill manifest validation  | Supply an explicit effective command surface to the validator.               |
| **Optional**         | Test subprocesses          | Depend on `ortho_config_test_helpers` and call `run_with_deadline`.          |

_Table 1: Application-facing work when moving from v0.10.0 to v0.11.0._

## 1. Update dependency versions

Update the runtime crate, and any direct macro or tooling dependency, as one
unit.

Before:

```toml
[dependencies]
ortho_config = { version = "0.10.0", features = ["yaml"] }
```

After:

```toml
[dependencies]
ortho_config = { version = "0.11.0", features = ["yaml"] }
```

## 2. Name `renderer` in documentation-IR literals

`ortho_config::docs::DocMetadata` and the `cargo-orthohelp` mirror of it gained
a `renderer` field. The field is optional and is omitted when undeclared, so
JSON written by an earlier release still deserializes, and a document that
declares nothing looks exactly as it did before.

Before:

```rust,no_run
let metadata = DocMetadata {
    ir_version: "2.0".to_owned(),
    app_name: "demo".to_owned(),
    // … the remaining fields, as before
};
```

After:

```rust,no_run
let metadata = DocMetadata {
    ir_version: "2.0".to_owned(),
    app_name: "demo".to_owned(),
    renderer: None,
    // … the remaining fields, as before
};
```

A document that does declare renderer metadata names both records instead.

```rust,no_run
let metadata = DocMetadata {
    ir_version: "2.0".to_owned(),
    app_name: "demo".to_owned(),
    renderer: Some(RendererMetadata {
        human: Some(RendererRecord {
            supported: Some(true),
            ..RendererRecord::default()
        }),
        machine: None,
    }),
    // … the remaining fields, as before
};
```

## 3. Supply a renderer contract on `AgentCommand`

`ortho_config::agent_context::AgentCommand` gained the same field, typed as
`RendererContract`. `AgentCommand` does not implement `Default`, so a struct
literal must now name the field:

After:

```rust,no_run
let command = AgentCommand {
    // … the remaining fields, as before
    renderer: RendererContract::default(),
};
```

`RendererContract::default()` is the legacy claim, not an observation: human
output supported, machine output not. Use it for a command that declares
nothing. If your code already declares renderer metadata, project that
declaration instead.

## 4. The wire contract is unchanged

The field is `#[serde(default)]` on both the authoring IR and the agent-context
wire. As a result:

- JSON produced by v0.10.0 still decodes, and the `renderer` key is simply
  absent;
- documentation IR stays at 2.0, so consumers pinned to that version keep
  decoding documents that carry the new field;
- unknown policy tokens still normalize to `unknown` on the tolerant
  agent-context side, and still fail on the strict authoring side.

This is a Rust source break only. Roadmap 13.1 tracks the constructor-based
remedy that will remove it.

## 5. Optionally declare how a command renders

Adoption is a sibling attribute to `behaviour(...)`, scoped the same way.

```rust,ignore
use clap::Parser;
use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Parser, Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "DEMO",
    renderer(
        human(supported = true, colour = "auto", width = "terminal"),
        machine(supported = true, stdout = "structured")
    )
)]
struct DemoConfig {
    /// Recipient to greet.
    recipient: String,
}
```

The declaration is descriptive. It records what the author claims, and nothing
is inferred from command names, flags, terminal detection, or the locale. A
subcommand does not inherit its parent's declaration. The vocabulary, the
defaults, and the omission-versus-null rule live in
[ADR-0016](adrs/adr-0016-dual-renderer-metadata-surface.md), and the
[user's guide](users-guide.md#declare-how-a-command-renders-its-output) shows
the accepted keys with worked examples.

## 6. Collect bridge-cache metrics from the CLI

The `metrics` feature is new in `cargo-orthohelp` v0.11.0; the published
v0.10.0 package does not carry it. The feature emits
`cargo_orthohelp.bridge_cache.events` with the bounded `operation` and
`outcome` labels described in the
[user's guide](users-guide.md#collect-bridge-cache-metrics). The standalone
`cargo-orthohelp` binary installs a `tracing`-backed recorder at startup, so a
build with `--features metrics` exports each counter update to standard error
as a structured `tracing` event under the `cargo_orthohelp::metrics` target:

```console
RUST_LOG=cargo_orthohelp::metrics=debug cargo orthohelp --cache --package acme
```

Pass `--cache` to make repeated runs reuse the cached IR and emit the lookup
hit or miss event; without `--cache` or `--no-build`, an invocation consults no
cache and records no lookup.

A host that installs its own `metrics` recorder before starting the CLI keeps
it, because the CLI claims the process-wide recorder slot only when it is free.
Nothing is required to upgrade: the feature is off by default, and a build
without it behaves exactly as before.

## 7. Run test subprocesses through the shared helpers

The `ortho_config_test_helpers` crate publishes its bounded subprocess support
as `ortho_config_test_helpers::process`. A test that starts another process
calls `run_with_deadline` for a single budgeted command, or
`run_with_deadline_until` when an enclosing sequence owns the deadline. The
helpers own execution, capture, and descendant cleanup; the test keeps
responsibility for the command, its environment, serialization, and the
interpretation of the result.

Adoption is optional for downstream consumers and required inside this
repository, where the helpers are the only permitted process-tree
implementation in tests. See the
[user's guide](users-guide.md#run-test-subprocesses-under-a-deadline) for the
contract and the [developer's guide](developers-guide.md) for why the
repository routes Cargo, host-tool, and documented-binary commands through it.

## 8. Optionally validate skill manifests

`cargo_orthohelp::skill_validation` checks the command references and
structured examples a consumer extracted from a skill manifest against the same
command contract agent context describes. Adoption is opt-in: the library reads
no manifest, parses no Markdown, and runs no example. A consumer supplies
normalized claims, an explicit effective command surface, and a resolved
policy, and receives one ordered report.

See
[Validate skill manifests against commands](users-guide.md#validate-skill-manifests-against-commands)
for the contract and a worked example.
