//! Resolution of a clap declaration's value and repetition settings.
//!
//! The rules mirror `clap_derive`'s implicit method emission, with explicit
//! `num_args` bounds and actions taking precedence over the type-derived
//! defaults. They live apart from the builder so each decision stays small
//! enough to review on its own.

use syn::Type;

use super::value_syntax::{ValueRange, clap_option_inner};

/// Whether a value may be omitted for one occurrence.
///
/// `default_missing_value` supplies a value for the bare spelling, and
/// `Option<Option<T>>` takes clap's inferred `num_args(0..=1)`. An explicit
/// `num_args` bound outranks the type inference.
pub(super) fn value_optional(
    takes_value: bool,
    has_default_missing_value: bool,
    num_args: Option<ValueRange>,
    ty: &Type,
) -> bool {
    takes_value
        && (has_default_missing_value
            || num_args.map_or_else(|| is_nested_option(ty), |range| range.min == 0))
}

/// Signals carried into the repetition decision.
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each flag records an independent clap property."
)]
#[derive(Clone, Copy)]
pub(super) struct RepetitionFlags {
    /// Whether the declared action appends repeated values.
    pub(super) action_appends: bool,
    /// Whether the declared action replaces rather than accumulates values.
    pub(super) action_set: bool,
    /// Whether the field's type is a vector.
    pub(super) is_vec: bool,
    /// Whether the field is a positional operand.
    pub(super) is_positional: bool,
}

/// Whether the argument accepts repeated occurrences or values.
///
/// A `Vec` option repeats across occurrences through clap's inferred
/// `ArgAction::Append`, even when `num_args` bounds each occurrence to one
/// value; an explicit `Set` action replaces rather than accumulates. A bounded
/// positional consumes its values in one occurrence, so only a multi-value
/// bound makes it repeated.
pub(super) fn multiple(flags: RepetitionFlags, num_args: Option<ValueRange>) -> bool {
    let repeats_per_occurrence = num_args.is_some_and(|range| range.max > 1);
    let repeats_across_occurrences = flags.action_appends
        || (!flags.action_set && flags.is_vec && (!flags.is_positional || num_args.is_none()));
    repeats_across_occurrences || repeats_per_occurrence
}

/// Whether a positional accepts more than one value.
///
/// An explicit `num_args` bound replaces clap's implicit `1..` for a `Vec`
/// positional, so a bound of one value is not variadic.
pub(super) const fn variadic(is_vec: bool, num_args: Option<ValueRange>) -> bool {
    match num_args {
        Some(range) => range.max > 1,
        None => is_vec,
    }
}

/// Whether a type is `Option<Option<T>>`.
///
/// `clap_derive` infers `num_args(0..=1)` for this shape, so a bare flag is a
/// valid spelling even though the type is not a plain boolean.
fn is_nested_option(ty: &Type) -> bool {
    clap_option_inner(ty).is_some_and(|inner| clap_option_inner(inner).is_some())
}
