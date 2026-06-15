# CLI localization surface design

Status: proposed.

This document promotes the localization helpers currently shipped in the
`hello_world` example to first-class OrthoConfig crate surface. It also widens
the existing clap-error translation coverage, names a locale-resolution
lifecycle, and bridges OrthoConfig with `i18n-embed` so applications need not
maintain two Fluent bundles. The detailed product rationale is informed by the
review summarized in
[feedback-from-hello-world-example.md](feedback-from-hello-world-example.md)
and by the consumer evidence in §3 below.

The companion roadmap entries live in [roadmap.md](roadmap.md) phase 11.
Related background:

- [Architecting Localizable Rust Libraries with Fluent](localizable-rust-libraries-with-fluent.md);
- [Design Document: The `OrthoConfig` Crate](design.md);
- [Agent-native CLI assistance design](agent-native-cli-design.md);
- [Improved error message design](improved-error-message-design.md).

## 1. Purpose and scope

OrthoConfig already ships a `Localizer` trait, a `FluentLocalizer` backed by
embedded en-US resources, and `localize_clap_error` / `clap_error_formatter`
helpers that surface translated parse errors. Two downstream consumers (Weaver
and Netsuke) rely on those types as load-bearing public surface. Netsuke
already extends `Localizer` with a layered wrapper. Spycatcher-harness, by
contrast, bypasses the crate's helpers and consumes `i18n-embed` directly
because the supplied surface does not yet cover library-message use cases.
Podbot has not adopted any localization surface yet.

The same review identified a second class of friction. The pieces an
application reaches for first — the `LocalizeCmd` extension trait on
`clap::Command`, the `try_parse_localized*` helpers, locale detection from
environment variables, and the warn-then-NoOp fallback — live in the example
crate. Every adopter copies and adapts them. The same pattern is being
reinvented in Weaver and Netsuke today and will be reinvented by every future
consumer.

This design promotes those load-bearing pieces to crate API, widens the
`clap-error-*` translation matrix so coverage tracks clap stable releases,
stops the error pipeline from discarding clap's coloured suggestion text,
defines a locale-resolution lifecycle that survives the chicken-and-egg between
locale flags and parse errors, bridges OrthoConfig with `i18n-embed`'s
`FluentLanguageLoader`, and extends the derive so localization identifiers are
generated rather than hand-authored.

### 1.1 Out of scope

The crate must remain a layered-configuration plus localization-adapter crate.
The following are explicitly out of scope:

- a translation editor or Translation Management System (TMS) integration;
- a runtime locale swapper that mutates a process-wide Fluent bundle in place;
- making environment-variable locale detection the only path (it must remain
  an opt-in policy, not a default that ships with the crate);
- replacing `i18n-embed` rather than bridging with it.

The crate continues to ship a `LocaleResolver` implementation for environment
detection, and applications may opt into it. Daemons, embedded interfaces, and
Windows services need different policies; they are entitled to write their own
resolver.

### 1.2 Snapshot-per-parse contract

The `Localizer` an application holds is a **snapshot**, not a live binding.
Translations resolved during a parse correspond to the locale at the moment
`try_parse_localized*` returned. Long-lived services that swap locales at
runtime must hold the localizer behind their own swap primitive
(`arc_swap::ArcSwap<dyn Localizer>` is the documented baseline) and re-read on
every request boundary; the crate does not own that swap because the correct
rebuild policy is service-shaped, not library-shaped.

For command-line interface (CLI) processes, which is the dominant case in §3,
snapshot semantics match the lifecycle exactly: one parse, one merge, one
rebuild, then the localizer is stable for the rest of the process.

### 1.3 Adopter quick-start

The minimum viable adoption uses five public concepts:

1. `EnvLocaleResolver` — detect the user's locale from the environment.
2. `BootLocalizer::new` and `BootHandle::finalize` — build the localizer and
   record the merged locale (§5.2).
3. `LocalizeCmd::localize` — translate the `clap::Command` tree (§4).
4. `LocalizedParse::try_parse_localized_from` — parse with localized errors
   (§4.2).
5. The `#[derive(OrthoConfig)]` derive, which generates the identifier
   constants for free (§8).

Everything else in this document is either an extension point (§5.1 custom
resolvers, §7 `i18n-embed` bridge, §9 translator diagnostics) or a backwards
compatibility shim (§6.4 eager localization as the supported path, with a
formatter-swap escape hatch). New adopters should not need to reach for them on
day one.

## 2. Design principles

1. **The load-bearing pieces live in the crate, not the example.** If two of
   four consumers already reach for a helper, the helper is API, not
   illustration.
2. **Preserve clap's rich diagnostics.** Localization must not strip the
   coloured usage tail, suggested alternatives, or context that clap renders.
   Replace messages in place; do not rebuild errors with `Error::raw`.
