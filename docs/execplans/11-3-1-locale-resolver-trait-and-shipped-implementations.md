# 11.3.1 Add the `LocaleResolver` trait and shipped implementations

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (design review applied; awaiting user approval)

Branch: `11-3-1-locale-resolver-trait-and-shipped-implementations`

Roadmap item: 11.3.1 in `docs/roadmap.md`.

## Purpose / big picture

OrthoConfig is a Rust library that merges command-line, environment, and file
configuration. It also translates command-line help and parse errors. To
translate anything, an application must first choose a *locale*: a language tag
such as `ja-JP`. The choice is awkward because a user may name the locale with
a flag, yet an error in parsing that flag must already be translated. There are
therefore two moments: the *boot* locale, chosen before any flag is read, and
the *merged* locale, chosen after configuration has been merged.

Today the crate offers nothing for either moment. The `hello_world` example
holds the only implementation in this repository, and consumers write their
own. Netsuke, a downstream build tool, has a separate normalizer; Weaver and
Spycatcher-harness each have different rules.

After this change an application can write:

```rust
use ortho_config::{EnvLocaleResolver, LocaleResolver, MapEnv};
use std::sync::Arc;

let env = Arc::new(MapEnv::new().with_var("LANG", "ja_JP.UTF-8"));
let resolver = EnvLocaleResolver::new(env);
assert_eq!(resolver.boot_locale().to_string(), "ja-JP");
assert_eq!(resolver.merged_locale(Some("en-GB")).to_string(), "en-GB");
assert_eq!(resolver.merged_locale(None).to_string(), "ja-JP");
```

The change adds one trait, `LocaleResolver`, and three implementations:
`EnvLocaleResolver` (POSIX environment variables), `FixedLocaleResolver` (one
constant boot locale), and `ConfigLocaleResolver` (a primary resolver plus a
locale read from merged configuration). Nothing in the crate uses them unless
an application constructs one, so existing applications behave exactly as
before. The `hello_world` example starts using `EnvLocaleResolver`, which
deliberately changes its behaviour for the inputs listed under D4.

This plan does not build a localizer from the chosen locale. That is
`BootLocalizer`, roadmap item 11.3.2.

## Approval requested

Implementation must not begin until the user approves this plan and the
following items. Each names the decision that explains it.

- A1 (D1). The new types live in `ortho_config`, not in `ortho_l10n`.
- A2 (D4). `EnvLocaleResolver` corrects the example's POSIX rules rather than
  mirroring them "exactly", as design §5.1 asks. D4 tabulates every change.
- A3 (D5). An environment value that names no locale is skipped, and the next
  variable is consulted.
- A4 (D6). An explicit override is trimmed, which design §5.1 does not do, and
  one that does not parse is treated as no override. It is reported by a `warn`
  event that omits the value. This is a deliberate waiver of the preference for
  quiet library calls.
- A5 (D7). `ConfigLocaleResolver` takes an already-parsed locale and applies the
  order explicit, then configured, then primary. The review found that a
  command-line application which passes its merged value to `merged_locale`
  does not need this type. The plan keeps it, because the roadmap names it, and
  corrects the design's description of its purpose.
- A6 (D8). `FixedLocaleResolver` fixes the boot locale only. A valid explicit
  override still wins.
- A7 (D13). Tags are validated more strictly than `unic-langid` does, so a few
  malformed overrides that design §5.1 would accept are rejected.
- A8 (D10). The example's `detect_locale` is replaced now rather than in 11.7.1.
- A9 (D11). A new Architectural Decision Record (ADR) records the decisions, and
  the developers' guide gains a third permitted reader of `EnvSource`.

## Constraints

- Implementation begins only after explicit approval of this plan.
- The trait keeps the shape in design §5.1: `LocaleResolver: Send + Sync`,
  `boot_locale(&self) -> LanguageIdentifier`, and a provided
  `merged_locale(&self, explicit: Option<&str>) -> LanguageIdentifier`. It must
  remain usable as `Arc<dyn LocaleResolver>`, because 11.3.2 holds it that way.
  It is not sealed, because applications are entitled to their own resolvers.
- Environment detection is opt-in. No existing function, default, derive, or
  builder may construct an `EnvLocaleResolver`, and it must not implement
  `Default` (design §1.1; roadmap 11.3.1).
- All environment access goes through `ortho_config::EnvSource`, by exact
  variable name. No new call to `std::env` is permitted in `ortho_config`, and
  no enumeration (RFC 0001). `ProcessEnv::get` stays the crate's only
  `std::env::var_os` call.
- `ortho_l10n` must not change. ADR-0015 forbids it from depending on
  `ortho_config` and leaves requested-locale policy outside it.
- `merged_locale` and `boot_locale` must not panic for any input, including
  values that are not Unicode.
- Events must not contain environment values or override text. Every field is a
  member of a closed `&'static str` vocabulary, as in
  `ortho_config/src/merge_telemetry.rs`. No `#[tracing::instrument]` attribute
  may be used on these methods, because it records arguments by default.
- No new dependency, Cargo feature, or manifest change. `googletest`,
  `pretty_assertions`, `rstest`, `rstest-bdd`, `proptest`, and `trybuild` are
  already development dependencies of `ortho_config`.
- Minimum supported Rust version stays 1.89. `unsafe` is not used.
- Every file stays at or below 400 lines. Workspace lints deny
  `missing_docs_in_private_items`, `missing_const_for_fn`, `must_use_candidate`,
  `expect_used`, `indexing_slicing`, `string_slice`, `shadow_reuse`, and
  `self_named_module_files`, among others; use `mod.rs` directories. Whitaker
  holds cognitive complexity at 9.
- Tests do not change the process environment. Only a child process started
  through `assert_cmd` may have its environment set.
- Completed ExecPlans and accepted ADR text are history and are not edited.
- The API is pre-1.0 and new. No compatibility layer is permitted.

## Tolerances (exception triggers)

The comparison base is the merge base with `origin/main` when implementation
starts. Count net lines (additions minus deletions) from
`git diff --numstat "$(git merge-base HEAD origin/main)"`, excluding
`docs/execplans/**`. Classify whole files. Each limit applies independently.

