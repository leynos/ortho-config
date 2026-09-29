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

/// Reports the empty spelling separately so callers can return its specific diagnostic.
const fn is_empty_long(long: &str) -> bool {
    long.is_empty()
}

/// Long names omit leading dashes and underscores because clap supplies the prefix.
fn has_invalid_prefix(long: &str) -> bool {
    long.starts_with(['-', '_'])
}

/// Restricts long names to the ASCII spelling accepted by this derive's CLI contract.
fn has_invalid_chars(long: &str) -> bool {
    !long.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Returns the targeted diagnostic for a forbidden leading dash or underscore.
fn invalid_prefix_message(long: &str) -> Option<String> {
    if !has_invalid_prefix(long) {
        return None;
    }
    let prefix = if long.starts_with('-') { '-' } else { '_' };
    Some(format!(
        "invalid `cli_long` '{long}': must not start with '{prefix}'"
    ))
}

/// Produces the first applicable syntax error for a long flag spelling.
fn long_validation_error(long: &str) -> Option<String> {
    if is_empty_long(long) {
        Some(format!("invalid `cli_long` '{long}': must be non-empty"))
    } else if let Some(message) = invalid_prefix_message(long) {
        Some(message)
    } else if has_invalid_chars(long) {
        Some(format!(
            "invalid `cli_long` '{long}': must contain only ASCII alphanumeric characters or '-'"
        ))
    } else {
        None
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
    if let Some(message) = long_validation_error(long) {
        return Err(syn::Error::new_spanned(name, message));
    }
    if RESERVED_LONGS.contains(&long) {
        return Err(syn::Error::new_spanned(
            name,
            format!("reserved `cli_long` '{long}' conflicts with global clap flags"),
        ));
    }
    Ok(())
}