3. **Make coverage observable.** When a translation is missing or a clap
   `ErrorKind` is not in the matrix, the gap should be visible in tracing
   output and in tests, not silently masked by the en-US fallback.
4. **Composable lifecycles, not monolithic builders.** The crate ships
   primitives (`LocaleResolver`, `BootLocalizer`, `FluentEmbedLocalizer`) that
   applications combine. It does not ship a single God-builder that hides the
   two-phase parse pattern.
5. **One Fluent bundle per application.** When an application already owns a
   `FluentLanguageLoader` for its library messages, OrthoConfig delegates to it
   rather than parsing the same Fluent Translation List (FTL) twice.
6. **Scope discipline.** Stateful locale-switching APIs, hot reload, and
   bundle merging across processes are different problems and stay out of this
   crate.

## 3. Evidence base

Promotion is driven by concrete repository evidence, not anticipation.

- `weaver` (`crates/weaver-cli/src/localizer.rs`): consumes
  `ortho_config::{FluentLocalizer, Localizer, NoOpLocalizer}`. Embeds its own
  `locales/en-US/messages.ftl`. References `localize_clap_error` from its
  design and execution-plan documents.
- `netsuke` (`src/cli_localization.rs`): consumes `FluentLocalizer`,
  `FluentLocalizerBuilder`, `Localizer`, `NoOpLocalizer`, and
  `LanguageIdentifier`. Ships `locales/en-US/` and `locales/es-ES/`. Implements
  a `LayeredLocalizer` wrapper that provides primary-then-fallback chaining,
  evidence that the trait is being extended in the field.
- `spycatcher-harness` (`src/i18n.rs`): depends on `i18n-embed` directly with
  the `fluent-system` feature, embeds via `RustEmbed`, and writes its own
  `localize_harness_error` free function. The module's own doc comment notes
  that OrthoConfig's surface does not cover library messages well enough for
  it. This is the case for the `FluentEmbedLocalizer` bridge in §6.
- `podbot`: depends on `ortho_config` and `clap` but has no `locales/`
  directory yet. A future consumer of the promoted helpers.

In none of these four projects does a `LocalizeCmd` extension trait on
`clap::Command` exist; only the `hello_world` example provides one. The example
is the reference implementation that every consumer either copies inline or
omits.

## 4. `LocalizeCmd`: a promoted extension trait

The example trait localizes `about`, `long_about`, and `override_usage` at the
root and recurses through `Command::get_subcommands_mut`. The crate version
generalizes that to the full clap surface:

```rust
pub trait LocalizeCmd: Sized {
    /// Apply the localizer to this command and every subcommand in its tree.
    #[must_use]
    fn localize(self, localizer: &dyn Localizer) -> Self;

    /// Apply the localizer without recursing into subcommands. Intended for
    /// in-place rewrites after subcommand selection.
    #[must_use]
    fn localize_self(self, localizer: &dyn Localizer) -> Self;
}

impl LocalizeCmd for clap::Command { /* ... */ }
```

The recursion covers, for every command in the tree:

- `about` and `long_about`;
- `override_usage`;
- `version` and `long_version` (when the application opts in by registering
  a `<command>.version` identifier in its catalogue — otherwise the stock clap
  value is preserved);
- the help template footer (replacing the ad-hoc `after_long_help` pattern
  that downstream code uses today).

For each argument visited via `Command::get_arguments_mut`:

- `help` and `long_help`, keyed `<command-path>.args.<arg-id>.help` and
  `<command-path>.args.<arg-id>.long_help`;
- `value_name`, keyed `<command-path>.args.<arg-id>.value_name`;
- per-value documentation for `clap::builder::PossibleValue` is exposed via
  the IR mechanism only; clap does not currently allow per-value help to be
  mutated post-hoc and the design defers this to a future iteration when clap
  supports it.

The 11.1.1 implementation does not localize clap's built-in `--help` and
`--version` argument text, per-`PossibleValue` help, or the `help_template`
layout string. Applications that need localized built-in flags can disable
clap's built-ins and provide explicit arguments in a later iteration. The
footer slots `after_help` and `after_long_help` are localized because clap
exposes setters for them on `Command`.

### 4.1 Identifier convention

The identifier shape is a documented pure function of the command path:

```text
<base>             ::= <root> "." <segment>...
<segment>          ::= command name as an ASCII Fluent identifier segment
<command id>       ::= <base> "." ( "about" | "long_about" | "usage" | "version" )
<argument id>      ::= <base> ".args." <arg id> "." ( "help" | "long_help" | "value_name" )
```

`<root>` is configurable via `LocalizeCmd::with_base("…")` (defaulting to the
binary name normalized to a Fluent identifier) so applications can share one
catalogue across multiple binaries by giving each a distinct prefix. The
canonical function is exposed as
`ortho_config::message_id_for(&command_path, suffix)` so applications, tests,
and the IR pipeline can produce identical identifiers without re-implementing
the convention (see [ADR-006](adr-006-identifier-derivation-panics.md)).

