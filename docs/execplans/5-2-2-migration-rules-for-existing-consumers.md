# Record migration rules for existing consumers

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
 `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / big picture

Roadmap item 5.2.2 records how existing `cargo-orthohelp` consumers are kept
whole while OrthoConfig grows new agent-native metadata. After this change,
maintainers and downstream crates can read one documented compatibility policy
that says which existing outputs are stable, how new metadata fields default
when older derives do not provide them, and what human-documentation-only
consumers may safely ignore.

The observable success condition is documentation and tests that make the
existing public behaviour of `cargo orthohelp --format ir`, `--format man`,
`--format ps`, and `--format all` explicit. No implementation may begin from
this plan until the user approves it.

## Constraints

- Do not implement this plan until explicit approval is received.
- Preserve the current `--format` option domain: `ir`, `man`, `ps`, and `all`
  remain accepted values, and the default remains `ir`.
- Preserve the current `--format all` contract: generate localized IR, then
  man pages, then PowerShell artefacts, reporting success or failure through
  process exit status.
- Preserve the IR output path contract:
  `<out>/ir/<locale>.json`.
- Preserve the man-page output path contract:
  `<out>/man/man<section>/<name>.<section>` for one locale and
  `<out>/<locale>/man/man<section>/<name>.<section>` for multiple locales.
- Preserve the PowerShell output path contract under
  `<out>/powershell/<ModuleName>/`, including module files, localized MAML
  help, about topics, and default `en-US` support unless `--ensure-en-us false`
  is supplied.
- Keep documentation IR and future agent-context schemas independently
  versioned. Do not make an agent-context addition silently require a
  documentation IR migration.
- New metadata fields must have explicit legacy defaults. Do not infer
  mutation, JSON support, interaction mode, exit classes, pagination,
  capability provenance, or profile support from command names or absent data.
- Human-facing documentation consumers must be able to keep consuming generated
  man pages and PowerShell help without adopting agent-context metadata.
- Avoid public API signature changes unless the user explicitly approves them
  after a tolerance-triggering escalation.
- Avoid new crate dependencies unless the user explicitly approves them after a
  tolerance-triggering escalation.
- Avoid circular crate dependencies at all costs. Shared contracts must live in
  lower-level crates rather than back edges from generators into consumers.
- Keep all Markdown in en-GB Oxford style and follow
  `docs/documentation-style-guide.md`.
- Use the repository Makefile targets for gates. Run
  `make check-fmt`, `make lint`, and `make test` sequentially with `tee`.
- Run `coderabbit review --agent` after each major milestone and clear all
  concerns before proceeding.
- Commit after each approved implementation milestone only after its gates
  pass.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 12 files or more than
  900 net lines, stop and escalate.
- Interface: if any public Rust API signature, CLI option spelling, generated
  file path, or generated output format must change, stop and escalate.
- Dependencies: if a new external crate or tool dependency is required, stop
  and escalate.
- Schema: if the documentation IR major version must change, stop and
  escalate.
- Behaviour: if any existing golden, BDD, or end-to-end test must be rewritten
  because the old behaviour is no longer valid, stop and escalate.
- Tests: if the same gate fails twice after targeted fixes, stop and escalate
  with the failure summary and options.
- Review: if `coderabbit review --agent` reports a concern that would require
  violating a constraint, stop and escalate.
- Ambiguity: if multiple valid migration policies exist and the choice affects
  downstream compatibility, stop and present the options.

## Risks

- Risk: compatibility wording could promise more stability than the current
  implementation can enforce. Severity: high. Likelihood: medium. Mitigation:
  add characterization tests before changing policy text, and phrase
  documentation around behaviours actually covered by tests.

- Risk: adding optional metadata fields to JSON shapes could still break strict
  downstream parsers. Severity: medium. Likelihood: medium. Mitigation:
  document strict-parser expectations and require explicit schema migrations
  before adding fields to existing stable outputs.

- Risk: human-facing consumers may accidentally be forced into future
  agent-context contracts. Severity: medium. Likelihood: medium. Mitigation:
  document the split in `docs/agent-native-cli-design.md`,
  `docs/cargo-orthohelp-design.md`, and `docs/users-guide.md`.

- Risk: documentation-only edits can drift from code.
  Severity: medium. Likelihood: medium. Mitigation: include tests that capture
  current format behaviour and run the full gates before committing.

- Risk: `coderabbit review --agent` or GitHub operations may be unavailable in
  the local environment. Severity: medium. Likelihood: low. Mitigation: record
  the exact command and failure, then escalate rather than claiming review or
  PR creation succeeded.

## Progress

- [x] (2026-05-20T23:06:46Z) Loaded the `execplans`, `leta`,
  `rust-router`, `arch-crate-design`, `firecrawl-mcp`, `en-gb-oxendict-style`,
  `commit-message`, and `pr-creation` skills.
- [x] (2026-05-20T23:06:46Z) Renamed the working branch from
  `feat/522-migration-rules` to `5-2-2-migration-rules-for-existing-consumers`.
- [x] (2026-05-20T23:06:46Z) Used a Wyvern agent team for read-only
  reconnaissance of code and documentation surfaces.
- [x] (2026-05-20T23:06:46Z) Used Firecrawl to check external prior art for
  schema evolution, SemVer public API compatibility, JSON Schema defaults, and
  CLI output-generation practice.
- [x] (2026-05-20T23:06:46Z) Created a finalised context pack named
  `5-2-2-migration-rules-planning` for team context exchange.
- [x] (2026-05-20T23:06:46Z) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-20T23:21:00Z) Ran `make check-fmt`, `make lint`,
  `make test`, and `make markdownlint` for the plan branch.
- [x] (2026-05-20T23:26:00Z) Ran `coderabbit review --agent` and addressed
  all minor documentation-style findings in this ExecPlan.
- [x] (2026-05-23T02:40:00Z) Rebasing onto `origin/main` completed without
  conflicts after commit `2b992b67bc5d5e4e763a8ee814bc0314b34cd99b` landed.
- [x] (2026-05-23T02:45:00Z) Updated this ExecPlan to build on the completed
  roadmap item 5.2.1 schema ownership baseline.
- [x] (2026-05-24T12:32:40Z) Received explicit user approval to proceed with
  implementation of this ExecPlan.
- [x] (2026-05-24T12:36:00Z) Established the implementation baseline:
  `make check-fmt`, `make lint`, `make test`, and `make typecheck` all passed
  sequentially with logs under `/tmp`.
- [x] (2026-05-24T12:51:00Z) Added characterization coverage for legacy
  `--format` parsing and observable `cargo-orthohelp` output paths, then ran
  `cargo test -p cargo-orthohelp --test rstest_bdd --all-features` successfully
  after wiring the dormant BDD harness into Cargo.
- [x] (2026-05-24T13:12:00Z) Ran the characterization milestone gates:
  `make check-fmt`, `make test`, `make typecheck`, `make lint`, and
  `make markdownlint` all passed after fixing dormant BDD harness lint issues.
- [x] (2026-05-24T13:22:00Z) Ran `coderabbit review --agent` for the
  characterization milestone, addressed its two trivial harness-entry findings,
  and reran `make check-fmt`, `make test`, `make typecheck`, `make lint`, and
  `make markdownlint` successfully.
- [ ] Implement the approved plan milestone by milestone.
- [ ] Mark roadmap item 5.2.2 done only after the approved implementation and
  validation are complete.

## Surprises & discoveries

- Observation: `docs/agent-native-cli-design.md` already contains a defaulting
  table for legacy derives. Evidence: section 8.1 lists defaults such as
  `supports_json = false`, `interaction_mode = "unknown"`, and
  `renderer.machine.supported = false`. Impact: implementation should extend
  and cross-link this existing policy instead of creating a separate migration
  document unless review proves a standalone guide is needed.

- Observation: `docs/cargo-orthohelp-design.md` already describes planned
  `--format agent-context`, `--json`, and `--check-agent-native` additions as
  not yet implemented. Evidence: section 6.1 states these are planned
  agent-native additions and that existing formats remain the implemented
  surface. Impact: implementation should make the migration boundary around
  existing formats more explicit in that section.

- Observation: JSON Schema `default` is annotation metadata, not validator
  mutation. Evidence: Firecrawl research of JSON Schema annotation
  documentation found that validation does not fill missing values. Impact:
  OrthoConfig generators and readers must implement their own legacy defaults
  rather than assuming schema validation supplies them.

- Observation: commit `2b992b67bc5d5e4e763a8ee814bc0314b34cd99b` completed
  roadmap item 5.2.1 and introduced the concrete schema ownership split.
  Evidence: `ortho_config::agent_context` owns
  `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`, `cargo_orthohelp::policy` owns
  `ORTHO_POLICY_REPORT_SCHEMA_VERSION`, and
  `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` records
  the ownership decision. Impact: item 5.2.2 should document migration rules on
  top of that accepted boundary, not re-decide ownership.

- Observation: commit `2b992b67bc5d5e4e763a8ee814bc0314b34cd99b` added a
  `.config/nextest.toml` timeout override for trybuild integration tests.
  Evidence: the commit message calls out longer timeouts for child Cargo builds
  in separate target directories. Impact: future validation may keep using the
  repository Make targets, and any optional nextest runs should respect the
  checked-in profile rather than introducing ad hoc timeout workarounds.

- Observation: the `cargo-orthohelp/tests/rstest_bdd/` feature and step files
  were present but were not compiled by Cargo because no
  `cargo-orthohelp/tests/rstest_bdd.rs` integration-test entry point existed.
  Evidence: `cargo test -p cargo-orthohelp --test rstest_bdd --all-features`
  initially reported no such test target. Impact: implementation added the
  missing harness entry point and corrected stale step wiring so the planned
  behavioural coverage actually runs.

- Observation: the fixture binary's generated command name is `fixture`, while
  the Cargo package remains `orthohelp_fixture`. Evidence: a retained manual
  `--format all` run wrote `man/man1/fixture.1` and locale-specific
  `<locale>/man/man1/fixture.1` files. Impact: BDD assertions now check the
  generated command name rather than the package name.

## Decision log

- Decision: Treat this branch as a pre-implementation plan branch and leave
  roadmap item 5.2.2 unchecked until implementation completes. Rationale: the
  user explicitly said the plan must be approved before it is implemented.
  Marking the roadmap done now would misrepresent the feature state.
  Date/Author: 2026-05-20T23:06:46Z / Codex.

- Decision: Use explicit compatibility preservation rather than version bumping
  as the default strategy for this task. Rationale: roadmap item 5.2.2 says
  existing `ir`, `man`, `ps`, and `all` behaviours remain compatible until a
  versioned migration is explicitly approved. Date/Author: 2026-05-20T23:06:46Z
  / Codex.

- Decision: Put migration rules into existing design and guide documents first,
  not into a new standalone migration guide. Rationale: the current
  documentation already has versioning, defaulting, and consumer-facing
  sections. A new document would add navigation cost unless the implementation
  uncovers enough policy to justify it. Date/Author: 2026-05-20T23:06:46Z /
  Codex.

- Decision: Use characterization tests for existing output contracts before
  editing implementation code. Rationale: the policy must be backed by
  executable evidence that current behaviour remains stable. Date/Author:
  2026-05-20T23:06:46Z / Codex.

- Decision: Keep the conflict-free rebase result and update this plan as a
  follow-up commit rather than amending the original plan commit. Rationale:
  `origin/main` introduced a substantive 5.2.1 schema baseline after the draft
  plan was opened, and the branch history should show the rebase-driven plan
  adjustment separately from the initial draft. Date/Author:
  2026-05-23T02:45:00Z / Codex.

- Decision: Serialize `cargo-orthohelp` BDD scenarios with a shared test
  fixture lock. Rationale: the scenarios intentionally remove and rebuild the
  shared `target/orthohelp` bridge cache, so parallel scenario execution can
  invalidate another scenario's cache assertion. Date/Author:
  2026-05-24T12:51:00Z / Codex.

- Decision: Treat the missing `rstest_bdd` integration-test target as part of
  the characterization milestone. Rationale: the roadmap requires behavioural
  tests using `rstest-bdd` where applicable, and feature files that Cargo does
  not run cannot validate the migration contract. Date/Author:
  2026-05-24T12:51:00Z / Codex.

## Outcomes & retrospective

This section is intentionally empty while the plan is in draft. During
implementation, record which compatibility rules were documented, which tests
were added or strengthened, which review concerns were cleared, and whether any
follow-up roadmap items were created.

Implementation began on 2026-05-24 after explicit user approval.

## Context and orientation

OrthoConfig is a Rust workspace. The `ortho_config` crate owns runtime
configuration loading, merge behaviour, localization, and documentation IR
types. The `ortho_config_macros` crate derives metadata and loading code. The
`cargo-orthohelp` binary consumes documentation IR and generates user-facing
documentation artefacts.

The current output format enum lives in `cargo-orthohelp/src/cli.rs` as
`OutputFormat::{Ir, Man, Ps, All}`. The parsed `--format` option defaults to
`OutputFormat::Ir`.

The format dispatch lives in `cargo-orthohelp/src/main.rs`. It currently maps
`Ir` and `All` to `generate_ir`, `Man` and `All` to `generate_man`, and `Ps` and
 `All` to `generate_powershell`. In one `all` run the order is IR, man, then
PowerShell.

The base documentation IR type lives in `ortho_config/src/docs/ir.rs`.
`DocMetadata` contains `ir_version`, application naming, sections, fields,
recursive `subcommands`, and optional `windows` metadata. This is the human
documentation contract. The future agent-context schema is a sibling machine
contract and must be versioned separately.

After roadmap item 5.2.1, the sibling machine contracts are no longer only
future design notes. Reusable agent context types live in
`ortho_config/src/agent_context/mod.rs` with
`ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`. Policy-report types live in
`cargo-orthohelp/src/policy/mod.rs` with `ORTHO_POLICY_REPORT_SCHEMA_VERSION`.
The implementation for this plan should preserve that ownership split and add
migration rules around compatibility, defaulting, and downstream documentation
consumers.

The `cargo-orthohelp` schema mirror lives in `cargo-orthohelp/src/schema/`. The
localized IR writer lives in `cargo-orthohelp/src/output.rs`. Roff man-page
generation lives under `cargo-orthohelp/src/roff/`. PowerShell wrapper and MAML
generation live under `cargo-orthohelp/src/powershell/`.

Existing behavioural tests live under `cargo-orthohelp/tests/features/` and
`cargo-orthohelp/tests/rstest_bdd/behaviour/`. Existing golden tests live under
`cargo-orthohelp/tests/golden/`. Windows-specific PowerShell validation lives in
 `cargo-orthohelp/tests/powershell_windows.rs`.

Relevant documentation sources are:

- `docs/roadmap.md` for item 5.2.2 and completion state.
- `docs/design.md` for high-level architecture and decision history.
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` for the
  accepted schema ownership decision from roadmap item 5.2.1.
