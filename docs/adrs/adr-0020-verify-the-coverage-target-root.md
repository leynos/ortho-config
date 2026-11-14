# Architectural decision record (ADR) 0020: Verify the coverage target root in use

## Status

Accepted.

Date: 2026-11-14.

## Context and problem statement

Issue #612 recorded a Windows coverage failure that no local gate could see. The
`ortho_config` library test's rustc invocation reached 33,497 characters and
`CreateProcess` refused it with `os error 206` before the compiler ran. Cargo
repeats the target root 302 times in that invocation, so the root is the lever:
the platform choosers recorded in
[ADR-0018](adr-0018-adopt-a-dated-nightly-and-the-development-build-standard.md)
now set `CARGO_TARGET_DIR` and `CARGO_LLVM_COV_TARGET_DIR` to
`D:\llvm-cov-target` on Windows, and both variables to
`$RUNNER_TEMP/llvm-cov-target` on Linux.

Those steps publish a root to `GITHUB_ENV`. Publishing is not receipt. A later
edit that adds a step-level `env`, that reorders a chooser after the actions,
or that lets a platform's value arrive empty would leave the chooser looking
correct while the coverage processes built somewhere else — and the first
evidence would again be a refused command line on one leg. Reading the
variables back, and requiring the derived directory to exist, turns that class
of edit into a failure where it happens.

## Decision Drivers

- The failure is observable only on the hosted runner, so the check must run
  there rather than in a local gate.
- The check must not change what coverage measures, add a target, or rerun a
  test.
- It must not become a second source of truth for the root. A hardcoded path
  would drift from the chooser and disagree with it silently.
- It must distinguish a variable that is set to the wrong value from one that
  is absent, because `cargo-llvm-cov` rejects an empty value rather than
  treating it as unset.
- A passing run should say which root it observed.

## Options considered

### Option A: Read the environment back after both coverage passes

One step per platform reads both variables after the second coverage action and
requires the directory the coverage action built in. The expected value is
recomputed from the same expression the chooser uses, so the two cannot drift:
the Windows step re-derives the drive root from `RUNNER_TEMP`, and the Linux
step rebuilds `$RUNNER_TEMP/llvm-cov-target`.

### Option B: Trust the chooser and keep only the contract tests

The workflow contracts already pin the chooser's condition, shell, writes and
ordering. They read the workflow as text, so they cannot observe what a hosted
process received; that is exactly the gap the original failure fell through.

### Option C: Log the exported root only

Logging what a chooser wrote repeats the intent, not the observation, and a log
line cannot fail a lane.

### Option D: Verify the command-line length directly

Measuring the real invocation needs the Windows runner, a successful compile,
and the length of a per-unit command line the action does not expose. The
projection contract derives it from the measured invocation instead; that
arithmetic cannot be re-run against a live process without changing the
coverage lane's tooling.

| Topic                      | Option A | Option B | Option C | Option D |
| -------------------------- | -------- | -------- | -------- | -------- |
| Observes the host          | Yes      | No       | No       | Yes      |
| Fails a lane               | Yes      | Yes      | No       | Yes      |
| Tracks the chooser         | Yes      | Yes      | Yes      | No       |
| Costs no extra compilation | Yes      | Yes      | Yes      | No       |

_Table 1: Comparison of the options against the decision drivers._

## Decision outcome

Adopt Option A. Two steps named `Verify the Windows coverage target root` and
`Verify the Linux coverage target root` follow the second coverage pass. Each
reads both variables, compares them with the root its chooser derives, and
requires that directory to exist. Each writes the root it observed to the step
summary, so a passing run records the value rather than the expectation.

What the steps observe is bounded, and their success line says so: "Chosen
coverage target root exists and the published variables match". Both coverage
passes inherit one job environment, so the variables record what each pass was
given, not what it did with it, and the derived directory is written by
whichever pass runs first. A pass that built elsewhere would not change either
check. Distinguishing the passes would need evidence from each coverage
invocation, which is Option D's cost; the projection contract remains the guard
for the failure this ADR responds to.

The Linux step is plain Bash over `$RUNNER_TEMP`, so its acceptance and
rejection paths are exercised by the contract suite on every host. The Windows
step derives its root from the drive containing `RUNNER_TEMP`; on a POSIX host
that drive is `/`, so the suite observes its two rejection paths there and
leaves acceptance to the hosted Windows leg, which runs the step with the real
directory present. `coverage_target_root_behaviour_test.py` says so in its
module comment rather than leaving the gap implicit.

`coverage_target_root_verification_test.py` pins the steps' platform
conditions, shells, positions after both coverage actions, the variables they
read, the expression they share with the chooser, the directory they require,
and the summary they write. It also asserts that no workflow, job or step `env`
block declares either variable, because such a block would give a consumer a
value the check never inspects.

## Goals and non-goals

- Goals:
  - Fail the lane that can see a coverage root going astray.
  - Derive the expected root from the chooser's own expression.
  - Report the observed root on success.
- Non-goals:
  - Re-measuring command-line length at run time.
  - Changing which targets coverage builds.
  - Replacing the projection arithmetic in `coverage_command_line_test.py`.

## Known risks and limitations

The check observes the environment and the directory, not the length of any
command line. A root that is selected correctly and still overflows would pass
these steps and fail at the compiler; the projection contract is the guard for
that, and it works from measured numbers rather than from the runner.

The Windows acceptance path has no local observer. A defect that made the
Windows step fail only when the directory exists would be caught by the hosted
Windows leg on the first run rather than by a Linux gate.

The check trusts the coverage action not to rewrite either variable after the
chooser. `generate-coverage` reads them from the environment and does not write
`GITHUB_ENV`; an action that began to do so would fail these steps, which is
the intended direction for that change.

## Architectural rationale

The workflow already decides the root once, per platform. The verification
reads that decision back instead of restating it, so there is still one place
to change and one place that notices. Keeping the check inside each leg means a
Windows-only constraint is answered on Windows and a Linux-only one on Linux,
and a failure names the variable and the directory rather than the symptom the
failure eventually produced.