Identifier normalization is a documented function: lowercase American Standard
Code for Information Interchange (ASCII) letters pass through, ASCII digits,
hyphens, and underscores pass through unchanged, and any other character is
rejected. Underscores are preserved because Fluent identifiers allow
`[a-zA-Z0-9_-]` after the leading letter. Author-facing Fluent Translation List
(FTL) keys use dotted ids such as `hello_world.cli.about`; OrthoConfig's
load-time normalizer rewrites that to the runtime id `hello_world-cli-about`.
The canonical function `message_id_for(["hello_world", "cli"], "about")`
produces the same runtime id directly.

Two segments that normalize to the same identifier are a build-time error in
the macros crate and a runtime panic in `LocalizeCmd::localize` for hand-built
command trees (see [ADR-006](adr-006-identifier-derivation-panics.md)). The
derive's identifier-generation pass therefore enforces uniqueness at compile
time, and ADR-006 records why the promoted runtime API panics rather than
returning `Result` when a hand-built command tree cannot produce unique Fluent
identifiers.

### 4.2 `try_parse_localized` helpers

```rust
pub trait LocalizedParse: clap::Parser {
    /// Equivalent to `Parser::try_parse`, but with localized error and metadata.
    fn try_parse_localized(localizer: &dyn Localizer) -> Result<Self, clap::Error> {
        Self::try_parse_localized_from(std::env::args_os(), localizer)
    }

    /// Equivalent to `Parser::try_parse_from`, but with localization.
    fn try_parse_localized_from<I, T>(iter: I, localizer: &dyn Localizer)
        -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone
    {
        Self::try_parse_localized_with_matches(iter, localizer)
            .map(|(value, _)| value)
    }

    /// Returns both the parsed struct and the raw `ArgMatches`.
    fn try_parse_localized_with_matches<I, T>(iter: I, localizer: &dyn Localizer)
        -> Result<(Self, clap::ArgMatches), clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone
    {
        parse_localized_command(Self::command().localize(localizer), iter, localizer)
    }
}

impl<P: clap::Parser> LocalizedParse for P {}
```

The blanket implementation removes the "copy this from the example" step. The
`*_with_matches` variant survives because OrthoConfig's
`load_and_merge_with_matches` requires the raw `ArgMatches`, which
`Parser::try_parse` discards.

The trait is the zero-configuration path for catalogues keyed by a command's
`bin_name`. Applications that need a custom root use the base-agnostic
primitive directly:

```rust
pub fn parse_localized_command<P, I, T>(
    command: clap::Command,
    iter: I,
    localizer: &dyn Localizer,
) -> Result<(P, clap::ArgMatches), clap::Error>
where
    P: clap::FromArgMatches,
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone;
```

Callers pass an already-localised command, such as
`Cli::command().with_base("acme.tool").localize(localizer)`. The unsuffixed
`try_parse_localized` method is the environment-reading variant; the older
example-only `_env` suffix is not part of the promoted API.

## 5. Locale-resolution lifecycle

There is a chicken-and-egg problem in every CLI: the requested locale is itself
a CLI flag, but parse errors must be localized before that flag is read. Today,
every consumer reinvents the same two-phase dance. The crate ships the
primitives.

### 5.1 `LocaleResolver` trait

```rust
pub trait LocaleResolver: Send + Sync {
    /// Returns the locale to use before any CLI flags are parsed.
    fn boot_locale(&self) -> LanguageIdentifier;

    /// Returns the locale to use after configuration has been merged, given
    /// any explicit override supplied via flag or file. Implementations may
    /// return the boot locale unchanged if no override applies.
    fn merged_locale(&self, explicit: Option<&str>) -> LanguageIdentifier {
        explicit
            .and_then(|raw| raw.parse().ok())
            .unwrap_or_else(|| self.boot_locale())
    }
}
```

The crate ships three implementations:

- `EnvLocaleResolver` — checks `LC_ALL`, `LC_MESSAGES`, and `LANG` in that
  order, normalizing POSIX values like `ja_JP.UTF-8` into Best Common Practices
  47 (BCP 47) tags. Special-cases `C` and `POSIX` as `en-US`. Mirrors the
  example's `parse_posix_locale` rules exactly.
- `FixedLocaleResolver` — wraps a single `LanguageIdentifier` for tests and
  for non-locale-aware deployments.
- `ConfigLocaleResolver` — composes a primary resolver with an override read
  from configuration after the merge phase. This is the resolver applications
  most commonly want once a `--locale` flag exists.

### 5.2 `BootLocalizer` factory and `BootHandle` typestate

