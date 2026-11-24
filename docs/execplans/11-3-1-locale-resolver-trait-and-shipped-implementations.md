# 11.3.1 Add the `LocaleResolver` trait and shipped implementations

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review; not yet approved)

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
holds the only implementation, and consumers write their own. Netsuke, a
downstream build tool, has a separate normalizer; Weaver and Spycatcher-harness
each have different rules.

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
constant locale), and `ConfigLocaleResolver` (a primary resolver plus an
override read from merged configuration). Nothing in the crate uses them unless
an application constructs one, so existing applications behave exactly as
before. The `hello_world` example starts using `EnvLocaleResolver`, which
deliberately changes its behaviour for a few inputs listed under D4.

This plan does not build a localizer from the chosen locale. That is
`BootLocalizer`, roadmap item 11.3.2.

## Approval requested

Implementation must not begin until the user approves this plan and the
following items. Each names the decision that explains it.

- A1 (D1). The new types live in `ortho_config`, not in `ortho_l10n`.
- A2 (D4). `EnvLocaleResolver` corrects the example's POSIX rules rather than
  mirroring them "exactly" as design §5.1 says. Four input shapes change.
- A3 (D5). An environment value that names no locale is skipped, and the next
  variable is consulted. An ignored value is reported only by a `debug` event.
- A4 (D6). An explicit override that does not parse is treated as no override
  and reported by a `warn` event that omits the value.
- A5 (D7). `ConfigLocaleResolver` takes an already-parsed override and applies
  the order explicit, then configured, then primary.
- A6 (D8). `FixedLocaleResolver` fixes the boot locale only. A valid explicit
  override still wins.
- A7 (D9). `EnvLocaleResolver` gains `with_fallback`, which neither the roadmap
  nor the design names.
- A8 (D10). The example's `detect_locale` is replaced now rather than in 11.7.1.
- A9 (D11). A new ADR records the decisions, and the developers' guide gains a
  third permitted reader of `EnvSource`.

## Constraints

- Implementation begins only after explicit approval of this plan.
- The trait keeps the shape in design §5.1: `LocaleResolver: Send + Sync`,
  `boot_locale(&self) -> LanguageIdentifier`, and a provided
  `merged_locale(&self, explicit: Option<&str>) -> LanguageIdentifier`. It must
  remain usable as `Arc<dyn LocaleResolver>`, because 11.3.2 holds it that way.
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
  `ortho_config/src/merge_telemetry.rs`.
- No new dependency, Cargo feature, or manifest change. `googletest`,
  `pretty_assertions`, `rstest`, `rstest-bdd`, `proptest`, and `trybuild` are
  already development dependencies of `ortho_config`.
- Minimum supported Rust version stays 1.89. `unsafe` is not used.
- Every file stays at or below 400 lines. Workspace lints deny
  `missing_docs_in_private_items`, `expect_used`, `indexing_slicing`,
  `string_slice`, `shadow_reuse`, and `self_named_module_files`, among others;
  use `mod.rs` directories. Whitaker holds cognitive complexity at 9.
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
  `tests.rs`, plus `examples/hello_world/src/**`. Limit: 450 net lines.
  Estimate 330, of which about half is required Rustdoc.
- Test code: `ortho_config/tests/**`, `ortho_config/src/**/*tests.rs`, and
  `examples/hello_world/tests/**`. Limit: 1,300 net lines. Estimate 950.
- Configuration: `Cargo.toml`, `Cargo.lock`, `.config/**`, `.github/**`,
  `Makefile`, `typos*.toml`. Limit: 10 net lines. Estimate 0.
- Documentation: `docs/**` except ExecPlans, `CHANGELOG.md`, and READMEs. Limit:
  550 net lines. Estimate 380.
- Files: 36 non-ExecPlan files. Estimate 26.
- Interface: any public item beyond those in "Interfaces and dependencies".
- Dependencies: any.
- Architecture: any change to `ortho_l10n`, `Localizer`, `EnvSource`, or derive
  output.