- Production code: `ortho_config/src/**` except files whose names end in
  `tests.rs`, counted on its own. Limit: 500 net lines. Estimate 360, of which
  about half is required Rustdoc. `examples/hello_world/src/**` is counted
  separately and must not grow.
- Test code: `ortho_config/tests/**`, `ortho_config/src/**/*tests.rs`, and
  `examples/hello_world/tests/**`. Limit: 1,400 net lines. Estimate 1,050.
- Configuration: `Cargo.toml`, `Cargo.lock`, `.config/**`, `.github/**`,
  `Makefile`, `typos*.toml`. Limit: 10 net lines. Estimate 0.
- Documentation: `docs/**` except ExecPlans, `CHANGELOG.md`, and READMEs. Limit:
  550 net lines. Estimate 400.
- Files: 40 non-ExecPlan files. Estimate 31.
- Interface: any public item beyond those in "Interfaces and dependencies".
- Dependencies: any.
- Architecture: any change to `ortho_l10n`, `Localizer`, `EnvSource`, or derive
  output.
- Iterations: a gate that still fails after three distinct repairs.
- Time: a milestone that takes more than eight working hours, excluding time
  spent waiting for the shared compiler queue.
- Ambiguity: a reading of design §5 that changes an observable result.

When a limit is reached, stop, record options in `Decision log`, and ask.

## Risks

- Risk: the base moves. Draft PR #588 (11.2.1) edits
  `ortho_config/src/localizer/mod.rs`, `ortho_config/src/lib.rs`,
  `CHANGELOG.md`, both guides, and design §6. Severity: low. Likelihood: high.
  Mitigation: new code sits in a new directory. Expect textual conflicts only
  in re-export lists and prose. `lib.rs` is 364 lines, so re-check the cap
  after any rebase.
- Risk: the ADR number collides. Three unmerged branches already add an
  `adr-0026`. Severity: low. Likelihood: high. Mitigation: allocate the next
  number free on `origin/main` in `EP-M4`, and re-check before merge. This plan
  calls it "the locale-resolution ADR".
- Risk: an image exports `LC_ALL=C.UTF-8`, and a user who set only `LANG` gets
  English and reports a defect. Severity: low. Likelihood: medium. Mitigation:
  POSIX and gettext require it; the guide explains it; and the `debug` event in
  D14 names the variable that decided.
- Risk: a word that is not a language shadows a later variable. `unic-langid`
  accepts any 2, 3, or 5 to 8 letters, so `LANG=japanese`, `LC_ALL=unset`, and
  `LC_ALL=und` are all usable. Severity: low. Likelihood: low. Mitigation:
  documented and pinned by named cases. A list of real languages is out of
  scope.
- Risk: a `@modifier` carries meaning that is lost; `sr_RS@latin` becomes
  `sr-RS` and `ca_ES@valencia` becomes `ca-ES`. Severity: medium for those
  users. Likelihood: low. Mitigation: documented limitation; the example
  rejects these values entirely today. See D4.
- Risk: the compiler queue on the shared host makes gates slow. Severity: low.
  Likelihood: high. Mitigation: queued builds are normal; do not restart them.

## Progress

- [x] (2026-11-24 20:05Z) Four read-only Wyvern surveys: code, tests,
  documentation history, and the `unic-langid` parser with downstream consumers.
