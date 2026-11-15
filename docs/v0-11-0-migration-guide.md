# Migration guide: v0.10.0 to v0.11.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.10.0 to v0.11.0. Applications that derive their configuration and read agent
context need no code changes: every new capability is opt-in, and the wire
contract stays additive.

Two areas deserve a review before release:

- Rust code that builds documentation IR or an `AgentCommand` with a struct
  literal must name the new `renderer` and `output` fields;
- test code that imports `ortho_config_test_helpers::figment` must move to
  injected sources; and
- authors who want to describe how a command renders, or what structured output
  it supports, can now adopt a `renderer(...)` or `output(...)` declaration.

JSON consumers need no change, and neither do Fluent catalogues. The
documentation IR version stays at 2.0, because the new fields are optional and
omitted when undeclared.

## Impact at a glance

| Priority               | Area                       | What to do                                                                           |
| ---------------------- | -------------------------- | ------------------------------------------------------------------------------------ |
| **Required**           | Dependency versions        | Update every OrthoConfig crate requirement to v0.11.0 together.                      |
| **Required if called** | Documentation-IR literals  | Name `renderer: None` and `output: None`, or the declared records, in `DocMetadata`. |
| **Required if called** | `AgentCommand` literals    | Supply `renderer: RendererContract::default()` and `output: None`.                   |
| **Required if called** | Figment Jail test helpers  | Replace `figment::with_jail` and `figment_error` with injected sources.              |
| **Optional**           | Renderer declarations      | Add `renderer(human(...), machine(...))` to a command that needs it.                 |
| **Optional**           | Output declarations        | Add `output(supports_json = true, stdout = "data", ...)` to a command that needs it. |
| **Optional**           | Agent-native policy checks | Read the projected `renderer` and `output` blocks from agent context.                |
| **Optional**           | Skill manifest validation  | Supply an explicit effective command surface to the validator.                       |
| **Optional**           | Bridge-cache metrics       | Enable the `metrics` feature; the CLI exports the counters as `tracing`.             |
| **Optional**           | Test subprocesses          | Depend on `ortho_config_test_helpers` and call `run_with_deadline`.                  |
| **Optional**           | Process-mutating helpers   | Replace `env::set_var`, `env::lock`, `cwd::set_dir` with a `MapEnv`.                 |
| **Optional**           | Injected environment       | Load with `load_from_iter_with_env` and pin the directory on a `MapEnv`.             |
| **Review if matched** | Absent-required errors     | Match `OrthoError::MissingRequiredValues`, not a Serde error.                |
| **Required if used**  | Subcommand merge helpers   | The `_for_*` helpers now also require `RequiredConfigSchema`.                |
| **Optional**          | Nested requirement schemas | Mark a child `nested` and derive `RequiredConfigSchema` on it.               |

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

## 2. Name `renderer` and `output` in documentation-IR literals

`ortho_config::docs::DocMetadata` and the `cargo-orthohelp` mirror of it gained
a `renderer` field and an `output` field. Both are optional and are omitted
when undeclared, so JSON written by an earlier release still deserializes, and
a document that declares nothing looks exactly as it did before.

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
    output: None,
    // … the remaining fields, as before
};
```

A document that does declare renderer or output metadata names the declared
records instead.

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
    output: Some(OutputMetadata {
        supports_json: Some(true),
        stdout: Some(StdoutPurpose::Data),
        stderr_diagnostics: None,
        exit_classes: Vec::new(),
    }),
    // … the remaining fields, as before
};
```

## 3. Supply the renderer and output contracts on `AgentCommand`

`ortho_config::agent_context::AgentCommand` gained the same fields, typed as
`RendererContract` and `Option<OutputContract>`. `AgentCommand` does not
implement `Default`, so a struct literal must now name them:

After:

```rust,no_run
let command = AgentCommand {
    // … the remaining fields, as before
    renderer: RendererContract::default(),
    output: None,
};
```

`RendererContract::default()` is the legacy claim, not an observation: human
output supported, machine output not. Use it for a command that declares
nothing. `output: None` is different in kind: it is the absence of a claim, and
it stays distinct from a declared
`Some(OutputContract { supports_json: Some(false), .. })`, which records a
refusal. If your code already declares either block, project that declaration
instead of a default.

## 4. The wire contract is unchanged

Both fields are `#[serde(default)]` on the authoring IR and the agent-context
wire, and what each layer omits differs. `output` is omitted when absent on
both layers. `renderer` is omitted from `DocMetadata` when absent, but
`AgentCommand::renderer` is a plain `RendererContract` that is always
serialized, carrying its documented legacy defaults. As a result:

- JSON produced by v0.10.0 still decodes, and the `renderer` and `output` keys
  are simply absent;
- documentation IR stays at 2.0, so consumers pinned to that version keep
  decoding documents that carry the new fields;
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

## 6. Optionally declare a command's structured output

Adoption is the same sibling-attribute shape, and it composes with
`renderer(...)` in one `#[ortho_config(...)]` list.

```rust,ignore
use clap::Parser;
use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Parser, Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "DEMO",
    output(
        supports_json = true,
        stdout = "data",
        stderr_diagnostics = true,
        exit_class(name = "success", meaning = "the report was written"),
        exit_class(name = "failure", meaning = "the run failed before writing")
    )
)]
struct DemoConfig {
    /// Recipient to greet.
    recipient: String,
}
```

`stdout` records what the normal successful result _is_ — `data`, `path`,
`summary`, or `none` — independently of how it is encoded. `supports_json`
records a claim about a `--json` capability, not an observation of one, and
`supports_json = false` stays distinguishable from leaving it undeclared. Exit
classes are documented in declaration order; they do not assign process status
numbers. As with `renderer`, nothing is inferred from command names, flags, or
a parent command, and an omitted block stays omitted rather than defaulting.
`cargo-orthohelp` lints only declarations, through
`cargo_orthohelp::policy::rules::output::check_output`; the derivation of
runtime output is roadmap 7.2.4 and the status mapping is 7.2.5. The vocabulary
and omission rules live in
[ADR-0023](adrs/adr-0023-explicit-structured-output-declarations.md), and the
[user's guide](users-guide.md#lint-declared-structured-output) shows the rule's
findings.

## 7. Collect bridge-cache metrics from the CLI

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

## 8. Run test subprocesses through the shared helpers

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

## 9. Replace the Figment Jail helpers with injected sources

`ortho_config_test_helpers::figment` (`with_jail` and `figment_error`) is
removed, and the crate no longer depends on `figment`. A test that used a jail
to supply configuration files and environment variables now injects them:

- write the file into a temporary directory through a `cap_std` handle;
- build a `MapEnv` that names it with an absolute `CONFIG_PATH` selector (use
  the struct's own prefix, such as `APP_CONFIG_PATH`) and carries the variables
  the test needs; and
- call the derive-generated `load_from_iter_with_sources`,
  `compose_layers_from_iter_with_sources`, or
  `load_with_profile_from_iter_with_sources` with that map as both the
  discovery and merge source.

No jail, working-directory change, or process-environment write is then needed,
so the test no longer has to run serially. See the
[user's guide](users-guide.md#resolve-discovery-and-merging-without-process-state)
for the full pattern.

## 10. Stop changing the process in tests

The `ortho_config_test_helpers` helpers that change the process environment or
working directory are deprecated: `env::set_var`, `env::remove_var`,
`env::lock`, `env::scope`, `env::scope_with`, `env::scope_from_iter`,
`env::with_lock`, `EnvVarGuard`, `EnvVarLock`, `EnvScope`, `cwd::set_dir` and
`CwdGuard`. They still work, and warn. They force every test in a binary to run
one at a time, because the process environment is shared, so they leave runner
cores idle. Rust 2024 also makes `std::env::set_var` unsafe.

Replace them in one of two ways:

- **Inject the environment.** Build a `MapEnv` with the variables the test
  needs, pin the working directory with `MapEnv::with_current_dir`, and call a
  `_with_env` loader: `Config::load_from_iter_with_env`,
  `compose_layers_from_iter_with_env`, `load_with_profile_from_iter_with_env`,
  or `SubcmdConfigMerge::load_and_merge_with_env`. Name `XDG_CONFIG_HOME` and
  `XDG_CONFIG_DIRS` in the map when host configuration directories must stay
  out. Nothing then reads or writes the process, so the test runs concurrently.
- **Run a child process.** A test of a code path that reads the live process
  environment on purpose (`load_from_iter`, `load_and_merge`) re-runs the test
  binary with `std::process::Command`, giving it `env_clear`, the variables it
  needs, and `current_dir`. The parent changes nothing in its own process.

Add the matching lints to your own `clippy.toml` to keep it that way:

```toml
disallowed-methods = [
    { path = "std::env::set_var", reason = "mutates process-wide state" },
    { path = "std::env::remove_var", reason = "mutates process-wide state" },
    { path = "std::env::set_current_dir", reason = "mutates process-wide state" },
]
```

A site that must stay carries
`#[expect(clippy::disallowed_methods, reason = "...")]`. The helpers will be
removed in a later release.

## 11. Load with one injected environment

Nothing is required. The derive-generated `_with_sources` loaders keep working.
Tests and embedding applications that want the whole load off the process can
now use one `MapEnv` behind both layers:

- `Config::load_from_iter_with_env(args, Arc::new(env))` replaces
  `load_from_iter_with_sources(args, discovery, merge)` when one map serves
  both, and `compose_layers_from_iter_with_env`,
  `load_with_profile_from_iter_with_env` and the subcommand
  `load_and_merge_with_env` follow the same shape.
- `EnvSource` gains a defaulted `current_dir` method. Custom implementors need
  not change: the default reads the process working directory. Pin it on a map
  with `MapEnv::with_current_dir`, and name `XDG_CONFIG_HOME`,
  `XDG_CONFIG_DIRS` and `HOME` so no host directory contributes.
- A relative path the source names (a config-path selector, `XDG_CONFIG_HOME`,
  `HOME`) now resolves against the source's working directory, which for the
  process source is the process's own, so existing callers see no change. In
  subcommand path discovery a relative `HOME` counts as unset, like an empty
  one.

See the
[user's guide](users-guide.md#use-one-environment-and-inject-the-working-directory).

## 11. Optionally validate skill manifests

`cargo_orthohelp::skill_validation` checks the command references and
structured examples a consumer extracted from a skill manifest against the same
command contract agent context describes. Adoption is opt-in: the library reads
no manifest, parses no Markdown, and runs no example. A consumer supplies
normalized claims, an explicit effective command surface, and a resolved
policy, and receives one ordered report.

See
[Validate skill manifests against commands](users-guide.md#validate-skill-manifests-against-commands)
for the contract and a worked example.

## 12. Match the enumerated missing-value error

A derived loader no longer reports an absent required value as a Serde
deserialization error. After every layer is merged, the generated `finish`
collects the requirements the merged value does not satisfy and fails once with
`OrthoError::MissingRequiredValues`. The payload names every absent path in
declaration order, with the routes that can supply each one.

The change is additive to the error enum but not to the error text. Code that
matched a deserialization error to detect absence, or asserted on a Serde
message, must match the new variant:

```rust,ignore
use ortho_config::OrthoError;

match error.as_ref() {
    OrthoError::MissingRequiredValues(report) => {
        for item in report.items() {
            eprintln!("{}", item.path().render());
        }
    }
    other => return Err(other.into()),
}
```

A requirement is absent only when the key is absent. A present value — including
`null` and an empty collection — counts as supplied, because the loader
accepts it and any domain validation is a different error. When the merged
value contains both absent requirements and invalid present values, the report
covers the absence set first; correcting it may then surface the invalid
values. One round of repair is promised only when missing values are the sole
defect.

Ordinary loads, injected-source loads, profile loads, and the selected
subcommand merge all use this path. The raw `DeserializeOwned` subcommand
helpers, and their `_with_sources` forms, keep the earlier Figment finish.

## 13. Adopt nested requirement schemas, or widen your own helpers

`#[derive(OrthoConfig)]` already emits the companion `RequiredConfigSchema`
implementation, so most applications adopt nothing. Two cases need work.

A nested type must opt in twice: the parent field carries
`#[ortho_config(nested)]`, and the child type derives
`my_cfg::RequiredConfigSchema` on its own. A child still needs no CLI and no
loader, so it can keep deriving nothing else. Marked children are composed in
both directions; a cyclic composition is a `Validation` error rather than a
stack overflow. A child that is not marked contributes nothing, and an absent
optional parent deactivates its subtree.

```rust,ignore
use ortho_config::RequiredConfigSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, RequiredConfigSchema)]
struct Database {
    host: String,
}

#[derive(Debug, Deserialize, Serialize, ortho_config::OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct AppConfig {
    #[ortho_config(nested, skip_cli)]
    database: Database,
}
```

The four `load_and_merge_subcommand_for*` helpers, and the matching
`SubcmdConfigMerge` methods, now also bound their type parameter by
`RequiredConfigSchema`. A subcommand type that derives `OrthoConfig` satisfies
it. A handwritten implementation, or a generic wrapper that forwards these
helpers, must add the bound:

```rust,ignore
fn merge<T>(/* … */) -> ortho_config::OrthoResult<T>
where
    T: ortho_config::OrthoConfig + ortho_config::RequiredConfigSchema,
{
    // …
}
```

Enumeration is best effort by design. It covers declared fields whose
requiredness is decidable from attribute metadata, and does not predict a
handwritten `Deserialize`, an unmarked `#[serde(flatten)]`, or an enum payload.
Call `ortho_config::ensure_no_missing_required_values::<T>` directly to check a
value you have already merged yourself.
