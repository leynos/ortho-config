# Use serde-visible keys in generated CLI and collection merges (#436)

This ExecPlan is a living document. Keep `Progress`, `Surprises & discoveries`,
and `Decision log` up to date as work proceeds.

## Purpose / Big Picture

Two derive-generated merge paths key configuration layers by the Rust field
identifier instead of the key the final configuration type reads:

1. The hidden CLI struct and the defaults struct do not copy a field's
   `#[serde(rename)]` or the container's `#[serde(rename_all)]`. Their
   `Serialize` output therefore stores a renamed field under its Rust name, and
   the final type silently ignores the CLI or default override.
2. Declarative collection handling removes and reinserts the Rust identifier.
   A renamed `Vec` field never reaches its append accumulator, so each layer
   replaces the whole array instead of contributing items.

After this change, a renamed scalar override supplied on the command line wins
over file and environment values, and a renamed `Vec` with the default append
strategy accumulates items from every layer. A `#[serde(flatten)]` field that
would require a single generated key is rejected at the derive site with an
actionable diagnostic.

Observable success: the new integration test
`ortho_config/tests/serde_visible_keys.rs` loads a struct with
`#[serde(rename_all = "kebab-case")]` from defaults, file, environment, and CLI
and observes the CLI value; the vector cases observe
`["def", "file", "env", "cli"]`. Before the change these cases fail because the
CLI ledger is keyed by the identifier.

## Progress

- [x] (2026-11-21) Reconnaissance: located `FieldAttrs`, `parse_input`,
      `CliFieldInfo`, the hidden CLI/defaults builders, `CollectionStrategies`,
      declarative collection tokens, and the existing layered test harnesses.
- [x] (2026-11-21) Confirmed `serde_serialized_field_key` and
      `serde_deserialized_field_key` already exist and that flatten is
      supported for schema-only composition.
- [x] (2026-11-21) Phase 1: cache the configuration key on `FieldAttrs` during
      `parse_input`; route CLI info, docs file keys, and profile collision
      checks through it; add incompatible-flatten validation.
- [x] (2026-11-21) Phase 2: emit per-field `#[serde(rename)]` on the generated
      CLI and defaults structs; split `cli_default_as_absent` pruning metadata
      into `(arg_id, layer_key)` pairs; re-key extraction from a
      caller-authored parser.
- [x] (2026-11-21) Phase 3: carry the key through `CollectionStrategies` and
      declarative collection token generation; update unit fixtures and add a
      renamed-key case.
- [x] (2026-11-21) Phase 4: add layered end-to-end tests, a directional-rename
      extraction test, and two trybuild compile-fail fixtures.
- [x] (2026-11-21) Focused suites green: `serde_visible_keys` 9/9,
      `merge_strategy` 6/6, `cli_default_as_absent` 19/19, and the directional
      extraction case 1/1.
- [x] (2026-11-21) Red probe: temporarily reverting the rename emission and the
      collection key made 5 of the 9 `serde_visible_keys` cases fail (renamed
      scalar defaults and CLI override, both vector append cases, and the
      explicit-false case). The fix was restored byte-for-byte afterwards.
- [x] (2026-11-21) `check-fmt` and `markdownlint` pass; the two
      `#[serde(flatten)]` compile-fail snapshots are generated and committed.
- [x] (2026-11-21) Cleared the deterministic Clippy findings: an
      `Option::map_or_else` rewrite for the documentation file key, a redundant
      closure, and two `shadow_reuse` bindings in the new tests.
- [x] (2026-11-22) `check-fmt`, `lint`, `markdownlint`, and `nixie` pass at
      `44be0406`. The full `make test` ran for 6109 s: every executed binary
      passed except `cargo-orthohelp --test compile_time :: must_use_compile_tests`,
      which hit its unchanged 900 s child deadline. A standalone build of that
      child took 17 m 45 s under a ~500-deep admission queue, beyond the
      deadline before any UI case ran; the first UI case then passed. This is
      host-load evidence, not a branch defect.
- [ ] Clear CodeRabbit findings and record the composite test evidence in the
      pull request.
- [ ] Push the branch and open the draft pull request.

## Surprises & discoveries

- `parse_input` is shared by `OrthoConfig` and `RequiredConfigSchema`.
  Unconditionally rejecting `#[serde(flatten)]` there breaks the documented
  schema-only flatten support (for example
  `ortho_config/tests/support/missing_required_supply.rs`). The validation is
  therefore a separate pass called only from `expand_ortho_config`, and it
  rejects flatten only where a generated CLI argument, default, merge strategy,
  or `Vec` append would need one key.
- The hidden CLI struct does not replay a user `#[arg(id = ...)]`, so the clap
  argument ID for pruning remains the Rust field name. Only the serialized
  member key changes.
- A single key cannot serve both a caller-authored `Serialize` implementation
  and the final type's `Deserialize` when a directional rename is used.
  `CliFieldInfo` keeps both: the serialized spelling for extraction and the
  configuration key for layering.
- `serde_rename_rules` parses both directions even when only one is requested,
  so `parse_input` continues to reject an unsupported `rename_all` value in
  either direction, matching the existing diagnostics.
- The hidden CLI builder does not replay a user `#[arg(long = "...")]`; the
  generated flag comes from the Rust identifier or `ortho_config(cli_long)`.
  The renamed-boolean fixture therefore sets `cli_long = "enabled"` so the
  hidden parser and the manual parser agree.
