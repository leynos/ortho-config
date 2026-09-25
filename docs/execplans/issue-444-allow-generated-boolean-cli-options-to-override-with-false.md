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
- [x] (2026-09-24) All six deterministic gates green at `cb0a8c65`:
      `check-fmt`, `typecheck`, `lint`, `test`, `markdownlint`, `nixie`. The
      count is six because `make lint` is a single target that runs
      `lint-clippy` and `lint-whitaker` in sequence; entries below that name
      those two separately are enumerating the checks inside that one gate.
      Cleared the latent violations that clippy had been masking:
      `shadow_reuse`, `self_named_module_files`,
      `shadow_unrelated` x2, `struct_excessive_bools`, `too_many_arguments`;
      split `cli_flags.rs` and `agent_context/mod.rs` for `module-max-lines`;
      corrected six `-ise` spellings to the house `-ize` form.
- [x] (2026-09-25) CodeRabbit `--agent --committed --base main` reviewed the
      46-file diff and returned five distinct concerns, none re-litigating the
      accepted design. All five actioned: the roff optional-value placeholder
      now joins the flag (`--flag[=BOOL]`, matching clap's own
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
- [x] (2026-09-25) All six gates green at `2cfc9bb1`. The scrutineer's run
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
- [x] (2026-09-25) All six gates green at `e27e35fb`, on the rebased tree:
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
- [x] (2026-09-25) Scrutineer ran the six gates after the round-4 and
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
- [x] (2026-09-25) CodeRabbit round 5 reviewed the branch and returned six
      findings (five `minor`, one `trivial`). Two required rulings rather than
      mechanical edits.
- [x] (2026-09-25) Round 5, findings 1 and 2 contradicted each other on the
      same question. One asked to preserve the British `-ise` forms of three
      words in a Rust doc comment; the other asked to convert one of those same
      words to the accepted `-ize` form. Both cannot be right. `AGENTS.md:24`
      settles it — comments must use en-GB-oxendict ("-ize" / "-yse" /
      "-our") — and the repository's own spellcheck gate had already demanded
      `-ize` for these exact three words when they appeared in Markdown. So the
      `-ize` direction is correct, finding 1 is wrong, and the three words are
      now `-ize` in the Rust comments too. The finding that was wrong is the
      more interesting one, because acting on it would have violated the stated
      house style while looking like a review being actioned.
- [x] (2026-09-25) Round 5, finding 5 exposed a real documentation defect, and
      a wider one than it named. It said the users' guide should describe the
      man-page flag as `--excited[=BOOL]` with `BOOL` italic and no angle
      brackets. Reading clap's own source settled the premise:
      `stylize_arg_suffix` in `clap_builder-4.6.2/src/builder/arg.rs:4677`
      emits `[=` plus the placeholder plus `]`, and `render_arg_val` wraps
      every value name in angle brackets, so clap's `--help` prints
      `[=<BOOL>]`. The roff renderer does not wrap, and its golden confirms
      `\fI[=BOOL]\fR`. The docs had been describing *man-page* output in
      *clap's* notation. The finding named one site; the same false claim
      appeared in four, all fixed: the users' guide, two passages of ADR-009,
      and the migration guide. The two IR field doc comments were left alone —
      they say renderers bracket the placeholder without asserting a spelling,
      which remains true.
- [x] (2026-09-25) Round 5, findings 3, 4, and 6 were actioned: the PowerShell
      sentence now uses `concat!` (with an explicit `spelling = spelling`
      argument, because an empirical probe showed `format!(concat!(…))` cannot
      capture implicitly), and the ExecPlan's ADR-003 reference now names the
      file that exists.
- [x] (2026-09-25) All six gates green at `d211cc68` over the round-5 fixes:
      `check-fmt`, `typecheck`, `lint` (clippy and whitaker), `test` (1348
      passed, 0 failed, 15 ignored; pytest 87 passed, 5 skipped; no panics),
      `markdownlint`, `nixie`. Pushed as a fast-forward. Two gate regressions
      in this commit's own prose were caught and fixed first: the round-5
      Progress entry used a blank line mid-bullet, which markdownlint read as
      an indented code block (MD046), and substituting the longer ADR filename
      pushed a line past 80 columns (MD013). Both were found by the stop hook
      running the gates rather than by self-review, which is the argument for
      letting the hook run rather than reasoning about whether prose is
      compliant.

- [x] (2026-09-25) CodeRabbit round 6 reviewed `c23ce151` and returned nine
      findings, which resolve to six distinct claims because 1/9, 2/7 and 6/8
      share a site and a subject. Five of the six were actioned, none blocking:
      the changelog had clap's `[=<BOOL>]` help notation where the roff renderer
      prints `[=BOOL]`, the ADR described a marker-ignoring renderer as printing
      a bare switch (true before `takes_value` became unconditional for
      booleans), the migration guide stated the layering trigger more broadly
      than it holds, the PowerShell fallback now uses `concat!`, and the gate
      count is six throughout. The sixth asked for unrelated entries to be
      pruned from `typos.toml`; it was declined, because restoring that file to
      `main` and re-running `make spellcheck` rewrites it byte for byte — a test
      run directly rather than argued. `check-fmt`, `markdownlint` (including
      `spellcheck`), and the PowerShell unit tests were green over the fixes.

- [x] (2026-09-25) CodeRabbit round 7 reviewed `283a2333` and returned two
      findings, both low severity and both confirmed by measurement rather than
      reading. The first was a real renderer defect: `value_type_placeholder`
      returns `""` for `ValueType::Bool` and every roff call site feeds that
      into `format_option`, so a boolean without an explicit `value_name`
      rendered the malformed `--flag[=]`. Empty placeholders are now treated as
      absent, with three unit tests (fallback path, explicit empty string, and
      a negative control proving a real placeholder still wins) proven
      non-vacuous by reverting the fix and watching the two fallback tests
      fail. The second finding was the `require_equals` rationale, and
      re-measuring it falsified a claim this branch had already written into
      the user's guide, ADR-009, and the derive macro's own comment; see the
      first entry under `Surprises & discoveries` for the full reversal.
- [x] (2026-09-25) The round-7 fixes then exposed a gate-ordering lesson. The
      new test lines pushed `roff/escape.rs` to 445 lines against the 400-line
      module limit, which `lint-clippy` passes over but `lint-whitaker`
      rejects. Because clippy failed first on an unrelated `or_fun_call` style
      lint, `lint-whitaker` had never executed on this change set at all; it
      only ran once the clippy error was cleared. The module is now split into
      `roff/escape/mod.rs` plus `roff/escape/tests.rs`, matching the existing
      `agent_context/` and `powershell/maml/` layout the workspace's
      `self_named_module_files = "deny"` lint mandates. A second, quieter
      lesson: the `typos` gate scans files as recorded in the git index, so
      the deleted `escape.rs` had to be staged before `make spellcheck` could
      pass — the failure message named the removed path, not a typo.
- [x] (2026-09-25) All six gates green over the round-7 fixes at the
      then-current working tree: `check-fmt` (74 files), `typecheck`, `lint`
      (both `lint-clippy` and `lint-whitaker`), `test` (the three new
      `roff::escape` tests present and passing, no failures anywhere in 77
      green suites), `markdownlint` (75 files, 0 errors, including
      `spellcheck`), and `nixie`. `typos.toml` held its fixed point throughout
      (`4da00df0cbdf6d3f623dc143896cda360d381b3b1f610886845ee4b010840f98`).

- [x] (2026-09-25) CodeRabbit round 8 reviewed `e1beb607` (51 files) and
      returned two findings, both minor, with no duplicates between them. Both
      were verified and applied, and both are the kind of defect a reader
      notices but a gate cannot: the rustdoc on `replay.rs` broke the
      hyphenated compound `per-argument` across a line, which Markdown renders
      as "per- argument", and ADR-009 wrote the plural possessive "users'
      guide" where the project's own style guide and the document's title both
      use the singular "user's guide". Notably, none of the round-7 fixes drew
      a finding — the `--flag[=]` render fix, its three tests, and the
      corrected `require_equals` rationale all passed review cleanly.

- [x] (2026-09-25) CodeRabbit round 9 reviewed `cd30e431` (51 files) and
      returned two findings, which are duplicates of a single action item: the
      `Outcomes & retrospective` status paragraph still said "Seven rounds" and
      named round 7 as the most recent change, one round after the Progress log
      had moved to eight. The staleness was self-inflicted — the round-8 commit
      added the Progress entry but did not refresh the summary paragraph above
      it — and the finding is a fair catch: a living plan whose two halves
      disagree is worse than one that omits detail. The paragraph now reports
      eight rounds, presents the round-8 fixes as latest with round 7 retained
      as preceding, and states the remaining work. None of the round-8 fixes
      themselves drew a finding.

- [x] (2026-09-25) CodeRabbit round 10 reviewed `55eb3100` (51 files) and
      returned two findings. The first was correct and is applied: PowerShell
      help's flag-free fallback read "The value is optional: supplying it sets
      `true`", whose pronoun binds most naturally to *the value*, making the
      sentence circular — a value that "sets true" is not the case being
      described. It now reads "the flag without a value means `true`", and the
      unit test asserts that clause rather than only the explicit-false one, so
      the reword is covered and not incidentally passing. The phrase occurs
      once in the tree, so the one edit is exhaustive. The second finding was
      the `Outcomes & retrospective` status paragraph, which had drifted one
      round behind for the second consecutive time: the round-9 fix wrote
      "Eight rounds" and round 9 itself made that stale the moment it landed.
      That is a self-arming loop, not a defect that can be fixed by writing a
      fresher number, so the per-round narrative is gone — the paragraph now
      states the pattern and points here, which is the only disposition that
      ends the recurrence. CodeRabbit's own finding named this alternative. The
      trade accepted here is that `Outcomes` no longer summarizes the
      individual rounds; the entries in this section are the single source of
      truth for them, and a future round cannot desynchronize a count from a
      list it no longer contains.

- [x] (2026-09-25) CodeRabbit round 11 reviewed `407b21ee` (51 files) and
      returned one finding, the first new material it has raised in three
      rounds: `ortho_config/tests/docs_ir.rs` had reached 407 lines, over the
      400-line limit at `AGENTS.md:33`. The finding is fair and this branch is
      what caused it — the file was 393 lines on `origin/main` and the four
      `ensure!` blocks added for the boolean metadata pushed it over. Note the
      limit is not gate-enforced here: `lint-whitaker` runs with
      `--all-targets` and stayed green, because `dylint.toml` carries no
      `[module_max_lines]` section, so only human review caught this. The
      round-7 `roff/escape.rs` split was different — that one tripped the lint
      in a crate the config does cover. The split shares `DocsConfig` through a
      new `ortho_config/tests/support/docs_ir_config.rs`, alongside the existing
      `support/` modules, and moves the seven per-field tests plus
      `field_by_name` into a sibling `docs_ir_fields.rs`. The 48-line
      `#[ortho_config(...)]` attribute is the reason the config is shared rather
      than duplicated: two copies would drift the first time a feature was
      added. A probe established that `rstest` fixtures cannot be imported
      across modules without tripping `unused_import`, which is fatal under
      `-D warnings`, so each suite keeps a three-line fixture of its own rather
      than sharing one. `docs_ir.rs` falls to 182 lines and the new file is
      201, both with headroom. The test count is unchanged at fourteen (7 + 7),
      and non-vacuity was shown by inverting the moved `value_optional`
      assertion: `test_field_verbose` then fails for the intended reason and
      the other six pass.

- [x] (2026-09-25) CodeRabbit round 13 reviewed `02fd6b1b` (53 files), the
      first round to look at the rebased and split tree, and returned one minor
      finding that was a genuine defect: the generated `#[arg(...)]` never set
      `value_name`, so clap derived the help placeholder from the *field name*.
      `--is-excited` therefore rendered as `--is-excited[=<IS_EXCITED>]` while
      the documentation IR reported the value name `BOOL` — the two surfaces
      this branch exists to make agree disagreed. The fix names it explicitly,
      matching the precedent already in the tree: `config_flag.rs:65` sets
      `value_name = "PATH"` for the config-path flag, and
      `cargo-orthohelp/src/cli/mod.rs` hand-writes `value_name = "BOOL"` on its
      own boolean arguments. The boolean branch was the one place that had
      missed it.
      Two tests pin it, at different levels. The macro unit test
      `boolean_fields_accept_an_optional_value` now asserts the token text, and
      a new `clap_integration/help.rs` renders the *real* command the derive
      generates and asserts the rendered placeholder. The second level is the
      one that matters: a token-level check passes as soon as the attribute is
      present, but only rendering shows what a user reads. Both were proven
      non-vacuous by deleting the six added lines — the macro test then fails
      for the stated reason, and the rendered help reverts to
      `--is-excited[=<IS_EXCITED>]`. A third test in that file, asserting the
      documented `--flag` / `--flag=true` / `--flag=false` / absent round trip,
      passes in *both* states, which is what establishes that the pair is a
      genuine placeholder control rather than a behaviour change.
      A structural note for anyone extending this: the derive emits its hidden
      parser struct with private visibility, in the same module as the
      configuration type, so a test in a sibling module cannot name it and
      re-exporting it is rejected as a private-interface leak (E0365). The test
      struct therefore lives in `help.rs` rather than being shared from
      `common`. This is the same class of constraint as the `rstest` fixture
      import in round 11: generated and test-only surfaces are visible only
      where they are produced.

- [x] (2026-09-25) CodeRabbit round 14 reviewed `07e6ea53` (53 files) and
      returned **no findings** — the first clean round on this branch. All six
      gates were green on that HEAD, run sequentially by the gate-runner:
      `check-fmt`, `typecheck`, `lint` (rustdoc + Clippy + Whitaker),
      `test` (78 suites, 0 failed, plus 87 pytest passed and 5 skipped),
      `markdownlint` (with `spellcheck`), and `nixie`. The round-13
      `value_name = "BOOL"` finding was not re-raised, and neither were any of
      round 12's three.
      One count was checked rather than assumed. `clap_integration` reports 43
      tests in the gate log against the 42 seen in the filtered local run; the
      difference is the `#[cfg(feature = "yaml")]` test in `xdg.rs`, which
      `make test`'s `--all-features` enables and a default-feature local run
      skips. A name-by-name diff of the two runs is empty, so nothing regressed
      and the gate is the superset.
      Note on the 400-line rule, since it was raised as a risk for this commit:
      `cli_flags/mod.rs` is now 381 lines, and the six added lines were checked
      against it. The rule is not gate-enforced here (`dylint.toml` carries
      only `[no_std_fs_operations]`, and `module_max_lines` appears nowhere in
      the repository), so the file's 19-line headroom is a convention the gate
      will not defend. It was left at 381 rather than split, because the added
      doc comment explains the exact tokens it sits beside, and moving the
      token generation away from that explanation would cost more than the
      convention's headroom is worth.

## Surprises & discoveries

- **The documentation IR and clap's `--help` can disagree about the same
  field.** The IR reported `value_name: Some("BOOL")` for every boolean field,
  and the derive's own comment described the `=<BOOL>` value, but the generated
  `#[arg(...)]` never set `value_name`. Clap therefore derived a placeholder
  from the *field name*, so real help read `--is-excited[=<IS_EXCITED>]`.
  Nothing caught it because every test that touched the boolean surface
  asserted either the IR or the parse result, and the two happened to agree
  about what mattered in each. Round 13 caught it by asking what a user
  actually reads. The general form: when one change must make two descriptions
  of the same interface agree, at least one test has to assert the *rendered*
  artefact, because asserting the intermediate representation only proves the
  intermediate representation.
- **A token-level assertion cannot see this class of defect, and neither can
  a rendering test of a hand-built stand-in.** The macro unit test asserts the
  emitted `#[arg(...)]` text; adding `value_name = "BOOL"` satisfies it
  immediately. Proving the placeholder needs `render_help()` on the command the
  derive actually builds. That in turn runs into a second constraint: the
  hidden parser struct is emitted with private visibility in the same module as
  the configuration type, so a test in a sibling module cannot name it and
  re-exporting it is rejected as a private-interface leak (E0365). The test
  struct therefore lives in the file that asserts on it. This rhymes with the
  round-11 `rstest` fixture finding — generated and test-only surfaces are
  reachable only where they are produced — and the reusable move is to declare
  the test's own configuration type rather than trying to share one from a
  helper module.

- **A probe with an *undefined* argument reverses its own answer.** The
  first measurement of the `require_equals` justification used `--other`
  without ever defining an `--other` argument. Both configurations reported
  `unexpected argument '--other' found`, so the probe appeared to confirm the
  claim it was testing — but that error was firing because `--other` did not
  exist, not because of anything `require_equals` does. Defining the argument
  drops the masking error entirely: `--flag --other` parses identically with
  and without `require_equals`, because clap never reads a hyphen-leading token
  as an optional value. The hazard is positional instead. Without the rule,
  `--flag notes.txt` silently consumes the positional operand as the boolean
  value; with it, the operand survives. The falsified rationale had been copied
  into the user's guide, the ADR, and the derive macro's own comment, so all
  three were corrected to state the measured reason. The general lesson: when a
  probe's error path is the thing being measured, confirm the error is the one
  under test rather than a neighbouring failure that fires first for unrelated
  reasons.

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
  claim the error became clearer. The restriction is deliberate, but not for
  the reason first recorded here: a probe with `--other` *defined* shows that
  `--flag --other` parses the same with or without `require_equals`, because
  clap does not read a hyphen-leading token as an optional value. The real
  hazard is positional — without the rule, `--flag notes.txt` silently consumes
  `notes.txt` as the boolean value instead of leaving it for the trailing
  positional argument.
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
- **`typos.toml` cannot be pruned by hand, only committed at its fixed
  point.** The spellcheck gate regenerates this tracked file through
  `typos-config-builder`, whose pinned dictionary drifts ahead of the committed
  copy, so every gate run leaves it dirty. The added entries are unrelated to
  this branch: CSS alignment utilities, the camel-cased `currentColor`, and one
  entry that is itself a misspelling appear nowhere in the tree. A review
  finding asked for those unrelated entries to be dropped so the diff carried
  only in-scope changes. That is not achievable, and the test is decisive:
  restoring the file to `main` and re-running `make spellcheck` rewrites it,
  byte for byte, back to the same content, because the generator derives it
  from data this branch cannot change. The file is committed as that fixed
  point instead, once verified to be additive within the ignore list, to leave
  the overlay untouched, and to be the same bytes the gate reproduces. Three
  sibling worktrees, including one on an unrelated branch, carry the same
  churn, with two of them byte-identical to each other, which is independent
  evidence that it is upstream dictionary drift rather than this branch's doing.

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
  `--all-targets`, which excludes doctests, so the six gates never exercise it.
  A sibling branch, `harden-cargo-external-subcommand-doctest-20260924`,
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
- `docs/adr-003-define-schema-ownership-for-agent-native-contracts.md` — the
  documentation IR and the `cargo-orthohelp` schema mirror move together; new
  fields need an explicit default so older derives keep parsing.
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
- M3: `ortho_config/tests/docs_ir_fields.rs` asserts `takes_value == true`,
  the `BOOL` value name, `["true", "false"]` possible values, and
  `value_optional == true`; renderer goldens regenerate and are reviewed. The
  file was split out of `docs_ir.rs` in round 11 to restore the 400-line limit,
  and `support/docs_ir_config.rs` holds the shared configuration both halves
  describe.
- M3b: `ortho_config/tests/clap_integration/help.rs` renders the command the
  derive generates and asserts the `--flag[=<BOOL>]` placeholder that `--help`
  prints. This is the only check that spans both descriptions of the boolean
  surface: the `M3` assertions cover the documentation IR, and a token-level
  check of the generated `#[arg(...)]` passes even when clap substitutes a
  field-derived placeholder. A companion test asserts the field-derived form is
  absent, and a third pins the `--flag` / `=true` / `=false` / absent round
  trip in both states so the pair reads as a placeholder control rather than a
  behaviour change.
- M4: `make markdownlint` for prose and the tested-example fences; the
  executable documentation example runs under `make test`.
- Full commit gates run through `scrutineer` before each CodeRabbit review.

## Outcomes & retrospective

Status: **complete, pending review.** Every acceptance criterion in issue #444
is implemented and covered, all six gates have been green on the rebased tree,
and draft PR #532 is open against `main`. A CodeRabbit `--agent` review has
been requested at each milestone and every finding dispositioned. The
round-by-round log — including the two findings declined on evidence and the
one later re-opened and applied after re-measurement — lives in `Progress` and
is not restated here, so that a per-round count cannot drift out of step with
the log it summarizes.

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