- `docs/agent-native-cli-design.md` for agent-native versioning and legacy
  defaulting.
- `docs/cargo-orthohelp-design.md` for the documentation IR, generator, and
  output format contract.
- `docs/users-guide.md` for downstream consumer guidance.
- `docs/developers-guide.md` for internal practices and conventions.
- `docs/documentation-style-guide.md` for document structure, ADRs, and
  compatibility-and-migration sections.
- `docs/rust-testing-with-rstest-fixtures.md` for `rstest` unit test style.
- `docs/rstest-bdd-users-guide.md` for behavioural test style.
- `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/localizable-rust-libraries-with-fluent.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md` for supporting
  design and testing practice.
- `docs/execplans/5-2-1-define-ownership-models.md` for the completed
  predecessor plan that established the ownership model this plan depends on.

External prior art used while drafting this plan:

- Semantic Versioning, <https://semver.org/>, supports treating documented
  public output contracts as public API and reserving incompatible changes for
  versioned migrations.
- Confluent Schema Registry schema evolution documentation supports explicit
  backward, forward, full, and transitive compatibility language and the
  pattern of adding optional fields with defaults.
- JSON Schema annotation documentation,
  <https://json-schema.org/understanding-json-schema/reference/annotations>,
  clarifies that `default` is descriptive metadata and not a validation-time
  mutation.