- Iterations: a gate that still fails after three distinct repairs.
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
- Risk: users read `LC_ALL=C` as "unset" and expect another variable to apply.
  Severity: low. Likelihood: low. Mitigation: documentation and INV-portable.
- Risk: a `@modifier` carries meaning that is lost; `sr_RS@latin` becomes
  `sr-RS` and `ca_ES@valencia` becomes `ca-ES`. Severity: medium for those
  users. Likelihood: low. Mitigation: documented limitation; today these values
  are rejected entirely. See D4.
- Risk: the compiler queue on the shared host makes gates slow. Severity: low.
  Likelihood: high. Mitigation: queued builds are normal; do not restart them.

## Progress

- [x] (2026-11-24) Four read-only Wyvern surveys: code, tests, documentation
  history, and the `unic-langid` parser with downstream consumers.
- [x] (2026-11-24) Prior-art research and initial draft.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` trait, `FixedLocaleResolver`, and `ConfigLocaleResolver`.
- [ ] `EP-M2` `EnvLocaleResolver` and POSIX interpretation.
- [ ] `EP-M3` behavioural scenarios and the example's end-to-end route.
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
- Observation: `unic-langid` accepts `_` as a separator, so the example's
  replacement of `_` is redundant. It rejects `.`, `@`, extensions, empty
  subtags, non-ASCII text, and NUL. Impact: the interpreter only needs to strip
  suffixes and handle the two portable names.
- Observation: the developers' guide names exactly two production readers of
  `EnvSource` and says that adding one "requires a decision about scope, not a
  call site". Impact: D3 and the locale-resolution ADR.
- Observation: the requested reference `docs/rtest-bdd-users-guide.md` does not
  exist. The file is `docs/rstest-bdd-users-guide.md`.

## Decision log

- D1. Decision: put the trait and implementations in
  `ortho_config/src/localizer/locale_resolver/` and re-export them at the crate
  root. Rationale: ADR-0015 keeps configuration policy in `ortho_config` and
  says consumers own the requested locale. `EnvSource` is in `ortho_config`, and
  `ortho_l10n` cannot depend on it. Placing only the trait in `ortho_l10n` was
  rejected because no runtime-only consumer exists to use it; moving it later
  behind a re-export would be compatible. Date/Author: 2026-11-24, planner.
- D2. Decision: keep the trait infallible and `Send + Sync`. Rationale: design
  §5.1 and §5.2 fix this shape for `BootLocalizer`. A resolver always has some
  answer, so failure is reported through events and through the caller's own
  validation of typed input. Date/Author: 2026-11-24, planner.
- D3. Decision: `EnvLocaleResolver::new` takes a `SharedEnvSource`, and
  `EnvLocaleResolver::from_process_env` is the only process-backed constructor.
  There is no `Default`. The environment is read on each `boot_locale` call;
  the type stores no snapshot. Rationale: injection is the repository rule, the
  named constructor keeps the ambient choice visible, and the crate already
  treats locale as a per-parse snapshot held by the localizer (design §1.2).
  Date/Author: 2026-11-24, planner.
- D4. Decision: interpret a value as follows. Trim it. Take the text before the
  first `.` or `@`. If that base is exactly `C` or `POSIX`, the result is
  `en-US`. Otherwise parse the base as a language identifier. Rationale: design
  §5.1 asks for the example's rules "exactly", but the example contradicts the
  design's own intent for four shapes: `C.*` and `POSIX.*` are not treated as
  portable; `xx_YY@mod` is rejected while `xx_YY.enc@mod` is accepted; and
  `C@mod` is rejected. Netsuke and the `sys-locale` crate both strip at the
  first `.` or `@`. Every value the example accepts keeps its result except
  `POSIX.*` (INV-extension). Mapping `@latin` to a script subtag was rejected:
  it needs a table nobody has asked for. Design §5.1 is amended in `EP-M4`.
  Date/Author: 2026-11-24, planner.
- D5. Decision: skip a variable whose value names no locale and consult the
  next one. Rationale: POSIX says the first non-empty variable decides, and
  that an unrecognized value is unspecified, so skipping is permitted and
  matches the example, Netsuke, and design §5.1. A blank value counts as unset,
  as POSIX and gettext require. Skipping emits `debug`, not `warn`: the `rage`
  project had to fix a bug where `i18n-embed` logged an error in containers
  with no `LANG`. An unset or blank variable emits nothing. Date/Author:
  2026-11-24, planner.
- D6. Decision: an explicit override is trimmed and parsed as a language
  identifier with no POSIX suffix rules. A blank override is no override. An
  unparsable one is no override and emits one `warn` event. Rationale: this is
  the rule in design §5.1, made observable. It also means that a value accepted
  by a Clap `value_parser!(LanguageIdentifier)` is exactly a value the resolver
  accepts. Applications that want an error parse at their own boundary.
  Date/Author: 2026-11-24, planner.
- D7. Decision: `ConfigLocaleResolver::new(primary, configured)` takes
  `Arc<dyn LocaleResolver>` and `Option<LanguageIdentifier>`. `boot_locale`
  delegates to the primary. `merged_locale` returns the valid explicit
  override, else the configured locale, else `primary.merged_locale(None)`.
  Rationale: the design says the override is read "after the merge phase", so
  it must not affect boot. A typed override makes an invalid one
  unrepresentable, and the caller that parsed it can name its source.
  Forwarding raw text to the primary was rejected because an invalid flag would
  then discard a valid configured locale. Date/Author: 2026-11-24, planner.
- D8. Decision: `FixedLocaleResolver` uses the provided `merged_locale`.
  Rationale: the design gives it no different rule, and discarding explicit
  intent silently is worse than honouring it. A deployment that is not
  locale-aware passes `None`. Ignoring overrides, as Spring's type of the same
  name does, was rejected. Date/Author: 2026-11-24, planner.
- D9. Decision: `EnvLocaleResolver` falls back to `en-US`, and
  `with_fallback(LanguageIdentifier)` replaces that fallback. `C` and `POSIX`
  still mean `en-US`. Rationale: ADR-0015 says consumers own application
  fallback policy, and without this hook an application whose base language is
  not English cannot tell a fallback from a choice. Date/Author: 2026-11-24,
  planner.
- D10. Decision: replace the body of `hello_world::localizer::detect_locale`
  with `EnvLocaleResolver`, and delete the example's parser and variable list.
  Keep `DemoLocalizer` for 11.7.1. Rationale: two parsers with different rules
  would contradict design principle 1, and the example binary gives a real
  end-to-end route. Date/Author: 2026-11-24, planner.
- D11. Decision: record D1, D3, D4, D5, D6, and D7 in one ADR, referenced from
  design §5.1, and list `EnvLocaleResolver` among the permitted `EnvSource`
  readers in the developers' guide. Date/Author: 2026-11-24, planner.
- D12. Decision: no Verus or Kani artefact. See "Formal verification decision".
  Date/Author: 2026-11-24, planner.

## Outcomes & retrospective

Nothing is implemented. Complete this section at each milestone.

## Context and orientation

Facts were gathered at `cbba0d03` (`origin/main` on 2026-11-24). Re-locate
symbols by name rather than by line.

`ortho_l10n` is the localization runtime. It owns the `Localizer` trait and
`FluentLocalizer`, and re-exports `LanguageIdentifier` and `langid!` from the
`unic-langid` crate. `FluentLocalizer` selects its embedded catalogue by
language subtag only, so `ja` and `ja-JP` behave alike, and `fr` fails with
`UnsupportedLocale`. A resolver may therefore return a locale with no
catalogue; handling that belongs to 11.3.2.

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
`DemoLocalizer::new`. `examples/hello_world/tests/localised_help.rs` (373
lines) and `localised_help_bdd.rs` run the binary with a controlled environment.

Telemetry convention: no explicit target; an `event = "area.action"` field and
fields drawn from closed vocabularies. `ortho_config/tests/support/`
`tracing_capture.rs` provides `capture` and `only` for assertions and is
included by `#[path]`. No metrics are added: the module emits no failure an
operator would alert on, and the `metrics` feature remains for loaders.

