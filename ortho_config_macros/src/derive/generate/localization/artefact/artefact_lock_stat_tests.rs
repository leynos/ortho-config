//! Parsing `/proc/<pid>/stat` into a command name and a parent.
//!
//! The rule this pins is the reason the parser does not simply split the line:
//! a process may be called `a) b`, and a parser that stopped at the *first*
//! closing parenthesis would read the parent out of the command name. Nothing
//! in the real-procfs tests can show that — a process cannot rename itself to
//! `a) b` on cue, and the only assertion they make about a command is that it
//! is non-empty. So the rule is tested here, against literal stat lines.
//!
//! Split from the procfs module's own file for the file-length cap.

use anyhow::Result;

use super::lock_procfs::parse_stat;

/// Verifies the command is read from the last closing parenthesis.
///
/// `comm` may contain both spaces and parentheses, and the kernel does not
/// escape them. Locating the fields after the command from the *first* `)`
/// therefore splits a process called `a) b` at the wrong place: the "state"
/// would be `b`, and the "parent" would be read from whatever followed the
/// real state. The case below is built so that reading fails outright, which
/// is the loudest form the defect can take.
#[test]
fn a_command_containing_a_parenthesis_is_not_split_at_it() -> Result<()> {
    use anyhow::ensure;

    let parsed = parse_stat("1234 (a) b) S 42 1234 1234 0 -1")
        .expect("a command containing ')' and a space is still a well-formed stat line");

    ensure!(
        parsed.command() == "a) b",
        "the command must be everything between the first '(' and the last \
         ')'; a parser that stopped at the first ')' would read {:?}",
        parsed.command(),
    );
    ensure!(
        parsed.parent().to_string() == "42",
        "the parent must come from the field after the state, not from inside \
         the command name; got {}",
        parsed.parent(),
    );
    Ok(())
}

/// Verifies a command containing spaces is kept whole.
///
/// The simpler half of the same rule, and the one that catches a parser that
/// splits the line on whitespace before looking for the parentheses.
#[test]
fn a_command_containing_spaces_is_kept_whole() -> Result<()> {
    use anyhow::ensure;

    let parsed = parse_stat("7 (cargo test) R 1 7 7 0 -1")
        .expect("a command containing a space is still a well-formed stat line");

    ensure!(
        parsed.command() == "cargo test",
        "the whole command must survive, spaces included; got {:?}",
        parsed.command(),
    );
    ensure!(
        parsed.parent().to_string() == "1",
        "the parent must still be read from its own field; got {}",
        parsed.parent(),
    );
    Ok(())
}

/// Verifies a malformed stat line yields nothing rather than a wrong answer.
///
/// Every one of these is a line the parser cannot read the parent from, and
/// `None` is the answer the caller turns into an "unknown" relation. Returning
/// a `ProcessStat` built from a guess would name a process by a parent it does
/// not have, which is worse than saying the relation is unknown.
///
/// Each case names the field it removes, so a failure says which rule broke
/// rather than only that some line was rejected.
#[test]
fn a_malformed_stat_line_yields_nothing() {
    for (label, line) in [
        ("no parentheses at all", "1234 S 42"),
        ("no closing parenthesis", "1234 (cargo S 42"),
        ("no state field", "1234 (cargo)"),
        ("no parent field", "1234 (cargo) S"),
        ("non-numeric parent", "1234 (cargo) S not-a-pid"),
    ] {
        assert!(
            parse_stat(line).is_none(),
            "a stat line with {label} cannot be parsed and must not be \
             guessed at: {line:?}",
        );
    }
}