- [x] (2026-11-24 20:20Z) Prior-art research and initial draft (`7db7a394`).
- [x] (2026-11-24 20:50Z) Three-seat community-of-experts review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` trait, `FixedLocaleResolver`, and `ConfigLocaleResolver`.
- [ ] `EP-M2` `EnvLocaleResolver` and POSIX interpretation.
- [ ] `EP-M3` the example's end-to-end route.
- [ ] `EP-M4` ADR, guides, changelog, roadmap, and closure.

## Surprises & discoveries

- Observation: the example accepts `POSIX.UTF-8` as a language named `posix`.
  Evidence: `parse_posix_locale` compares the whole value with `POSIX` before
  it strips the codeset, and `unic-langid` accepts any 5 to 8 letter language.
  Impact: that locale has no catalogue, so the user loses the English copy. D4.
- Observation: the example rejects `C.UTF-8`, the usual container locale, and
  falls through. With `LC_ALL=C.UTF-8` and `LANG=ja_JP.UTF-8` it selects
  Japanese. GNU gettext says `C.UTF-8` "disables all localization as well".
  Impact: D4.
- Observation: `unic-langid` 0.9.6 does not validate a four-byte variant that
  begins with a digit, and never checks the first byte of any four-byte
  variant. Evidence: `Variant::from_bytes` in `src/subtags/variant.rs`. The
  strings `en-1.ab`, `en-@abc`, and `en-1` followed by an escape sequence all
  parse, and `en-zzzzz-abcd` displays as `en-abcd-zzzzz`, which reads back with
  a script. Impact: the first draft's parser axiom was false. D13.
- Observation: trimming only before the suffix is removed breaks the rule that a
  suffix never matters: `ja_JP .UTF-8` leaves a trailing space. Impact: D4
  trims twice.
- Observation: the review's corrections were checked at planning time by a
  throwaway Rust program, since deleted, that ran both algorithms against
  `unic-langid` 0.9.6 over 37,449 strings (all sequences of up to five tokens
  from `C`, `POSIX`, `aa`, `1`, `_`, `.`, `@`, and a space). INV-base,
  INV-portable, INV-extension, and INV-clean had no violations. The example
  accepted 2,740 strings, D4 newly accepted 4,236, and 3,078 were portable. The
  review's model found 754 violations of the first draft's INV-base and 12 of
  its INV-extension over the same strings, so the domain can expose the faults.
  This is planning evidence, not a substitute for the tests.
- Observation: the developers' guide names exactly two production readers of
  `EnvSource` and says that adding one "requires a decision about scope, not a
  call site". Impact: D3 and the locale-resolution ADR.
- Observation: the requested reference `docs/rtest-bdd-users-guide.md` does not
  exist. The file is `docs/rstest-bdd-users-guide.md`.

## Decision log

Unless stated otherwise, each decision was made by the planner on 2026-11-24.

- D1. Decision: put the trait and implementations in
  `ortho_config/src/localizer/locale_resolver/` and re-export them at the crate
  root. Rationale: ADR-0015 keeps configuration policy in `ortho_config` and
  says consumers own the requested locale. `EnvSource` is in `ortho_config`, and
  `ortho_l10n` cannot depend on it. Placing only the trait in `ortho_l10n` was
  rejected because no runtime-only consumer exists to use it; moving it later
  behind a re-export would be compatible.
- D2. Decision: keep the trait infallible and `Send + Sync`. Rationale: design
  §5.1 and §5.2 fix this shape for `BootLocalizer`. The review's strongest
  alternative was a chain of sources returning `Option<LanguageIdentifier>`,
  with `BootLocalizer` owning the default. It would make a fallback
  distinguishable from a choice and remove the need for `ConfigLocaleResolver`,
  but it abandons the approved boot and merged methods. The ADR records it as
  rejected.
- D3. Decision: `EnvLocaleResolver::new` takes a `SharedEnvSource`, and
  `EnvLocaleResolver::from_process_env` is the only process-backed constructor.
  There is no `Default`. The environment is read on each `boot_locale` call;
  the type stores no snapshot. Rationale: injection is the repository rule and
  the named constructor keeps the ambient choice visible. Consequence for
  11.3.2: two calls may differ if the environment changes, so `BootLocalizer`
  must call `boot_locale` once per handle and define what `finalize(None)`
  returns from that value. The ADR and the trait documentation state this.
- D4. Decision: interpret a value as follows. Trim it, take the text before the
  first `.` or `@`, and trim again. If that base is exactly `C` or `POSIX`, the
  result is `en-US`. Otherwise parse the base under D13. Rationale: design §5.1
  asks for the example's rules "exactly", but the example contradicts the
  design's intent. Netsuke and the `sys-locale` crate both strip at the first
  `.` or `@`. Mapping `@latin` to a script subtag was rejected: it needs a
  table nobody has asked for. Design §5.1 is amended in `EP-M2`. The table
  lists every class of change; all other values keep the example's result
  (INV-extension).

  | Value                          | Example today           | `EnvLocaleResolver` |
  | ------------------------------ | ----------------------- | ------------------- |
  | `C.UTF-8`, `C@x`               | ignored                 | `en-US`             |
  | `POSIX.UTF-8`                  | language `posix`        | `en-US`             |
  | `POSIX@x`                      | ignored                 | `en-US`             |
  | `de_DE@euro`                   | ignored                 | `de-DE`             |
  | `ja_JP .UTF-8`                 | ignored                 | `ja-JP`             |
  | `en_1@ab`                      | malformed tag `en-1@ab` | ignored             |
  | other illegal byte in the base | malformed tag accepted  | ignored             |

- D5. Decision: skip a variable whose value names no locale and consult the
  next one. Rationale: POSIX says the first non-empty variable decides, and
  that the effect of an unrecognized value is unspecified, so skipping is
  permitted. It matches the example, Netsuke, and design §5.1. An empty value
  counts as unset, as POSIX and gettext require; a value of only whitespace is
  treated alike, following the example. Skipping a non-empty value emits
  `debug`, not `warn`: the `rage` project had to fix a defect in which
  `i18n-embed` logged an error in containers with no `LANG`. An unset or blank
  variable is never reported as ignored.
- D6. Decision: an explicit override is trimmed and parsed under D13 with no
  POSIX suffix rules. A blank override is no override. One that does not parse
  is no override and emits one `warn` event per call. Rationale: design §5.1
  ignores an unparsable override; this keeps that and makes it visible.
  Trimming is an addition to §5.1. Every canonical tag accepted by a Clap
  `value_parser!(LanguageIdentifier)` is accepted here. The sets differ in two
  ways: surrounding whitespace is accepted only here, and the malformed tags of
  D13 only by Clap. An application that wants an error parses at its own
  boundary. The level is `warn` because the user's stated intent is discarded,
  and `localizer/clap_error.rs` already warns for a missing translation. An
  application that promises JSON-only standard error must already filter
  library events, and the guide says so.
- D7. Decision: `ConfigLocaleResolver::new(primary, configured)` takes
  `Arc<dyn LocaleResolver>` and `Option<LanguageIdentifier>`. `boot_locale`
  delegates to the primary. `merged_locale` returns the valid explicit
  override, else the configured locale, else `primary.merged_locale(None)`.
  Rationale: the design says the locale is read "after the merge phase", so it
  must not affect boot. A typed value cannot be unparsable, and the caller that
  parsed it can name its source. Design §5.1 defines `explicit` as "supplied
  via flag or file", which makes the type redundant for a plain merged value:

  | Flow                                | Without it               | What it adds                 |
  | ----------------------------------- | ------------------------ | ---------------------------- |
  | `--locale` flag                     | `finalize(flag)`         | nothing                      |
  | locale in merged configuration      | `finalize(merged value)` | nothing                      |
  | flag kept apart from file           | caller combines them     | the precedence rule          |
  | `finalize_with(None, resolver)`     | not expressible          | carries the configured value |
  | resolver handed to other components | each repeats the rule    | carries the configured value |

  The type is kept for the last three rows, and design §5.1's sentence that it
  is what applications "most commonly want" is corrected in `EP-M1`. A
  late-bound closure, which could be given to `BootLocalizer::new` before the
  merge, was rejected for its hidden state. Forwarding raw text to the primary
  was rejected because an invalid flag would discard a valid configured locale
  and would warn twice.
- D8. Decision: `FixedLocaleResolver` uses the provided `merged_locale`.
  Rationale: the design gives it no different rule, and discarding explicit
  intent silently is worse than honouring it. A deployment that is not
  locale-aware passes `None`. Ignoring overrides, as Spring's type of the same
  name does, was rejected. The documentation says "fixed" means boot only.
- D9. Decision: do not add `with_fallback`. The fallback is `en-US`. Rationale:
  the first draft added it, but nobody asked for it, and with a French fallback
  an unset environment would mean French while `LC_ALL=C` meant English. An
  application with another base language writes its own resolver, as ADR-0015
  allows. Adding the method later is compatible.
- D10. Decision: replace the body of `hello_world::localizer::detect_locale`
  with `EnvLocaleResolver`, and delete the example's parser and variable list.
  Keep `DemoLocalizer` for 11.7.1. Rationale: two parsers with different rules
  would contradict design principle 1, and the example binary gives a real
  end-to-end route.
- D11. Decision: record D1 to D9 and D13 in one ADR, referenced from design
  §5.1, and list `EnvLocaleResolver` among the permitted `EnvSource` readers in
  the developers' guide.
- D12. Decision: no Verus or Kani artefact. See "Formal verification decision".
- D13. Decision: one private `parse_tag` serves the environment and override
  paths. It rejects text containing any byte outside ASCII letters, digits,
  `-`, and `_` before it calls `LanguageIdentifier::from_str`. Rationale: the
  parser admits punctuation and control characters, and 11.3.2 will place tags
  in catalogue paths, logs, and terminals. A typed value given to
  `ConfigLocaleResolver` or `FixedLocaleResolver` is the caller's
  responsibility, and the documentation says so. Full RFC 5646 validation was
  rejected as out of proportion.
- D14. Decision: each `EnvLocaleResolver::boot_locale` call emits one `debug`
  event naming the variable that decided and whether it held a tag or a
  portable name, or that the fallback applied. Rationale: an operator asking
  "why is this in English?" needs the deciding variable. Boot usually precedes
  subscriber installation, so this event is often unseen; a later
  `merged_locale(None)` reads again and emits again. The plan claims no more
  than that. The override path has no such event.
- D15. Decision: provide no blanket implementations for `Arc<T>`, `Box<T>`, or
  `&T`. Rationale: `Localizer` has none, and consumers hold
  `Arc<dyn LocaleResolver>`.

## Outcomes & retrospective

Nothing is implemented. Complete this section at each milestone.

## Context and orientation

Facts were gathered at `cbba0d03` (`origin/main` on 2026-11-24). Re-locate
symbols by name rather than by line.

`ortho_l10n` is the localization runtime. It owns the `Localizer` trait and
`FluentLocalizer`, and re-exports `LanguageIdentifier` and `langid!` from the
`unic-langid` crate, built without its `serde` feature. `FluentLocalizer`
selects its embedded catalogue by language subtag only, so `ja` and `ja-JP`
behave alike, and `fr` fails with `UnsupportedLocale`. A resolver may therefore
return a locale with no catalogue; handling that belongs to 11.3.2.

`ortho_config/src/localizer/mod.rs` (18 lines) re-exports the runtime and adds
Clap adapters: `clap_command/`, `clap_error.rs`, and `localization_ids.rs`.
`ortho_config/src/lib.rs` (364 lines) lists every public name in
`pub use localizer::{…}`. No test snapshots the public API.

`ortho_config/src/env_source.rs` defines `EnvSource`, whose
`get(&self, key: &str) -> Option<OsString>` looks up one name. `ProcessEnv`
reads the live process and `MapEnv` holds a fixed map. `SharedEnvSource` is
`Arc<dyn EnvSource>`, and `process_env_source()` returns a shared `ProcessEnv`.
`EnvSource` requires `Debug`, and `MapEnv` prints no values.

`examples/hello_world/src/localizer.rs` (325 lines) holds `LOCALE_ENV_VARS`,
`detect_locale`, `parse_locale_from_env`, and `parse_posix_locale`. The last
trims, maps exactly `C` and `POSIX` to `en-US`, takes the text before the first
`.`, and parses it. `detect_locale` reads `LC_ALL`, `LC_MESSAGES`, and `LANG`
through `std::env::var`, and falls back to `en-US`. Its only caller is
`DemoLocalizer::new`. The module header lists the variables in the wrong order.
`examples/hello_world/tests/localised_help.rs` (373 lines) and
`localised_help_bdd.rs` run the binary with a controlled environment. The
package has two binaries, so `cargo run` needs `--bin hello_world`; the README
commands at lines 119 to 132 omit it and fail today.

Telemetry convention: no explicit target; an `event = "area.action"` field and
fields drawn from closed vocabularies. The shared test helper
`ortho_config/tests/support/tracing_capture.rs` records neither levels nor
spans, so the new tests carry their own small recording layer (see the pattern
in `ortho_config/tests/localized_parse.rs`). No metrics are added, because the
module performs no operation whose rate an operator would track.

Unit tests use a sibling `*_tests.rs` file declared with
`#[cfg(test)] #[path = "posix_tests.rs"] mod tests;`. Behavioural tests live in
`ortho_config/tests/features/*.feature`, bound in
`tests/rstest_bdd_support/behaviour/scenarios.rs`, with steps in
`behaviour/steps/` and contexts either in `scenario_state.rs` (364 lines) or,
as in `cargo_steps.rs`, beside the steps. Editing a `.feature` file does not
trigger a rebuild, so touch `tests/rstest_bdd.rs`. Step text must not begin
"the environment variable", which overlaps existing `DDLINT_*` steps.

