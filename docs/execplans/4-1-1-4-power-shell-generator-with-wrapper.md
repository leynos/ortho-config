# Ship PowerShell generator with wrapper module

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DONE

No PLANS.md file exists in this repository.

## Purpose / big picture

Deliver the PowerShell documentation output for `cargo-orthohelp` so a user can
run `Get-Help {BinName} -Full` in both Windows PowerShell 5.1 and PowerShell 7+
for the fixture config and see complete help. Success is observable when the
PowerShell module, Microsoft Assistance Markup Language (MAML) XML, and about
topic are generated with an `en-US` fallback, CommonParameters appear in
`Get-Help -Full`, and all tests (unit and behavioural) pass via the Makefile
gates.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Follow the PowerShell and Windows rules in
  `docs/cargo-orthohelp-design.md` section 7.2 (wrapper module, MAML, en-US
  fallback, CommonParameters, about topic, carriage return/line feed (CRLF),
  and byte order mark (BOM) expectations).
- Use `cap_std`/`cap_std::fs_utf8` and `camino` for filesystem access.
- Every new module must begin with a `//!` module comment and files must remain
  under 400 lines.
- Use en-GB spelling in documentation and wrap prose at 80 columns, per
  `docs/documentation-style-guide.md`.
- Tests must include unit tests with `rstest` and behavioural tests with
  `rstest-bdd` v0.4.0, per the user request and
  `docs/rstest-bdd-users-guide.md`.
- Do not introduce new dependencies unless strictly required; if a new crate is
  needed (for example for XML), stop and escalate first.
- Avoid `#[allow(...)]`; use `#[expect(..., reason = "...")]` only if a lint
  exemption is unavoidable and tightly scoped.
- Update `docs/users-guide.md`, `examples/hello_world`, and
  `docs/cargo-orthohelp-design.md` to reflect new behaviour and decisions.
- On completion, mark the 4.1.1 PowerShell generator item as done in
  `docs/roadmap.md`.

If satisfying the objective requires violating a constraint, do not proceed.
Document the conflict in `Decision Log` and escalate.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached.

- Scope: stop if implementation requires changes to more than 25 files or more
  than 1,500 net lines of code.
- Interface: stop if a public API in `ortho_config` must change in a breaking
  way.
- Dependencies: stop if any new external crate is required.
- Iterations: stop if tests still fail after two fix attempts.
- Ambiguity: stop if MAML schema requirements or PowerShell command mapping are
  still unclear after reviewing the design doc and existing examples.
- Platform: stop if PowerShell 5.1/7+ integration tests cannot be executed or
  safely skipped in continuous integration (CI) without breaking the
  requirement.

## Risks

Known uncertainties that might affect the plan.

- Risk: MAML schema nuances could cause `Get-Help` failures even when XML looks
  correct. Severity: high. Likelihood: medium. Mitigation: add Windows-only
  integration tests that run `Get-Help -Full` under both `powershell.exe` and
  `pwsh` when available; validate output contains expected headings and
  parameters.
- Risk: CRLF and UTF-8 BOM handling may be required for compatibility with
  Windows PowerShell. Severity: medium. Likelihood: high. Mitigation: add unit
  tests for line endings and BOMs and use explicit writers in the generator.
- Risk: Locale fallback could overwrite non-English outputs or produce duplicate
  files. Severity: medium. Likelihood: medium. Mitigation: define explicit
  `ensure_en_us` behaviour with unit tests and deterministic file ordering.
- Risk: Behavioural tests may need to skip when PowerShell is unavailable,
  risking unmet acceptance criteria. Severity: medium. Likelihood: medium.
  Mitigation: add a clear skip path in tests with evidence in logs and ensure
  Windows CI still exercises the commands.

## Progress

- [x] (2026-02-03 00:00Z) Draft ExecPlan created.
- [x] (2026-02-04 00:00Z) Plan approved; implementation started.
- [x] (2026-02-04 02:30Z) Review existing roff generator, IR structures, and
  fixture data for reuse.
- [x] (2026-02-04 03:30Z) Implement PowerShell generator modules and integrate
  CLI/metadata.
- [x] (2026-02-04 04:15Z) Add unit, golden, and behavioural tests (rstest +
  rstest-bdd).
- [x] (2026-02-04 04:30Z) Update documentation, examples, and roadmap entry.
- [x] (2026-02-04 06:45Z) Run validation gates and confirm `Get-Help -Full`
  acceptance.

## Surprises & discoveries

- Observation: none yet
  Evidence: none Impact: none

## Decision log

- Decision: Emit about topics under the locale folder.
  Rationale: PowerShell loads about topics from the culture folder and this
  avoids collisions when multiple locales are generated. Date/Author:
  2026-02-04 / Codex.

- Decision: Use `[package.metadata.ortho_config.windows]` for PowerShell
  defaults. Rationale: Mirrors the IR `windows` metadata and keeps overrides
  grouped without mixing with non-Windows settings. Date/Author: 2026-02-04 /
  Codex.

## Outcomes & retrospective