Tests: unit tests use a sibling file named `*_tests.rs`. Behavioural tests live
in `ortho_config/tests/features/*.feature`, bound in
`tests/rstest_bdd_support/behaviour/scenarios.rs`, with steps in
`behaviour/steps/` and contexts either in `scenario_state.rs` (364 lines) or,
as in `cargo_steps.rs`, beside the steps. Editing a `.feature` file does not
trigger a rebuild, so touch `tests/rstest_bdd.rs`. `ortho_config/tests/`
`localization_trybuild.rs` is a trybuild binary already listed in
`.config/nextest.toml`; adding a fixture to it needs no configuration change. A
new ordinary test binary needs no listing. Every fence in `docs/users-guide.md`
needs a `<!-- tested-example: id -->` marker, and a Rust fence is compiled and
run only if its id is in `STANDARD_RUST_EXAMPLES` in
`ortho_config/tests/documentation_examples_rust_tests.rs`. `make test` does not
run doctests; run them by hand.

Prior art, retrieved 2026-11-24: POSIX.1-2024 Base Definitions §8.2 (order
`LC_ALL`, the category variable, `LANG`; null values ignored; form
`language[_territory][.codeset][@modifier]`; unrecognized values unspecified);
the GNU gettext manual §2.3.1 and §2.3.2 (`C` and `C.UTF-8` disable
localization; `@variant` may be a script or dialect); the `sys-locale` crate's
`unix.rs` (strips at `.` or `@`, replaces `_`, performs no portable-name
mapping); and `str4d/rage` issue 156. The gettext `LANGUAGE` list is out of
scope: neither the roadmap nor the design names it.