The factory is split from the handle so non-invocation of the merge-phase step
is detectable rather than silent. `BootLocalizer::build` returns a
**typestate** handle — `BootHandle<Boot>` — that callers must transition to
`BootHandle<Final>` via `finalize` before extracting a usable
`Arc<dyn Localizer>`. Dropping a `BootHandle<Boot>` without finalizing emits a
`warn`-level tracing event so operators see the missed lifecycle step.

```rust
pub struct BootLocalizer { /* ... */ }

impl BootLocalizer {
    pub fn new(resolver: Arc<dyn LocaleResolver>) -> Self;

    pub fn with_resources<F>(self, f: F) -> Self
    where
        F: Fn(&LanguageIdentifier) -> Vec<&'static str> + Send + Sync + 'static;

    pub fn with_telemetry_sentinel(self, name: &'static str) -> Self;

    pub fn with_error_reporter(self, reporter: FormattingIssueReporter) -> Self;

    /// Builds the boot-phase handle. The returned handle owns the
    /// `Arc<dyn Localizer>` to use during parsing and exposes it via
    /// `boot_localizer()`. Callers must transition the handle through
    /// `finalize(...)` once the merged locale is known.
    pub fn build(self) -> BootHandle<Boot>;
}

pub struct BootHandle<S> { /* ... */ }

/// Available on both `BootHandle<Boot>` and `BootHandle<Final>` so callers
/// can query degraded-mode status before they parse and after they merge.
impl<S> BootHandle<S> {
    /// Returns `true` when the underlying Fluent build failed and the
    /// handle is operating on a `NoOpLocalizer` substitute.
    pub fn build_failed(&self) -> bool;
}

impl BootHandle<Boot> {
    /// The localizer snapshot to use during the parse phase.
    pub fn boot_localizer(&self) -> &Arc<dyn Localizer>;

    /// Records the merged locale (`None` when the merge did not change
    /// anything) and transitions to the finalized state. The returned
    /// handle exposes the live `Arc<dyn Localizer>` for downstream use.
    pub fn finalize(self, merged: Option<&str>) -> BootHandle<Final>;

    /// Optionally supply a fresh resolver for the merge phase. Daemons that
    /// prefer configuration-file precedence over environment detection can
    /// hand in a `ConfigLocaleResolver` here without rebuilding the
    /// factory. Resolves [Open question 13.1].
    pub fn finalize_with(
        self,
        merged: Option<&str>,
        resolver: Arc<dyn LocaleResolver>,
    ) -> BootHandle<Final>;
}

impl BootHandle<Final> {
    pub fn localizer(&self) -> &Arc<dyn Localizer>;
    pub fn locale(&self) -> &LanguageIdentifier;
}

impl Drop for BootHandle<Boot> {
    fn drop(&mut self) { /* warn-level tracing event */ }
}
```

`BootLocalizer::build` substitutes `Arc::new(NoOpLocalizer::new())` on failure
and emits a `warn`-level tracing event with the configured sentinel as a
`target`. The handle records the failure in shared state owned by the
typestate, so `build_failed()` is meaningful on both `BootHandle<Boot>` (where
callers may want to surface a "translations unavailable; parse errors will be
English" banner before parsing) and `BootHandle<Final>` (where the same state
drives a sustained degraded-mode banner for the rest of the process).
`finalize` re-emits the failure event on every retry while the failure
persists, with exponential backoff to bound log volume.

### 5.3 Documented two-phase pattern

The intended lifecycle is:

