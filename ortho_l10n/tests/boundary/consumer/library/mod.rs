//! The library bodies a rendered consumer root compiles and tests.
//!
//! A consumer root is only evidence if its body does something an application
//! would do: build a localiser, look a message up, fall back when there is no
//! translation, and observe a formatting failure. The bodies here do exactly
//! that, and nothing more, so a failure names one of five things:
//!
//! - the crate is not reachable under its expected name;
//! - a public type is missing, renamed, or has lost a bound;
//! - the lookup path does not render a placeholder;
//! - a missing message does not fall back to caller copy;
//! - a formatting failure is not reported to the installed reporter.
//!
//! The mixed body's second job is nominal identity. It takes a value built by
//! `ortho_l10n`'s spelling of a type and hands it to `ortho_config`'s spelling
//! of the same name, in both directions. That only compiles if the two paths
//! name the same type, so a re-export that drifted into a wrapper - or a
//! facade that quietly forked the runtime - fails to compile rather than
//! passing a shape-only check.
//!
//! # Why raw strings
//!
//! Each body below is Rust source *inside* Rust source. Raw strings keep the
//! emitted text literal: every brace in the template is a brace in the
//! generated file, so the template can be read as the file it produces. The
//! expected values spell Fluent's bidi isolation characters as `\u{2068}` and
//! `\u{2069}` escape text rather than the invisible characters themselves;
//! they are part of the contract, because an assertion that stripped them
//! would also pass for a localiser that never interpolated anything.

use super::Dependency;

/// Render the body for `dependency`.
///
/// The runtime and mixed shapes share one runtime body, because the point of
/// the mixed shape is that the *second* import path resolves to the same
/// items. Only the extra cross-assignment distinguishes them.
pub(crate) fn body(dependency: Dependency) -> String {
    match dependency {
        Dependency::Runtime => String::from(RUNTIME_BODY),
        Dependency::Configuration => String::from(CONFIGURATION_BODY),
        Dependency::Mixed => String::from(MIXED_BODY),
    }
}

/// A consumer of `ortho_l10n` alone.
///
/// Written as an application would write it: a catalogue of its own, one
/// interpolation, one deliberate miss, and one message whose pattern cannot be
/// formatted because its argument is absent.
const RUNTIME_BODY: &str = r#"//! A consumer of the standalone localisation runtime.
//!
//! Written as an application would write it: a catalogue of its own, one
//! interpolation, one deliberate miss, and one message whose pattern cannot be
//! formatted because its argument is absent.
use ortho_l10n::{
    FluentLocalizer, FluentValue, FormattingIssue, FormattingIssueReporter, LocalizationArgs,
    Localizer, langid,
};
use std::sync::{Arc, Mutex};

/// The catalogue a consumer application would layer over the defaults.
const CATALOGUE: &[&str] = &[
    "welcome = Hello, { $name }!",
    "broken = Missing { $absent } value",
];

/// What the consumer observed, so one test can assert all of it.
#[derive(Debug)]
pub struct Observations {
    /// The interpolated greeting, or `None` when lookup missed.
    pub greeting: Option<String>,
    /// The unformattable message's lookup, which must have missed.
    pub unformattable: Option<String>,
    /// The caller's copy, returned because no such message exists.
    pub fallback: String,
    /// Identifiers reported to the installed formatting-issue reporter.
    pub reported: Vec<String>,
}

/// Build a localiser, exercise it, and return what it did.
pub fn observe() -> Observations {
    let reported: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&reported);
    let reporter: FormattingIssueReporter = Arc::new(move |issue: &FormattingIssue| {
        if let Ok(mut ids) = sink.lock() {
            ids.push(issue.id.clone());
        }
    });
    let localizer = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(CATALOGUE.iter().copied())
        .with_error_reporter(reporter)
        .try_build()
        .expect("the consumer catalogue is valid");
    let mut args: LocalizationArgs<'_> = LocalizationArgs::new();
    args.insert("name", FluentValue::from("Ada"));
    let greeting = localizer.lookup("welcome", Some(&args));
    // `broken` references an argument the caller never supplies. Looking it up
    // is what exercises the reporting path; asserting on the list without
    // looking it up would pass for a localiser that never reported anything.
    let unformattable = localizer.lookup("broken", None);
    Observations {
        greeting,
        unformattable,
        fallback: localizer.message("absent", None, "Fallback copy"),
        reported: reported.lock().map(|ids| ids.clone()).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::observe;

    #[test]
    fn interpolates_into_the_consumer_catalogue() {
        let observations = observe();
        assert_eq!(
            observations.greeting.as_deref(),
            Some("Hello, \u{2068}Ada\u{2069}!"),
            "the greeting must come from the consumer catalogue, interpolated",
        );
    }

    #[test]
    fn falls_back_to_caller_copy_for_an_unknown_message() {
        assert_eq!(observe().fallback, "Fallback copy");
    }

    #[test]
    fn reports_the_unformattable_message_once() {
        let observations = observe();
        assert_eq!(
            observations.unformattable, None,
            "a pattern with an absent argument must not render",
        );
        assert_eq!(
            observations.reported,
            vec![String::from("broken")],
            "exactly the unformattable message must reach the reporter",
        );
    }
}
"#;