- Clap documentation, <https://docs.rs/clap/latest/clap/>, identifies adjacent
  Rust CLI output-generation tooling such as `clap_mangen` and `clap_complete`,
  reinforcing that generated human artefacts are stable consumer surfaces.

## Plan of work

Stage A: approval and baseline. Stop until the user approves this plan. After
approval, check `git status --short --branch`, confirm the branch name, and run
the existing gates once to establish the starting state. Use `tee` for logs in
`/tmp` and do not run format, lint, or tests in parallel.

Stage B: characterize existing output behaviours. Add or strengthen focused
tests without changing production behaviour. Use `rstest` unit or golden tests
where the behaviour is a pure path, config, or generator contract. Use
`rstest-bdd` behavioural scenarios where the behaviour is externally observable
through `cargo orthohelp`. At minimum, ensure coverage proves: default
`--format ir`; accepted `ir`, `man`, `ps`, and `all` values; `--format all`
generator coverage; localized IR file paths; single-locale and multi-locale man
paths; PowerShell file layout; and explicit failure for an unsupported format
through Clap's `ValueEnum` parsing.

Stage C: document migration rules. Update `docs/agent-native-cli-design.md` to
extend the now-accepted schema ownership policy with normative
legacy-defaulting rules. Explicitly say defaults are applied by OrthoConfig
readers or generators, not by JSON Schema validation. Update
`docs/cargo-orthohelp-design.md` to list the stable legacy format behaviours
and to state that `agent-context`, `--json`, and policy output cannot break
`ir`, `man`, `ps`, or `all` without an approved migration. Update
`docs/design.md` with the compatibility boundary: OrthoConfig owns reusable
metadata contracts and generated documentation compatibility; downstream
applications own semantic execution. Reference ADR-003 where ownership matters
instead of re-litigating the decision. Update `docs/users-guide.md` with
consumer-facing compatibility notes for crates that only consume generated
human documentation. Update `docs/developers-guide.md` with internal practice
for adding metadata fields: define defaults, test legacy derives, keep policy
reports in `cargo_orthohelp::policy`, keep compact agent context in
`ortho_config::agent_context`, and avoid format drift. Update `CHANGELOG.md`
only if the implementation changes user-visible wording or behaviour. Do not
add a new ADR unless implementation uncovers a substantive migration decision
that cannot be captured cleanly in the design documents or ADR-003.

