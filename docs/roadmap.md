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

- [ ] **Introduce declarative configuration merging**

  - [ ] Design a library-level merging trait or API that composes layered
    configuration structs without the ad hoc Figment plumbing required in the
    example crate. [[Feedback](feedback-from-hello-world-example.md)]

  - [ ] Ensure merge failures map to `OrthoError` consistently so downstream
    binaries no longer need bespoke error conversion when combining loaders.
    [[Feedback](feedback-from-hello-world-example.md)]

  - [ ] Document the declarative merge flow with examples that cover global
    defaults, file overrides and CLI adjustments to codify expected behaviour.
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
toward the capabilities described in the design documents and bridging the gaps
observed in the current implementation.