`ortho_config/tests/localization_trybuild.rs` is a trybuild binary already
listed in `.config/nextest.toml`; adding a fixture to it needs no configuration
change. A new ordinary test binary needs no listing.

A Rust fence in `docs/users-guide.md` is a complete program with `fn main`,
preceded by `<!-- tested-example: id -->`. Its identifier goes in three lists:
`EXPECTED_EXAMPLE_IDS` in `ortho_config/tests/documentation_examples_tests.rs`,
compared for exact equality; `STANDARD_RUST_EXAMPLES` in
`ortho_config/tests/documentation_examples_rust_tests.rs`, which compiles it;
and `SIMPLE_RUNS` in
`ortho_config/tests/documentation_examples/simple_runs.rs`, which runs it and
compares standard output. `make test` does not run doctests; run them by hand.

Prior art, retrieved 2026-11-24: POSIX.1-2024 Base Definitions §8.2 (order
`LC_ALL`, the category variable, `LANG`; null values ignored; form
`language[_territory][.codeset][@modifier]`; unrecognized values unspecified);
the GNU gettext manual §2.3.1 and §2.3.2 (`C` and `C.UTF-8` disable
localization; `@variant` may be a script or dialect); the `sys-locale` crate's
`unix.rs` (strips at `.` or `@`, replaces `_`, performs no portable-name
mapping); and `str4d/rage` issue 156. The gettext `LANGUAGE` list is out of
scope: neither the roadmap nor the design names it.

