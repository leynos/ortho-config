# Roadmap

The following is a proposed roadmap for the remaining work on **OrthoConfig**,
distilled from the design documents and a comparison with the current
repository implementation. Each item summarises an outstanding task and
references the relevant design guidance.

- [x] **Add a dedicated error for missing required values**

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

- [x] **Implement comma‑separated list parsing for environment variables**

  - [x] Update the env provider or derive macro to accept comma‑separated values
    for array fields, allowing syntax such as `DDLINT_RULES=A,B,C`.
    [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [x] Ensure consistent handling of list values across environment variables,
    CLI arguments and configuration files.

- [x] **Add configuration inheritance (extends)**

  - [x] Design and implement an `extends` key for configuration files, so a
    config can inherit from a base file, with current settings overriding those
    from the base. [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [x] Define the layering semantics (base file → extended file → env → CLI)
    and update the loader accordingly.

  - [x] Document how inheritance interacts with prefixes and subcommand
    namespaces.

- [x] **Refine subcommand merging behaviour**

  - [x] Simplify `load_and_merge` for subcommands to merge CLI‑provided values
    over file/env defaults without requiring all fields to exist in the
    lower‑precedence layers.
    [[Subcommand Refinements](subcommand-refinements.md)]

  - [x] Remove the need for workarounds such as `load_with_reference_fallback`
    in client applications by ensuring missing required values can be satisfied
    by the already‑parsed CLI struct.
    [[Subcommand Refinements](subcommand-refinements.md)]

  - [x] Remove `load_subcommand_config` and its `_for` variant in favour of a
    unified `load_and_merge` API (completed in v0.5.0).

- [x] **Finish `clap` integration in the derive macro**

  - [x] Generate a hidden `clap::Parser` struct that automatically derives long
    and short option names from field names (underscores → hyphens) unless
    overridden via `#[ortho_config(cli_long = "…")]`. [[Design](design.md)]

  - [x] Ensure the macro sets appropriate `#[clap(long, short)]` attributes and
    respects default values specified in struct attributes.

  - [x] Confirm that CLI arguments override environment variables and file
    values in the correct order. [[Design](design.md)]

- [x] **Improve merging and error reporting when combining CLI and
  configuration sources**

  - [x] Distinguish between values explicitly provided on the command line and
    those left as `None` so that default values from env/file are not
    incorrectly overridden.
    [[Clap Dispatch](clap-dispatch-and-ortho-config-integration.md)]

  - [x] Aggregate errors from `clap` parsing, file loading and environment
    deserialization into a coherent `OrthoError` chain.
    [[Clap Dispatch](clap-dispatch-and-ortho-config-integration.md)]

  - [x] Consider interactions with `#[clap(flatten)]` and nested argument
    structs to ensure predictable behaviour.
    [[Clap Dispatch](clap-dispatch-and-ortho-config-integration.md)]

- [x] **Enhance documentation and examples**

  - [x] Expand user and developer documentation to cover new features such as
    extends, comma‑separated lists, dynamic tables and ignore patterns.
    [[Design](design.md)]

  - [x] Provide worked examples demonstrating how to rename the config path
    flag, how to use subcommand defaults via the `cmds` namespace, and how to
    interpret improved error messages.

- [x] **Ship the `hello_world` example crate**

  - [x] Scaffold the binary crate with `Cargo.toml`, a `main.rs`, and supporting
        modules for configuration and message rendering.
  - [x] Demonstrate global switches and repeated parameters with defaults and
        validation enforced in code rather than at call sites.
  - [x] Implement `greet` and `take-leave` subcommands with layered
        configuration, unit tests, and behavioural coverage.
  - [x] Cover the example with `rstest` unit tests and a `cucumber` suite that
        exercises the compiled binary end-to-end.

  - [x] Provide demo scripts and sample configuration files that demonstrate
        configuration precedence on POSIX and Windows.
  - [x] Layer `.hello_world.toml` discovery across XDG, platform-specific, and
        working-directory locations so the sample overrides excite the greeting
        and update `cmds.greet` defaults used by both subcommands.

- [x] **Support custom option names for the configuration path**

  - [x] Allow renaming of the auto‑generated `--config-path` flag and its
    environment variable (e.g. to `--config`) via an attribute on the
    configuration struct. [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [x] Update documentation and examples to illustrate this override.

- [x] **Enable dynamic tables for arbitrary keys**

  - [x] Accept map types (e.g. `BTreeMap<String, RuleConfig>`) in configuration
    structs to support dynamic rule tables such as `[rules.consistent-casing]`.
    [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [x] Ensure these maps deserialize correctly from files, environment
    variables and CLI.

- [x] **Implement ignore‑pattern list handling**

  - [x] Provide support for ignore pattern lists using comma‑separated
    environment variables and CLI flags.
    [[DDLint Gap Analysis](ddlint-gap-analysis.md)]

  - [x] Document the precedence rules and the relationship to defaults (e.g.
    `[".git/", "build/", "target/"]`).

- [x] **Reduce error payload size** (target: v0.4.0)

  - [x] Wrap expansive error variants in `Arc` to shrink `Result` sizes and
    eliminate the need for `#[expect(clippy::result_large_err)]`.
    - Link: <https://github.com/leynos/ortho-config/issues/>
    - Done when:
      - Public `Result<_, OrthoError>` signatures use `Arc` (via the
        alias `OrthoResult<T>`).
      - All `#[expect(clippy::result_large_err)]` are removed or scoped to
        private internals with a rationale.

- [x] **Abstract configuration discovery**

  - [x] Provide a cross-platform discovery helper that surfaces the same
    search order currently hand-coded in `hello_world`, consolidating explicit
    paths, XDG directories, Windows locations, and project roots into a single
    call. [[Feedback](feedback-from-hello-world-example.md)]

  - [x] Integrate the helper with the derive macro so applications can opt in
    via attributes to customise config file names and generated CLI flags
    without duplicating boilerplate.
    [[Feedback](feedback-from-hello-world-example.md)]

- [x] **Introduce declarative configuration merging**

  - [x] Define the `DeclarativeMerge`, `MergeLayer`, and `MergeComposer` design
    that replaces hand-written Figment wiring in the hello_world example and
    future clients. [[Feedback](feedback-from-hello-world-example.md)]

  - [x] Document declarative merging with examples covering defaults, file
    overrides, environment variables, and CLI adjustments to codify expected
    behaviour. [[Feedback](feedback-from-hello-world-example.md)]

  - [x] Derive `DeclarativeMerge` alongside `OrthoConfig`, generating
    field-level merge arms and attribute-driven strategies for collections.

    - [x] Sketch the derive macro surfaces, ensuring every struct that
      already implements `OrthoConfig` can auto-derive `DeclarativeMerge`
      without additional boilerplate. Mirror the trait signatures from the
      declarative design doc before implementing. [[Design](design.md)]

    - [x] Generate merge arms for each field, respecting existing
      `#[ortho_config(...)]` metadata so nested structures, optional values,
      and enums flow through consistently. Use the dependency injection
      patterns documented for testing to keep fixtures focused on precedence
      rules.
      [[Reliable Testing](reliable-testing-in-rust-via-dependency-injection.md)]

    - [x] Provide attribute-driven strategies for collections (e.g.
      `Vec`, `BTreeMap`) so authors can pick append, replace, or keyed merges.
      Document the expected strategies with doctests that reuse the dry-guide
      patterns and behavioural cucumber coverage.
      [[Rust Doctest Dry Guide](rust-doctest-dry-guide.md)]
      [[Behavioural Testing](behavioural-testing-in-rust-with-cucumber.md)]

    - [x] Cover the derive macro with `rstest`-powered fixture suites
      that enumerate precedence permutations and validate generated code via
      `trybuild` where necessary.
      [[rstest fixtures guide](rust-testing-with-rstest-fixtures.md)]

- [ ] **Introduce Fluent localisation for `clap` integration**

  - [x] Define a `Localizer` trait and `NoOpLocalizer` implementation that wrap
    message lookup and expose argument-aware helpers. [[Design](design.md)]

  - [x] Ship a `FluentLocalizer` that layers consumer bundles over embedded
    defaults, logging formatting errors and falling back cleanly when lookups
    fail. [[Design](design.md)]

  - [x] Embed default `.ftl` catalogues for supported locales and provide a
    loader that constructs the baseline `FluentBundle` for a requested language
    identifier. Success is measured by loading at least the bundled English
    resources without runtime allocation failures. [[Design](design.md)]

  - [ ] Extend the derive macro builder, so applications can pass a
    `&dyn Localizer`, override help message identifiers, and surface localised
    copy in generated `clap::Command` structures. Behavioural coverage should
    confirm defaults remain functional when localisation is disabled.
    [[Design](design.md)]

  - [x] Provide a custom `clap` error formatter that maps `ErrorKind` variants
    onto Fluent identifiers and forwards argument context, with unit tests that
    verify fallback to the stock `clap` message when no translation exists.
    [[Design](design.md)]

  - [x] Emit a `MergeComposer` builder that discovers file layers and serializes
    CLI and environment input into `MergeLayer` instances without exposing
    Figment publicly.

  - [ ] Replace `load_global_config` and related helpers in examples with the
    new API. Add regression coverage using the behavioural testing
    fixtures.[^roadmap-behavioural] Reuse the parameterised setups from the
    rstest fixture guide.[^roadmap-rstest]

  - [ ] Route merge failures through `OrthoError::Merge` so binaries rely on a
    single shared error surface when combining loaders.
    [[Feedback](feedback-from-hello-world-example.md)]

- [ ] **Streamline subcommand configuration overrides**

  - [ ] Make `load_and_merge` treat CLI defaults as absent when the user did
    not override them, allowing subcommand sections such as `[cmds.greet]` to
    flow through automatically.
    [[Feedback](feedback-from-hello-world-example.md)]

  - [ ] Provide an attribute- or trait-based hook for bespoke subcommand merge
    logic so advanced cases can adjust the merged struct without manual glue
    code. [[Feedback](feedback-from-hello-world-example.md)]

  - [ ] Offer a unified API that returns merged global and selected subcommand
    configuration in one call, eliminating the repetitive `match` scaffolding
    in `hello_world`. [[Feedback](feedback-from-hello-world-example.md)]

- [x] **Replace `serde_yaml` with `serde-saphyr` for YAML parsing**
  [[ADR-001](adr-001-replace-serde-yaml-with-serde-saphyr.md)]

  - [x] Update `ortho_config/Cargo.toml` features to remove the indirect
    `figment/yaml` dependency, add optional `serde_saphyr` and `serde_json`
    entries, and wire the YAML feature to these crates.
    [[ADR-001](adr-001-replace-serde-yaml-with-serde-saphyr.md)]

  - [x] Implement the `SaphyrYaml` provider in `ortho_config/src/file.rs` that
    reads YAML files, deserialises them with `serde-saphyr`, and converts the
    output into `figment::value::Dict`.
    [[ADR-001](adr-001-replace-serde-yaml-with-serde-saphyr.md)]

  - [x] Switch `parse_config_by_format` to use the new provider for `.yaml` and
    `.yml` files, ensuring feature-gated builds continue to compile.
    [[ADR-001](adr-001-replace-serde-yaml-with-serde-saphyr.md)]

  - [x] Extend `ortho_config/src/file/file_tests.rs` with YAML 1.2 compliance
    coverage (`key: yes` remains a string, duplicates are rejected) and add
    failure-path tests for malformed YAML inputs.
    [[ADR-001](adr-001-replace-serde-yaml-with-serde-saphyr.md)]

  - [x] Document the migration in `CHANGELOG.md`, update user guides to call out
    YAML 1.2 compliance, and plan a minor version bump for the release.
    [[ADR-001](adr-001-replace-serde-yaml-with-serde-saphyr.md)]

- [x] **Replace `cucumber-rs` behavioural tests with `rstest-bdd`**
  [[ADR-002](adr-002-replace-cucumber-with-rstest-bdd.md)]

  - [x] Add `rstest-bdd` scaffolding (dev-dependencies, fixture modules, and a
    canary scenario) inside `ortho_config` and `hello_world` so the macros run
    under `cargo test` without disabling the harness.

  - [x] Port every module under `ortho_config/tests/steps` to
    `rstest_bdd_macros`, bind the existing feature files via `scenarios!` or
    `#[scenario]`, and delete the bespoke `tests/cucumber.rs` runner.

  - [x] Migrate the `hello_world` example suite: convert the world helpers to
    `rstest` fixtures, rebind the feature file using compile-time tag filters,
    and drop the `tests/cucumber` harness plus its `[[test]]` entry.

  - [x] Remove the `cucumber`/`gherkin` dev-dependencies, clean the unused
    Tokio bits, and update the behavioural documentation plus the CHANGELOG to
    describe the new workflow.

- [ ] **Address future enhancements**

  - [ ] Explore asynchronous loading of configuration files and environment
    variables for applications that need non‑blocking startup.
    [[Design](design.md#7-future-work)]

  - [ ] Provide an API for registering custom `figment` providers (e.g. secrets
    managers or remote key‑value stores). [[Design](design.md#7-future-work)]

  - [ ] Investigate live reloading of configuration when files change,
    acknowledging that this lies outside the initial scope but is part of the
    long‑term vision. [[Design](design.md#7-future-work)]

  These items collectively define a coherent roadmap for advancing OrthoConfig
  toward the capabilities described in the design documents and bridging the
  gaps observed in the current implementation.

[^roadmap-behavioural]: `docs/behavioural-testing-in-rust-with-cucumber.md`.
[^roadmap-rstest]: `docs/rust-testing-with-rstest-fixtures.md`.