- The unchanged `cargo-orthohelp` compile-time suite has a fixed 900 s child
  budget. Under a saturated shared admission queue the child's fixture build
  alone took 17 m 45 s, so the deadline expired before a code error could be
  observed. The suite is outside this branch's change surface; the budget is
  not adjusted here.

## Decision log

- **The cached key is the deserialization key.** Every generated layer is
  serialized only so the final configuration type can deserialize it, so the
  member name must be the one `Deserialize` reads. This also makes
  `rename(serialize = ...)` and `rename(deserialize = ...)` behave consistently
  with the configuration type, which the issue explicitly requires. The field
  keeps the plan's `serialized_key` name because it is the key the generated
  structs serialize under.
- **Flatten is rejected conditionally, not unconditionally.** The CodeRabbit
  plan proposed rejecting every `#[serde(flatten)]`. That contradicts the
  repository's supported schema-only flatten composition and would fail
  existing tests. The implemented rule accepts `skip_cli` fields with no
  generated default, merge strategy, or `Vec` append, and rejects the rest.
- **Extraction is re-keyed rather than duplicated.** `CliFieldInfo` stores the
  caller's serialized spelling and the configuration key. This preserves the
  `CliValueExtractor` contract for a caller-authored parser while making the
  CLI layer carry the final key.
- **`cli_default_as_absent` pruning carries two keys.** The previous metadata
  used one literal for both `ArgMatches::value_source` and `values.remove`.
  After a rename those differ; the pair keeps both correct.
- **Legacy test-only override tokens are left behaviourally unchanged.** They
  destructure the new tuple shape but continue to use `stringify!` because they
  are `#[cfg(test)]` scaffolding, not generated runtime code.
- **The `cargo-orthohelp` deadline failure is treated as infrastructure.** The
  failing test and its 900 s budget are unchanged by this branch, the queue was
  hundreds of requests deep, and the child build alone exceeded the budget
  before any assertion ran. Raising the unrelated deadline would mask a host
  capacity problem rather than improve the code.

## Outcomes & retrospective

Pending completion of the gates and review.

## Conformance basis

- Issue 436, "Use serde-visible keys in generated CLI and collection merges":
  the required outcome supplies the supported rename forms, the flatten
  rejection, and the layered test requirement.
- The repository's `AGENTS.md` documentation, testing, spelling, and gate
  rules; `docs/documentation-style-guide.md` for prose.
- `docs/developers-guide.md` section "Serde rename and field-key reproduction"
  is the internal contract this change extends.
- `docs/users-guide.md` collection-strategy section documents the user-facing
  behaviour.

## Constraints

- No new dependency, feature, or public API surface.
- Generated code continues to reference the resolved crate alias.
- `#[serde(flatten)]` schema-only support must keep compiling and passing.
- Every changed behaviour has a test that fails before the change and passes
  after it.
- Files stay under 400 lines; en-GB-oxendict spelling in prose and comments.

## Tolerances (exception triggers)

- If a supported rename form cannot be threaded without changing a public API,
  stop and record the options.
- If a deterministic gate fails for an unrelated pre-existing reason, record
  the evidence and continue only when it is demonstrably unrelated.
- If CodeRabbit raises a finding that conflicts with the issue requirements,
  record the conflict in the decision log before resolving it.

## Risks

- Trybuild `.stderr` snapshots can drift with the toolchain; generate them with
  the pinned toolchain and inspect the diff.
- The defaults struct now depends on `FieldAttrs.serialized_key`. A caller that
  constructs `FieldAttrs` without `parse_input` would get an empty rename; no
  production path does so, and the type is crate-private.
- New collection tuple fields increase churn in test fixtures.

## Verification plan

- Invariant: every generated layer member for a field equals the key the final
  type deserializes. Verified by the unit key-model table, CLI/defaults token
  tests, and the layered end-to-end cases. Non-vacuity: the end-to-end cases
  fail on the pre-change code because the CLI and defaults layers use the
  identifier; the unit cases include a serialize-only rename whose
  serialization and deserialization keys differ.
- Invariant: a renamed collection reaches its declared strategy. Verified by
  the strategy unit cases and the two append cases, which fail if extraction
  looks up the identifier. The red probe observed 5 failures with the defect
  reintroduced, including both append cases; 4 cases passed because they do not
  depend on the generated layer key (file or environment supplies the value, or
  the case pins the deserialization-side semantics).
- Invariant: incompatible flatten is rejected while schema-only flatten remains
  accepted. Verified by the parse unit table, the two trybuild fixtures, and
  the existing flatten integration suites.
- Axiom: Serde's struct-field naming rules are reproduced by
  `SerdeRenameAll::apply`; treated as the documented third-party contract and
  covered by the existing required-schema tests.
- Commands: `make check-fmt`, `make lint`, `make test`, `make markdownlint`,
  `make nixie`, followed by `coderabbit review --agent`.

## Milestones

1. Key model and consumers (Phases 1 and 2 in code): parsing caches the key;
   CLI, defaults, extraction, docs, and profile checks use it. Plateau: the
   derive compiles and unit tests pin the keys.
2. Collection merge keys (Phase 3): strategies and declarative tokens use the
   cached key; fixtures updated. Plateau: declarative unit suites pass.
3. End-to-end proof (Phase 4): layered tests and compile-fail fixtures.
   Plateau: the focused integration suites pass.
4. Gates and review: full deterministic gates, CodeRabbit, commit, draft PR.

## Revision note

Initial draft written during implementation on 2026-11-21. It records the
conditional-flatten and deserialization-key decisions after reconnaissance
falsified two plan assumptions. Revised after the focused suites passed, adding
the red-probe evidence, the map-strategy unit case, and the `cli_long` finding.