Budget survey. The 31 files are: eight sources (`locale_resolver/` with
`mod.rs`, `fixed.rs`, `configured.rs`, `environment.rs`, `posix.rs`, and
`posix_tests.rs`, then `localizer/mod.rs` and `lib.rs`); five test files
(`locale_resolver.rs`, its two `locale_resolver_cases/` modules, the trybuild
fixture, and `localization_trybuild.rs`); four behavioural files; three
documentation-example registries; three example files; and eight documents.
Comparable history: 11.1.1 landed as 31 files and 2,240 insertions, and 11.1.2
as 57 files and 2,951 insertions, both including ExecPlans. 11.1.3 set 25 files
and 2,500 lines and reached 50 and 4,073; 9.1.1 set 2,500 and was granted an
exception at 8,695. Neither limit was raised repeatedly; both overran because
tests and review repairs were under-counted. This plan therefore budgets tests
separately, at nearly three times production, with about a third as contingency.

Signposts. Documents: `docs/cli-localization-design.md` §1.1, §1.2, §5;
`docs/design.md` §4.12; `docs/localizable-rust-libraries-with-fluent.md` (the
application is the sole authority on locale);
`docs/reliable-testing-in-rust-via-dependency-injection.md`;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rust-doctest-dry-guide.md`;
`docs/rstest-bdd-users-guide.md`;
`docs/complexity-antipatterns-and-refactoring-strategies.md`;
`docs/agent-native-cli-design.md` (protocol identifiers are never localized);
`docs/cargo-orthohelp-design.md`, `docs/improved-error-message-design.md`,
`docs/clap-dispatch-and-ortho-config-integration.md`, and
`docs/scripting-standards.md` (no constraint on this change; no script is
added); and `docs/documentation-style-guide.md`. Skills: `leta` or
`codegraph-mcp` for navigation; `rust-router`, then `rust-types-and-apis`,
`rust-unit-testing`, and `proptest`; `arch-decision-records` for the ADR;
`addressing-whitaker-findings` if Whitaker objects; `commit-message`; and
`sdlc-implementation` to execute. Run full gates through `scrutineer`.

## Conformance basis

There is no Terms of Reference document. Upstream artefacts at `cbba0d03`:
`docs/roadmap.md` 11.3.1; `docs/cli-localization-design.md` (status: proposed)
§1.1, §1.2, §5.1, §5.2; ADR-0015; RFC 0001 §5.3; and `AGENTS.md`.

- REQ-trait: `LocaleResolver` exists with `boot_locale` and `merged_locale`.
- REQ-env: `EnvLocaleResolver` consults `LC_ALL`, then `LC_MESSAGES`, then
  `LANG`, with POSIX normalization and the `C` and `POSIX` special cases.
- REQ-fixed: `FixedLocaleResolver` wraps one identifier.
- REQ-config: `ConfigLocaleResolver` composes a primary with a locale read after
  the merge phase.
- REQ-opt-in: environment detection is documented and built as opt-in.
- GAP-portable: the example's rule and the design's "exactly" contradict the
  portable-locale intent (D4).
- GAP-malformed: the parser accepts tags with punctuation and control bytes
  (D13).
- GAP-observable: a rejected override or variable leaves no trace today.
- GAP-scope: the developers' guide does not permit a third `EnvSource` reader.

```plaintext
REQ-trait  -> design 5.1 -> D1, D2, D15   -> EP-M1 -> trybuild fixture, INV-explicit
REQ-fixed  -> design 5.1 -> D8            -> EP-M1 -> INV-explicit table
REQ-config -> design 5.1 -> D7            -> EP-M1 -> INV-config, INV-boot, BDD
REQ-env    -> design 5.1 -> D3, D4, D5    -> EP-M2 -> INV-first, INV-base, INV-portable, BDD
REQ-env    -> D10                         -> EP-M3 -> localised_help.rs
REQ-opt-in -> design 1.1 -> D3            -> EP-M2 -> INV-names, construction search
REQ-opt-in -> roadmap 11.3.1              -> EP-M4 -> guide example and ADR
GAP-portable   -> D4       -> EP-M2 -> INV-portable, INV-extension; design 5.1 amended
GAP-malformed  -> D13      -> EP-M1 -> INV-clean
GAP-observable -> D5, D6, D14 -> EP-M1, EP-M2 -> INV-private
GAP-scope      -> D11      -> EP-M4 -> ADR and developers' guide
```

## Verification plan

### Abstract model

Let `Tag` be the set of language identifiers. A variable reading is one of
*unset*, *blank* (only whitespace), *non-Unicode*, *unrecognized*, or *usable
t*, for `t` in `Tag`. Let readings `v1, v2, v3` follow the order `LC_ALL`,
`LC_MESSAGES`, `LANG`:

```plaintext
boot(v1, v2, v3) = t       where the first usable vi is usable t
                 = en-US   when no vi is usable

tag(s)       = none        when s has a byte outside [A-Za-z0-9_-]
             = parse(s)    otherwise
base(s)      = trim(text of trim(s) before the first '.' or '@')
interpret(s) = en-US       when base(s) is "C" or "POSIX"
             = tag(base(s)) otherwise

explicit(raw) = none       when raw is absent or blank
              = tag(trim(raw)) otherwise

merged_default(raw, boot)               = explicit(raw) or boot
merged_config(raw, configured, primary) = explicit(raw) or configured
                                          or primary.merged(none)
