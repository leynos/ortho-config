# Reconcile missing required value errors

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This plan covers roadmap item 5.1.1 only. It was approved for implementation on
2026-05-17.

## Purpose / big picture

OrthoConfig's documentation currently preserves a proposed design for better
missing-required-value diagnostics while the public error surface does not
contain the proposed `OrthoError::MissingRequiredValues` variant. The purpose
of this work is to make the documentation set truthful before phase 7 begins:
users should be able to read the design document, users guide, changelog, and
roadmap and understand exactly what the crate does today and what remains
planned work.

After the approved implementation completes, a maintainer can verify success by
reading `docs/improved-error-message-design.md`, `docs/users-guide.md`,
`CHANGELOG.md`, and `docs/roadmap.md`. These files should all agree that
missing required values currently surface through existing merge, gathering,
deserialization, or command-line parsing errors, and that implementing the
dedicated aggregate diagnostic belongs to phase 7 unless code inspection proves
otherwise.

## Constraints

Hard invariants that must hold throughout implementation. These are not
suggestions; violation requires escalation, not workarounds.

- Do not implement `OrthoError::MissingRequiredValues` in this phase unless the
  truth audit proves the feature already exists under another name and only
  needs documentation reconciliation.
- Keep phase 5 focused on documentation truth and historical correction. Move
  any absent implementation work to roadmap phase 7, especially item 7.3.1.
- Preserve public API compatibility. If a public API signature must change,
  stop and ask for approval.
- Use en-GB-oxendict spelling and grammar in documentation and comments, except
  for external API names such as `color`.
- Follow `docs/documentation-style-guide.md`, including fenced code block
  language identifiers and 80-column wrapping for Markdown prose.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests where
  tests are needed to validate a code change. If the approved implementation is
  documentation-only, do not add vacuous tests.
- Protect architecture boundaries: missing-required-value policy belongs in
  the domain/runtime error and merge-loading surface, while command-line,
  environment, and file providers remain adapters feeding structured layers.
- Do not add new dependencies without explicit approval.
- Keep files under 400 lines. If a changed code file would exceed that limit,
  split the work or escalate.
- Run required gates sequentially, not in parallel, and capture output with
  `tee` into `/tmp` log files.
- Do not expand beyond the approved documentation reconciliation unless a later
  finding is recorded in this plan and explicitly approved.

If satisfying the objective requires violating a constraint, stop, document the
conflict in `Decision Log`, and ask for direction.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: stop if implementation requires changes to more than 8 files or more
  than 350 net lines, excluding generated lockfile churn if any.
- Interface: stop if adding, removing, or renaming a public `OrthoError`
  variant appears necessary during phase 5.
- Dependencies: stop if any new crate or tool dependency is needed.
- Tests: stop if `make check-fmt`, `make lint`, or `make test` still fails
  after two focused fix attempts.
- Documentation: stop if the current behaviour cannot be described consistently
  without also changing code.
- Ambiguity: stop if "missing required value" could validly mean both a
  required CLI argument rejected by `clap` and a required configuration field
  rejected during merge in a way that changes the phase 5 scope.
- Process: stop if branch rename, upstream tracking, push, or draft pull
  request creation fails because the remote branch already exists or a pull
  request already targets the same branch.

## Risks

Known uncertainties that might affect the plan. Each risk records severity,
likelihood, and mitigation.

- Risk: The documentation may already be partially corrected in some files but
  stale in historical roadmap or agent-native design notes. Severity: medium.
  Likelihood: high. Mitigation: search the full documentation set for
  `MissingRequiredValues`, `missing required`, and stale completion language,
  then update only files needed to make the current truth coherent.
- Risk: Required-value failures may enter through several paths: `clap`
  parsing, declarative layer merging, subcommand merging, and direct Figment
  extraction. Severity: medium. Likelihood: high. Mitigation: document the
  current error channels separately and avoid claiming a single dedicated
  variant exists.
- Risk: Implementing the phase 7 diagnostic later will be subtler than the
  original design suggests because `Option`, `#[serde(default)]`,
  `#[ortho_config(default = ...)]`, nested structs, flattened structures, and
  `cli_default_as_absent` affect whether a field is genuinely required.
  Severity: high. Likelihood: medium. Mitigation: leave those concerns as phase
  7 design notes and do not solve them in this phase.