Budget survey. The change touches one new source directory, two re-export
lists, one example module, one trybuild list, one documentation-example list,
the BDD registries, and about eight documents. Comparable history: 11.1.1
landed as 31 files and 2,240 insertions, and 11.1.2 as 57 files and 2,951
insertions, both including ExecPlans. 11.1.3 set 25 files and 2,500 lines and
reached 50 and 4,073; 9.1.1 set 2,500 and was granted an exception at 8,695.
Neither limit was raised repeatedly; both overran because tests and review
repairs were under-counted. This plan therefore budgets tests separately, at
nearly three times production, with about 35 per cent contingency.

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
- REQ-config: `ConfigLocaleResolver` composes a primary with an override read
  after the merge phase.
- REQ-opt-in: environment detection is documented and built as opt-in.
- GAP-portable: the example's rule and the design's "exactly" contradict the
  portable-locale intent (D4).
- GAP-observable: a rejected override or variable leaves no trace today.
- GAP-scope: the developers' guide does not permit a third `EnvSource` reader.

```plaintext
REQ-trait  -> design 5.1 -> D1, D2        -> EP-M1 -> trybuild fixture, locale_resolver.rs
REQ-fixed  -> design 5.1 -> D8            -> EP-M1 -> INV-explicit table
REQ-config -> design 5.1 -> D7            -> EP-M1 -> INV-config table, INV-boot
REQ-env    -> design 5.1 -> D3, D4, D5    -> EP-M2 -> INV-first, INV-base, INV-portable
REQ-env    -> D10                         -> EP-M3 -> BDD scenarios, localised_help.rs
REQ-opt-in  -> design 1.1 -> D3            -> EP-M4 -> guide example and ADR
GAP-portable -> D4  -> EP-M2 -> INV-portable, INV-extension; design 5.1 amended in EP-M4
GAP-observable -> D5, D6 -> EP-M1, EP-M2 -> INV-private, event tests
GAP-scope  -> D11 -> EP-M4 -> ADR and developers' guide
```

## Verification plan

### Abstract model