```

There are no preconditions on input text. The environment is read afresh on
each call, so each claim is about one `EnvSource` state.

### Invariants and lemmas

- INV-first (REQ-env): `boot` equals the first usable reading, else `en-US`.
- L-transparent: replacing any unusable reading with another unusable reading
  does not change `boot`.
- L-shadow: readings after the first usable one do not change `boot`.
- INV-base (REQ-env): for any `b` without `.` or `@`, and any suffix that begins
  with one of them, `interpret(b + suffix)` equals `interpret(b)`. For such a
  `b`, surrounding whitespace and exchanging `_` with `-` also do not matter.
- INV-portable (GAP-portable): a value whose base is `C` or `POSIX` is usable
  and means `en-US`, so it shadows later variables.
- INV-extension (GAP-portable): let `x` be the text the example would parse,
  that is, `trim(s)` before its first `.`. If the example accepts `s`, every
  byte of `x` is in `[A-Za-z0-9_-]`, and `base(s)` is not `POSIX`, then
  `interpret(s)` gives the same identifier. If `base(s)` is `POSIX`, the result
  is `en-US`.
- INV-clean (GAP-malformed): the text of any identifier obtained from the
  environment or an override contains only `[A-Za-z0-9-]`.
- INV-explicit (REQ-trait): an override that parses is returned by every shipped
  resolver. A blank or unparsable one gives the same result as none.
- INV-config (REQ-config): explicit, then configured, then primary's merged
  answer.
- INV-boot (REQ-config): `boot_locale` of a `ConfigLocaleResolver` equals that
  of its primary, whatever is configured.
- INV-live (D3): after the source changes, the next call reflects the change.
- INV-total: no function panics, for any text or native string.
- INV-private (GAP-observable): the exact sequence and level of events is that
  of D5, D6, and D14, once per consulted variable per call. No event or span
  carries input text.
- INV-names (REQ-opt-in): only the three named variables are looked up, in
  order, and none after the first usable one.

### Axioms (trusted, not verified here)

- AXIOM-parse: for text in `[A-Za-z0-9_-]`, `unic-langid` 0.9
  `LanguageIdentifier::from_str` is total and deterministic and treats `-` and
  `_` alike, and `Display` emits only `[A-Za-z0-9-]`. Source: its parser
  source. No round trip from `Display` is assumed. The tests use the real
  parser.
- AXIOM-env: `MapEnv::get` returns the value for the exact name, or `None`.
  `ProcessEnv` does too, except that Windows ignores case in names.
- AXIOM-std: `str::trim`, `str::split`, and `find_map` behave as documented.

### Methods per obligation

Commands appear under "Concrete steps" by milestone. Every module named here is
part of one integration binary, `ortho_config/tests/locale_resolver.rs`, with
submodules in `ortho_config/tests/locale_resolver_cases/` included by
`#[path]`, to keep link cost down.

- INV-first, L-transparent, L-shadow, INV-portable, INV-names. Method:
  exhaustive enumeration. Domain: eight class representatives (unset, empty,
  whitespace, non-Unicode, unrecognized, portable, portable with codeset,
  usable tag) for each of three variables, 512 states, with a distinct tag per
  variable. Oracle: a fold over class labels that never sees the strings. Each
  state also asserts the recorded lookup sequence, which identifies the winner
  where portable and fallback results coincide. The non-Unicode value is
  `ja_JP.` followed by byte `0xFF` on Unix and by `0xD800` on Windows, so that
  a lossy decode would make it usable. Artefact:
  `locale_resolver_cases/precedence.rs`. Non-vacuity: assert that 512 states
  ran and that each variable and the fallback won at least once. This is
  complete over representatives, not over strings. Negative controls: swap two
  names; stop at an unrecognized value; treat `C` as unset; decode lossily;
  read all three variables eagerly; change the fallback.
- INV-base, INV-portable, INV-extension, INV-clean. Method: bounded exhaustive
  enumeration of every sequence of up to five tokens from `C`, `POSIX`, `aa`,
  `1`, `_`, `.`, `@`, and a space, which is 37,449 strings, using the real
  parser. Oracle for INV-extension: a frozen copy of the example's parser in
  the test file. Artefact:
  `ortho_config/src/localizer/locale_resolver/posix_tests.rs`. Non-vacuity:
  assert the string count, and that the oracle accepts some inputs, the
  interpreter newly accepts some, and some are portable; the planning program
  found 2,740, 4,236, and 3,078. Negative controls: strip only at `.`; compare
  the portable names before stripping; trim once; remove the byte check.
- INV-base over valid tags, INV-total, INV-clean. Method: `proptest`. One
  property builds an identifier with `LanguageIdentifier::from_parts` from
  generated subtags, spells it with a random mixture of separators, case, and
  suffixes, and asserts equality with the constructed value, so the oracle does
  not use `from_str`. A second feeds arbitrary strings and asserts only
  INV-total and INV-clean. Artefact: `posix_tests.rs`. Negative controls: return
  `None` always; return the uppercase text.
- Named cases: one `rstest` table holds every row of D4's table, the
  well-formed non-languages (`und`, `english`, `unset`), `en-1.ab`, `en-@abc`,
  and an escape sequence. Artefact: `posix_tests.rs`.
- INV-explicit, INV-config, INV-boot. Method: `rstest` tables covering every
  combination of explicit (absent, blank, unparsable, valid) and configured
  (absent, present), with a recording primary whose boot and merged answers
  differ. Artefact: `ortho_config/tests/locale_resolver.rs`. Negative controls:
  prefer configured to explicit; return configured from `boot_locale`; call the
  primary's `boot_locale` from `merged_locale`; forward raw text to the primary.
- INV-live. Method: a named test with an `EnvSource` whose value changes between
  calls. Negative control: cache at construction.
- INV-private. Method: named tests that record levels, fields, and span
  attributes, and compare the whole event vector. The sentinel is
  `sentinel!value`, which cannot parse; the test first asserts that the event
  exists, then that no field or span holds the sentinel. Artefact:
  `locale_resolver_cases/telemetry.rs`. Negative controls: add the value to an
  event; change a level; add `#[tracing::instrument]`.
- REQ-trait object safety. Method: trybuild pass fixture implementing the trait
  outside the crate and holding each type as `Arc<dyn LocaleResolver>`.
  Artefact: `ortho_config/tests/trybuild/locale_resolver_public_paths.rs`.
  `EP-M1` covers two types; `EP-M2` adds the third.
- REQ-opt-in construction. Method: the search under "Validation and acceptance".
  A compile-fail fixture for `Default` was rejected because its expected
  diagnostic changes with the pinned nightly.

Property tests use the default case count and no committed seed unless a
failure is found; a found regression file is committed. Each negative control
is applied once by hand at its milestone, its failing test is recorded in
`Artefacts and notes`, and it is reverted with a reverse patch.

### Formal verification decision

Neither Verus nor Kani is selected. After D4 and D13 the contractual logic is a
short composition of a trim, a cut, a byte check, and a first-match over three
variables. The precedence space is finite over classes and is enumerated. The
text rules are enumerated to a bound that is known to expose the faults the
review found, against the real parser, and are then sampled beyond it. A Verus
proof would be about a model of strings that nothing refines to the executed
code, and the parser would stay an axiom, which is where the faults were. Kani
would spend its bound exploring the same third-party parser. Stating the lemmas
was still worthwhile: formulating INV-base exposed the missing second trim.
Revisit this decision if a later item adds the gettext `LANGUAGE` list or
matching against supported locales, whose unbounded sequences would justify a
proof. ADR-0017 tooling is installed and would then apply.

