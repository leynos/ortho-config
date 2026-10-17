//! Validate CLI spellings and allocate unique short flags.
//!
//! Long names follow the derive's documented ASCII contract. Short-name
//! allocation walks fields in declaration order and reserves clap's global
//! help and version flags before user fields are considered.

use std::collections::HashSet;

use syn::Ident;

use crate::derive::parse::FieldAttrs;

/// Short flags owned by clap's generated global help and version arguments.
pub(super) const RESERVED_SHORTS: &[char] = &['h', 'V'];
/// Long flags owned by clap's generated global help and version arguments.
const RESERVED_LONGS: &[&str] = &["help", "version"];

/// Borrows one field and its raw long spelling for a single validation pass.
///
/// Construction does not certify the spelling: callers must use `validate` before
/// treating it as a long flag accepted by this derive.
struct LongFlagCandidate<'a> {
    /// Keeps diagnostics anchored to the source field without taking ownership.
    field: &'a Ident,
    /// Retains the original spelling so validation errors preserve exact text.
    spelling: &'a str,
}

impl LongFlagCandidate<'_> {
    /// Identifies a leading dash or underscore forbidden by the long-name grammar.
    ///
    /// For example, `-alpha` yields `Some('-')`, while `alpha` yields `None`.
    fn invalid_prefix(&self) -> Option<char> {
        self.spelling
            .chars()
            .next()
            .filter(|&prefix| matches!(prefix, '-' | '_'))
    }

    /// Reports characters outside the derive's ASCII alphanumeric-or-dash grammar.
    ///
    /// For example, `alpha-1` passes this check, while `alpha_beta` does not.
    fn has_invalid_characters(&self) -> bool {
        !self
            .spelling
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    }

    /// Returns the first syntax diagnostic in empty, prefix, then character order.
    ///
    /// For example, `-bad/flag` reports the leading dash before the slash.
    fn syntax_error(&self) -> Option<String> {
        if self.spelling.is_empty() {
            Some(format!(
                "invalid `cli_long` '{}': must be non-empty",
                self.spelling
            ))
        } else if let Some(prefix) = self.invalid_prefix() {
            Some(format!(
                "invalid `cli_long` '{}': must not start with '{prefix}'",
                self.spelling
            ))
        } else if self.has_invalid_characters() {
            Some(format!(
                "invalid `cli_long` '{}': must contain only ASCII alphanumeric characters or '-'",
                self.spelling
            ))
        } else {
            None
        }
    }

    /// Validates syntax before reserved names and attaches failures to the field span.
    ///
    /// For example, `alpha-1` succeeds, while `help` returns a reserved-name error.
    fn validate(&self) -> syn::Result<()> {
        if let Some(message) = self.syntax_error() {
            return Err(syn::Error::new_spanned(self.field, message));
        }
        if RESERVED_LONGS.contains(&self.spelling) {
            return Err(syn::Error::new_spanned(
                self.field,
                format!(
                    "reserved `cli_long` '{}' conflicts with global clap flags",
                    self.spelling
                ),
            ));
        }
        Ok(())
    }
}

/// Resolves a short CLI flag ensuring uniqueness and validity.
///
/// # Examples
///
/// Validates a user-supplied short flag and records it if free.
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::validate_user_cli_short;
/// use syn::parse_quote;
///
/// let name: syn::Ident = parse_quote!(field);
/// let mut used = HashSet::new();
/// let ch = validate_user_cli_short(&name, 'f', &used).expect("short flag");
/// used.insert(ch);
/// assert_eq!(ch, 'f');
/// ```
pub(in crate::derive::build::cli) fn validate_user_cli_short(
    name: &Ident,
    user: char,
    used_shorts: &HashSet<char>,
) -> syn::Result<char> {
    if !user.is_ascii_alphanumeric() {
        return Err(syn::Error::new_spanned(
            name,
            format!("invalid `cli_short` '{user}': must be ASCII alphanumeric"),
        ));
    }
    if RESERVED_SHORTS.contains(&user) {
        return Err(syn::Error::new_spanned(
            name,
            format!("reserved `cli_short` '{user}' conflicts with global flags"),
        ));
    }
    if used_shorts.contains(&user) {
        return Err(syn::Error::new_spanned(name, "duplicate `cli_short` value"));
    }
    Ok(user)
}

/// Chooses an explicit short flag or the first available letter from the field name.
///
/// Explicit and derived values are inserted into `used_shorts` only after validation,
/// so later fields observe a complete reservation set and duplicate flags fail early.
pub(in crate::derive::build::cli) fn resolve_short_flag(
    name: &Ident,
    attrs: &FieldAttrs,
    used_shorts: &mut HashSet<char>,
) -> syn::Result<char> {
    if let Some(user) = attrs.cli_short {
        let ch = validate_user_cli_short(name, user, used_shorts)?;
        used_shorts.insert(ch);
        return Ok(ch);
    }

    let derived = name
        .to_string()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(|c| [c.to_ascii_lowercase(), c.to_ascii_uppercase()])
        .find(|candidate| !RESERVED_SHORTS.contains(candidate) && !used_shorts.contains(candidate))
        .ok_or_else(|| {
            syn::Error::new_spanned(
                name,
                "unable to derive a short flag; supply `cli_short` to disambiguate",
            )
        })?;
    used_shorts.insert(derived);
    Ok(derived)
}

/// Rejects malformed long names, including clap's reserved global help and version flags.
pub(in crate::derive::build::cli) fn validate_cli_long(
    name: &Ident,
    long: &str,
) -> syn::Result<()> {
    LongFlagCandidate {
        field: name,
        spelling: long,
    }
    .validate()
}