Let `Tag` be the set of language identifiers. A variable reading is one of
*unset*, *blank* (only whitespace), *non-Unicode*, *unrecognized*, or *usable
t*, for `t` in `Tag`. Let `read` classify a raw value. For readings
`v1, v2, v3` in the order `LC_ALL`, `LC_MESSAGES`, `LANG`, and a fallback `f`:

```plaintext
boot(v1, v2, v3, f) = t   where the first usable vi is usable t
                    = f   when no vi is usable

explicit(raw) = none      when raw is absent, blank, or does not parse
              = some t    otherwise

merged_default(raw, boot)                = explicit(raw) or boot
merged_config(raw, configured, primary)  = explicit(raw) or configured
                                           or primary.merged(none)

base(s)      = trim(s) up to the first '.' or '@'
interpret(s) = en-US      when base(s) is "C" or "POSIX"
             = parse(base(s)) otherwise
```

There are no preconditions on input text. The environment is read afresh on
each call, so each claim is about one `EnvSource` state.

### Invariants and lemmas

- INV-first (REQ-env): `boot` equals the first usable reading, else the
  fallback.
- L-transparent: replacing any unusable reading with another unusable reading
  does not change `boot`.
- L-shadow: readings after the first usable one do not change `boot`.
- INV-base (REQ-env): for any `b` without `.` or `@`, and any suffix starting
  with `.` or `@`, `interpret(b + suffix)` equals `interpret(b)`. Surrounding
  whitespace and the choice of `_` or `-` also do not matter.
- INV-portable (GAP-portable): a value whose base is `C` or `POSIX` is usable
  and means `en-US`, so it shadows later variables.
- INV-extension (GAP-portable): if the example's original parser accepts `s` and
  `base(s)` is not `POSIX`, `interpret(s)` gives the same identifier. If
  `base(s)` is `POSIX`, the result is `en-US`.
- INV-explicit (REQ-trait): a parsable override is returned for every shipped
  resolver. A blank or unparsable override gives the same result as none.
- INV-config (REQ-config): explicit, then configured, then primary.
- INV-boot (REQ-config): `boot_locale` of a `ConfigLocaleResolver` equals that
  of its primary, whatever is configured.
- INV-total: no function panics, for any text or native string.
- INV-private (GAP-observable): an event is emitted exactly for a non-Unicode or
  unrecognized variable that is consulted, and for a non-blank override that
  does not parse. No event field carries input text.
- INV-names (REQ-opt-in): only the three named variables are looked up, in
  order, and none after the first usable one.

### Axioms (trusted, not verified here)

- AXIOM-parse: `unic-langid` 0.9 `LanguageIdentifier::from_str` is total and
  deterministic, accepts `-` and `_`, rejects text containing `.` or `@`, and
  its `Display` output parses back to an equal value. Source: its parser
  source. The interpreter depends on it; the property tests use the real parser.
- AXIOM-env: `EnvSource::get` returns the value for the exact name, or `None`.
- AXIOM-std: `str::trim`, `str::split`, and iterator `find_map` behave as
  documented.

### Methods per obligation

- INV-first, L-transparent, L-shadow, INV-portable. Method: exhaustive
  enumeration. Domain: eight classes (unset, empty, whitespace, non-Unicode,
  unrecognized, portable, portable with codeset, usable tag) for each of three
  variables, 512 states, with a distinct tag per variable and a fallback of
  `fr`, so that the winner is identifiable. Oracle: a fold over class labels
  that never sees the strings. Artefact:
  `ortho_config/tests/locale_resolver_precedence.rs`. Evidence: the test
  asserts that it visited 512 states and that each of `LC_ALL`, `LC_MESSAGES`,
  `LANG`, and the fallback won at least once. Rationale: the domain is finite,
  so enumeration is complete up to AXIOM-parse. Negative controls: swap two
  names; stop at an unrecognized value; treat `C` as unset.