`insta` snapshots are not used, because no new output format exists. The
example's existing snapshots must not change.

### Suggested BDD features

Adopted with the existing `rstest-bdd` runner as
`ortho_config/tests/features/locale_resolver.feature`. The configuration
scenarios (REQ-config) arrive in `EP-M1`, and the environment scenarios
(REQ-env) in `EP-M2`. Run with
`cargo test -p ortho_config --test rstest_bdd locale`.

```gherkin
Feature: Locale resolution
  Applications choose a boot locale before parsing and a merged locale after
  configuration has been loaded.

  Scenario: A configured locale applies only after the merge
    Given the primary locale is en-GB
    And the configured locale is ja
    When the boot locale is resolved with configuration
    Then the resolved locale is en-GB
    When the merged locale is resolved without an override
    Then the resolved locale is ja

  Scenario: An explicit override beats the configured locale
    Given the primary locale is en-GB
    And the configured locale is ja
    When the merged locale is resolved with override fr-CA
    Then the resolved locale is fr-CA

  Scenario: An invalid override is ignored
    Given the primary locale is en-GB
    And the configured locale is ja
    When the merged locale is resolved with override not_a_locale!
    Then the resolved locale is ja

  Scenario: The message category overrides the general language
    Given the locale variable LC_MESSAGES is ja_JP.UTF-8
    And the locale variable LANG is en_GB.UTF-8
    When the boot locale is resolved from the environment
    Then the resolved locale is ja-JP

  Scenario: The portable locale disables localization
    Given the locale variable LC_ALL is C.UTF-8
    And the locale variable LANG is ja_JP.UTF-8
    When the boot locale is resolved from the environment
    Then the resolved locale is en-US

  Scenario: An unusable variable does not hide a usable one
    Given the locale variable LC_ALL is 12345
    And the locale variable LANG is ja_JP.UTF-8
    When the boot locale is resolved from the environment
    Then the resolved locale is ja-JP
```

## Plan of work

