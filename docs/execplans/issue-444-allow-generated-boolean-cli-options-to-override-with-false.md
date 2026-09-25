# Allow generated boolean CLI options to override with `false` (#444)

This ExecPlan is a living document. Keep `Progress`, `Surprises & discoveries`,
and `Decision log` up to date as work proceeds.

## Purpose / Big Picture

Generated boolean command-line flags are currently presence-only: the derive
emits `ArgAction::SetTrue`, so `--flag` means `true` and saying nothing means
"unset". There is no way to say `false` from the command line. A `true` that a
configuration file or an environment variable supplied therefore cannot be
cleared by the user; the CLI silently loses that precedence rung.

After this change, every generated boolean option accepts an optional value:

```plaintext
--flag            # true   (unchanged spelling)
--flag=true       # true
--flag=false      # explicit false, clears a lower-precedence true
-f=false          # short form, same semantics
```

Saying nothing still leaves the field absent, so configuration files and
environment variables continue to win when the flag is not supplied. The
generated help, the documentation IR, and the roff/PowerShell renderers are
updated so the `[=<BOOL>]` spelling is discoverable rather than folklore.

Observable success: a config file that sets `enabled = true` combined with
`--enabled=false` on the command line resolves to `false`, while
`enabled = true` alone still resolves to `true`.

## Progress

- [x] (2026-09-24) Reconnaissance: mapped every boolean-flag surface
      (macro, merge, docs IR, renderers, goldens, migration guides).
- [x] (2026-09-24) Empirical probe of clap 4.6 semantics for
      `num_args(0..=1) + require_equals(true) + default_missing_value("true")`.
- [x] (2026-09-24) ExecPlan drafted.
- [x] M1 (Task 1): derive macro emits the optional-value form; unit tests
      updated; `make test` for `ortho_config_macros` green (131 passed).
- [x] (2026-09-24) Fixed the pre-existing `differs_from_defaults` layering
      defect that discarded the whole CLI layer when its sanitized object
      equalled the defaults object. Red→green verified with two new
      `compose_layers` regression tests over a one-field struct; all
      `clap_integration`, `cli_default_as_absent*`, subcommand, BDD, and
      `hello_world` suites green afterwards.
- [x] M2 (Task 2): runtime merge/precedence verified with integration fixtures
      (10-case matrix in `clap_integration` plus the doc-example flow).
- [x] M3 (Task 3): docs IR `value_optional` marker, ADR-009, renderers,
      goldens.
- [x] M4 (Task 4): users-guide section, changelog entries, v0.10.0 migration
      guide sections (flag spellings, explicit-value precedence) with impact
      table and upgrade checklist.
- [x] (2026-09-24) All seven deterministic gates green at `cb0a8c65`
      (`check-fmt`, `typecheck`, `lint-clippy`, `lint-whitaker`, `test`,
      `markdownlint`, `nixie`). Cleared the latent violations that clippy had
      been masking: `shadow_reuse`, `self_named_module_files`,
      `shadow_unrelated` x2, `struct_excessive_bools`, `too_many_arguments`;
      split `cli_flags.rs` and `agent_context/mod.rs` for `module-max-lines`;
      corrected six `-ise` spellings to the house `-ize` form.
- [x] (2026-09-25) CodeRabbit `--agent --committed --base main` reviewed the
      46-file diff and returned five distinct concerns, none re-litigating the
      accepted design. All five actioned: the roff optional-value placeholder
      now joins the flag (`--flag[=<BOOL>]`, matching clap's own
      `require_equals` suffix) with a regression test proven non-vacuous; the
      406-line doc-example test file split into
      `documentation_examples/boolean_override.rs`; ADR-009 links made
      same-directory; the migration-guide "Before" block now shows the real
      `TooManyValues` parse failure instead of output the old code could not
      produce; and the ExecPlan itself is indexed in `docs/contents.md`.