1. Construct an `EnvLocaleResolver` (or the application's chosen resolver).
2. Build a `BootLocalizer` from the resolver-detected locale and call
   `.build()` to obtain a `BootHandle<Boot>`.
3. Call `Cli::try_parse_localized_from(args, handle.boot_localizer())`. Any
   parse errors render in the boot locale, which is the locale the user's
   environment requested.
4. Merge configuration via `OrthoConfig::load_and_merge_with_matches`.
5. Call `handle.finalize(merged_locale)`. The transition is enforced by the
   typestate: the application cannot retrieve a long-lived `Arc<dyn Localizer>`
   for library messages without performing this step. Dropping the
   `BootHandle<Boot>` without finalizing emits a `warn`-level tracing event, so
   non-invocation is observable in production logs rather than silent.

Step 5 is the point applications get wrong today, and the typestate is the
mechanism that makes the invariant enforceable rather than ceremonial. Parse
errors already shown stay in the boot locale; this is the correct behaviour
because they describe what the user actually typed.

## 6. Widened clap-error coverage

The crate ships translations for four clap `ErrorKind` variants today
(`MissingRequiredArgument`, `UnknownArgument`, `InvalidValue`,
`MissingSubcommand`). clap's `ErrorKind` is `#[non_exhaustive]` and stable clap
4.x exposes many more parse-raisable variants. The current behaviour is silent
fall-through, which degrades quietly as clap adds variants.

### 6.1 Complete en-US matrix

The crate ships en-US translations for every raisable variant in stable clap
4.x: `InvalidValue`, `UnknownArgument`, `InvalidSubcommand`, `NoEquals`,
`ValueValidation`, `TooManyValues`, `TooFewValues`, `WrongNumberOfValues`,
`ArgumentConflict`, `MissingRequiredArgument`, `MissingSubcommand`,
`InvalidUtf8`, `Io`, and `Format`. The three display-only variants
(`DisplayHelp`, `DisplayHelpOnMissingArgumentOrSubcommand`, `DisplayVersion`)
are recorded but **not** translated — clap renders the help or version text
itself, and the matrix marks them with a `DisplayOnly` sentinel so the
mechanical gate (§6.1.1) can prove exhaustive coverage of `ErrorKind` without
tying the assertion to a hand-maintained subset count.

The matrix is **exhaustive** over `ErrorKind`. Every declared variant appears
exactly once, either with a translated Fluent identifier or with the
`DisplayOnly` sentinel. In clap 4.5.60 (the version currently locked in
`Cargo.lock`) that is 14 translated entries plus 3 sentinel entries, totalling

1. The matrix shape lets the gate reduce to a single
`len() == ORTHO_CONFIG_CLAP_ERROR_KIND_COUNT` comparison without filtering or
subtraction, which is the property the mechanical gate requires.

#### 6.1.1 Mechanical coverage gate

`clap::error::ErrorKind` is `#[non_exhaustive]`; the crate must therefore
detect coverage gaps automatically rather than relying on a release-time audit.
The gate is a single mechanism, described once here and referenced from the
roadmap:

1. A build script (`build.rs` in the `ortho_config` crate) inspects the
   `clap::error::ErrorKind` enum exposed by the resolved clap dependency and
   emits `cargo:rustc-env=ORTHO_CONFIG_CLAP_ERROR_KIND_COUNT=<n>`, where `<n>`
   is the count of declared variants (all of them — the build script does
   **not** classify variants as raisable or display-only). Cargo's resolver
   picks the clap version from the workspace's `Cargo.lock`, so the build
   script sees the same clap minor or patch the test will run against.
2. A const-evaluated test reads that constant and compares it to the
   length of the exhaustive `CLAP_ERROR_IDS`:

   ```rust
   // In ortho_config's test suite.
   const CLAP_ERROR_KIND_COUNT: usize =
       env!("ORTHO_CONFIG_CLAP_ERROR_KIND_COUNT").parse().unwrap();
   const_assert_eq!(CLAP_ERROR_IDS.len(), CLAP_ERROR_KIND_COUNT);
   ```

When clap adds a variant in a minor or patch release, the lengths diverge and
CI fails before publish. The fix is mechanical: add a new `CLAP_ERROR_IDS`
entry that either points at a new Fluent identifier (for a raisable variant) or
marks the variant `DisplayOnly` (for a new display shape). This replaces a
maintenance ritual with a mechanical contract. Roadmap §11.2.1 references this
section rather than restating the mechanism.

### 6.2 Canonical mapping constant

The matrix is an exhaustive mapping from `ErrorKind` to a small enum that
distinguishes translated entries from display-only ones:

```rust
/// Outcome the crate has registered for a clap `ErrorKind`.
#[non_exhaustive]
pub enum ClapErrorTranslation {
    /// The crate ships an en-US translation under this Fluent identifier
    /// and consumer overrides should target it.
    Translated(&'static str),
    /// clap renders the surface itself (help text, version text). The
    /// localization pipeline must not substitute a translated body.
    DisplayOnly,
}

pub const CLAP_ERROR_IDS: &[(clap::error::ErrorKind, ClapErrorTranslation)] = &[
    // 14 translated entries: InvalidValue → "clap-error-invalid-value", ...
    // 3 display-only entries:
    //   (ErrorKind::DisplayHelp, ClapErrorTranslation::DisplayOnly),
    //   (ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
    //       ClapErrorTranslation::DisplayOnly),
    //   (ErrorKind::DisplayVersion, ClapErrorTranslation::DisplayOnly),
];
```

The constant lets consumers:

- iterate the matrix and produce coverage tests against their own
  catalogues, filtering with `matches!(translation, Translated(_))` to ignore
  display-only variants;
- validate that overrides cover every shipped translated identifier before
  release;
- generate translator briefs that list every translated identifier and its
  English source string.

A `ClapErrorCoverage` builder consumes the constant, filters to `Translated`
entries, and returns a report listing identifiers the consumer's `Localizer`
fails to resolve.

### 6.3 Observable fallback

The `localize_clap_error_with_command` path currently swallows missing
identifiers silently. The new path emits tracing events at severities matched
to channel: `warn` when the missing identifier originates from a clap error
(because the user is staring at an untranslated parse error and operators want
to alert on it), and `debug` for application-message origins (where the en-US
fallback is usually acceptable). Events carry the identifier, the `ErrorKind`,
and the locale, and are also surfaced through a `MissingTranslationReporter`
hook (§9). The default reporter is a no-op so production binaries pay no cost
when telemetry is off.

### 6.4 Preserve clap's rich context

`localize_clap_error_with_command` currently builds a `clap::error::Error` via
`Error::raw`, discarding suggestions, the usage tail, and styling. The
supported path is **eager localization** inside the parse helpers, using clap's
stable mutation surface available under the `error-context` feature:

```rust
pub fn insert(&mut self, kind: ContextKind, value: ContextValue) -> Option<ContextValue>;
pub fn remove(&mut self, kind: ContextKind) -> Option<ContextValue>;
pub fn format(self, cmd: &mut Command) -> Error<F>;
```

The pipeline runs synchronously inside `try_parse_localized*`, before the error
escapes the helper's stack frame:

1. Take the original `clap::error::Error` produced by
   `try_get_matches_from_mut`.
2. Resolve the localized message body for the matched
   `(ErrorKind, ContextKind)` pair using `CLAP_ERROR_IDS` plus per-variant
   Fluent
   argument extraction.
3. Replace the `ContextKind::Custom` slot via `Error::insert`, so
   `RichFormatter` uses the supplied text in place of the kind-message portion
   while keeping clap's usage tail, suggestion list, and styling intact.
4. Call `Error::format(cmd)` to re-attach command context if the command
   tree was rebuilt during localization.

This is **the** supported path. It runs synchronously, requires no thread-local
state, and works identically in sync and async call sites: because localization
happens before the error is returned, the formatter that eventually renders the
error never needs to look up the localizer itself.

#### 6.4.1 Escape hatch: monomorphised formatter

For adopters who want clap to defer rendering (for example, a custom error
formatter that interleaves localization with structured logging), the crate
exposes a generic `LocalizedFormatter<L: Localizer + 'static>`:

```rust
pub struct LocalizedFormatter<L>(PhantomData<L>);

impl<L: Localizer + Default + 'static> clap::error::ErrorFormatter
    for LocalizedFormatter<L>
{
    fn format_error(error: &clap::error::Error<Self>) -> clap::builder::StyledStr {
        /* delegate to RichFormatter with L::default() supplying lookups */
    }
}
```

The formatter is monomorphised over a concrete `Localizer + Default` type,
sidestepping clap's non-dyn-compatible `ErrorFormatter` trait. Adopters who
need a runtime-chosen localizer can use a thin newtype that reads from a
process-wide `OnceLock<Arc<dyn Localizer>>`. The crate does **not** ship a
thread-local-backed dynamic formatter: eager localization in §6.4 covers the
dynamic case, and the thread-local pattern has known async footguns (empty
thread-local after a parse helper drops mid-render).

The escape hatch is an advanced opt-in. The default adoption path uses §6.4
eager localization; the typestate handle in §5.2 plus the parse helpers in §4.2
make this the easy path.

## 7. Bridge with `i18n-embed`

`FluentLocalizer` and `i18n_embed::fluent::FluentLanguageLoader` cannot share a
`FluentBundle` today, so applications that already own a loader for library
messages compile every FTL twice and negotiate locale twice. The
spycatcher-harness case study above shows the consequence: a consumer that
needed library messages routed around OrthoConfig's helpers entirely.

The crate ships an adapter that wraps an existing loader and presents the
`Localizer` interface:

```rust
pub struct FluentEmbedLocalizer {
    loader: Arc<i18n_embed::fluent::FluentLanguageLoader>,
}

impl FluentEmbedLocalizer {
    pub fn new(loader: Arc<i18n_embed::fluent::FluentLanguageLoader>) -> Self;
}

impl Localizer for FluentEmbedLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        if !self.loader.has(id) {
            return None;
        }
        let rendered = match args {
            Some(args) => self.loader.get_args_concrete(id, fluent_args_from(args)),
            None => self.loader.get(id),
        };
        Some(rendered)
    }
}
```

Presence is queried via `FluentLanguageLoader::has(message_id)`. As of
`i18n-embed` 0.16, `has` is the documented public API; it delegates to the
underlying Fluent bundle's `has_message` but is the only stable entry point the
adapter is allowed to call. The `loader.get(id) == id` heuristic is rejected
because it is unsound on three Fluent shapes: a message with only attributes
and no value, a message whose value transforms to the identifier string, and a
message with an empty-string value. `has` returns `true` for the first two
cases (correctly marking the identifier as present) and `false` for
nothing-defined, which matches the `Localizer::lookup -> Option<String>`
contract honestly.

If a future `i18n-embed` release renames `has`, the adapter's build script
asserts the method exists and fails fast with a clear migration pointer rather
than silently degrading; see §12.

Trade-offs the documentation must record:

- Applications that already own a `FluentLanguageLoader` get one bundle and
  one locale-negotiation pass. Library messages and CLI messages share one
  source of truth.
- Applications that do not own a loader keep using `FluentLocalizer`. The
  two implementations remain in parity: any feature added to one ships in the
  other within the same release. The crate does **not** add a constructor that
  builds a `FluentLanguageLoader` from `i18n_embed::I18nAssets`; that would
  make OrthoConfig a partial re-export of `i18n-embed` and obscures which crate
  owns the bundle lifecycle. Resolves [Open question 13.2].
- `FluentLanguageLoader::select_languages` returns a *new* loader. The
  adapter therefore re-wraps the new loader in a fresh `Arc` on locale switch;
  callers update their `Arc<dyn Localizer>` reference via the
  `BootHandle::finalize_with` path. For long-lived services that swap locale
  under concurrent traffic, the §1.2 snapshot-per-parse contract applies:
  services hold an `ArcSwap<dyn Localizer>` and re-read on each request
  boundary, and the adapter's per-swap cost is one fresh `Arc` allocation per
  locale change.

## 8. Derive support

A `#[derive(Parser)]` user should not have to hand-author Fluent identifiers
for every argument. The `OrthoConfig` derive is extended (a separate
`LocalizedParser` derive is rejected: the existing derive already owns the clap
surface, and a second derive would duplicate field walks).

### 8.1 `OrthoConfigLocalization` trait

Identifier constants live on a dedicated trait, **not** on `OrthoConfigDocs`.
Adopters who never opt into the documentation IR (the spycatcher-harness shape
in §3) need identifiers without dragging in the docs surface:

```rust
pub trait OrthoConfigLocalization {
    /// Identifier for the command's `about` text.
    const ABOUT_ID: &'static str;
    /// Identifier for `long_about`.
    const LONG_ABOUT_ID: &'static str;
    /// Identifier for the override usage string.
    const USAGE_ID: &'static str;
    /// Identifier triples for every argument, in declaration order. Each
    /// element is `(help_id, long_help_id, value_name_id)`.
    const ARG_IDS: &'static [(&'static str, &'static str, &'static str)];
}
```

`OrthoConfigDocs::ABOUT_ID` (and friends) is implemented via a blanket impl
that delegates to `OrthoConfigLocalization`, so the docs pipeline picks up the
same identifiers without taking ownership of them.

### 8.2 Derive behaviour

The extended derive:

- Generates the localization identifier for each argument from the command
  path and the field's `id` (or, when absent, the kebab-cased field name).
  Identifiers are exposed as `OrthoConfigLocalization` associated constants so
  application code can refer to them without string concatenation.
- Optionally embeds the doc-comment-derived default English text into the
  binary. The flag is **per-field**, not per-struct, because help strings
  dominate binary size while value names are tiny and the cost should be paid
  per field:

  ```rust
  #[derive(OrthoConfig)]
  struct Cli {
      /// Recipient of the greeting.
      #[ortho_config(localized_default = "help")] // embed help, skip value_name
      recipient: Option<String>,

      /// Quiet mode.
      #[ortho_config(localized_default = "all")] // embed everything for this field
      is_quiet: bool,
  }
  ```

  Permitted values are `none` (default), `help`, `long_help`, `value_name`,
  `help+long_help`, and `all`. A struct-level
  `#[ortho_config( localized_default = "...")]` sets the default that fields
  inherit unless they override it.

- Emits a build-time artefact at `${OUT_DIR}/ortho-config/cli-identifiers.json`
  listing every generated identifier, its source span, and its embedded
  default. The artefact is capped at 1 MiB; larger trees split across files
  named `cli-identifiers.<n>.json` with an index file. `cargo-orthohelp`
  consumes the artefact (see §11) so translators receive an authoritative
  identifier inventory without scraping Fluent Translation List (FTL) files by
  hand.

The convention matches §4.1 exactly so identifiers generated by the derive,
emitted by `message_id_for`, and referenced in hand-authored FTL agree
byte-for-byte. The derive enforces collision detection at compile time (§4.1):
two fields whose identifiers normalize to the same value produce a
`compile_error!`. A regression test in the macros crate compares derive output
against `message_id_for` for a fixture command tree to lock the agreement.

## 9. Translator diagnostics

The current `FormattingIssueReporter` fires only when a Fluent formatter
encounters an error. The interesting question for translators is which
identifiers were requested but not present, especially under fallback. The
crate adds a `MissingTranslationReporter`:

```rust
pub trait MissingTranslationReporter: Send + Sync {
    fn missing(&self, event: MissingTranslationEvent<'_>);
}

pub struct MissingTranslationEvent<'a> {
    pub id: &'a str,
    pub locale: &'a LanguageIdentifier,
    pub fallback_used: Option<&'a LanguageIdentifier>,
    pub origin: TranslationOrigin,
}

pub enum TranslationOrigin {
    ClapError { kind: clap::error::ErrorKind },
    CommandMetadata { suffix: &'static str },
    Application,
}
```

Wired into `FluentLocalizer`, `FluentEmbedLocalizer`, and the clap-error path.
The default reporter is a no-op so production builds pay nothing.
`cargo-orthohelp` ships a built-in reporter that aggregates events into a
JavaScript Object Notation (JSON) report suitable for translator workflows,
written under `target/orthohelp/missing-translations/<locale>.json`.

## 10. Compatibility

The promoted helpers ship in OrthoConfig 0.9. The example crate's
`localization.rs` and `localizer.rs` modules collapse into thin wrappers that
re-export the crate types for one release, then are removed in 0.10.

The clap-error widening is additive: any consumer who has overridden one of the
existing four identifiers continues to work, and the new identifiers return the
embedded en-US text unless the consumer provides a translation. The supported
error-localization path is the eager pipeline in §6.4; the
`localize_clap_error_with_command` function remains in 0.9 as a deprecated shim
that delegates to the eager path with a deprecation warning, and is removed in
0.10.

The derive change is opt-in via `localized_default` on fields (or as a
struct-level default). Consumers that do not set the attribute see no
behavioural change.

`FluentEmbedLocalizer` adds an optional dependency on `i18n-embed` behind a new
`i18n-embed-bridge` cargo feature so the existing dependency footprint is
unchanged for consumers that do not need the bridge.

## 11. Interaction with `cargo-orthohelp`

The build-time identifier artefact (§8) and the missing-translation report (§9)
feed `cargo-orthohelp`. The reference CLI gains two subcommands:

- `cargo orthohelp i18n list-ids` — emits the identifier inventory in
  human-readable, JSON, and Fluent stub formats. The Fluent stub is a catalogue
  scaffold containing every identifier with empty values, ready for translators.
- `cargo orthohelp i18n coverage --locale <tag>` — walks the consumer's
  `Localizer` (built via `FluentLocalizerBuilder` or `FluentEmbedLocalizer`)
  and reports identifiers the locale fails to resolve. Non-zero exit when
  coverage is below a configurable threshold, for use in continuous integration
  (CI).

Both commands honour the agent-context output contracts already defined in
[agent-native-cli-design.md](agent-native-cli-design.md) §6.2.

## 12. Failure modes and risks

- **clap patch releases adding `ErrorKind` variants.** `ErrorKind` is
  `#[non_exhaustive]` and clap may add variants in a patch release. The §6.1
  const assertion catches this in CI before publish; the §6.3 `warn`-level
  tracing event catches it in production. The matrix is therefore mechanically
  gated rather than ceremonially audited.
- **`BootHandle` non-finalization.** A consumer who drops a
  `BootHandle<Boot>` without calling `finalize` loses the merge-phase locale.
  The typestate prevents extraction of a long-lived `Arc<dyn Localizer>` for
  library messages, and the `Drop` impl emits a `warn`-level event so the
  missed step is observable.
- **i18n-embed bundle ownership and concurrent locale swap.** Locale
  switching returns a new loader rather than mutating in place. Daemons must
  hold an `ArcSwap<dyn Localizer>` (or equivalent) and re-read on each request
  boundary. The §1.2 snapshot-per-parse contract names this responsibility
  explicitly so it is not surprise behaviour.
- **Derive scope creep.** It is tempting to grow the derive into a general
  IR for help text. The scope is held to identifier generation, optional
  default embedding, and artefact emission. Anything beyond that belongs in the
  documentation IR pipeline.
- **Fluent presence-method drift.** The adapter calls
  `FluentLanguageLoader::has` (the public API in `i18n-embed` 0.16). If a
  future release renames or removes the method, a build script in the
  `i18n-embed-bridge` feature asserts the symbol resolves at compile time and
  fails with a migration pointer rather than silently degrading.
- **`LocalizedFormatter` escape hatch (§6.4.1) used incorrectly.** The
  monomorphised formatter is opt-in for advanced cases. Misusing it with a
  process-wide `OnceLock<Arc<dyn Localizer>>` that is never initialised falls
  back to en-US silently. The rustdoc for the formatter must call out this case
  and recommend the eager path for almost every adopter.

## 13. Open questions

1. Should the en-US clap-error matrix be split per cargo feature so
   minimal-dependency builds can omit the strings? The current design ships
   them unconditionally because the binary cost is small and skipping them
   silently reproduces the bug §6 is designed to fix. Revisit if profile data
   shows the strings are material to small embedded binaries.
2. Should `BootHandle::finalize_with` accept a *list* of resolvers so
   composite policies (environment + file + flag) can be evaluated in a single
   call? The current shape pushes composition onto the resolver implementation
   (`ConfigLocaleResolver` already composes one chain). Revisit if downstream
   applications start writing their own chain adapters.

Open questions 13.1 (resolver on rebuild) and 13.2 (loader-from-assets
constructor) from the prior draft are resolved in §5.2 and §7 respectively.