Each milestone follows Red-Green-Refactor. A missing Rust type fails by not
compiling, so first add each type with a body returning the undetermined tag
`und`, and observe failed assertions. Invariance and negative obligations
(INV-base, INV-total, L-transparent, L-shadow, "blank equals none", and "no
event") can pass against that stub. Their red evidence is the negative control
named beside them, not the stub. The trybuild fixture is also exempt.

### `EP-M1` Trait, fixed, and configured resolvers

Create `ortho_config/src/localizer/locale_resolver/mod.rs` with the trait, the
private `parse_tag` (D13) and `explicit_locale` (D6) helpers, and module
documentation; `fixed.rs`; and `configured.rs`. Declare the module in
`localizer/mod.rs` and extend the `pub use localizer::{…}` list in `lib.rs`.
The trait documentation says that a custom resolver should normally override
only `boot_locale`, so that it keeps the override rules and the warning, and
that a caller should call `boot_locale` once. Add
`ortho_config/tests/locale_resolver.rs`, the override half of
`locale_resolver_cases/telemetry.rs`, the trybuild fixture, and the
configuration scenarios with `behaviour/steps/locale_resolver_steps.rs`, which
holds its own `LocaleResolverContext` as `cargo_steps.rs` does. Use
`googletest` matchers for classification and `pretty_assertions` for equality.
Correct design §5.1's description of `ConfigLocaleResolver`, and note the typed
value in §5.2. Every public item has a runnable Rustdoc example.

### `EP-M2` `EnvLocaleResolver`

Add `posix.rs` with
`interpret_posix_locale(&str) -> Option<LanguageIdentifier>` and
`environment.rs` with the resolver, a private reading enum, and the event
helper. Add `posix_tests.rs`, `locale_resolver_cases/precedence.rs`, the
variable half of the telemetry tests, the third trybuild type, and the
environment scenarios. The red run of the 512-state test fails on its first
state, where the fallback `en-US` is expected and `und` returned. The
type-level documentation says that detection is opt-in and lists daemons,
embedded interfaces, and Windows services as callers that should write their
own. Amend design §5.1: drop "exactly", give D4's rules, and state D13.

### `EP-M3` The example's end-to-end route

First extend the precedence table in `localised_help.rs` with red cases that
run the binary: `LC_ALL=C.UTF-8` and `LC_ALL=POSIX.UTF-8`, each with Japanese
`LANG`, give the English catalogue copy, and `LANG=ja_JP@x` gives Japanese. The
file has 27 lines of headroom. Then change
`examples/hello_world/src/localizer.rs`: `detect_locale` becomes
`EnvLocaleResolver::from_process_env().boot_locale()`; delete `LOCALE_ENV_VARS`,
`parse_locale_from_env`, `parse_posix_locale`, and their table test; and
correct the module header. Add `--bin hello_world` to the README commands,
describe the portable and modifier rules there, and add the `CHANGELOG.md`
entry under `Unreleased` as `### Added (locale resolution)`. All existing
snapshots must be unchanged.

### `EP-M4` Documentation and closure

Write the locale-resolution ADR and index it, and this ExecPlan, in
`docs/contents.md`. Reference the ADR from design §5.1. Add "Resolve the user's
locale" under the localization section of `docs/users-guide.md`. It states that
environment detection is opt-in and who should not use it, the variable order,
the portable and modifier rules, how to validate an override with Clap, when
`ConfigLocaleResolver` is needed, and that a rejected override warns, which
matters to an application promising JSON-only standard error. Register
`guide-locale-resolver` in the three lists named above, with empty expected
output. Update `docs/developers-guide.md` ("CLI localization surface" and
"Environment access boundary", with the reuse rule: new locale sources implement
`LocaleResolver`; the POSIX interpreter and `parse_tag` are private to the
module) and `docs/design.md` §4.12. Add an optional-adoption entry to the next
release's migration guide only if that guide exists by then: nothing released
changes behaviour, and the example is not published. Search for stale claims
with `grep -rn 'parse_posix_locale\|LocaleResolver\|LC_MESSAGES' docs examples`,
ignoring completed ExecPlans. Mark roadmap 11.3.1 and its three sub-items done.

## Milestones and plateaus

Each boundary check is: assigned requirements are met; the trait still matches
design §5.1; upstream assumptions hold; no public item beyond the list below,
no dependency, and no manifest changed; no trust boundary or persisted format
changed; and trace links are current. The compatibility decision for every
milestone is *none*: the API is new and pre-1.0.

- `EP-M1`. End state: a usable trait with two resolvers that read nothing from
  the process. Discharges REQ-trait, REQ-fixed, REQ-config, GAP-malformed.
  Evidence: `locale_resolver` tables, override telemetry, trybuild, and three
  scenarios. Recovery: revert the commit. Remaining: REQ-env.
- `EP-M2`. End state: all three resolvers work against an injected environment,
  and design §5.1 agrees with the code. Discharges REQ-env, GAP-portable,
  GAP-observable, and the construction half of REQ-opt-in. Evidence: 512
  states, 37,449 strings, properties, telemetry, and three scenarios. Recovery:
  revert the commit. Remaining: example and guides.
- `EP-M3`. End state: one POSIX interpreter in the workspace, exercised through
  a real process. Evidence: `localised_help`. Recovery: revert; the example
  returns to its own parser. Remaining: guides and ADR.
- `EP-M4`. End state: documentation matches code. Discharges the documentation
  half of REQ-opt-in, and GAP-scope. Evidence: documentation-example tests and
  the stale-claim search. Remaining: none.

## Concrete steps

Run from the repository root. Run each milestone's command for its red and
green stages.

```sh
# EP-M1
cargo test -p ortho_config --test locale_resolver --test localization_trybuild
cargo test -p ortho_config --test rstest_bdd locale
cargo test -p ortho_config --doc locale_resolver
# EP-M2, in addition
cargo test -p ortho_config --lib locale_resolver
# EP-M3
cargo test -p hello_world --test localised_help --test localised_help_bdd
# EP-M4
cargo test -p ortho_config --test documentation_examples_tests
cargo test -p ortho_config --test documentation_examples_rust_tests
```

After each milestone, format documentation with `make fmt`, then run these in
order through `scrutineer`, one at a time, for example:

```sh
set -o pipefail
make check-fmt 2>&1 | tee "/tmp/check-fmt-ortho-config-$(git branch --show-current).out"
```

```sh
make check-fmt
make lint
make test
```

For changed Markdown, also run `make markdownlint` and `make nixie`. Commit
after each milestone passes.

## Validation and acceptance

- Each focused command passes. Each behavioural test was seen to fail first
  against the stub, and each invariance test against its negative control.
- The first command below prints the English catalogue copy ("layered
  greetings"); before this change it prints Japanese. The second prints
  Japanese, as before.

```sh
run="cargo run -p hello_world --bin hello_world -- --help"
env -u LC_MESSAGES LC_ALL=C.UTF-8 LANG=ja_JP.UTF-8 $run
env -u LC_ALL -u LC_MESSAGES LANG=ja_JP.UTF-8 $run
```

- `grep -rn 'std::env' ortho_config/src/localizer/locale_resolver` prints
  nothing.
- `grep -rn 'EnvLocaleResolver' ortho_config/src ortho_config_macros/src` finds
  only the new directory and the two re-export lists.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` succeed. No Verus command applies (D12).

## Idempotence and recovery

Every step is an ordinary source edit and can be repeated. Restore a seeded
fault with a reverse patch, not `git checkout --`, which discards other
unstaged work. If a gate log stops without a diagnostic, treat the run as an
infrastructure failure and read the log before re-running.

## Artefacts and notes

Record red-run and negative-control transcripts here during implementation.

## Interfaces and dependencies

The complete new public surface of `ortho_config`:

```rust
pub trait LocaleResolver: Send + Sync {
    fn boot_locale(&self) -> LanguageIdentifier;
    #[must_use]
    fn merged_locale(&self, explicit: Option<&str>) -> LanguageIdentifier { /* D6 */ }
}

#[derive(Debug, Clone)]
pub struct EnvLocaleResolver { /* private */ }
impl EnvLocaleResolver {
    #[must_use]
    pub const fn new(env: SharedEnvSource) -> Self;
    #[must_use]
    pub fn from_process_env() -> Self;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLocaleResolver { /* private */ }
impl FixedLocaleResolver {
    #[must_use]
    pub const fn new(locale: LanguageIdentifier) -> Self;
}

#[derive(Clone)] // Debug is written by hand with `finish_non_exhaustive`.
pub struct ConfigLocaleResolver { /* private */ }
impl ConfigLocaleResolver {
    #[must_use]
    pub const fn new(
        primary: Arc<dyn LocaleResolver>,
        configured: Option<LanguageIdentifier>,
    ) -> Self;
}
```

Each constructor is `const` only where the compiler accepts it. Events, all
without input text:

- `locale.boot_resolved` at `debug`: `variable` (`LC_ALL`, `LC_MESSAGES`,
  `LANG`, `none`) and `form` (`tag`, `portable`, `fallback`);
- `locale.variable_ignored` at `debug`: `variable` and `reason` (`non_unicode`,
  `unrecognized`); and
- `locale.override_rejected` at `warn`.

Dependencies: none added.

## Revision note

2026-11-24, first draft: written from four reconnaissance surveys and prior-art
research.

2026-11-24, design review: three seats (API, verification, and executability)
returned "proceed with conditions" or "revise before approval". Changed: a
false parser axiom replaced by a byte check (D13); INV-base and INV-extension
corrected and checked against the real parser; `with_fallback` removed (D9);
the redundancy of `ConfigLocaleResolver` disclosed (D7); a deciding-variable
event added (D14); sampled near misses replaced by bounded enumeration; BDD
scenarios moved beside the code they cover; red evidence for invariance tests
redefined; and guide registries, acceptance commands, file budget, and lint
attributes corrected. Not adopted: a compile-fail fixture for `Default`, a
non-Unicode child-process case (the `MapEnv` case takes the same path, and the
example's test file is near its cap), an unconditional migration note, and
lowering the override event to `debug`; each reason is recorded above.
Remaining work: approval, then `EP-M1` to `EP-M4`.