- INV-base, INV-extension, INV-total. Method: `proptest`. Domain: generated
  language, script, region, and variant subtags; generated suffixes; arbitrary
  strings; and a class of near misses built from `C`, `POSIX`, `.`, `@`, `_`,
  `-`, whitespace, NUL, and non-ASCII text. INV-extension uses a frozen copy of
  the example's parser as a test-only oracle. Artefact:
  `ortho_config/src/localizer/locale_resolver/posix_tests.rs`. Non-vacuity: a
  companion named-example table covers each class, and the extension property
  counts accepted inputs and fails if fewer than a quarter of the generated
  values are accepted. Negative controls: strip only at `.`; compare the
  portable names before stripping.
- INV-explicit, INV-config, INV-boot. Method: `rstest` tables covering every
  combination of explicit (absent, blank, unparsable, valid) and configured
  (absent, present), with a recording primary. Artefact:
  `ortho_config/tests/locale_resolver.rs`. Negative controls: prefer configured
  to explicit; return configured from `boot_locale`.
- INV-private, INV-names. Method: named tests with the tracing capture helper
  and a recording `EnvSource`. The input text is a distinctive sentinel that
  must not appear in any field. Artefact: `ortho_config/tests/`
  `locale_resolver_telemetry.rs`. Negative controls: add the value to an event;
  read all three variables eagerly.
- REQ-trait object safety. Method: trybuild pass fixture implementing the trait
  outside the crate and holding each type as `Arc<dyn LocaleResolver>`.
  Artefact: `ortho_config/tests/trybuild/locale_resolver_public_paths.rs`.

Property tests use the default case count and no committed seed unless a
failure is found; a found regression file is committed. Each negative control
is applied once by hand at its milestone, its failing test is recorded in
`Artefacts and notes`, and it is reverted with a reverse patch.

### Formal verification decision

Neither Verus nor Kani is selected. The contractual logic is a first-match over
three variables, whose complete state space is enumerated by test, and a suffix
strip whose result feeds a third-party parser. A Verus proof of the fold would
restate `find_map`, and a proof of the strip would cover `str::split`; neither
reaches AXIOM-parse, where a defect would matter. Kani over strings would bound
the same third-party parser at a cost the sampled property does not justify,
and would require the repository's first production harness, which ADR-0017
reserves for a feature that needs one. If review finds an unbounded obligation,
such as a variable-length list of variables, revisit this decision before
implementing it.

### Suggested BDD features

Adopted with the existing `rstest-bdd` runner as
`ortho_config/tests/features/locale_resolver.feature` (REQ-env, REQ-config,
`EP-M3`). Run with `cargo test -p ortho_config --test rstest_bdd locale`.

```gherkin
Feature: Locale resolution
  Applications choose a boot locale before parsing and a merged locale after
  configuration has been loaded.

  Scenario: The message category overrides the general language
    Given the environment variable LC_MESSAGES is ja_JP.UTF-8
    And the environment variable LANG is en_GB.UTF-8
    When the boot locale is resolved from the environment
    Then the resolved locale is ja-JP

  Scenario: The portable locale disables localization
    Given the environment variable LC_ALL is C.UTF-8
    And the environment variable LANG is ja_JP.UTF-8
    When the boot locale is resolved from the environment
    Then the resolved locale is en-US

  Scenario: An unusable variable does not hide a usable one
    Given the environment variable LC_ALL is 12345
    And the environment variable LANG is ja_JP.UTF-8
    When the boot locale is resolved from the environment
    Then the resolved locale is ja-JP

  Scenario: A configured locale applies only after the merge
    Given the environment variable LANG is en_GB.UTF-8
    And the configured locale is ja
    When the boot locale is resolved with configuration
    Then the resolved locale is en-GB
    When the merged locale is resolved without an override
    Then the resolved locale is ja

  Scenario: An explicit override beats the configured locale
    Given the environment variable LANG is en_GB.UTF-8
    And the configured locale is ja
    When the merged locale is resolved with override fr-CA
    Then the resolved locale is fr-CA

  Scenario: An invalid override is ignored
    Given the environment variable LANG is en_GB.UTF-8
    And the configured locale is ja
    When the merged locale is resolved with override not_a_locale!
    Then the resolved locale is ja
```

