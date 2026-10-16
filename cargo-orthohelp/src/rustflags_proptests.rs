//! Property tests for coverage-token sanitisation of `RUSTFLAGS` variables.
//!
//! `apply_sanitized_rustflags` must strip the `-Cinstrument-coverage` tokens
//! `cargo-llvm-cov` injects while leaving every other flag exactly as it found
//! it, for both the whitespace-delimited `RUSTFLAGS` form and the
//! unit-separator-delimited `CARGO_ENCODED_RUSTFLAGS` form. The sibling `tests`
//! module pins the spellings somebody thought to write down; these properties
//! generate flag sequences from a token model and hold both sanitizers to it.
//!
//! The model, not the implementation, decides what survives. Each generated
//! element states the tokens it contributes and whether sanitisation must drop
//! them, so every expectation is built from the generator rather than read back
//! out of the code under test; a change that dropped or kept the wrong token
//! has to contradict the model in order to pass.
//!
//! # Scope
//!
//! Generated token text comes from a hyphen-free alphabet, and the only
//! hyphenated tokens are the audited literals in [`CANONICAL_KEPT_TOKENS`].
//! That keeps the kept and coverage classes disjoint by construction, with no
//! filtering anywhere in the generator: a bare fragment cannot spell `-C` or
//! `instrument-coverage`, because it cannot spell a hyphen.
//!
//! Two shapes of the real input space stay out of reach. Empty segments between
//! encoded separators cannot be generated, though the wholly empty input can,
//! and no generated token contains whitespace, since the plain form would split
//! it in two. Both remain in the example table in the sibling `tests` module,
//! which these properties extend rather than replace.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use super::*;

/// One element of a generated `RUSTFLAGS` token sequence.
#[derive(Clone, Debug)]
enum FlagElement {
    /// A flag or value that sanitisation must keep.
    Kept(String),
    /// A compact `-Cinstrument-coverage[=value]` token to drop.
    CompactCoverage(String),
    /// A split `-C` plus `instrument-coverage[=value]` pair to drop.
    SplitCoverage(String),
    /// A `-C` whose neighbour is not a coverage option, so it stays.
    BareCodegenFlag,
}

impl FlagElement {
    /// The raw tokens this element contributes, in order.
    fn tokens(&self) -> Vec<String> {
        match self {
            Self::Kept(token) | Self::CompactCoverage(token) => vec![token.clone()],
            Self::SplitCoverage(option) => vec!["-C".to_owned(), option.clone()],
            Self::BareCodegenFlag => vec!["-C".to_owned()],
        }
    }

    /// Whether sanitisation must drop every token this element contributes.
    fn is_coverage(&self) -> bool {
        matches!(self, Self::CompactCoverage(_) | Self::SplitCoverage(_))
    }
}

/// The delimiter convention a `RUSTFLAGS` variable uses.
#[derive(Clone, Copy, Debug)]
enum FlagForm {
    /// `RUSTFLAGS`: whitespace-delimited, and collapsed on a rewrite.
    Plain,
    /// `CARGO_ENCODED_RUSTFLAGS`: unit-separator-delimited, keeping empties.
    Encoded,
}

impl FlagForm {
    /// Sanitises `flags` using this form's delimiter.
    fn sanitize(self, flags: &str) -> Option<String> {
        match self {
            Self::Plain => sanitize_plain_rustflags(flags),
            Self::Encoded => sanitize_encoded_rustflags(flags),
        }
    }

    /// Splits `flags` the way this form's sanitizer does.
    fn split(self, flags: &str) -> Vec<String> {
        match self {
            Self::Plain => flags.split_whitespace().map(str::to_owned).collect(),
            Self::Encoded => flags
                .split(ENCODED_RUSTFLAGS_SEPARATOR)
                .map(str::to_owned)
                .collect(),
        }
    }

    /// Joins `tokens` the way this form's sanitizer does when it rewrites.
    fn join(self, tokens: &[String]) -> String {
        match self {
            Self::Plain => tokens.join(" "),
            Self::Encoded => tokens.join(&ENCODED_RUSTFLAGS_SEPARATOR.to_string()),
        }
    }

    /// Joins `tokens` into an input string, leaving the plain form's
    /// insignificant whitespace free to vary.
    ///
    /// The encoded form has no equivalent freedom: its delimiter is the only
    /// legal separator, so the rendered input is the canonical join.
    fn render_input(self, tokens: &[String], filler: &str) -> String {
        match self {
            Self::Plain => tokens.join(filler),
            Self::Encoded => self.join(tokens),
        }
    }
}

/// Every raw token `elements` contributes, in order.
fn all_tokens(elements: &[FlagElement]) -> Vec<String> {
    elements.iter().flat_map(FlagElement::tokens).collect()
}

/// The tokens `elements` contributes to sanitised output, in order.
fn retained_tokens(elements: &[FlagElement]) -> Vec<String> {
    elements
        .iter()
        .filter(|element| !element.is_coverage())
        .flat_map(FlagElement::tokens)
        .collect()
}