- [x] (2026-09-25) CodeRabbit round 2 returned three further findings, none
      blocking and none re-litigating the design. All three actioned: the five
      `figment::Jail::try_with` sites in `ortho_config/tests/compose_layers.rs`
      now go through `test_helpers::figment::with_jail`, the repository's own
      shared env-guard helper, per `AGENTS.md:253-256` (bare `figment::Jail`
      calls are direct environment mutation, which the policy forbids in
      tests); the PowerShell optional-value sentence falls back to the short
      flag and, when neither flag exists, names no spelling instead of the
      nonsense `the flag=false`; and the `--excited --port 3000` illustration
      no longer names a `port` field the example's `Config` does not declare.
- [x] (2026-09-25) All seven gates green at `2cfc9bb1`. The scrutineer's run
      caught one blocker the previous rounds had missed:
      `clippy::too_many_arguments` (5/4) on the new MAML test, because the
      rstest function took the `minimal_doc` fixture plus four `#[case]`
      parameters. Grouping the two expected phrases into a tuple brings it to
      four. `cargo test` had reported a false green on this lint, since
      `too_many_arguments` is a Clippy-only lint and is not raised by the
      `RUSTFLAGS="-D warnings"` test build. An earlier note in this plan
      claimed `make lint` was complicit, on the theory that it stops at
      `lint-clippy` and never reaches `lint-whitaker`. That claim is false:
      `Makefile:99` defines `lint: lint-clippy lint-whitaker`, identically on
      `origin/main`. The single false green was `cargo test`'s, and the lesson
      is to read the Makefile's target definition before blaming the build
      system for a gate that did not run.
- [x] (2026-09-25) CodeRabbit round 3 returned thirteen findings (two of them
      duplicates of the same `Invocation` refactor). All actioned: five `-ise`
      spellings on branch-added lines corrected to the house `-ize` form; the
      `replay.rs` doc rewritten so it no longer contradicts its own
      `is_bool.then(…)` branch; the user's-guide precedence paragraph
      reworded to describe `--excited=false` defeating a lower-precedence
      `true` rather than an incoherent "higher-precedence source"; a stray
      space removed from this plan; `enum_values` now returns empty for
      `ValueType::Bool` with a new non-vacuous unit test, and the three
      agent-context goldens regenerated (every hunk is the boolean revert —
      the `log_level` variants survive); and `assert_run_with_environment`
      now takes a named `Invocation` grouping `args` with `environment`,
      removing the `#[expect(clippy::too_many_arguments)]` suppression. One
      style preference (`concat!` over backslash-continued literals) was
      declined: it is 4 uses versus 10 in `src`, so the finding asked for
      deviation from the dominant local idiom. Non-vacuity of the
      `Invocation` refactor was proven by reverting to the 5-parameter
      signature with the suppression deleted and observing
      `clippy::too_many_arguments` fire.