## Plan of work

Each milestone follows Red-Green-Refactor: write the named tests, confirm that
they fail for the stated reason, implement, and refactor. Because a missing
Rust type fails by not compiling, first add the type with a body returning
`und`, so the red run is a failed assertion rather than a build error.

### `EP-M1` Trait, fixed, and configured resolvers

Create `ortho_config/src/localizer/locale_resolver/mod.rs` with the trait, the
private `explicit_locale` helper that performs D6, and the module documentation;
`fixed.rs`; and `configured.rs`. Declare the module in `localizer/mod.rs` and
extend the `pub use localizer::{…}` list in `lib.rs`. Add
`ortho_config/tests/locale_resolver.rs` for INV-explicit, INV-config, and
INV-boot, the override half of `locale_resolver_telemetry.rs`, and the trybuild
fixture, registered in `localization_trybuild.rs`. Use `googletest` matchers
for classification and `pretty_assertions` for equality. Red: the configured
cases receive `und`. Green: implement. Every public item has a runnable Rustdoc
example.

### `EP-M2` `EnvLocaleResolver`

Add `posix.rs` with
`interpret_posix_locale(&str) -> Option<LanguageIdentifier>` and
`environment.rs` with the resolver, a private reading enum, and the event
helper. Add `posix_tests.rs`, `locale_resolver_precedence.rs`, and the variable
half of `locale_resolver_telemetry.rs`. Red: 512-state enumeration fails on its
first usable state. Non-Unicode cases use `OsStrExt` on Unix and `OsStringExt`
with an unpaired surrogate on Windows, so the class exists on both. Green:
implement. Record the negative controls.

### `EP-M3` Behaviour and end-to-end route

Add the feature, `behaviour/steps/locale_resolver_steps.rs` with its own
`LocaleResolverContext` (following `cargo_steps.rs`), and the registrations.
Then change `examples/hello_world/src/localizer.rs`: `detect_locale` becomes
`EnvLocaleResolver::from_process_env().boot_locale()`; delete `LOCALE_ENV_VARS`,
`parse_locale_from_env`, `parse_posix_locale`, and their table test. First
extend `localised_help.rs` with red cases that run the binary: `C.UTF-8` and
`POSIX.UTF-8` in `LC_ALL` against Japanese `LANG` give English catalogue copy,
and `LANG=ja_JP@x` gives Japanese. The file has 27 lines of headroom, so extend
the existing precedence table. All existing snapshots must be unchanged; no new
`insta` snapshot is warranted because no output format is added.

### `EP-M4` Documentation and closure

Write the locale-resolution ADR and index it in `docs/contents.md`. Amend
design §5.1 (drop "exactly", state D4 to D9, reference the ADR) and note in
§5.2 that `ConfigLocaleResolver` takes a typed override. Add "Choose the
locale" under the localization section of `docs/users-guide.md`, stating that
environment detection is opt-in, that daemons, embedded interfaces, and Windows
services write their own resolver, that `@modifier` is dropped, and how to
validate an override with Clap; register `guide-locale-resolver` in
`STANDARD_RUST_EXAMPLES`. Update `docs/developers-guide.md` ("CLI localization
surface" and "Environment access boundary", with the reuse rule: new locale
sources implement `LocaleResolver`; the POSIX interpreter is private and has
one caller), `docs/design.md` §4.12, `examples/hello_world/README.md`, and
`CHANGELOG.md` under `Unreleased`. Add an optional-adoption entry to the next
release's migration guide only if that guide exists by then. Search for stale
claims with `grep -rn 'parse_posix_locale\|LocaleResolver' docs examples`,
ignoring completed ExecPlans. Mark roadmap 11.3.1 and its three sub-items done.

## Milestones and plateaus

- `EP-M1`. End state: a usable trait with two resolvers that read nothing from
  the process. Discharges REQ-trait, REQ-fixed, REQ-config. Recovery: revert
  the commit. Remaining: REQ-env.