/// Whether `elements` contains any token sanitisation must drop.
fn has_coverage(elements: &[FlagElement]) -> bool {
    elements.iter().any(FlagElement::is_coverage)
}

/// The string sanitisation must produce for `input`, per the element model.
///
/// Returns `None` exactly when sanitisation must drop every token. `input` is
/// consulted only for the rewrite-free case, where the implementation promises
/// to hand back the original string untouched.
fn expected_output(form: FlagForm, input: &str, elements: &[FlagElement]) -> Option<String> {
    if !has_coverage(elements) {
        return Some(input.to_owned());
    }
    let retained = retained_tokens(elements);
    (!retained.is_empty()).then(|| form.join(&retained))
}

/// Whether `token` spells a coverage flag or a coverage option.
///
/// This restates the requirement in literals of its own instead of deferring to
/// the module's private predicates, so a mistake in those predicates cannot
/// hide behind this check.
fn spells_coverage(token: &str) -> bool {
    let option = token.strip_prefix("-C").unwrap_or(token);
    option == COVERAGE_OPTION || option.starts_with(COVERAGE_OPTION_VALUE)
}

/// Whether `tokens` still holds a coverage flag or a split coverage pair.
fn retains_coverage(tokens: &[String]) -> bool {
    let compact = tokens.iter().any(|token| spells_coverage(token));
    let split = tokens.windows(2).any(|pair| {
        pair.first().is_some_and(|lead| lead == "-C")
            && pair.get(1).is_some_and(|option| spells_coverage(option))
    });
    compact || split
}

/// The compact flag `cargo-llvm-cov` injects.
const COVERAGE_FLAG: &str = "-Cinstrument-coverage";

/// The option name the split spelling of the same flag carries.
const COVERAGE_OPTION: &str = "instrument-coverage";

/// The option name as it appears when it carries a value.
const COVERAGE_OPTION_VALUE: &str = "instrument-coverage=";

/// Characters a generated token fragment may contain.
///
/// The set excludes the plain form's whitespace delimiters, the encoded form's
/// unit separator, and the hyphen, so a bare fragment can never spell a coverage
/// token. That is what lets the generator build kept tokens without filtering.
const FRAGMENT_CHARS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'i', 'l', 'm', 'n', 'o', 'p', 'r', 's', 't', 'u', 'v', '0',
    '1', '2', '3', '_', '=', ':', '/', '.', '+', ',',
];

/// Flag prefixes that cannot begin a coverage token.
const KEPT_PREFIXES: &[&str] = &[
    "--cfg=",
    "-Clink-arg=",
    "-Copt-level=",
    "-Cdebuginfo=",
    "-Z",
    "-L",
    "-l",
    "--extern=",
];

/// Kept tokens borrowed from the sibling `tests` module's example table.
const CANONICAL_KEPT_TOKENS: &[&str] = &[
    "--cfg",
    "caller_gate",
    "-Clink-arg=/SAFESEH:NO",
    "-Copt-level=3",
    "-Zunstable-options",
];

/// Whitespace runs the plain form may use between its tokens.
const FILLERS: &[&str] = &[" ", "  ", "\t", " \t "];

/// A short, hyphen-free token fragment.
fn fragment() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENT_CHARS.to_vec()), 1..=10)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// A token sanitisation must keep.
fn kept_token() -> impl Strategy<Value = String> {
    prop_oneof![
        (prop::sample::select(KEPT_PREFIXES.to_vec()), fragment())
            .prop_map(|(prefix, suffix)| format!("{prefix}{suffix}")),
        prop::sample::select(CANONICAL_KEPT_TOKENS.to_vec()).prop_map(str::to_owned),
        fragment(),
    ]
}

/// A compact coverage flag, with and without a value.
fn compact_coverage_token() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(COVERAGE_FLAG.to_owned()),
        fragment().prop_map(|value| format!("{COVERAGE_FLAG}={value}")),
    ]
}

/// The option a split `-C` carries for coverage, with and without a value.
fn split_coverage_option() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(COVERAGE_OPTION.to_owned()),
        fragment().prop_map(|value| format!("{COVERAGE_OPTION}={value}")),
    ]
}

/// An element sanitisation must drop.
fn coverage_element() -> impl Strategy<Value = FlagElement> {
    prop_oneof![
        compact_coverage_token().prop_map(FlagElement::CompactCoverage),
        split_coverage_option().prop_map(FlagElement::SplitCoverage),
    ]
}

/// An element sanitisation must keep.
fn kept_element() -> impl Strategy<Value = FlagElement> {
    prop_oneof![
        kept_token().prop_map(FlagElement::Kept),
        Just(FlagElement::BareCodegenFlag),
    ]
}

/// Any element, so kept and coverage tokens interleave at any position.
fn flag_element() -> impl Strategy<Value = FlagElement> {
    prop_oneof![kept_element(), coverage_element()]
}

