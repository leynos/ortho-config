//! Rust flag filtering for bridge builds.

use std::process::Command;

const ENCODED_RUSTFLAGS_SEPARATOR: char = '\x1f';

pub(crate) fn apply_sanitized_rustflags(command: &mut Command) {
    apply_sanitized_rustflags_var(command, "RUSTFLAGS", sanitize_plain_rustflags);
    apply_sanitized_rustflags_var(
        command,
        "CARGO_ENCODED_RUSTFLAGS",
        sanitize_encoded_rustflags,
    );
}

fn apply_sanitized_rustflags_var(
    command: &mut Command,
    name: &str,
    sanitize: fn(&str) -> Option<String>,
) {
    let Ok(value) = std::env::var(name) else {
        return;
    };
    match sanitize(&value) {
        Some(sanitized) => {
            command.env(name, sanitized);
        }
        None => {
            command.env_remove(name);
        }
    }
}

fn sanitize_plain_rustflags(flags: &str) -> Option<String> {
    let (tokens, was_changed) = filtered_rustflag_tokens(flags.split_whitespace());
    if was_changed {
        non_empty_join(&tokens, " ")
    } else {
        Some(flags.to_owned())
    }
}

fn sanitize_encoded_rustflags(flags: &str) -> Option<String> {
    let (tokens, was_changed) = filtered_rustflag_tokens(flags.split(ENCODED_RUSTFLAGS_SEPARATOR));
    if was_changed {
        non_empty_join(&tokens, &ENCODED_RUSTFLAGS_SEPARATOR.to_string())
    } else {
        Some(flags.to_owned())
    }
}

fn filtered_rustflag_tokens<'a>(tokens: impl IntoIterator<Item = &'a str>) -> (Vec<&'a str>, bool) {
    let mut filtered = Vec::new();
    let mut was_changed = false;
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if is_instrument_coverage_codegen_flag(token) {
            was_changed = true;
            continue;
        }
        if token == "-C"
            && iter
                .peek()
                .is_some_and(|next| is_instrument_coverage_option(next))
        {
            let _ = iter.next();
            was_changed = true;
            continue;
        }
        filtered.push(token);
    }
    (filtered, was_changed)
}

fn non_empty_join(tokens: &[&str], separator: &str) -> Option<String> {
    (!tokens.is_empty()).then(|| tokens.join(separator))
}

fn is_instrument_coverage_codegen_flag(token: &str) -> bool {
    token
        .strip_prefix("-C")
        .is_some_and(is_instrument_coverage_option)
}

fn is_instrument_coverage_option(token: &str) -> bool {
    token == "instrument-coverage" || token.starts_with("instrument-coverage=")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(
        "  --cfg   caller_gate   -Clink-arg=/SAFESEH:NO  ",
        Some("  --cfg   caller_gate   -Clink-arg=/SAFESEH:NO  ".to_owned())
    )]
    #[case(
        "--cfg caller_gate -Clink-arg=/SAFESEH:NO",
        Some("--cfg caller_gate -Clink-arg=/SAFESEH:NO".to_owned())
    )]
    #[case(
        "-Cinstrument-coverage --cfg caller_gate",
        Some("--cfg caller_gate".to_owned())
    )]
    #[case(
        "-C instrument-coverage --cfg caller_gate",
        Some("--cfg caller_gate".to_owned())
    )]
    #[case("-Cinstrument-coverage", None)]
    fn plain_rustflags_preserve_user_flags_while_stripping_coverage(
        #[case] flags: &str,
        #[case] expected: Option<String>,
    ) {
        assert_eq!(sanitize_plain_rustflags(flags), expected);
    }

    #[test]
    fn encoded_rustflags_preserve_user_flags_while_stripping_coverage() {
        let separator = ENCODED_RUSTFLAGS_SEPARATOR.to_string();
        let flags = [
            "-Cinstrument-coverage",
            "--cfg",
            "caller_gate",
            "-C",
            "link-arg=/SAFESEH:NO",
        ]
        .join(&separator);
        let expected = ["--cfg", "caller_gate", "-C", "link-arg=/SAFESEH:NO"].join(&separator);

        assert_eq!(sanitize_encoded_rustflags(&flags), Some(expected));
    }
}