- Risk: Behavioural tests can become misleading if phase 5 is documentation
  only. Severity: low. Likelihood: medium. Mitigation: require tests only when
  code behaviour changes; otherwise validate with documentation linting and the
  standard Rust gates.
- Risk: The language server may be unavailable, making `leta` semantic
  navigation unusable. Severity: low. Likelihood: medium. Mitigation: fall back
  to `rg` plus direct file inspection and record that limitation in
  `Surprises & Discoveries`.

## Progress

Use this list to summarize granular steps. Every stopping point must be
documented here, even if it requires splitting a partially completed task into
two.

- [x] (2026-05-16) Draft ExecPlan created for approval.
- [x] (2026-05-16) Verified from `ortho_config/src/error/types.rs` that
  `OrthoError::MissingRequiredValues` is absent from the active public enum.
- [x] (2026-05-16) Verified from
  `ortho_config_macros/src/derive/load_impl.rs` that generated loading composes
  layers and delegates final extraction to `merge_from_layers` rather than
  preflighting all missing required fields.
- [x] (2026-05-16) Verified from the active roadmap that absent implementation
  work should move to phase 7.
- [x] (2026-05-17) Obtained explicit plan approval.
- [x] (2026-05-17) Renamed the branch to
  `5-1-1-reconcile-design-with-actual-error-surface` and set upstream tracking
  to `origin/5-1-1-reconcile-design-with-actual-error-surface`.
- [x] (2026-05-17) Opened draft pull request
  `https://github.com/leynos/ortho-config/pull/322`.
- [x] (2026-05-17) Reconfirmed the error surface before editing:
  `OrthoError::MissingRequiredValues` remains absent, generated loading still
  delegates to `merge_from_layers`, and deserialization failures in merge
  contexts still route to `OrthoError::Merge`.
- [x] (2026-05-17) Reconciled `docs/improved-error-message-design.md`,
  `docs/users-guide.md`, `CHANGELOG.md`, and `docs/roadmap.md` so the current
  behaviour is documented and the implementation remains phase 7 work.
- [x] (2026-05-17) Ran `coderabbit review --agent` after the documentation
  milestone; it completed with zero findings.
- [x] (2026-05-17) Ran validation gates. `make check-fmt`, `make lint`,
  `make test`, and `make nixie` passed. `make fmt` still fails on existing
  repository-wide Markdown line-length violations outside this task's edited
  sections.
- [x] (2026-05-17) Committed the documentation reconciliation as
  `81062f8 Reconcile missing required error docs`.
- [ ] Commit the final ExecPlan status update.
- [ ] Push the implementation commits.

## Surprises & discoveries

Unexpected findings during implementation that were not anticipated as risks.
Document with evidence so future work benefits.

- Observation: The active `docs/improved-error-message-design.md` and
  `docs/users-guide.md` already state that `OrthoError::MissingRequiredValues`
  is planned rather than implemented. Evidence: Both files explicitly say the
  public enum does not currently expose the variant. Impact: Implementation may
  be smaller than the roadmap wording implies, but adjacent stale claims still
  need checking.
- Observation: `CHANGELOG.md` has no current entry about
  `MissingRequiredValues`. Evidence: The `Unreleased` section lists added,
  changed, and fixed entries unrelated to this variant. Impact: The phase 5
  change should add a concise release-note correction if documentation truth
  changes are made.
- Observation: `leta workspace add .` succeeded, but `rust-analyzer` failed to
  start for this workspace. Evidence: The command reported that the Rust
  language server connection closed and suggested installing or restarting
  `rust-analyzer`. Impact: Planning used text search and direct inspection
  rather than semantic LSP references.
- Observation: Firecrawl was used to confirm clap prior art for required
  command-line argument errors. Evidence: docs.rs for `clap::error::ErrorKind`
  documents `MissingRequiredArgument`, `MissingSubcommand`, and
  `DisplayHelpOnMissingArgumentOrSubcommand`. Impact: The plan distinguishes
  clap's adapter-level missing argument surface from OrthoConfig's planned
  configuration-field diagnostic.
