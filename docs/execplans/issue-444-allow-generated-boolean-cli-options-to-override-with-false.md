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
- [x] M3 (Task 3): docs IR `value_optional` marker, ADR-008, renderers,
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
      `documentation_examples/boolean_override.rs`; ADR-008 links made
      same-directory; the migration-guide "Before" block now shows the real
      `TooManyValues` parse failure instead of output the old code could not
      produce; and the ExecPlan itself is indexed in `docs/contents.md`.
- [ ] Re-run gates; push; draft PR.

## Surprises & discoveries

- **`-ffalse` is rejected.** With `require_equals(true)`, an attached short
  value is an `ArgumentConflict`; only `-f=false` works. Worth one sentence in
  the user guide.
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
- **`--flag false` is rejected.** `require_equals` forces the `=` spelling, so
  a space-separated value is an `UnknownArgument`. This is deliberate: without
  it, `--flag --other x` would try to consume `--other` as the flag's value.
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

## Verification plan

- M1: `cargo test -p ortho_config_macros` — attribute assertions for both
  `bool` and `Option<bool>`, plus a figment check that `Some(false)` serializes
  to `false` and `None` yields no value.
- M2: `ortho_config/tests/clap_integration/parsing.rs` rstest cases covering
  absent, `--flag`, `--flag=false`, file/env `true` with no flag (lower layer
  wins), and file/env `true` with `--flag=false` (CLI clears to false).
- M3: `ortho_config/tests/docs_ir.rs` asserts `takes_value == true`, the
  `BOOL` value name, `["true", "false"]` possible values, and
  `value_optional == true`; renderer goldens regenerate and are reviewed.
- M4: `make markdownlint` for prose and the tested-example fences; the
  executable documentation example runs under `make test`.
- Full commit gates run through `scrutineer` before each CodeRabbit review.
