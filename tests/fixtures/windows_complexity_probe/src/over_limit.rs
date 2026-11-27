//! The Windows-only example the gate must report.

/// Accumulates a score with ten sibling decisions.
///
/// The metric adds one for each `if` in a function body, so ten of them score
/// above the configured limit of 9, which is the failure the probe requires.
/// The ladder is deliberately flat rather than nested, so the score says
/// exactly what the gate counts — a decision — and nothing about where it
/// sits. Only the feature-enabled build compiles this module, which is what
/// lets one crate prove both directions.
pub fn over_limit(value: u8) -> u8 {
    let mut total = 0;

    if value > 0 {
        total += 1;
    }

    if value > 1 {
        total += 2;
    }

    if value > 2 {
        total += 3;
    }

    if value > 3 {
        total += 4;
    }

    if value > 4 {
        total += 5;
    }

    if value > 5 {
        total += 6;
    }

    if value > 6 {
        total += 7;
    }

    if value > 7 {
        total += 8;
    }

    if value > 8 {
        total += 9;
    }

    if value > 9 {
        total += 10;
    }

    total
}