- [x] (2026-09-25) Rebasing onto the advanced `origin/main` (`8835347c`, PR
      #416) found and fixed a defect no gate could catch: this branch's ADR-008
      collided with the ADR-008 that PR #416 had already merged, so two
      different ADRs would have claimed number 008. This branch's
      unpublished ADR renumbers to 009. Four conflicts were resolved by
      hand — three in
      `docs/contents.md` and one each in `docs/v0-10-0-migration-guide.md`,
      `cargo-orthohelp/src/agent_context/mod.rs` (main moved `CANONICAL_VERBS`
      into `policy::vocabulary` while this branch split the module), and the
      `agent_context__fixture.json.snap` header. All are additive on both
      sides, so every resolution keeps both. The four hand-resolved surfaces
      are named in the gate report together with the test that covers each.
- [x] (2026-09-25) All seven gates green at `e27e35fb`, on the rebased tree:
      76 suites, 1337 passed, 0 failed, 0 panics, 0 `FAILED` markers, pytest 87
      passed / 5 skipped, and no pending snapshots. The count is up from 1159
      because main's policy surface now merges cleanly into this tree.
- [x] (2026-09-25) Pushed to
      `origin/issue-444-allow-generated-boolean-cli-options-to-override-with-false`
      and opened draft PR #532. The remote branch was still at the pre-rebase
      merge base `c144641e`, so the push was a plain fast-forward and rewrote
      no published history. `gh pr view` reports `MERGEABLE`, base `main`.
- [x] (2026-09-25) CodeRabbit round 4 reviewed the rebased tree and returned
      four findings, all `minor` and all docs-only. One was actioned as a
      genuine correction (see the `--flag false` note below); the other three
      were the first-person pronouns and a garbled sentence in this plan.
- [x] (2026-09-25) Added the `Option<bool>` half of the acceptance matrix,
      which the issue requires and which earlier rounds had covered only at
      the token and figment levels. `ortho_config/tests/clap_integration/`
      `option_cases.rs` gains eight cases over `OptionBoolConfig`. Writing them
      exposed two further defects; both are fixed (see `Surprises &
      discoveries`), and all ten `parses_option_*` cases are green.
- [x] (2026-09-25) Scrutineer ran the seven gates after the round-4 and
      `Option<bool>` work. Five passed; two failed, both regressions this plan
      itself introduced. `make check-fmt` failed because `mdtablefix` wanted to
      reflow prose in this plan and in `docs/v0-10-0-migration-guide.md`, and
      `make markdownlint` failed at the spellcheck stage on three words in this
      plan that had been written with the British `-ise` suffix where the house
      en-GB-oxendict style requires `-ize`. Both are fixed below.
- [x] (2026-09-25) Corrected a false claim this plan had made about the build
      system: it asserted `make lint` stops at `lint-clippy` and never reaches
      `lint-whitaker`. `Makefile:99` defines `lint: lint-clippy lint-whitaker`,
      identically on `origin/main`, so both targets always run. The incorrect
      note had been used to explain why whitaker caught something the gates
      missed; the real explanation is that the `too_many_arguments` false green
      belonged to `cargo test`, which builds with `--all-targets` and does not
      raise that Clippy-only lint.
- [x] (2026-09-25) Markdown gates re-run green over the corrected docs:
      `check-fmt` (74 files unchanged, `cargo fmt` and `mdtablefix` clean) and
      `markdownlint` (75 files, 0 errors, spellcheck clean). The first
      spellcheck fix was itself instructive: describing the three bad spellings
      inline re-tripped the same gate, because the tokenizer reads inline code
      spans. The note now describes the fault without reproducing it.
- [ ] Push and request CodeRabbit round 5.

## Surprises & discoveries

- **ADR-008 was already taken.** This branch minted
  `docs/adr-008-optional-value-boolean-cli-metadata.md`, but `origin/main`
  gained `docs/adr-008-agent-native-policy-configuration.md` (dated 2026-08-12)
  via PR #416 while this branch was in flight. Two different ADRs would
  therefore have claimed number 008 on merge, and the collision is invisible
  from this branch alone because the branch predates that merge. Numbering
  follows landing order, so the unpublished ADR renumbers to **ADR-009**; the
  file, its title, `docs/contents.md`, and this plan's M3 and round-2 entries
  all move together. Detected by probing `git merge-tree` against `origin/main`
  before opening the pull request, not by a gate.

- **`-ffalse` is rejected.** With `require_equals(true)`, an attached short
  value raises `clap::error::ErrorKind::ArgumentConflict`; only `-f=false`
  works. Verified against a minimal `clap::Parser` fixture, together with the
  other spellings: `--flag=false` and `-f=false` parse, plain `--flag` parses,
  `--flag=bogus` raises `InvalidValue`, and `--flag false` raises
  `UnknownArgument`.
- **The roff renderer copied the wrong spacing.**
  `format_flag_with_optional_value` was modelled on `format_flag_with_value`,
  which separates the flag from its placeholder with a space. That is correct
  for a required value but wrong here: `require_equals` means the value must
  follow `=`, so the documented `--flag [=BOOL]` form implies the invalid
  `--flag =BOOL`. `clap`'s own `stylize_arg_suffix` emits the suffix as
  `(placeholder, "[=")` with no separator space, versus `(placeholder, " [")`
  when `require_equals` is unset. The placeholder is now joined directly to the
  flag.
- **A CHANGELOG claim outran the code.** The entry said the PowerShell renderer
  printed the same `--flag[=<BOOL>]` form, but `push_cli_paragraphs` emits a
  prose sentence instead. The entry and the migration guide now describe what
  each renderer actually produces.
- **`--flag false` is rejected, and always was.** `require_equals` forces the
  `=` spelling, so a space-separated value raises
  `clap::error::ErrorKind::UnknownArgument`. A fixture confirms the error text
  is byte-identical to what the old presence-only flag produced — so this
  spelling is unchanged, not newly rejected, and the migration guide must not
  claim the error became clearer. The restriction is deliberate: without it,
  `--flag --other x` would try to consume `--other` as the flag's value.
- **`clap_derive` would infer `ArgAction::Set` for `Option<bool>`** and add a
  `.required(...)` obligation for a bare `bool`. The generated `Option<bool>`
  field type plus *explicit* attributes is therefore required, not optional.
- **`should_skip_non_flag_input`, not `is_non_cli_field`.** The task plan names
  a function that does not exist; the real name is
  `cargo-orthohelp/src/agent_context/mod.rs::should_skip_non_flag_input`.
- **`default_value` replay still behaves.** With both `default_value = "false"`
  and the new attributes, absent → `DefaultValue` source, explicit forms →
  `CommandLine`.
- **`differs_from_defaults` was broken, and the earlier note that it "still
  gates correctly" was wrong.** The guard compared the whole sanitized CLI
  object against the whole defaults object and skipped `composer.push_cli` when
  the two were equal. For any configuration whose explicit CLI value equals its
  struct default, the entire CLI layer was discarded, so a lower file or
  environment value silently won over an explicit user argument. The one-field
  case is the minimal reproduction:

  ```plaintext
  # Defaults={"excited":false}  Environment={"excited":true}
  probe --excited=false   ->  before: excited=true  (CLI layer dropped)
                          ->  after:  excited=false (CLI layer retained)
  ```

  The bug is **not boolean-specific**: `port: u16` with `default = 8080` and
  `ACME_PORT=9000` ignored an explicit `--port 8080` in exactly the same way.
  It is pre-existing (introduced with `LayerComposition` in #246) and was
  previously masked because every test fixture carried enough other fields to
  make the two objects unequal. Boolean `--flag=false` is simply the value most
  likely to coincide with a default, which is why #444's acceptance criteria
  expose it. `defaults_value` was removed from the generated code and the guard
  now asks clap's per-argument `matches.value_source()` instead.
- **Do not run `cargo test --workspace` while editing.** The `cargo-orthohelp`
  behavioural scenarios spawn the real `target/debug/cargo-orthohelp` binary;
  recompiling it mid-run produced nine spurious failures that vanish when the
  scenario runs against a quiescent tree. Re-verify after edits settle.
- **The derived short flag for `is_excited` is `i`,** because `r` and `s` are
  already claimed by `recipient` and `salutations`.
- **Bare `figment::Jail` calls are direct environment mutation.** The jail is a
  process-wide lock, not a guard registered in a shared crate, so a test file
  that calls `figment::Jail::try_with` itself violates `AGENTS.md:253-256`. The
  repository already ships `test_helpers::figment::with_jail` for exactly this
  purpose, and it takes a closure returning `figment::error::Result<T>`, so the
  call-site bodies survive the conversion untouched.
  `test_helpers::figment::figment_error` replaces the raw
  `figment::Error::from(…)` conversions. Six further test files still call the
  jail directly and would benefit from the same conversion; that is out of
  scope here.
- **The PowerShell fallback was a latent defect, not a live one.** Reachability
  needs metadata carrying `value_optional: true` with neither a long nor a
  short flag. No current fixture produces that, so the goldens never exercised
  the branch — the new unit test pins it directly rather than through a golden.
- **`typos.toml` churn is not this branch's to commit.** The spellcheck gate
  regenerates this tracked file through `typos-config-builder`, whose pinned
  dictionary drifts ahead of the committed copy, so every gate run leaves it
  dirty. The entries it currently adds are unrelated to this branch: CSS
  alignment utilities, the camel-cased `currentColor`, and one entry that is
  itself a misspelling appear nowhere in the tree. The sole exception is a
  `tokio::test` attribute default, which is already recorded in
  `docs/rstest-bdd-users-guide.md` and `typos.local.toml` on `main`. Three
  sibling worktrees, including one on an unrelated branch, carry the same
  churn, with two of them byte-identical to each other. The file is therefore
  left out of the commit; the spelling gate is green with it dirty.

- **A merge that supplies nothing failed for every struct shape, not just this
  branch's.** The generated declarative state derives `Default`, so its
  accumulator starts as `serde_json::Value::Null`; `merge_layer` deliberately
  skips empty maps rather than seating it; so `finish` handed `Null` to the
  deserializer and reported `invalid type: null, expected struct …`. Calling
  `merge_from_layers([])` reproduces it for a defaulted one-field struct, an
  all-`Option` struct, and an empty struct alike, and every file in the path —
  `generate/declarative/merge_tokens.rs`, `guards.rs`,
  `generate/declarative/mod.rs`, `src/declarative/*` — is byte-identical to
  `main`. `Null` can carry no other meaning there: a layer whose whole value is
  `null` is rejected by the non-object guard in `merge_layer`. The fix is one
  guard in the generated `finish`: normalize a `Null` accumulator to an empty
  object before deserializing. The negative control matters as much as the fix:
  a struct with a required field must still report `missing field …`, which the
  new test pins.
- **`OptionConfig` passed only by accident.** It is unprefixed, so it builds
  `CsvEnv::raw()`, whose object is non-empty whenever *any* environment
  variable is set; that non-empty layer seated the accumulator and hid the
  defect above. Any prefixed, all-optional struct reaches the failing path as
  soon as nothing supplies a value — which is the ordinary `absent` case this
  ticket's acceptance criteria name.
- **A prefixed struct does not read `.config.toml`.** `compute_dotfile_name`
  derives `.optl.toml` from `prefix = "OPTL_"`, so the file cases in the new
  matrix had to be repointed; as first written, `cli_false_clears_file_true`
  passed *vacuously* because no file layer existed at all. Non-vacuity is now
  proven by mutation: flipping the fixture file to `flag = false` fails
  `file_true_without_flag` and nothing else.
- **`--flag false` is not newly rejected, and its error is not clearer.** A
  `clap::Parser` fixture rendering both the old presence-only flag and the new
  hybrid flag shows byte-identical `unexpected argument 'false' found` output
  (`ErrorKind::UnknownArgument`) for that spelling. The real change is that
  `--flag=false` used to fail with `TooManyValues` and is now accepted. The
  migration guide previously claimed the space-separated error had improved;
  that claim was unevidenced and is corrected. The same fixture confirmed the
  other spellings: `-f=false` and `--flag` parse, `--flag=bogus` raises
  `InvalidValue`, and `-ffalse` raises `ArgumentConflict` as this plan already
  said.

- **A doctest fails, and no gate can see it.** `cargo test --doc` fails on
  `ortho_config/src/cargo/mod.rs - cargo::external_subcommand (line 90)` with
  `ErrorKind::InvalidValue` on `--all <all>`: the example declares
  `.arg(clap::Arg::new("all").long("all"))`, which takes a value, and then
  passes bare `--all`. This is pre-existing and out of scope. The file arrives
  from #419, which is an ancestor of `origin/main`; the failure reproduces with
  this branch's changes stashed, so it is not caused here; and `make test` runs
  `--all-targets`, which excludes doctests, so the seven gates never exercise
  it. A sibling branch, `harden-cargo-external-subcommand-doctest-20260924`,
  already carries a "Fix Cargo external subcommand doctest" commit, so the
  defect is owned elsewhere and is reported here rather than duplicated.

## Decision log

- **Single value-taking flag, not a `--flag`/`--no-flag` pair.** A negation
  pair breaks the one-`arg_id`-per-field model that `CliValueExtractor` and
  `value_source()` rely on, and contradicts the preference against
  auto-generated `--no-x` pairs recorded in `docs/agent-native-cli-design.md`
  §5.
- **Add a marker to `CliMetadata` rather than overloading existing fields.**
  `takes_value` cannot express "value is optional"; the docs IR version bumps
  and the schema mirror stays byte-for-byte aligned per ADR-003.
- **Keep omitting `skip_serializing_if` for booleans.** `strip_nulls` already
  removes `None`, so absence survives, while `Some(false)` must serialize as
  `false` to clear lower layers. Adding the skip hook would erase the
  distinction the ticket exists to create.

## Constraints

- One clap argument per field; do not change `CliFieldInfo` or the single
  `arg_id` model.
- Do not refactor the merge composer beyond the ticket need.
- `cargo-orthohelp/src/schema/mod.rs` must stay byte-for-byte aligned with
  `ortho_config/src/docs` (guarded by `ir_version_matches_ortho_config`).
- ADR-003: new optional metadata fields need explicit defaults for older
  derives.
- Code files stay within 400 lines; 80-column Markdown prose; en-GB-oxendict
  spelling.

## Tolerances (exception triggers)

- Escalate if the optional-value form cannot preserve absent-versus-present
  semantics, or if `Some(false)` cannot reach the CLI provider layer.
- Escalate before deleting or rewriting any golden snapshot rather than
  reviewing each changed line.

## Risks

- Golden snapshots across `cargo-orthohelp` and `examples/hello_world` bake in
  the bare `--is-excited` spelling; each must be re-reviewed, not
  blind-accepted.
- `arg_takes_value` in the localizer matches `ArgAction::Set | Append`, so a
  value-taking boolean becomes localizable for `value_name`.
- Downstream struct-literal consumers of `CliMetadata` face a source break
  until roadmap item 13.1.1 (constructors) lands.

## Conformance basis

The upstream artefact is GitHub issue #444. There is no separate Terms of
Reference or technical design document for this work, so the issue body is the
requirement source and its acceptance criteria are quoted verbatim in
`Purpose / Big Picture`.

Governing documents this plan conforms to:

- `docs/agent-native-cli-design.md` §5 — no auto-generated `--no-x` negation
  pairs. This is why the plan adopts a single value-taking flag rather than a
  two-flag pair.
- `docs/adr-003-documentation-ir-schema-ownership.md` — the documentation IR
  and the `cargo-orthohelp` schema mirror move together; new fields need an
  explicit default so older derives keep parsing.
- `docs/adr-009-optional-value-boolean-cli-metadata.md` — added by this branch;
  records the `value_optional` marker, the IR version bump, and the rejected
  alternatives.
- `AGENTS.md` — 400-line code files, 80-column Markdown prose, en-GB-oxendict
  spelling.

The trace chain from requirement to evidence runs:

```plaintext
#444 (bool and Option<bool>; absent/true/false/file/env/CLI-false)
  -> EP-M1 derive macro        -> ortho_config_macros unit tests
  -> EP-M2 runtime precedence  -> clap_integration parsing.rs + option_cases.rs
  -> EP-M2b empty accumulator  -> declarative_merge_empty_layers.rs
  -> EP-M3 documentation IR    -> docs_ir.rs + renderer goldens
  -> EP-M4 prose and migration -> markdownlint + executable doc example
```

## Verification plan

- M1: `cargo test -p ortho_config_macros` — attribute assertions for both
  `bool` and `Option<bool>`, plus a figment check that `Some(false)` serializes
  to `false` and `None` yields no value.
- M2: `ortho_config/tests/clap_integration/parsing.rs` rstest cases covering
  absent, `--flag`, `--flag=false`, file/env `true` with no flag (lower layer
  wins), and file/env `true` with `--flag=false` (CLI clears to false). The
  `Option<bool>` half of the same matrix lives in
  `option_cases.rs::parses_option_bool_across_sources`, whose `absent` row
  asserts `None` — the row that distinguishes an unwritten field from an
  explicit `false`, and the row that exposed the empty-accumulator defect.
  Non-vacuity of the file rows is proven by mutation: writing `flag = false`
  into the fixture fails exactly `file_true_without_flag`.
- M2b: `ortho_config/tests/declarative_merge_empty_layers.rs` pins the
  empty-accumulator fix: an empty layer list yields all-`None` through both
  `merge_from_layers` and the generated `load` path, while a required field is
  still reported as `missing field` rather than silently defaulted.
- M3: `ortho_config/tests/docs_ir.rs` asserts `takes_value == true`, the
  `BOOL` value name, `["true", "false"]` possible values, and
  `value_optional == true`; renderer goldens regenerate and are reviewed.
- M4: `make markdownlint` for prose and the tested-example fences; the
  executable documentation example runs under `make test`.
- Full commit gates run through `scrutineer` before each CodeRabbit review.

## Outcomes & retrospective

Status: **not yet complete.** Every acceptance criterion in issue #444 is
implemented and covered, all seven gates have been green on the rebased tree at
`e27e35fb`, and draft PR #532 is open against `main`. What remains is a
docs-only delta: absorbing the `mdtablefix` reflow, correcting three en-GB
spellings, and correcting the false `make lint` claim recorded in `Progress`.
Once those re-run green, the plan is complete.

What was achieved, in the order the work forced it:

The ticket asked for one thing — a generated spelling for an explicit `false` —
and the implementation of it exposed two pre-existing defects that no gate
could see. Both were found by trying to *use* the feature rather than by
reading the code, and both were fixed here because the feature is unsound
without them. The first is the layering guard: it compared whole parsed
objects, so a command line that restated a struct default discarded its own
layer. The second is the empty accumulator: a merge supplying no values at all
reached the deserializer as `null`. The second is the more instructive, because
`OptionConfig` passes it by accident — `CsvEnv::raw()` always yields a
non-empty object, which seats the accumulator. Any prefixed, all-optional
struct hits the failure as soon as nothing supplies a value, which is precisely
the "absent" case this ticket names.

Three lessons worth carrying forward. **A test that cannot fail is worse than
no test**: the `Option<bool>` file rows initially passed vacuously because the
fixture struct was `OPTL_`-prefixed and therefore read a different dotfile than
the cases wrote; non-vacuity was established by mutation, not by assertion.
**Verify a defect is pre-existing before disclaiming it**: the `cargo/mod.rs`
doctest failure was shown to reproduce with this branch's changes stashed, and
`make test` uses `--all-targets`, which is why no gate sees it. **Read the
build system rather than assuming its behaviour**: the plan asserted
`make lint` short-circuits before whitaker; `Makefile:99` says otherwise, and
that claim had been used to explain a false green it did not cause.

The codebase was left healthy with respect to this change: `make clippy` and
the Whitaker suite are green, no lint or type violations remain on branch
lines, and the two latent Clippy violations the boolean work would have masked
(`shadow_reuse`, `too_many_arguments`) were cleared rather than suppressed.
