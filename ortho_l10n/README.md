# ortho_l10n

Reusable Fluent catalogue lookup, formatting reports, and message identifiers.
Applications can use this crate directly without configuration discovery,
merging, configuration derives, Figment, or Clap.

```toml
[dependencies]
ortho_l10n = "0.11.0"
```

## Load and format a catalogue

```rust
use ortho_l10n::{FluentLocalizer, FluentValue, LocalizationArgs, Localizer, langid};

let localizer = FluentLocalizer::builder(langid!("en-GB"))
    .with_consumer_resources(["welcome = Hello, { $name }!"])
    .try_build()?;
let mut args = LocalizationArgs::new();
args.insert("name", FluentValue::from("Ada"));
let rendered = localizer.lookup("welcome", Some(&args)).expect("welcome message");
assert!(rendered.contains("Ada"));
assert_eq!(localizer.message("missing", None, "Fallback"), "Fallback");
# Ok::<(), ortho_l10n::FluentLocalizerError>(())
```

Arguments retain Fluent's `FluentValue` representation. `FluentValue`,
`langid`, and `LanguageIdentifier` are re-exported so callers can use matching
types without declaring their own versions of those dependencies. Lookup
returns owned strings, and `Localizer: Send + Sync` supports shared application
state.

The application chooses its locale and fallback text. Static consumer
catalogues can also use `include_str!("messages.ftl")`; loading a catalogue
never invokes configuration discovery. Product catalogues and mappings from
application diagnostics to message identifiers belong to the consumer.

### Bidirectional isolation

Fluent wraps every interpolated value in U+2068 FIRST STRONG ISOLATE and U+2069
POP DIRECTIONAL ISOLATE by default. The marks keep right-to-left values
readable inside left-to-right text, but many terminals, fonts, and log viewers
do not display them, and they break `grep`, string comparison, and snapshot
tests on diagnostics. Disable them when the rendered text is known to be
left-to-right:

```rust
use ortho_l10n::{FluentLocalizer, FluentValue, LocalizationArgs, Localizer, langid};

let localizer = FluentLocalizer::builder(langid!("en-GB"))
    .with_consumer_resources(["welcome = Hello, { $name }!"])
    .with_use_isolating(false)
    .try_build()?;
let mut args = LocalizationArgs::new();
args.insert("name", FluentValue::from("Ada"));
assert_eq!(localizer.lookup("welcome", Some(&args)).as_deref(), Some("Hello, Ada!"));
# Ok::<(), ortho_l10n::FluentLocalizerError>(())
```

This is not general bidirectional sanitization. Keep the default when a
right-to-left value may be interpolated into the text.

## Defaults and precedence

`FluentLocalizer::builder` retains the existing embedded English and Japanese
catalogues. Any English language tag selects the English resource data, and any
Japanese language tag selects Japanese data; reports retain the requested
locale. This includes compatibility CLI, Clap-error, and profile-error copy from
`ortho_config`.

Defaults are built first, followed by consumer resources in their supplied
order. Registration conflicts return `FluentLocalizerError::Registration`. A
prebuilt consumer bundle takes precedence over resource strings, even invalid
strings. Lookup checks the consumer catalogue before defaults, trying an exact
identifier before its dotted-to-hyphen spelling. Missing messages return `None`;
`Localizer::message` supplies the application's fallback.

An explicit `with_use_isolating` value is applied once at build time to both
the embedded defaults and the consumer bundle, including a prebuilt bundle
supplied through `with_consumer_bundle`. It therefore governs fallback lookups
as well as consumer messages, and the order of the builder calls does not
matter. Without an explicit setting, bundles built from resources keep Fluent's
isolating default and a supplied bundle keeps whatever setting its creator
chose.

Other languages need `disable_defaults()`, even when consumer resources are
supplied. A consumer-only localizer can also be empty.

```rust
use ortho_l10n::{FluentLocalizer, Localizer, langid};

let localizer = FluentLocalizer::builder(langid!("fr"))
    .disable_defaults()
    .with_consumer_resources(["welcome = Bonjour"])
    .try_build()?;
assert_eq!(localizer.lookup("welcome", None).as_deref(), Some("Bonjour"));
# Ok::<(), ortho_l10n::FluentLocalizerError>(())
```

## Report formatting failures

Formatting failures invoke a synchronous `FormattingIssueReporter` before
lookup continues to the next catalogue. The issue contains the original
identifier, requested locale, catalogue source, and Fluent errors. The default
reporter emits a structured tracing warning; the library installs no global
subscriber. Applications can provide their own reporter:

```rust
use std::sync::{Arc, Mutex};
use ortho_l10n::{FluentLocalizer, Localizer, langid};

let issues = Arc::new(Mutex::new(Vec::new()));
let captured = Arc::clone(&issues);
let localizer = FluentLocalizer::builder(langid!("en-US"))
    .with_consumer_resources(["broken = { $missing }"])
    .with_error_reporter(Arc::new(move |issue| {
        captured.lock().expect("report mutex").push(issue.clone());
    }))
    .try_build()?;
assert_eq!(localizer.lookup("broken", None), None);
assert_eq!(issues.lock().expect("report mutex").len(), 1);
# Ok::<(), ortho_l10n::FluentLocalizerError>(())
```

## Share message identifiers

`message_id_for` retains its programmer-error panic contract.
`try_message_id_for` returns `IdentifierError` for dynamic input. Both use the
same ASCII normalization: lowercase letters, preserve digits, underscores and
hyphens, join path segments and dotted suffixes with `MESSAGE_ID_SEPARATOR`,
and require an ASCII-letter root. Empty segments and other characters fail.
`try_normalize_segment` permits digit-leading fragments; `try_join_identifier`
validates a complete identifier assembled by a code generator.

```rust
use ortho_l10n::try_message_id_for;

assert_eq!(
    try_message_id_for(&["Demo", "cli"], "args.log_level.help")?,
    "demo-cli-args-log_level-help",
);
# Ok::<(), ortho_l10n::IdentifierError>(())
```

## Configuration compatibility

`ortho_config` re-exports these same runtime types. Old root imports and direct
`ortho_l10n` imports can be mixed without adapters or wrapper traits. Full
applications can depend on both crates when they also need configuration or
Clap integration. `OrthoConfigLocalization`, `ArgLocalizationIds`, and the
documentation IR remain in `ortho_config`; tools consuming that IR read
`ortho_config::docs::ORTHO_DOCS_IR_VERSION` rather than hard-coding its version.