/// A bounded, arbitrary interleaving of kept and coverage elements.
fn flag_elements() -> impl Strategy<Value = Vec<FlagElement>> {
    prop::collection::vec(flag_element(), 0..=6)
}

/// Elements that never contain a coverage token.
fn clean_elements() -> impl Strategy<Value = Vec<FlagElement>> {
    prop::collection::vec(kept_element(), 0..=6)
}

/// Elements guaranteed to hold both a kept token and a coverage token.
///
/// The guarantee keeps the properties that rewrite something from passing
/// vacuously: a rewrite of non-empty content always happens, so their
/// assertions are always reached.
fn rewritten_elements() -> impl Strategy<Value = Vec<FlagElement>> {
    (kept_element(), coverage_element(), flag_elements()).prop_map(|(kept, coverage, mut rest)| {
        let mut elements = vec![kept, coverage];
        elements.append(&mut rest);
        elements
    })
}

/// Both delimiter conventions, so every property exercises each one.
fn flag_form() -> impl Strategy<Value = FlagForm> {
    prop_oneof![Just(FlagForm::Plain), Just(FlagForm::Encoded)]
}

/// A whitespace run for the plain form to place between tokens.
fn filler() -> impl Strategy<Value = String> {
    prop::sample::select(FILLERS.to_vec()).prop_map(str::to_owned)
}

proptest! {
    /// Sanitisation keeps exactly the tokens the model marks as kept.
    ///
    /// A coverage token that survived, a kept token that vanished, a rewrite
    /// that lost the plain form's canonical spacing, or a `None` returned while
    /// kept tokens remained all disagree with the model.
    #[test]
    fn sanitisation_matches_the_token_model(
        form in flag_form(),
        filler in filler(),
        elements in flag_elements(),
    ) {
        let input = form.render_input(&all_tokens(&elements), &filler);

        let sanitised = form.sanitize(&input);
        prop_assert_eq!(sanitised, expected_output(form, &input, &elements));
    }

    /// Input holding no coverage token comes back byte for byte.
    ///
    /// The plain form's whitespace runs are regenerated, so this fails if
    /// sanitisation rewrites a string through its token join when it removed
    /// nothing.
    #[test]
    fn coverage_free_input_is_returned_byte_for_byte(
        form in flag_form(),
        filler in filler(),
        elements in clean_elements(),
    ) {
        let input = form.render_input(&all_tokens(&elements), &filler);

        prop_assert_eq!(form.sanitize(&input), Some(input));
    }

    /// No coverage flag or split coverage pair survives sanitisation.
    #[test]
    fn sanitised_output_retains_no_coverage_token(
        form in flag_form(),
        filler in filler(),
        elements in flag_elements(),
    ) {
        let input = form.render_input(&all_tokens(&elements), &filler);

        if let Some(sanitised) = form.sanitize(&input) {
            prop_assert!(
                !retains_coverage(&form.split(&sanitised)),
                "coverage survived sanitisation: {sanitised:?}"
            );
        }
    }

    /// Every kept token survives verbatim, in order and unchanged in count.
    #[test]
    fn kept_tokens_survive_verbatim(
        form in flag_form(),
        filler in filler(),
        elements in rewritten_elements(),
    ) {
        let input = form.render_input(&all_tokens(&elements), &filler);

        let Some(sanitised) = form.sanitize(&input) else {
            return Err(TestCaseError::fail(format!(
                "sanitisation dropped every token of {input:?}, but the generator kept one"
            )));
        };
        prop_assert_eq!(form.split(&sanitised), retained_tokens(&elements));
    }

    /// Sanitising an already sanitised value changes nothing.
    #[test]
    fn sanitisation_is_idempotent(
        form in flag_form(),
        filler in filler(),
        elements in rewritten_elements(),
    ) {
        let input = form.render_input(&all_tokens(&elements), &filler);

        let Some(sanitised) = form.sanitize(&input) else {
            return Err(TestCaseError::fail(format!(
                "sanitisation dropped every token of {input:?}, but the generator kept one"
            )));
        };
        let again = form.sanitize(&sanitised);
        prop_assert_eq!(again, Some(sanitised));
    }

    /// `None` comes back exactly when every token is a coverage token.
    ///
    /// The empty sequence is excluded from that equivalence, because the empty
    /// input is not a tokenless one: splitting an empty encoded value yields a
    /// single empty segment, which sanitisation keeps.
    #[test]
    fn none_is_returned_exactly_when_all_tokens_are_coverage(
        form in flag_form(),
        filler in filler(),
        elements in flag_elements(),
    ) {
        let input = form.render_input(&all_tokens(&elements), &filler);

        let sanitised = form.sanitize(&input);
        let every_token_is_coverage =
            !elements.is_empty() && elements.iter().all(FlagElement::is_coverage);
        prop_assert_eq!(sanitised.is_none(), every_token_is_coverage);
    }
}