- Observation: Targeted `markdownlint` still reports existing long lines in
  `docs/users-guide.md` outside the edited error-handling section. Evidence:
  lines such as the aggregate example remain over 80 columns. Impact:
  validation should report this as pre-existing repository debt unless later
  scope explicitly includes reflowing unrelated guide sections.
- Observation: `coderabbit review --agent` reported no findings for the
  documentation milestone. Evidence: the review completed with `findings: 0`.
  Impact: no follow-up edits were needed before validation.
- Observation: The required Rust validation gates pass after the documentation
  reconciliation. Evidence: `make check-fmt`, `make lint`, and `make test`
  completed successfully. `make nixie` also validated all Mermaid diagrams.
  Impact: the documentation-only implementation did not regress formatting,
  linting, tests, or diagram validation.

## Decision log

Record every significant decision made while working on the plan. Include
decisions to escalate, decisions on ambiguous requirements, and design choices.

- Decision: Treat `OrthoError::MissingRequiredValues` as absent planned work,
  not a renamed implemented feature, unless implementation uncovers contrary
  evidence before editing. Rationale: The public enum lacks the variant, macro
  loading delegates to merge extraction, and the design/user guide already
  describe the variant as planned. Date/Author: 2026-05-16 (assistant).
- Decision: Keep this phase documentation-first and move actual diagnostic
  implementation to phase 7.3.1. Rationale: The roadmap explicitly says to keep
  the design as proposed work and move the build into phase 7 if implementation
  is absent. Date/Author: 2026-05-16 (assistant).
- Decision: Do not add tests for a documentation-only reconciliation.
  Rationale: Tests should validate observable code behaviour; a test that
  merely asserts the absence of a future enum variant would freeze an intended
  phase 7 change. Date/Author: 2026-05-16 (assistant).
- Decision: Use hexagonal architecture as a boundary check, not as a
  restructuring mandate. Rationale: The requested work needs clear
  responsibility boundaries between runtime error policy and source adapters,
  but no architecture transplant. Date/Author: 2026-05-16 (assistant).

## Outcomes & retrospective

Summarize outcomes, gaps, and lessons learned at major milestones or at
completion. Compare the result against the original purpose.

- Outcome: The phase 5.1.1 documentation reconciliation is complete. The
  design note and users guide now state that `OrthoError::MissingRequiredValues`
  is absent from the current public error surface, the changelog records the
  clarification, and the roadmap marks item 5.1.1 done while leaving the actual
  diagnostic implementation in phase 7.3.1.
- Gap: Repository-wide `make fmt` still fails on pre-existing Markdown
  line-length violations in unrelated documents. The edited ExecPlan, design
  note, roadmap, and changelog pass direct Markdown linting; the edited users
  guide section is wrapped, but the file contains older long lines elsewhere.

## Context and orientation

The active roadmap item is `5.1.1` in `docs/roadmap.md` under "Repair current
truth". It asks for three concrete outcomes: verify whether
`OrthoError::MissingRequiredValues` exists, update
`docs/improved-error-message-design.md`, `docs/users-guide.md`, and release
notes to describe current behaviour accurately, and move absent implementation
work into phase 7.

The active public error enum is `OrthoError` in
`ortho_config/src/error/types.rs`. At drafting time it contains these variants:
`CliParsing`, `File`, `CyclicExtends`, `Gathering`, `Merge`, `Validation`, and
`Aggregate`. It does not contain `MissingRequiredValues`.

The generated load path is in `ortho_config_macros/src/derive/load_impl.rs`.
`compose_layers_from_iter` gathers CLI, defaults, file, and environment layers
into a `LayerComposition`. `load_from_iter` then calls
`composition.into_merge_result(|layers| Config::merge_from_layers(layers))`.
The generated declarative merge path eventually deserializes merged JSON values
through helpers such as `ortho_config/src/declarative/convert.rs`, where merge
failures become `OrthoError::Merge`.

