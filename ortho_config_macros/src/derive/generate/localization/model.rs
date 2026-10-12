//! Identifier model produced by one localization pass.
//!
//! These types are the vocabulary shared between the localization generator,
//! the documentation IR builder, and the artefact renderer. Splitting them out
//! keeps `mod.rs` inside the file-length cap and gives the wire-facing fields
//! one place to be documented, rather than spreading half-explained shapes
//! across three producers.

use proc_macro2::Span;

use super::identifier::{join_identifier, normalize_segment};
use super::suffix::MessageSuffix;

/// Dotted catalogue root every identifier of one configuration extends.
///
/// Held as the author wrote it (or as the derived application-name default
/// spells it) and normalized only where a Fluent id is built, so a diagnostic
/// can quote the spelling that appears in the source.
#[derive(Debug, Clone)]
pub(crate) struct LocalizationBase(pub(super) String);

impl LocalizationBase {
    /// Normalizes each dotted catalogue-base segment for Fluent.
    pub(super) fn normalize(&self, span: Span) -> syn::Result<FluentSegments> {
        self.0
            .split('.')
            .map(|segment| normalize_segment(segment, span))
            .collect::<Result<Vec<_>, _>>()
            .map(FluentSegments)
    }
}

impl AsRef<str> for LocalizationBase {
    /// Returns the original dotted catalogue base.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Raw Clap argument id that one configuration field declares.
///
/// Deliberately not a [`FluentMessageId`]: this value is emitted verbatim into
/// the generated `clap` attribute, while every Fluent id built from it has
/// been normalized.
#[derive(Debug, Clone)]
pub(crate) struct ClapArgId(pub(super) String);

impl ClapArgId {
    /// Normalizes each dotted Clap argument-id segment for Fluent.
    pub(super) fn normalize(&self, span: Span) -> syn::Result<FluentSegments> {
        self.0
            .split('.')
            .map(|segment| normalize_segment(segment, span))
            .collect::<Result<Vec<_>, _>>()
            .map(FluentSegments)
    }
}

impl AsRef<str> for ClapArgId {
    /// Returns the raw Clap argument id for code emission.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Ordered, already-normalized segments of an identifier under construction.
///
/// Carrying the segments rather than a joined string lets the builder append a
/// suffix without re-splitting, and keeps the single leading-letter check on
/// the same value that will be joined.
#[derive(Debug, Clone)]
pub(crate) struct FluentSegments(pub(super) Vec<String>);

impl FluentSegments {
    /// Ensures the first normalized segment starts with an ASCII letter.
    pub(super) fn ensure_leading_ascii_letter(&self, span: Span) -> syn::Result<()> {
        let Some(first) = self.0.first() else {
            return Err(syn::Error::new(
                span,
                "Fluent identifier must start with an ASCII letter: missing command root",
            ));
        };
        let Some(ch) = first.chars().next() else {
            return Err(syn::Error::new(
                span,
                "Fluent identifier must start with an ASCII letter: empty command root",
            ));
        };
        if ch.is_ascii_alphabetic() {
            Ok(())
        } else {
            Err(syn::Error::new(
                span,
                format!("Fluent identifier must start with an ASCII letter: {first:?}"),
            ))
        }
    }

    /// Returns a copy with one additional segment.
    fn with_segment(&self, segment: impl Into<String>) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment.into());
        Self(segments)
    }

    /// Returns a copy with one fixed message suffix.
    pub(super) fn with_suffix(&self, suffix: MessageSuffix) -> Self {
        self.with_segment(suffix.as_ref())
    }

    /// Joins the segments into a validated Fluent message id.
    pub(super) fn join(&self, span: Span) -> syn::Result<FluentMessageId> {
        join_identifier(&self.0, span).map(FluentMessageId)
    }
}

/// A validated Fluent message id, ready for catalogue lookup or emission.
///
/// Normalization and the leading-ASCII-letter rule have both been applied by
/// the time one of these exists, so a value of this type never needs
/// re-checking.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct FluentMessageId(pub(super) String);

impl AsRef<str> for FluentMessageId {
    /// Returns the joined Fluent message id.
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Identifier model for one eligible configuration argument.
///
/// Flattened, subcommand, and `skip_cli` fields never get one, which is why
/// `ARG_IDS` and this vector are not simply the field list.
#[derive(Debug, Clone)]
pub(crate) struct ArgIdsModel {
    /// Rust field name, used to match the argument back to its declaration.
    pub field_name: String,
    /// Clap argument id emitted into the generated attribute.
    pub name: ClapArgId,
    /// Catalogue key for the argument's one-line help.
    pub help_id: FluentMessageId,
    /// Catalogue key for the argument's long help.
    pub long_help_id: FluentMessageId,
    /// Catalogue key for the argument's displayed value name.
    pub value_name_id: FluentMessageId,
}

#[derive(Debug, Clone)]
#[expect(
    clippy::struct_field_names,
    reason = "each field is a distinct identifier constant; the `_id` postfix is the point"
)]
pub(crate) struct CommandIds {
    /// Catalogue key for the command's one-line about text.
    pub about_id: FluentMessageId,
    /// Catalogue key for the command's long about text.
    pub long_about_id: FluentMessageId,
    /// Catalogue key for the command's usage string.
    pub usage_id: FluentMessageId,
    /// Catalogue key for the command's short version.
    pub version_id: FluentMessageId,
    /// Catalogue key for the command's long version.
    pub long_version_id: FluentMessageId,
    /// Catalogue key for the text shown after the command's help.
    pub after_help_id: FluentMessageId,
    /// Catalogue key for the text shown after the command's long help.
    pub after_long_help_id: FluentMessageId,
}

/// Every identifier one derive expansion generates for its configuration.
///
/// This is the value that flows into `OrthoConfigLocalization` emission, the
/// documentation IR, and the opt-in artefact, so it is the one place where a
/// missing or extra identifier should be noticed.
#[derive(Debug, Clone)]
pub(crate) struct LocalizationIds {
    /// Catalogue root every id below extends.
    pub base: LocalizationBase,
    /// Command-level identifiers.
    pub command: CommandIds,
    /// One entry per eligible argument, in declaration order.
    pub args: Vec<ArgIdsModel>,
}