- `EP-M2`. End state: all three resolvers work against an injected environment.
  Discharges REQ-env and GAP-portable in code. Recovery: revert the commit.
  Remaining: example and documentation.
- `EP-M3`. End state: one POSIX interpreter in the workspace, exercised through
  a real process. Recovery: revert; the example returns to its own parser.
- `EP-M4`. End state: documentation matches code. Discharges REQ-opt-in and
  GAP-scope.

At each boundary confirm: assigned requirements are met; the trait still
matches §5.1; no public item beyond the list below exists; no dependency, trust
boundary, or persisted format changed; and trace links are current.
Compatibility decision for every milestone: none. The API is new and pre-1.0.

## Concrete steps

Run from the repository root. Focused commands:

```sh
cargo test -p ortho_config --lib locale_resolver
cargo test -p ortho_config --test locale_resolver --test locale_resolver_precedence
cargo test -p ortho_config --test locale_resolver_telemetry --test localization_trybuild
cargo test -p ortho_config --test rstest_bdd locale
cargo test -p hello_world --test localised_help --test localised_help_bdd
cargo test -p ortho_config --doc locale_resolver
cargo test -p ortho_config --test documentation_examples_rust_tests
```

After each milestone, run these in order through `scrutineer`, with output sent
to `/tmp/$ACTION-ortho-config-$(git branch --show-current).out`:

```sh
make check-fmt
make lint
make test
```

For changed Markdown, also run `make markdownlint` and `make nixie`. Commit
after each milestone passes.

## Validation and acceptance

- Each focused command passes, and each was seen to fail first for its stated
  reason.
- `LC_ALL=C.UTF-8 LANG=ja_JP.UTF-8 cargo run -p hello_world -- --help` prints
  the English catalogue copy ("layered greetings"). Before this change it
  prints Japanese.
- `LANG=ja_JP.UTF-8 cargo run -p hello_world -- --help` prints Japanese, as
  before.
- `grep -rn 'std::env' ortho_config/src/localizer` prints nothing.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` succeed. No Verus command applies (D12).

## Idempotence and recovery

Every step is an ordinary source edit and can be repeated. Restore a seeded
fault with a reverse patch, not `git checkout --`, which discards other
unstaged work. If a gate log stops without a diagnostic, treat the run as
infrastructure failure and read the log before re-running.

## Artefacts and notes

Record red-run and negative-control transcripts here during implementation.

## Interfaces and dependencies

The complete new public surface of `ortho_config`:

```rust
pub trait LocaleResolver: Send + Sync {
    fn boot_locale(&self) -> LanguageIdentifier;
    fn merged_locale(&self, explicit: Option<&str>) -> LanguageIdentifier { /* D6 */ }
}

#[derive(Debug, Clone)]
pub struct EnvLocaleResolver { /* private */ }
impl EnvLocaleResolver {
    pub fn new(env: SharedEnvSource) -> Self;
    pub fn from_process_env() -> Self;
    pub fn with_fallback(self, fallback: LanguageIdentifier) -> Self;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedLocaleResolver { /* private */ }
impl FixedLocaleResolver {
    pub const fn new(locale: LanguageIdentifier) -> Self;
}

#[derive(Clone)]
pub struct ConfigLocaleResolver { /* private */ }
impl ConfigLocaleResolver {
    pub fn new(primary: Arc<dyn LocaleResolver>, configured: Option<LanguageIdentifier>) -> Self;
}
```

Events, all without input text: `locale.variable_ignored` at `debug`, with
`variable` (`LC_ALL`, `LC_MESSAGES`, `LANG`) and `reason` (`non_unicode`,
`unrecognized`); and `locale.override_rejected` at `warn`.

Dependencies: none added.

## Revision note

2026-11-24: initial draft from four reconnaissance surveys and prior-art
research. Remaining work: design review, approval, then `EP-M1` to `EP-M4`.
