# Documentation contents

- [Documentation contents](contents.md): start here to find the repository's
  documentation set.

## Primary guides

- [User's guide](users-guide.md): use OrthoConfig in application and library
  code, including configuration loading, merge behaviour, generated
  documentation metadata, and error handling.
- [Developer's guide](developers-guide.md): maintain and extend this
  repository, including build, test, lint, release, and contribution workflows.
- [Repository layout](repository-layout.md): understand where source code,
  tests, fixtures, generated artefacts, plans, and long-lived references live.

## Product and architecture

- [Design Document: The `OrthoConfig` Crate](design.md): understand the core
  crate architecture, configuration model, merge pipeline, localization
  strategy, and future extension points.
- [Agent-native CLI assistance design](agent-native-cli-design.md): use the
  canonical agent-native command-contract and boundary document for the
  metadata surfaces, policy model, and consumer application responsibilities.
- [OrthoConfig IR documentation design for cargo-orthohelp](cargo-orthohelp-design.md):
  understand the documentation intermediate representation (IR), bridge
  pipeline, localized reference generation, PowerShell help, and future
  agent-context output.
- [Roadmap](roadmap.md): track active future work, numbered after the archived
  v0.8.0 roadmap.

## Decisions and archives

- [ADR-001: Replace `serde_yaml` with `serde-saphyr`](adr-001-replace-serde-yaml-with-serde-saphyr.md):
  review the accepted YAML parser replacement and migration consequences.
- [ADR-002: Replace `cucumber-rs` with `rstest-bdd`](adr-002-replace-cucumber-with-rstest-bdd.md):
  review the accepted behavioural testing migration and expected workflow.
- [ADR-003: Define schema ownership for agent-native contracts](adr-003-define-schema-ownership-for-agent-native-contracts.md):
  review the accepted ownership split for documentation IR, agent context, and
  policy reports.
- [ADR-004: Cargo external-subcommand entry-point architecture](adr-004-cargo-external-subcommand-entry-point.md):
  review the accepted Cargo dispatch boundary and wrapper shape.
- [ADR-005: Subcommand docs companion trait](adr-005-subcommand-docs-companion-trait.md):
  review the accepted companion trait for recursive subcommand documentation
  metadata.
- [ADR-006: Identifier-derivation panics](adr-006-identifier-derivation-panics.md):
  review the accepted panic-on-collision and panic-on-unrepresentable-segment
  contract for `message_id_for` and `LocalizeCmd::localize`.
- [Archived v0.8.0 roadmap](archive/v0-8-0-roadmap.md): review completed
  phases, steps, and tasks from the roadmap that preceded the active
  agent-native plan.

## Requests for comments (RFCs)

- [RFC 0002: Customizable configuration layering policy](rfcs/0002-config-layer-resolution-policy.md):
  review the proposed generic file-layer resolution policy — ordered explicit
  selectors, multi-scope stacking, and a reusable resolver — with Netsuke as the
  proof case.

## Feature and migration notes

- [Clap dispatch and OrthoConfig integration](clap-dispatch-and-ortho-config-integration.md):
  review the design notes for combining `clap-dispatch` with OrthoConfig
  loaders.
- [Clap mangen to cargo-orthohelp migration guide](clap-mangen-cargo-orthohelp-migration-guide.md):
  migrate documentation generation from `clap_mangen` to `cargo-orthohelp`.
- [DDLint gap analysis](ddlint-gap-analysis.md): review historical DDLint
  requirements and how they now inform agent-native command policy.
- [Feedback from hello_world example](feedback-from-hello-world-example.md):
  review historical proposals derived from the example crate.
- [Improved error message design](improved-error-message-design.md): review
  the proposed missing-required-values diagnostic design.
- [Subcommand refinements](subcommand-refinements.md): review historical
  subcommand merge proposals and the later whole-command introspection need.
- [v0.6.0 migration guide](v0-6-0-migration-guide.md): migrate through the
  v0.6.0 changes.
- [v0.7.0 migration guide](v0-7-0-migration-guide.md): migrate through the
  v0.7.0 changes.

## Testing and documentation references

- [Behavioural testing with cucumber](behavioural-testing-in-rust-with-cucumber.md):
  review the legacy behaviour-driven development (BDD) approach retained as
  historical reference.
- [Behavioural tests](behavioural-tests.md): understand the current behavioural
  test organization and expectations.
- [Complexity antipatterns and refactoring strategies](complexity-antipatterns-and-refactoring-strategies.md):
  identify common complexity smells and refactoring responses.
- [Documentation style guide](documentation-style-guide.md): follow the house
  style, document type conventions, Markdown rules, and repository layout
  requirements.
- [Localizable Rust libraries with Fluent](localizable-rust-libraries-with-fluent.md):
  understand the Fluent localization approach used by OrthoConfig.
- [CLI localization surface design](cli-localization-design.md): proposed
  promotion of the load-bearing localization helpers (`LocalizeCmd`,
  `BootLocalizer`, widened clap-error coverage, `i18n-embed` bridge,
  derive-emitted identifiers) from the `hello_world` example to crate API.
- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md):
  apply dependency injection patterns in tests.
- [Rstest BDD users guide](rstest-bdd-users-guide.md): use the current
  `rstest-bdd` testing workflow.
- [Rstest BDD v0.5.0 migration guide](rstest-bdd-v0-5-0-migration-guide.md):
  migrate to `rstest-bdd` v0.5.0.
- [Rust doctest dry guide](rust-doctest-dry-guide.md): write maintainable Rust
  doctests.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md):
  structure shared fixtures and parameterized Rust tests.

