//! Check that a command's deadline is fixed before the child is spawned.
//!
//! The instant used to be computed after the spawn and the reader setup, so
//! that work was charged to nobody: a sequence deadline with a fraction of a
//! second left still handed the child its whole command budget afterwards.
//! These cases pass `now` explicitly and step it forward, which is exactly what
//! a slow spawn looks like to the selector.

use std::time::{Duration, Instant};

use super::{COMMAND_SCOPE, SEQUENCE_SCOPE, effective_deadline};

/// A command budget long enough that the sequence deadline is the tight one.
const GENEROUS: Duration = Duration::from_secs(30);

/// How much of the sequence budget a slow spawn consumes.
const SLOW_SPAWN: Duration = Duration::from_millis(400);

/// Total sequence budget in the slow-spawn case.
const SEQUENCE_BUDGET: Duration = Duration::from_millis(500);

#[test]
fn a_command_budget_is_measured_from_the_callers_now() {
    let start = Instant::now();

    let deadline = effective_deadline(GENEROUS, None, start, "probe").expect("a generous budget");

    assert_eq!(deadline.at, start + GENEROUS);
    assert_eq!(deadline.duration, GENEROUS);
    assert_eq!(deadline.scope, COMMAND_SCOPE);
}

#[test]
fn an_earlier_sequence_deadline_wins() {
    let start = Instant::now();
    let sequence = start + SEQUENCE_BUDGET;

    let deadline =
        effective_deadline(GENEROUS, Some(sequence), start, "probe").expect("a sequence deadline");

    assert_eq!(deadline.at, sequence);
    assert_eq!(deadline.duration, SEQUENCE_BUDGET);
    assert_eq!(deadline.scope, SEQUENCE_SCOPE);
}

#[test]
fn a_later_sequence_deadline_leaves_the_command_budget_alone() {
    let start = Instant::now();

    let deadline = effective_deadline(GENEROUS, Some(start + GENEROUS * 2), start, "probe")
        .expect("an ample sequence deadline");

    assert_eq!(deadline.at, start + GENEROUS);
    assert_eq!(deadline.duration, GENEROUS);
    assert_eq!(deadline.scope, COMMAND_SCOPE);
}

#[test]
fn spawn_time_is_charged_to_the_sequence_deadline() {
    let start = Instant::now();
    let sequence = start + SEQUENCE_BUDGET;

    let prompt = effective_deadline(GENEROUS, Some(sequence), start, "probe").expect("a budget");
    let delayed = effective_deadline(GENEROUS, Some(sequence), start + SLOW_SPAWN, "probe")
        .expect("a budget");

    assert_eq!(delayed.at, prompt.at, "the absolute instant does not move");
    assert_eq!(
        delayed.duration,
        SEQUENCE_BUDGET - SLOW_SPAWN,
        "the spawn is part of the budget, not free time before it",
    );
}

#[test]
fn a_budget_that_overflows_the_clock_is_refused() {
    let start = Instant::now();

    assert!(effective_deadline(Duration::MAX, None, start, "probe").is_none());
}
