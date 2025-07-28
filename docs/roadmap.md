# Roadmap

The following is a proposed roadmap for the remaining work on **OrthoConfig**,
distilled from the design documents and a comparison with the current
repository implementation. Each item summarises an outstanding task and
references the relevant design guidance.

- **Add a dedicated error for missing required values**

  - [x] Introduce a `MissingRequiredValues` variant to `OrthoError` and update
    the derive macro to check for missing required fields before
    deserialization.
    [[Improved Error Message Design](improved-error-message-design.md)]

  - [x] Aggregate all missing fields, then generate a single, user‑friendly
    error message listing each missing path and showing how to supply it via
    CLI flags, environment variables and file entries.
    [[Improved Error Message Design](improved-error-message-design.md)]

  - [x] Write unit and `trybuild` tests to ensure the new error behaves
    correctly.
    [[Improved Error Message Design](improved-error-message-design.md)]

- **Implement comma‑separated list parsing for environment variables**

  - [x] Update the env provider or derive macro to accept comma‑separated values
    for array fields, allowing syntax such as `DDLINT_RULES=A,B,C`.
    [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [x] Ensure consistent handling of list values across environment variables,
    CLI arguments and configuration files.

- **Add configuration inheritance (extends)**

  - [ ] Design and implement an `extends` key for configuration files, so a
    config can inherit from a base file, with current settings overriding those
    from the base. [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [ ] Define the layering semantics (base file → extended file → env → CLI)
    and update the loader accordingly.

  - [ ] Document how inheritance interacts with prefixes and subcommand
    namespaces.

- **Support custom option names for the configuration path**

  - [ ] Allow renaming of the auto‑generated `--config-path` flag and its
    environment variable (e.g. to `--config`) via an attribute on the
    configuration struct. [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [ ] Update documentation and examples to illustrate this override.

- **Enable dynamic tables for arbitrary keys**

  - [ ] Accept map types (e.g. `BTreeMap<String, RuleConfig>`) in configuration
    structs to support dynamic rule tables such as `[rules.consistent-casing]`.
    [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [ ] Ensure these maps deserialise correctly from files, environment
    variables and CLI.

- **Implement ignore‑pattern list handling**

  - [ ] Provide support for ignore pattern lists using comma‑separated
    environment variables and CLI flags.
    [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [ ] Document the precedence rules and the relationship to defaults (e.g.
    `[".git/", "build/", "target/"]`).

- **Refine subcommand merging behaviour**

  - [ ] Simplify `load_and_merge` for subcommands to merge CLI‑provided values
    over file/env defaults without requiring all fields to exist in the
    lower‑precedence layers.
    [[Subcommand Refinements](subcommand-refinements.md)]

  - [ ] Remove the need for workarounds such as `load_with_reference_fallback`
    in client applications by ensuring missing required values can be satisfied
    by the already‑parsed CLI struct.
    [[Subcommand Refinements](subcommand-refinements.md)]

  - [ ] Deprecate and eventually remove `load_subcommand_config` and its `_for`
    variant in favour of a unified `load_and_merge` API.

- **Finish** `clap` **integration in the derive macro**

  - [ ] Generate a hidden `clap::Parser` struct that automatically derives long
    and short option names from field names (snake_case → kebab‑case) unless
    overridden via `#[ortho_config(cli_long = "…")]`. [[Design](design.md)]

  - [ ] Ensure the macro sets appropriate `#[clap(long, short)]` attributes and
    respects default values specified in struct attributes.

  - [ ] Confirm that CLI arguments override environment variables and file
    values in the correct order. [[Design](design.md)]

- **Improve merging and error reporting when combining CLI and configuration
  sources**

  - [ ] Distinguish between values explicitly provided on the command line and
    those left as `None` so that default values from env/file are not
    incorrectly overridden.
    [[Clap Dispatch](clap-dispatch-and-ortho-config-integration.md)]

  - [ ] Aggregate errors from `clap` parsing, file loading and environment
    deserialization into a coherent `OrthoError` chain.
    [[Clap Dispatch](clap-dispatch-and-ortho-config-integration.md)]

  - [ ] Consider interactions with `#[clap(flatten)]` and nested argument
    structs to ensure predictable behaviour.
    [[Clap Dispatch](clap-dispatch-and-ortho-config-integration.md)]

- **Enhance documentation and examples**

  - [ ] Expand user and developer documentation to cover new features such as
    extends, comma‑separated lists, dynamic tables and ignore patterns.
    [[Design](design.md)]

  - [ ] Provide worked examples demonstrating how to rename the config path
    flag, how to use subcommand defaults via the `cmds` namespace, and how to
    interpret improved error messages.

- **Address future enhancements**

  - [ ] Explore asynchronous loading of configuration files and environment
    variables for applications that need non‑blocking startup.
    [[Design](design.md#7-future-work)]

  - [ ] Provide an API for registering custom `figment` providers (e.g. secrets
    managers or remote key‑value stores). [[Design](design.md#7-future-work)]

  - [ ] Investigate live reloading of configuration when files change,
    acknowledging that this lies outside the initial scope but is part of the
    long‑term vision. [[Design](design.md#7-future-work)]

These items collectively define a coherent roadmap for advancing OrthoConfig
toward the capabilities described in the design documents and bridging the gaps
observed in the current implementation.