## Execution plans

- [ExecPlans directory](execplans/): review living and historical execution
  plans for substantial work.
  - [PowerShell generator with wrapper](execplans/4-1-1-4-power-shell-generator-with-wrapper.md):
    plan for the PowerShell help generator, wrapper module, and validation.
  - [OrthoConfigDocs IR schema in derive macro](execplans/4-1-1-ortho-config-docs-ir-schema-in-derive-macro.md):
    plan for generating documentation IR from the derive macro.
  - [OrthoConfig help bridge pipeline](execplans/4-1-1-ortho-config-help-bridge-pipeline.md):
    plan for the `cargo-orthohelp` bridge pipeline.
  - [Roff generator](execplans/4-1-3-roff-generator.md): plan for generated
    roff man pages.
  - [Reconcile design with actual error surface](execplans/5-1-1-reconcile-design-with-actual-error-surface.md):
    plan for roadmap item 5.1.1 and the documentation alignment required after
    the implemented error model changed.
  - [Retire stale retrospective roadmap items](execplans/5-1-2-retire-stale-retrospective-roadmap-items.md):
    plan for roadmap item 5.1.2, which separates active guidance from
    historical roadmap and design context.
  - [Agent-native documentation index](execplans/5-1-3-agent-native-documentation-index.md):
    plan for roadmap item 5.1.3 and the canonical documentation index and
    repository layout.
  - [Schema ownership ExecPlan](execplans/5-2-1-define-ownership-models.md):
    plan for roadmap item 5.2.1 and its approval gate.
  - [Consumer dependency boundaries](execplans/5-2-3-record-consumer-dependency-boundaries.md):
    plan for roadmap item 5.2.3 and the hard and soft dependency boundaries
    between OrthoConfig, Weaver, and Netsuke.
  - [Recursive documentation metadata](execplans/6-1-1-recursive-doc-metadata-subcommands-values.md):
    plan for recursive subcommand and value metadata in generated command
    documentation.
  - [Skill manifest metadata](execplans/6-3-1-skill-manifest-metadata.md):
    plan for roadmap item 6.3.1 and the additive agent-context types that
    model downstream skill manifests.
  - [Adopt rstest-bdd v0.5.0](execplans/adopt-rstest-bdd-v0-5-0.md): plan for
    the behavioural testing migration.
  - [Ortho agent CLI roadmap](execplans/ortho-agent-cli-roadmap.md): plan for
    the agent-native documentation and roadmap overhaul.