- Outcome: Completed. PowerShell output generation, tests, documentation
  updates, and continuous integration (CI) validation are in place;
  `make check-fmt`, `make lint`, and `make test` pass locally.

## Context and orientation

`cargo-orthohelp` currently emits localized intermediate representation (IR)
JSON and roff man pages. The IR schema and PowerShell requirements live in
`docs/cargo-orthohelp-design.md`. The roff generator is implemented under
`cargo-orthohelp/src/roff` and is a useful model for structuring output
modules, error handling, and tests. The PowerShell generator must consume
`LocalizedDocMetadata` from `cargo-orthohelp/src/ir.rs` and respect Windows
metadata in the IR (`WindowsMetadata`).

Important paths:

- `cargo-orthohelp/src/main.rs` dispatches formats and writes outputs.
- `cargo-orthohelp/src/cli.rs` defines CLI arguments, including PowerShell
  flags.
- `cargo-orthohelp/src/metadata.rs` parses `package.metadata.ortho_config`,
  including Windows defaults for PowerShell output.
- `tests/fixtures/orthohelp_fixture` provides the fixture config and Fluent
  messages used by integration tests and should be expanded for PowerShell.
- Behavioural tests live under `cargo-orthohelp/tests/rstest_bdd` and already
  run `cargo-orthohelp` against the fixture crate.

The desired artefacts for PowerShell output are, per locale:

- `out/powershell/{ModuleName}/{ModuleName}.psm1`
- `out/powershell/{ModuleName}/{ModuleName}.psd1`
- `out/powershell/{ModuleName}/{culture}/{ModuleName}-help.xml`
- `out/powershell/{ModuleName}/{culture}/about_{ModuleName}.help.txt`

`en-US` must always exist. If another locale is generated without `en-US`, copy
that locale into `en-US`.

## Plan of work

Stage A: Understand and specify inputs (no code changes).

Review `docs/cargo-orthohelp-design.md` (PowerShell section), current
`cargo-orthohelp` modules (`ir.rs`, `main.rs`, `cli.rs`, `metadata.rs`), and
the fixture crate. Document any ambiguities in the Decision Log before
proceeding.

Stage B: Scaffolding and core data model (small, verifiable diffs).

Add a new `cargo-orthohelp/src/powershell` module with submodules for XML
(MAML) generation, wrapper module writing, manifest writing, and about-topic
rendering. Introduce a `PowerShellConfig` (name to be finalized) that resolves
settings from CLI flags, Cargo metadata, and IR `WindowsMetadata` in that order
and captures:

- `module_name`
- `export_aliases`
- `include_common_parameters`
- `split_subcommands`
- `help_info_uri`
- `ensure_en_us`
- resolved `bin_name`

Stage C: Implement generator and integration.

Implement a `powershell::generate` entry point that writes the module tree
under `out/powershell/{ModuleName}/` using `cap_std` and `camino`. The
generator must:

- Emit `.psm1` and `.psd1` with CRLF line endings.
- Emit MAML XML with UTF-8 BOM and CRLF line endings.
- Emit `about_{ModuleName}.help.txt` per locale (use about/discovery/precedence
  text from the IR; if details are missing, emit a minimal about topic with the
  app name and description).
- Always ensure `en-US` exists, copying from the first rendered locale if
  needed.
- Include CommonParameters in the MAML when `include_common_parameters` is
  true.
- Register argument completion in the wrapper, using `-Native` only when the
  PowerShell runtime supports it.
- Resolve the executable path relative to `$PSScriptRoot` and forward `@Args`.

Update `cargo-orthohelp/src/cli.rs` with a `PowerShellArgs` struct and the new
flags:

- `--ps-module-name`
- `--ps-split-subcommands`
- `--ps-include-common-parameters`
- `--ps-help-info-uri`
- `--ensure-en-us`

Update `metadata.rs` to parse optional PowerShell defaults from
`package.metadata.ortho_config` and thread them into configuration resolution.
Update `main.rs` to remove the `UnsupportedFormat` error for `ps`, route
`OutputFormat::Ps`/`All` to the PowerShell generator, and build a config
instance from CLI + metadata + IR.

Stage D: Tests (unit, golden, behavioural, Windows integration).

Add unit tests with `rstest` for:

- MAML XML escaping and type mapping from `ValueType`.
- `en-US` fallback behaviour.
- Wrapper module content (function name, `$PSScriptRoot`, completion logic).
- Manifest output (`ExternalHelp`, `HelpInfoUri` omission when absent).

Add golden tests for:

- Generated `.psm1` and `.psd1` content.
- MAML XML structure for the fixture config (use stable ordering).
- about topic output.

Add behavioural tests with `rstest-bdd` that:

- Run `cargo-orthohelp --format ps` against the fixture crate.
- Assert output file layout exists for locales, including `en-US` fallback.
- Validate key strings are present (command name, parameter names, help text).

Add Windows-only integration tests (guarded with `#[cfg(windows)]`) that:

- Import the generated module under `powershell.exe` and `pwsh` if present.
- Run `Get-Help {BinName} -Full` and assert it contains the about text and
  CommonParameters section.

Stage E: Documentation and examples.