Stage D: review and harden. Run `make fmt` if Markdown formatting changes are
needed. Then run `make check-fmt`, `make lint`, and `make test` sequentially
with `tee`. Run `make typecheck` when the Makefile exposes that target. Also run
 `make markdownlint` because Markdown files change. Run `make nixie` if Mermaid
diagrams are added or edited. Run `coderabbit review --agent`, address every
concern that fits within this plan's constraints, and rerun affected gates. If
CodeRabbit asks for work outside tolerance, record it in this plan and escalate.

Stage E: commit and completion. Commit the implementation only after gates and
review pass. Mark item 5.2.2 in `docs/roadmap.md` done only after the approved
implementation is complete. Push the branch to
`origin/5-2-2-migration-rules-for-existing-consumers` and open or update a
draft PR whose title includes `(5.2.2)`.

## Concrete steps

Run all commands from the repository root:

```sh
pwd
```

Expected output ends with:

```plaintext
/home/leynos/.lody/repos/github---leynos---ortho-config/worktrees/45ca0a69-d4a0-4858-8b87-8b74efd1c0f6
```

After approval, establish the baseline:

```sh
git status --short --branch
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-ortho-config-$(git branch --show-current).out"
```

Expected result: each Make target exits with status 0. If a log is truncated in
the terminal, inspect the matching file under `/tmp`.

