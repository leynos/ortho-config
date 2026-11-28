//! Resolution of a clap declaration's value and repetition settings.
//!
//! The rules mirror `clap_derive`'s implicit method emission, with explicit
//! `num_args` bounds and actions taking precedence over the type-derived
//! defaults. They live apart from the builder so each decision stays small
//! enough to review on its own.

use heck::ToShoutySnakeCase;
use syn::Type;

use super::super::ClapRenameAll;
use super::value_syntax::{ValueRange, clap_option_inner, is_bare_bool, is_vec_field};

/// A name that either follows clap's casing rule or is written literally.
#[derive(Debug, Clone)]
pub(super) enum NameChoice {
    /// Derive the spelling from the field's clap argument id.
    Derived,
    /// Use the author's literal spelling.
    Explicit(String),
}

/// A short flag that either follows clap's casing rule or is literal.
#[derive(Debug, Clone, Copy)]
pub(super) enum ShortChoice {
    /// Derive the character from the field's clap argument id.
    Derived,
    /// Use the author's literal character.
    Explicit(char),
}

/// Inputs for resolving a field's flag spellings.
#[derive(Clone, Copy)]
pub(super) struct SpellingInputs<'a> {
    /// Long spelling choice, when `long` was declared.
    pub(super) long: Option<&'a NameChoice>,
    /// Short spelling choice, when `short` was declared.
    pub(super) short: Option<&'a ShortChoice>,
    /// Explicit clap argument id, when one was assigned.
    pub(super) explicit_id: Option<&'a str>,
    /// Field name with the raw-identifier prefix removed.
    pub(super) raw_field: &'a str,
    /// Container-level casing applied to derived spellings.
    pub(super) rename_all: ClapRenameAll,
}

/// Resolves the long and short spellings from the field's clap id.
///
/// A bare `long`/`short` follows the container's `rename_all` casing, as
/// `clap_derive` does. An assigned id is already a literal name and is not
/// re-cased.
pub(super) fn spellings(inputs: SpellingInputs<'_>) -> (Option<String>, Option<char>) {
    let derived = inputs
        .explicit_id
        .map_or_else(|| inputs.rename_all.apply(inputs.raw_field), str::to_owned);
    let long = match inputs.long {
        None => None,
        Some(NameChoice::Explicit(value)) => Some(value.clone()),
        Some(NameChoice::Derived) => Some(derived.clone()),
    };
    let short = match inputs.short {
        None => None,
        Some(ShortChoice::Explicit(value)) => Some(*value),
        Some(ShortChoice::Derived) => derived.chars().next(),
    };
    (long, short)
}

/// Resolves the value placeholder from the declaration or clap's casing.
///
/// `clap_derive` renders an unset value name as the field ident in
/// `SCREAMING_SNAKE_CASE` regardless of the container's `rename_all`, so the
/// placeholder is intentionally independent of that casing rule.
pub(crate) fn value_name(
    explicit_value: Option<&str>,
    explicit_id: Option<&str>,
    raw_field: &str,
) -> String {
    explicit_value.map_or_else(
        || explicit_id.map_or_else(|| raw_field.to_shouty_snake_case(), str::to_owned),
        str::to_owned,
    )
}

/// Whether a value may be omitted for one occurrence.
///
/// clap checks the minimum value count before applying any
/// `default_missing_value`, so a bare flag is valid only when the range admits
/// zero values. `Option<Option<T>>` takes clap's inferred `num_args(0..=1)`;
/// an explicit bound outranks that inference.
pub(super) fn value_optional(takes_value: bool, num_args: Option<ValueRange>, ty: &Type) -> bool {
    takes_value && num_args.map_or_else(|| is_nested_option(ty), |range| range.min == 0)
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
    /// Whether the declared action counts repeated occurrences.
    pub(super) action_counts: bool,
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
        || flags.action_counts
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

/// Signals carried into clap's requiredness decision.
#[derive(Clone, Copy)]
pub(super) struct RequirednessSignals {
    /// Explicit `required` setting, which wins over every inference.
    pub(super) explicit: Option<bool>,
    /// Whether a clap default supplies the value when it is absent.
    pub(super) has_default: bool,
    /// Whether the declared action consumes a value, after arity overrides.
    pub(super) action_takes_value: Option<bool>,
}

/// Resolves clap's requiredness from explicit and inferred signals.
///
/// An argument with no clap default is required, except that `Option` and
/// `Vec` shapes and the value-free actions clap infers for a bare boolean are
/// never required. Requiredness therefore does not follow an `num_args` arity
/// override. The configuration loader's Serde defaults do not relax a clap
/// declaration, so a declared argument must not fall back to those when it is
/// missing a value.
pub(super) fn required(signals: RequirednessSignals, ty: &Type) -> bool {
    if let Some(required) = signals.explicit {
        return required;
    }
    if signals.has_default {
        return false;
    }
    if signals.action_takes_value == Some(false) {
        return false;
    }
    if signals.action_takes_value.is_none() && is_bare_bool(ty) {
        return false;
    }
    !(is_vec_field(ty) || clap_option_inner(ty).is_some())
}
