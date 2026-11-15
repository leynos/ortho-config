//! Compile and runtime checks for mixed direct-runtime and configuration paths.

use std::sync::Arc;

/// Accepts a directly constructed runtime through the compatibility trait.
fn through_configuration(localizer: &dyn ortho_config::Localizer) -> Option<String> {
    localizer.lookup("custom", None)
}

/// Accepts a compatibility value through the directly imported runtime type.
fn through_runtime(localizer: &ortho_l10n::FluentLocalizer) -> Option<String> {
    ortho_l10n::Localizer::lookup(localizer, "custom", None)
}

#[test]
fn mixed_imports_share_nominal_types() {
    let reporter: ortho_config::FormattingIssueReporter =
        Arc::new(|_: &ortho_l10n::FormattingIssue| {});
    let direct = ortho_l10n::FluentLocalizer::builder(ortho_l10n::langid!("en-US"))
        .with_consumer_resources(["custom = Shared type\nformatted = { $shared }"])
        .with_error_reporter(reporter)
        .try_build()
        .expect("valid consumer catalogue");
    let facade: ortho_config::FluentLocalizer = direct;
    assert_eq!(
        through_configuration(&facade).as_deref(),
        Some("Shared type")
    );
    assert_eq!(through_runtime(&facade).as_deref(), Some("Shared type"));

    let mut args: ortho_config::LocalizationArgs<'_> = ortho_l10n::LocalizationArgs::new();
    let value: ortho_config::FluentValue<'_> = ortho_l10n::FluentValue::from("shared argument");
    args.insert("shared", value);
    let formatted = ortho_config::Localizer::lookup(&facade, "formatted", Some(&args))
        .expect("shared arguments format through the compatibility path");
    assert!(formatted.contains("shared argument"));
}

/// Both import paths preserve the documented thread-safety contract.
#[test]
fn compatibility_paths_remain_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ortho_config::FluentLocalizer>();
    assert_send_sync::<ortho_l10n::FluentLocalizer>();
    assert_send_sync::<ortho_config::NoOpLocalizer>();
    assert_send_sync::<Box<dyn ortho_config::Localizer>>();
    assert_send_sync::<ortho_config::FormattingIssueReporter>();
}
