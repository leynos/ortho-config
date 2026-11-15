//! Unit tests for localisation helpers and Fluent-backed behaviour.

use super::*;
use rstest::rstest;
use std::sync::{Arc, Mutex};

mod support_localizers {
    //! Localizer doubles shared by the localization tests.

    use crate as crate_root;
    include!("../../tests/support/localizers.rs");
}
use support_localizers::ArgumentEchoLocalizer;

#[rstest]
fn localizes_clap_errors_when_translation_exists() {
    use clap::{Arg, Command, error::ContextKind};

    let mut command = Command::new("demo").arg(Arg::new("path").required(true).value_name("PATH"));
    let error = command
        .try_get_matches_from_mut(["demo"])
        .expect_err("missing required argument should raise a clap error");
    let original = error.to_string();
    let expected_argument = match error.get(ContextKind::InvalidArg) {
        Some(clap::error::ContextValue::Strings(values)) => values.join(", "),
        Some(clap::error::ContextValue::String(value)) => value.clone(),
        _ => String::from("<none>"),
    };

    let localised = localize_clap_error(error, &ArgumentEchoLocalizer::new("<none>"));
    let rendered = localised.to_string();

    assert!(
        rendered.contains("clap-error-missing-argument"),
        "expected clap error id in localised output: {rendered}"
    );
    assert!(
        rendered.contains(&expected_argument),
        "expected argument context '{expected_argument}' in output: {rendered}"
    );
    assert_ne!(
        rendered, original,
        "localised output should differ from the default clap message"
    );
}

#[rstest]
fn falls_back_to_stock_clap_message_when_translation_missing() {
    use clap::{Arg, Command};

    let mut command = Command::new("demo").arg(Arg::new("path").required(true));
    let error = command
        .try_get_matches_from_mut(["demo"])
        .expect_err("missing required argument should raise a clap error");
    let original = error.to_string();

    let localised = localize_clap_error(error, &NoOpLocalizer::new());

    assert_eq!(
        localised.to_string(),
        original,
        "expected to fall back to the stock clap message when localisation fails"
    );
}

#[rstest]
fn localizes_clap_errors_with_command_enriches_valid_subcommands() {
    use clap::Command;

    struct CapturingLocalizer {
        captured: Arc<Mutex<Option<String>>>,
    }

    impl Localizer for CapturingLocalizer {
        fn lookup(&self, _id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
            // let-chains are edition-2024 only, stabilised in Rust 1.88 (2025-06-26).
            if let Some(args_map) = args
                && let Some(FluentValue::String(text)) = args_map.get("valid_subcommands")
            {
                let mut guard = self.captured.lock().expect("capture mutex poisoned");
                *guard = Some(text.to_string());
            }
            None
        }
    }

    let mut command = Command::new("demo")
        .subcommand(Command::new("greet"))
        .subcommand(Command::new("take-leave"))
        .subcommand_required(true);

    let error = command
        .try_get_matches_from_mut(["demo"])
        .expect_err("missing subcommand should produce a clap error");

    let captured = Arc::new(Mutex::new(None));
    let localizer = CapturingLocalizer {
        captured: Arc::clone(&captured),
    };

    let rendered = localize_clap_error_with_command(error, &localizer, Some(&command));
    assert!(
        !rendered.to_string().is_empty(),
        "localized error should render to a non-empty string"
    );

    let valid_subcommands = captured
        .lock()
        .expect("capture mutex poisoned")
        .clone()
        .expect("expected valid_subcommands localization arg to be provided");

    assert!(
        valid_subcommands.contains("greet") && valid_subcommands.contains("take-leave"),
        "expected valid_subcommands arg to contain all subcommands, got: {valid_subcommands}"
    );
}

#[rstest]
fn passes_through_display_help_errors() {
    use clap::error::ErrorKind;

    let help_error = clap::Error::raw(ErrorKind::DisplayHelp, "help text");
    let localised = localize_clap_error(help_error, &NoOpLocalizer::new());

    assert_eq!(
        localised.kind(),
        ErrorKind::DisplayHelp,
        "DisplayHelp errors should be passed through unchanged"
    );
}

#[rstest]
fn translated_error_preserves_tail() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TailCheckingLocalizer {
        called: AtomicBool,
    }

    impl Localizer for TailCheckingLocalizer {
        fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
            self.called.store(true, Ordering::SeqCst);
            Some(format!("{id}:translated"))
        }
    }

    let error = clap::Error::raw(
        clap::error::ErrorKind::UnknownArgument,
        "error: original\nUsage: demo [OPTIONS]",
    );
    let localizer = TailCheckingLocalizer {
        called: AtomicBool::new(false),
    };

    let localised = localize_clap_error(error, &localizer);
    let rendered = localised.to_string();

    assert!(
        localizer.called.load(Ordering::SeqCst),
        "localizer should have been invoked"
    );
    assert!(
        rendered.contains("clap-error-unknown-argument:translated"),
        "expected translated prefix, got {rendered}"
    );
    assert!(
        rendered.contains("Usage: demo [OPTIONS]"),
        "expected usage tail to be preserved, got {rendered}"
    );
}

#[rstest]
fn clap_error_formatter_is_cloneable_and_invokes_localizer() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct CountingLocalizer {
        hits: Arc<AtomicUsize>,
    }

    impl Localizer for CountingLocalizer {
        fn lookup(&self, _id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
            self.hits.fetch_add(1, Ordering::SeqCst);
            None
        }
    }

    let hits = Arc::new(AtomicUsize::new(0));
    let localizer = Arc::new(CountingLocalizer {
        hits: Arc::clone(&hits),
    });
    let formatter = clap_error_formatter(localizer);
    let cloned = formatter.clone();

    let err = clap::Error::raw(clap::error::ErrorKind::UnknownArgument, "boom");
    let _ = formatter(err);
    let err2 = clap::Error::raw(clap::error::ErrorKind::UnknownArgument, "boom");
    let _ = cloned(err2);

    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "formatter closure should delegate to localize_clap_error for each call"
    );
}
