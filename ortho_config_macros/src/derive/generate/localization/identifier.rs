//! Spanned diagnostics around the runtime's single identifier convention.
//!
//! Configuration emission retains source spans and diagnostic wording while
//! `ortho_l10n` owns normalization and complete-identifier validation.

use proc_macro2::Span;
use syn::Error;

/// Normalizes a fragment without imposing the complete ID's leading-letter rule.
pub(crate) fn normalize_segment(raw: &str, span: Span) -> Result<String, Error> {
    ortho_l10n::try_normalize_segment(raw).map_err(|error| Error::new(span, error))
}

/// Composes normalized segments and maps validation errors to the caller's span.
pub(crate) fn join_identifier(segments: &[String], span: Span) -> Result<String, Error> {
    ortho_l10n::try_join_identifier(segments).map_err(|error| Error::new(span, error))
}

#[cfg(test)]
mod tests {
    //! Error wording and source-span preservation at the macro boundary.

    use super::*;
    use rstest::rstest;

    /// Invalid fragments retain both the shared error text and a nonempty source span.
    #[rstest]
    #[case::empty("", "invalid Fluent identifier segment: segment must not be empty")]
    #[case::dot("bad.name", "invalid Fluent identifier segment: \"bad.name\"")]
    #[case::unicode("héllo", "invalid Fluent identifier segment: \"héllo\"")]
    fn segment_errors_keep_the_input_span(#[case] raw: &str, #[case] expected: &str) {
        let literal: syn::LitStr = syn::parse_str("\n  \"original span\"").expect("literal");
        let span = syn::spanned::Spanned::span(&literal);
        let error = normalize_segment(raw, span).expect_err("invalid segment");
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.span().start(), span.start());
        assert_eq!(error.span().end(), span.end());
        assert_ne!(span.start(), span.end(), "span check must not be vacuous");
    }

    /// Fragment normalization permits digit-leading argument ids.
    #[test]
    fn numeric_argument_fragments_remain_representable() {
        assert_eq!(
            normalize_segment("123_ABC", Span::call_site()).expect("fragment"),
            "123_abc"
        );
    }

    /// Complete identifiers keep the root diagnostic at the supplied source span.
    #[rstest]
    #[case::missing(vec![], "Fluent identifier must start with an ASCII letter: missing command root")]
    #[case::numeric(vec!["123".to_owned()], "Fluent identifier must start with an ASCII letter: \"123\"")]
    fn root_errors_keep_the_input_span(#[case] segments: Vec<String>, #[case] expected: &str) {
        let literal: syn::LitStr = syn::parse_str("\n  \"root span\"").expect("literal");
        let span = syn::spanned::Spanned::span(&literal);
        let error = join_identifier(&segments, span).expect_err("invalid root");
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.span().start(), span.start());
        assert_eq!(error.span().end(), span.end());
    }
}