Inspect the current output surface before writing tests:

```sh
sed -n '1,130p' cargo-orthohelp/src/cli.rs
sed -n '30,170p' cargo-orthohelp/src/main.rs
sed -n '1,120p' cargo-orthohelp/src/output.rs
find cargo-orthohelp/tests -maxdepth 3 -type f | sort
```

Add characterization tests in the smallest appropriate locations. Likely
targets are:

- `cargo-orthohelp/tests/features/orthohelp_ir.feature`
- `cargo-orthohelp/tests/features/orthohelp_roff.feature`
- `cargo-orthohelp/tests/features/orthohelp_powershell.feature`
- `cargo-orthohelp/tests/rstest_bdd/behaviour/steps.rs`
- `cargo-orthohelp/tests/rstest_bdd/behaviour/roff_steps.rs`
- `cargo-orthohelp/tests/rstest_bdd/behaviour/powershell_steps.rs`
- `cargo-orthohelp/tests/golden/roff_tests.rs`
- `cargo-orthohelp/tests/golden/powershell_tests.rs`

Run targeted tests while developing:

```sh
cargo test -p cargo-orthohelp --test rstest_bdd --all-features 2>&1 |
  tee "/tmp/rstest-bdd-ortho-config-$(git branch --show-current).out"
cargo test -p cargo-orthohelp --test golden_tests --all-features 2>&1 |
  tee "/tmp/golden-ortho-config-$(git branch --show-current).out"
```