Subcommand loading in `ortho_config/src/subcommand/mod.rs` also routes
deserialization failures through merge-style errors. Command-line required
argument failures remain `clap` parsing errors, represented in OrthoConfig as
`OrthoError::CliParsing`.

The proposed diagnostic design lives in
`docs/improved-error-message-design.md`. It should remain useful as design
rationale, but it must not read as implemented behaviour. The public user
contract is described in `docs/users-guide.md`, and release-note truth belongs
in `CHANGELOG.md`.

## Plan of work

Stage A: Reconfirm the error surface before editing. Read
`ortho_config/src/error/types.rs`, `ortho_config/src/error/constructors.rs`,
`ortho_config/src/result_ext.rs`,
`ortho_config_macros/src/derive/load_impl.rs`,
`ortho_config_macros/src/derive/generate/declarative/merge_tokens.rs`,
`ortho_config/src/declarative/convert.rs`, and
`ortho_config/src/subcommand/mod.rs`. Search the repository for
`MissingRequiredValues`, `MissingRequired`, `missing required`, `MissingField`,
and stale "complete" language. If code shows a real implemented equivalent
under another name, stop and update this plan before proceeding.

Stage B: Reconcile the design document. Update
`docs/improved-error-message-design.md` so its status block says the design is
proposed work and identifies the current error channels precisely. Preserve
historical rationale, but move any implementation checklist language that reads
as completed into an explicitly historical or future-work section. Link phase
7.3.1 as the intended implementation home.

Stage C: Reconcile the users guide. Update the "Error handling" section in
`docs/users-guide.md` so consumers know the current behaviour: CLI-missing
cases are `OrthoError::CliParsing`, merge/deserialization missing field cases
are `OrthoError::Merge` or related existing errors, and multiple source
failures may be `OrthoError::Aggregate`. Keep any proposed diagnostic example
clearly labelled as future work.

Stage D: Reconcile release notes and adjacent stale claims. Add a concise
`CHANGELOG.md` entry under `Unreleased` explaining that documentation now
clarifies the absent missing-required-values variant and tracks the
implementation as future phase 7 work. Search adjacent docs such as
`docs/archive/v0-8-0-roadmap.md` and `docs/agent-native-cli-design.md`; update
only stale claims that would otherwise contradict the required files.

Stage E: Update the roadmap. In `docs/roadmap.md`, mark item 5.1.1 done only
after the documentation and release-note changes are complete. Ensure the phase
7 item for rebuilding improved required-value diagnostics remains open and
clearly owns the implementation work.

Stage F: Validate. Because this change is expected to be documentation-only,
run documentation formatting/linting plus the required Rust gates:

```sh
make fmt 2>&1 | tee /tmp/fmt-ortho-config-5-1-1-reconcile-design-with-actual-error-surface.out
make markdownlint 2>&1 | tee /tmp/markdownlint-ortho-config-5-1-1-reconcile-design-with-actual-error-surface.out
make nixie 2>&1 | tee /tmp/nixie-ortho-config-5-1-1-reconcile-design-with-actual-error-surface.out
make check-fmt 2>&1 | tee /tmp/check-fmt-ortho-config-5-1-1-reconcile-design-with-actual-error-surface.out
make lint 2>&1 | tee /tmp/lint-ortho-config-5-1-1-reconcile-design-with-actual-error-surface.out
make test 2>&1 | tee /tmp/test-ortho-config-5-1-1-reconcile-design-with-actual-error-surface.out
```

If implementation approval expands scope into code changes despite this plan's
recommendation, add focused `rstest` unit tests and applicable `rstest-bdd`
behavioural tests before the code change. Cover happy paths where required
values are supplied by defaults, files, environment variables, and CLI input;
unhappy paths for one and many missing fields; edge cases for `Option`,
defaults, nested structs, and subcommand merging; and any externally observable
workflow affected by the new diagnostic.

Stage G: Commit, push, and open a draft pull request after gates pass. The
commit should be atomic and use a file-based commit message. The branch should
be named `5-1-1-reconcile-design-with-actual-error-surface` and track
`origin/5-1-1-reconcile-design-with-actual-error-surface`. The draft pull
request title should include `(5.1.1)`, and the summary should mention this
ExecPlan file. Run `echo ${LODY_SESSION_ID}` and include
`https://lody.ai/leynos/sessions/${LODY_SESSION_ID}` in a final `## References`
section of the pull request body.