/// A consumer of the configuration facade alone.
///
/// Existing applications import the localiser through `ortho_config`. This
/// body pins that path: it must keep naming the same runtime items, and the
/// facade's embedded catalogue must still be reachable through them.
const CONFIGURATION_BODY: &str = r#"//! A consumer of the configuration facade alone.
//!
//! Existing applications import the localiser through `ortho_config`. This
//! body pins that path: it must keep naming the same runtime items, and the
//! facade's embedded catalogue must still be reachable through them.
use ortho_config::{FluentLocalizer, Localizer, langid};

/// What the facade consumer observed.
#[derive(Debug)]
pub struct Observations {
    /// The facade's own embedded catalogue entry, or `None` on a miss.
    pub embedded: Option<String>,
    /// The consumer's own message, layered over that catalogue.
    pub layered: Option<String>,
}

/// Build a facade localiser and look up one message from each catalogue.
pub fn observe() -> Observations {
    let localizer = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["welcome = Hello from the facade consumer"])
        .try_build()
        .expect("the facade catalogue is valid");
    Observations {
        embedded: localizer.lookup("cli.about", None),
        layered: localizer.lookup("welcome", None),
    }
}

#[cfg(test)]
mod tests {
    use super::observe;

    #[test]
    fn reaches_the_facade_embedded_catalogue() {
        assert_eq!(
            observe().embedded.as_deref(),
            Some("OrthoConfig command-line interface."),
            "the facade's embedded catalogue must still resolve",
        );
    }

    #[test]
    fn layers_the_consumer_catalogue_over_it() {
        assert_eq!(
            observe().layered.as_deref(),
            Some("Hello from the facade consumer"),
        );
    }
}
"#;

/// A consumer importing both crates and crossing values between them.
///
/// Two things are proved here that neither single-crate consumer can prove.
/// First, the facade's re-export is the runtime's type rather than a
/// lookalike: a value built through one import path satisfies the other's
/// spelling of the type. Second, the runtime is usable as itself - a consumer
/// may depend on the small crate directly even while an application elsewhere
/// imports the facade.
const MIXED_BODY: &str = r#"//! A consumer importing the runtime and the facade side by side.
//!
//! Two things are proved here that neither single-crate consumer can prove.
//! First, the facade's re-export is the runtime's type rather than a
//! lookalike: a value built through one import path satisfies the other's
//! spelling of the type. Second, the runtime is usable as itself - a consumer
//! may depend on the small crate directly even while an application elsewhere
//! imports the facade.
use ortho_config::langid as facade_langid;
use ortho_config::{
    FluentLocalizer as FacadeLocalizer, FluentValue as FacadeValue,
    LocalizationArgs as FacadeArgs, Localizer as FacadeTrait,
};
use ortho_l10n::langid;
use ortho_l10n::{
    FluentLocalizer as RuntimeLocalizer, FluentValue as RuntimeValue,
    LocalizationArgs as RuntimeArgs, Localizer as RuntimeTrait,
};

/// Look a message up through the facade's spelling of the type and trait.
fn greet(localizer: &ortho_config::FluentLocalizer) -> Option<String> {
    let mut args: RuntimeArgs<'_> = RuntimeArgs::new();
    args.insert("name", RuntimeValue::from("Ada"));
    FacadeTrait::lookup(localizer, "welcome", Some(&args))
}

/// Build the runtime through the runtime's spelling of the builder.
pub fn runtime_localizer() -> RuntimeLocalizer {
    RuntimeLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["welcome = Hello, { $name }!"])
        .try_build()
        .expect("the consumer catalogue is valid")
}

/// Hand a runtime-built localiser to a function declaring the facade's type.
pub fn greet_through_the_facade() -> Option<String> {
    let localizer: FacadeLocalizer = runtime_localizer();
    greet(&localizer)
}

/// Build a localiser through the facade and use the runtime's trait on it.
pub fn greet_through_the_runtime() -> Option<String> {
    let localizer = FacadeLocalizer::builder(facade_langid!("en-GB"))
        .with_consumer_resources(["welcome = Hello, { $name }!"])
        .try_build()
        .expect("the facade catalogue is valid");
    let mut args: FacadeArgs<'_> = FacadeArgs::new();
    args.insert("name", FacadeValue::from("Grace"));
    RuntimeTrait::lookup(&localizer, "welcome", Some(&args))
}

/// Every shared type must stay `Send` and `Sync` under both spellings.
pub fn shared_types_keep_their_bounds() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FacadeLocalizer>();
    assert_send_sync::<RuntimeLocalizer>();
    assert_send_sync::<ortho_config::FormattingIssueReporter>();
    assert_send_sync::<ortho_l10n::FormattingIssueReporter>();
}

#[cfg(test)]
mod tests {
    use super::{greet_through_the_facade, greet_through_the_runtime};

    #[test]
    fn runtime_value_satisfies_the_facade_type() {
        assert_eq!(
            greet_through_the_facade().as_deref(),
            Some("Hello, \u{2068}Ada\u{2069}!"),
        );
    }

    #[test]
    fn facade_value_satisfies_the_runtime_trait() {
        assert_eq!(
            greet_through_the_runtime().as_deref(),
            Some("Hello, \u{2068}Grace\u{2069}!"),
        );
    }
}
"#;