If the targeted command names differ, use
`cargo test --workspace --all-targets --all-features` and record the discovered
exact names in this plan.

Update documentation:

```sh
sed -n '520,585p' docs/agent-native-cli-design.md
sed -n '470,490p' docs/cargo-orthohelp-design.md
sed -n '781,789p' docs/cargo-orthohelp-design.md
sed -n '1128,1265p' docs/users-guide.md
sed -n '1,110p' docs/developers-guide.md
sed -n '1,180p' docs/adr-003-define-schema-ownership-for-agent-native-contracts.md
```

Run review and gates:

```sh
make fmt 2>&1 | tee "/tmp/fmt-ortho-config-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-ortho-config-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-ortho-config-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-ortho-config-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-ortho-config-$(git branch --show-current).out"
coderabbit review --agent 2>&1 |
  tee "/tmp/coderabbit-ortho-config-$(git branch --show-current).out"
```

Run `make nixie` only if Mermaid diagrams are added or edited:

```sh
make nixie 2>&1 | tee "/tmp/nixie-ortho-config-$(git branch --show-current).out"
```

Commit with a file-based message:

```sh
git status --short
git diff -- docs cargo-orthohelp ortho_config
git add <changed files>
git diff --cached
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Record migration rules for existing consumers

Document the compatibility boundary for existing `cargo-orthohelp`
formats and add tests that preserve the current output contracts before
future agent-native metadata expands the schema surface.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Before creating the PR:

```sh
echo "${LODY_SESSION_ID}"
git push -u origin 5-2-2-migration-rules-for-existing-consumers
```

Create or update a draft PR titled:

```plaintext
Record migration rules for existing consumers (5.2.2)
```

The PR body must mention this ExecPlan and include:

```markdown
## References

- Lody session: https://lody.ai/leynos/sessions/${LODY_SESSION_ID}
```

## Validation and acceptance

The implementation is accepted when all of the following are true:

- Running `cargo orthohelp` with no `--format` still generates localized IR as
  before.
- Running `cargo orthohelp --format ir` writes one localized IR JSON file per
  resolved locale under `<out>/ir/`.
- Running `cargo orthohelp --format man` writes roff man pages under the
  existing single-locale and multi-locale path conventions.
- Running `cargo orthohelp --format ps` writes the existing PowerShell module,
  manifest, MAML help, and about-topic layout, including default `en-US`
  handling.
- Running `cargo orthohelp --format all` generates IR, man, and PowerShell
  artefacts in one invocation without introducing structured stdout.
- Passing an unsupported `--format` value still fails through Clap parsing.
- New tests use `rstest` for unit or golden checks and `rstest-bdd` for
  externally observable CLI workflows where applicable.
- Property tests, Kani, or Verus are not added unless implementation introduces
  a real invariant over a range of inputs, states, orderings, or transitions.
  Documentation-only policy and fixed format compatibility checks do not by
  themselves justify a proof harness.
- `docs/agent-native-cli-design.md`,
  `docs/cargo-orthohelp-design.md`, `docs/design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, and
  `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` describe
  the same migration policy without contradiction.
- `docs/roadmap.md` marks 5.2.2 done only after implementation is complete.
- `make check-fmt`, `make test`, `make typecheck`, `make lint`, and
  `make markdownlint` pass when those targets exist.
- `coderabbit review --agent` has no unresolved concerns within this plan's
  scope.

## Idempotence and recovery

The characterization tests and documentation edits are additive and safe to
rerun. If a targeted test command is wrong, discover the correct test target
with `cargo test -p cargo-orthohelp --all-targets --all-features -- --list` and
update this plan before continuing.

If `make fmt` changes unrelated Markdown or Rust files, inspect the diff before
staging. Do not commit unrelated user changes. If unrelated files are already
dirty, leave them unstaged and record the situation in `Decision Log`.