## Concrete steps

1. Confirm the current branch:

   ```sh
   git branch --show-current
   ```

2. After approval, rename the branch and establish upstream tracking:

   ```sh
   git branch -m 5-1-1-reconcile-design-with-actual-error-surface
   git fetch origin
   git push -u origin 5-1-1-reconcile-design-with-actual-error-surface
   ```

   If the remote branch already exists, inspect it before pushing and stop if
   it contains work not present locally.

3. Search for stale claims:

   ```sh
   rg -n \
     "MissingRequiredValues|MissingRequired|missing required|MissingField" \
     docs CHANGELOG.md ortho_config ortho_config_macros
   rg -n "phase 7|7\\.3\\.1|complete|done" docs CHANGELOG.md
   ```

4. Inspect the current public error enum and generated merge path:

   ```sh
   sed -n '1,120p' ortho_config/src/error/types.rs
   sed -n '200,285p' ortho_config_macros/src/derive/load_impl.rs
   sed -n '1,100p' ortho_config/src/declarative/convert.rs
   ```

5. Edit `docs/improved-error-message-design.md`,
   `docs/users-guide.md`, `CHANGELOG.md`, and `docs/roadmap.md`. Edit adjacent
   documentation only when search shows contradiction with the current truth.

6. Review the changed docs for wrapping and stale language:

   ```sh
   git diff -- docs/improved-error-message-design.md docs/users-guide.md \
     CHANGELOG.md docs/roadmap.md
   ```

7. Run the validation commands listed in Stage F sequentially. Inspect the
   `/tmp` logs if output is truncated.

8. Commit with a file-based commit message after gates pass. The subject should
   be similar to:

   ```plaintext
   Reconcile missing required error docs
   ```

9. Push the branch and create a draft pull request. The PR title should be:

   ```plaintext
   (5.1.1) Reconcile missing required error surface
   ```

   The PR summary should mention
   `docs/execplans/5-1-1-reconcile-design-with-actual-error-surface.md` and end
   with:

   ```markdown
   ## References

   - Lody session: https://lody.ai/leynos/sessions/${LODY_SESSION_ID}
   ```

## Acceptance criteria

The implementation is complete when all of the following are true:

- `docs/improved-error-message-design.md` clearly says the dedicated missing
  required values diagnostic is proposed phase 7 work, not current behaviour.
- `docs/users-guide.md` accurately describes current missing required value
  error routing through existing `OrthoError` variants.
- `CHANGELOG.md` records the documentation correction under `Unreleased`.
- `docs/roadmap.md` marks item 5.1.1 done and leaves the actual build in phase
  7.3.1.
- No adjacent documentation still claims that
  `OrthoError::MissingRequiredValues` is implemented.
- `make check-fmt`, `make lint`, and `make test` pass.
- Documentation checks relevant to Markdown changes pass.
- The final commit is pushed to
  `origin/5-1-1-reconcile-design-with-actual-error-surface`.
- A draft pull request exists with `(5.1.1)` in the title, this ExecPlan
  mentioned in the summary, and the Lody session link in `## References`.

## Notes for phase 7 implementers

Phase 7.3.1 should design and implement the actual aggregate diagnostic. The
implementation should treat requiredness as a domain policy over merged
configuration metadata, not as a side effect of one adapter. Unit tests should
use `rstest`; behavioural tests should use `rstest-bdd` where the message is
externally observable; and property tests, Kani, or Verus should be considered
only if the implementation introduces a substantive invariant over input
orders, source precedence, or state transitions.

The phase 7 work should cover supplied-value happy paths through defaults,
configuration files, environment variables, and CLI arguments. It should cover
unhappy paths for one missing field, multiple missing fields, nested fields,
optional fields, serde defaults, OrthoConfig defaults, and subcommand merges.
It should not freeze a misleading string-only API if a structured diagnostic
type would better serve downstream agent-native renderers.