Update `docs/cargo-orthohelp-design.md` with any decisions (for example, MAML
mapping details or fallback behaviour). Update `docs/users-guide.md` with
PowerShell generation usage and output layout. Update
`examples/hello_world/README.md` (and scripts if present) to show running
`cargo orthohelp --format ps` and how to import the module in PowerShell. Mark
the 4.1.1 PowerShell generator item as done in `docs/roadmap.md` when all
validation passes.

Stage F: Validation.

Run `make markdownlint`, `make fmt`, and `make nixie` after documentation
changes. Run the standard quality gates with log capture and verify success:

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/ps-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/ps-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/ps-test.log
```

## Concrete steps

1. Review design and existing implementation:

   - Read `docs/cargo-orthohelp-design.md` section 7.2.
   - Inspect `cargo-orthohelp/src/roff` for patterns to reuse.
   - Review `cargo-orthohelp/src/ir.rs` and `schema/mod.rs` for Windows
     metadata usage.

2. Add PowerShell CLI flags and metadata parsing:

   - Extend `cargo-orthohelp/src/cli.rs` with a `PowerShellArgs` struct and
     new CLI flags.
   - Extend `cargo-orthohelp/src/metadata.rs` to parse PowerShell defaults from
     `package.metadata.ortho_config`.

3. Implement PowerShell generator module:

   - Create `cargo-orthohelp/src/powershell/mod.rs` with a `generate` entry
     point.
   - Add helper modules (names may vary): `maml.rs`, `wrapper.rs`,
     `manifest.rs`,
     `about.rs`, `types.rs`, `writer.rs`.
   - Ensure each module has a `//!` comment and stays under 400 lines.

4. Update `cargo-orthohelp/src/main.rs` dispatch:

   - Remove the `UnsupportedFormat` error for `ps`.
   - Compute the PowerShell config and call `powershell::generate` for
     `OutputFormat::Ps` and `OutputFormat::All`.

5. Update the fixture crate for PowerShell content:

   - Add or refine fields to ensure there are CLI options, environment
     variables, file keys, enum values, and notes that should appear in MAML.
   - Update `tests/fixtures/orthohelp_fixture/locales/*/messages.ftl` with any
     new Fluent IDs used by PowerShell output.

6. Add tests:

   - Unit tests under `cargo-orthohelp/src/powershell` using `rstest`.
   - Golden tests under `cargo-orthohelp/tests/golden` for `.psm1`, `.psd1`,
     MAML XML, and about topic.
   - Behavioural tests under `cargo-orthohelp/tests/rstest_bdd` with a new
     feature file and step definitions for PowerShell output.
   - Windows-only integration tests for `Get-Help` where PowerShell is
     available.

7. Documentation and examples:

   - Update `docs/users-guide.md` and `docs/cargo-orthohelp-design.md`.
   - Update `examples/hello_world/README.md` and any related scripts.
   - Mark the roadmap entry as done in `docs/roadmap.md`.

8. Validation:

   - Run `make markdownlint`, `make fmt`, and `make nixie` after doc changes.
   - Run `make check-fmt`, `make lint`, and `make test` with tee and pipefail.

## Validation and acceptance

Acceptance is met when:

- `cargo-orthohelp --format ps` generates a PowerShell module containing
  `.psm1`, `.psd1`, MAML XML, and about topic output per locale, and an `en-US`
  fallback is always present.
- `Get-Help {BinName} -Full` works in Windows PowerShell 5.1 and PowerShell 7+
  for the fixture config, showing CommonParameters when enabled.
- Unit tests (rstest), behavioural tests (rstest-bdd), and golden tests pass.
- Documentation and examples reflect the new PowerShell output usage.
- `make check-fmt`, `make lint`, `make test` succeed, plus markdown linting and
  formatting checks for docs.

## Idempotence and recovery

All steps are safe to re-run. Output directories can be regenerated from
scratch. If tests fail, inspect the log files in `/tmp` and re-run the relevant
Make targets after fixes. Windows integration tests must be guarded so missing
PowerShell binaries produce a clear skip rather than a failure.

## Artifacts and notes

Key artefacts created:

- `cargo-orthohelp/src/powershell/*` new generator modules.
- `cargo-orthohelp/tests/golden/powershell/*` golden fixtures.
- `cargo-orthohelp/tests/features/orthohelp_powershell.feature` behavioural
  scenarios and matching step definitions.
- Updated fixture Fluent files under
  `tests/fixtures/orthohelp_fixture/locales/`.

## Interfaces and dependencies

- Use existing `LocalizedDocMetadata` and `WindowsMetadata` types from
  `cargo-orthohelp/src/ir.rs` and `schema/mod.rs`.
- Do not add new dependencies; implement XML output with manual escaping and
  structured builders.
- Ensure generator API mirrors `roff::generate` style:

```rust
pub fn generate(
    metadata: &LocalizedDocMetadata,
    config: &PowerShellConfig
) -> Result<PowerShellOutput, OrthohelpError>
```

- Output types should return the list of generated files for testing and
  logging.

## Revision note

Initial draft created for roadmap item 4.1.1 (PowerShell generator with wrapper
module).
