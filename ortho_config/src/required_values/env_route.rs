//! Spelling rules that decide whether an environment route is truthful.
//!
//! The provider derives a variable's key by trimming its name, stripping the
//! prefix, uppercasing the remainder, splitting it on `__`, and lowercasing
//! each part. A variable is advertised only when replaying those transforms
//! reproduces the key path exactly; otherwise the suggested variable would
//! populate a different key, or name something no process can hold.

use crate::error::ConfigPath;

/// Render the environment variable the configured provider reads, if any.
///
/// A route is offered only when the provider's transforms leave the key path
/// unchanged. Both the segments and the path they form are checked, because the
/// provider splits the *joined* name: a segment edge can merge with the
/// separator and produce a path no individual segment check can predict. The
/// prefix is checked as part of the completed name, because the provider trims
/// a variable's name before matching the prefix and because a prefix can carry
/// a byte no name may contain.
pub(super) fn environment_variable(path: &ConfigPath, prefix: Option<&str>) -> Option<String> {
    let segments = path.segments();
    if !segments.iter().all(|segment| is_expressible(segment)) {
        return None;
    }
    let joined = segments.join("__");
    if !provider_reads_the_same_path(&joined, segments) {
        return None;
    }
    let variable = joined.to_ascii_uppercase();
    let name = match prefix {
        Some(configured) => format!("{configured}{variable}"),
        None => variable,
    };
    name_can_be_observed(&name).then_some(name)
}

/// Report whether a completed variable name can exist in a process environment.
///
/// The provider trims each variable's name before it applies the prefix
/// transform, and trims the remainder afterwards, so a completed name with
/// whitespace at either end is never observed: a leading space breaks the
/// prefix match, and a trailing one is removed before the key is derived, so
/// the key no longer ends the way the path does. The two byte exclusions match
/// [`is_expressible`]'s: `=` separates name from value and NUL terminates the C
/// string, so neither can appear in any environment at all. Checking the
/// completed name subsumes the per-component prefix cases.
fn name_can_be_observed(name: &str) -> bool {
    !name.is_empty() && name == name.trim() && !name.bytes().any(|byte| byte == b'=' || byte == 0)
}

/// Replay the provider's split transform and require it to reproduce `path`.
///
/// The injected provider replaces *every* `__` in the joined name with a dot,
/// then lowercases. An underscore at a segment edge therefore merges with the
/// separator: joining `database_` and `host` gives `database___host`, which the
/// provider reads as `database` plus `_host`. Advertising `APP_DATABASE___HOST`
/// for `database_.host` would send the user to a variable that supplies a
/// different key, so such a path gets no environment route at all.
fn provider_reads_the_same_path(joined: &str, segments: &[String]) -> bool {
    joined
        .replace("__", ".")
        .split('.')
        .eq(segments.iter().map(String::as_str))
}

/// Report whether one path segment survives the provider's key transforms.
///
/// A literal dot nests instead of naming one key, an embedded `__` splits into
/// two segments, and surrounding whitespace is trimmed. The final lowercase
/// transform rewrites any ASCII uppercase letter.
///
/// Two byte values are rejected for a different reason: they cannot appear in
/// a variable *name* at all. Every process environment treats `=` as the
/// separator between name and value, so `APP_A=B` names `APP_A` and carries
/// `B`; a NUL terminates the C string the platform call receives. Advertising
/// either spelling would name a variable no loader can ever observe.
fn is_expressible(segment: &str) -> bool {
    !segment.is_empty()
        && segment == segment.trim()
        && !segment.contains('.')
        && !segment.contains("__")
        && !segment
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte == b'=' || byte == 0)
}