If a gate fails after an edit, use the matching `/tmp` log to identify the
failure, apply the smallest fix, and rerun the failed gate before continuing.
If the same gate fails twice, stop and escalate under the tolerances above.

If the branch push fails because the remote branch already exists, inspect the
remote state with
`git ls-remote --heads origin 5-2-2-migration-rules-for-existing-consumers` and
escalate before overwriting anything.

## Artefacts and notes

Wyvern reconnaissance produced these planning facts:

- `cargo-orthohelp/src/cli.rs` defines `OutputFormat::{Ir, Man, Ps, All}` and
  the default `--format ir`.
- `cargo-orthohelp/src/main.rs` maps `All` to all three existing generators in
  the current order.
- `cargo-orthohelp/tests/features/orthohelp_ir.feature`,
  `cargo-orthohelp/tests/features/orthohelp_roff.feature`, and
  `cargo-orthohelp/tests/features/orthohelp_powershell.feature` are the primary
  BDD feature entry points.
- `cargo-orthohelp/tests/golden/roff_tests.rs` and
  `cargo-orthohelp/tests/golden/powershell_tests.rs` are the primary golden
  output entry points.

Main-branch schema ownership work from
`2b992b67bc5d5e4e763a8ee814bc0314b34cd99b` added these facts:

- `ortho_config/src/agent_context/mod.rs` defines reusable compact
  agent-context contracts, including `AgentContext`, `AgentCommand`,
  `InteractionMode`, `MutationEffect`, and `ORTHO_AGENT_CONTEXT_SCHEMA_VERSION`.
- `cargo-orthohelp/src/policy/mod.rs` defines the `cargo-orthohelp` policy
  report contract, including `PolicyReport`, `PolicySummary`, machine-stable
  finding fields, and `ORTHO_POLICY_REPORT_SCHEMA_VERSION`.
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` is the
  accepted ownership record. This 5.2.2 plan should add migration rules that
  respect it rather than introducing a competing ownership model.
- `docs/users-guide.md` already states that existing `cargo-orthohelp
  --format ir`, `--format man`, `--format ps`, and `--format all`
  behaviour remains compatible while agent-context generation and policy
  checking remain future surfaces.

Context-pack exchange for the agent team was recorded in finalised context pack
`5-2-2-migration-rules-planning`.

Firecrawl prior-art findings:

- SemVer treats documented public APIs as compatibility surfaces.
- Schema-evolution practice distinguishes backward-compatible additions from
  versioned incompatible changes.
- Optional fields need explicit defaults or explicit reader behaviour.
- JSON Schema defaults are annotation metadata, so defaulting must be handled
  by OrthoConfig code or generator logic.

## Interfaces and dependencies

No new external dependency is planned.

The implementation should keep these interfaces stable:

```rust
pub enum OutputFormat {
    Ir,
    Man,
    Ps,
    All,
}
```

The implementation should not change these generator entry-point roles:

```rust
fn generate_ir(
    localized_docs: &[ir::LocalizedDocMetadata],
    out_dir: &Utf8PathBuf,
) -> Result<(), OrthohelpError>;

fn generate_man(
    localized_docs: &[ir::LocalizedDocMetadata],
    out_dir: &Utf8PathBuf,
    man_args: &cli::ManArgs,
) -> Result<(), OrthohelpError>;

fn generate_powershell(
    localized_docs: &[ir::LocalizedDocMetadata],
    ps_config: &powershell::PowerShellConfig,
) -> Result<(), OrthohelpError>;
```

If new metadata fields are introduced during approved implementation, their
reader-facing defaults must be documented beside the field definition and
tested with legacy fixture metadata that omits the field. If a field affects
agent-context only, keep it out of the existing localized IR, man-page, and
PowerShell output contracts unless a separate versioned migration is approved.

## Revision note

2026-05-20: Initial draft created from roadmap item 5.2.2, repository
reconnaissance, Wyvern agent findings, context-pack exchange, and Firecrawl
prior-art research. The plan is pre-implementation and requires explicit user
approval before any feature implementation begins.

2026-05-20: Updated the draft after CodeRabbit review to use sentence-case
heading text throughout and wrap prose lines. This does not change the
implementation sequence or approval gate.
