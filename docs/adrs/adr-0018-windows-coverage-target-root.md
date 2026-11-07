# Architectural decision record (ADR) 0018: Keep the Windows coverage target root under the CreateProcess limit

## Status

Accepted.

Date: 2026-11-07.

## Context and problem statement

The Windows coverage lane failed when a `cargo-orthohelp` bin-test compile
produced a command line of 33,406 characters. Windows rejects a whole command
line above 32,767 characters, so `CreateProcess` refused the invocation and
rustc reported `os error 206` without compiling anything.

Cargo repeats the target directory throughout the arguments it hands to each
rustc invocation; in the measured command the root appeared 251 times. Trimming
one argument therefore cannot help. The root itself has to be shorter.

The Linux leg has no comparable limit. `MAX_ARG_STRLEN` bounds a single
argument there, and the leg's headroom is an order of magnitude larger. A fix
that also redirected Linux would discard warm build reuse for no benefit.

## Decision Drivers

- The lane must still compile the same targets. Dropping a target to shorten
  the command line would reduce what coverage measures.
- Both Windows coverage passes must share one root, so the second pass reuses
  the trybuild builds the first produced.
- Later steps must see the workspace's default target root again.
- Linux must keep `CARGO_TARGET_DIR` genuinely unset, because an empty value
  is not equivalent to an unset variable.
- The margin must survive dependency growth rather than only fitting today.

## Options considered

### Option A: Redirect the Windows root to a short temporary directory

Export `CARGO_TARGET_DIR` as `$RUNNER_TEMP\cov` before the first coverage pass
and restore `$GITHUB_WORKSPACE\target` after the second. On the hosted runner,
cargo-llvm-cov derives `D:\a\_temp\cov\llvm-cov-target`, a 30-character root
that projects the command line to 27,633 characters and leaves 5,134 characters
of headroom.

### Option B: Shorten the workspace path

Move or rename the checkout so the workspace root is shorter. The path is set
by the runner, and changing it would affect every job rather than the single
lane that needs it.

### Option C: Reduce the argument count

Split the bin-test target or move tests out of it. That changes what the lane
measures, and the argument list would grow again with the next target added.

### Option D: Leave the lane as it is

Accept the failure. It is deterministic rather than flaky, so the lane would
stay red and the repository would lose its Windows coverage evidence.

| Topic                 | Option A | Option B | Option C | Option D |
| --------------------- | -------- | -------- | -------- | -------- |
| Fits the limit        | Yes      | Yes      | Yes      | No       |
| Preserves the targets | Yes      | Yes      | No       | Yes      |
| Scoped to one lane    | Yes      | No       | No       | Yes      |
| Keeps build reuse     | Yes      | Yes      | No       | Yes      |

_Table 1: Comparison of the options against the decision drivers._

## Decision outcome

Adopt Option A. Two Windows-only PowerShell steps bracket both coverage passes.
The export step writes `CARGO_TARGET_DIR=$RUNNER_TEMP\cov` to `GITHUB_ENV`, and
the restore step writes `CARGO_TARGET_DIR=$GITHUB_WORKSPACE\target`. Each step
also logs the root it resolved to the step output and the job summary, so a
failing run shows which root was in effect.

`GITHUB_ENV` cannot unset a variable, so the restore writes the workspace
default explicitly rather than deleting the entry. The restore is not an
`always()` step: if a coverage pass fails, the later Cargo consumers are
skipped anyway, and the always-running sccache statistics step does not build.

Linux keeps `CARGO_TARGET_DIR` genuinely unset. No workflow, job, or step
`env:` block declares it, and no other run block mentions it.

`tests/workflow_contracts/windows_coverage_root_test.py` holds the contract. It
pins the two steps, their Windows-only guard and `pwsh` shell, their exact
writes, and their positions around lint, both coverage actions, wrapper
validation, and the publish dry run. It models `GITHUB_ENV` propagation across
the step sequence to assert the effective root before, during, and after each
coverage pass on Windows, and that it stays unset throughout on Linux. Its
arithmetic holds the 30-character hosted root and at least 4,000 characters of
headroom, including the measured 251 root occurrences.

## Goals and non-goals

- Goals:
  - Keep the Windows coverage command line below the `CreateProcess` limit
    with a margin that survives dependency growth.
  - Preserve one warm target root across both coverage passes.
  - Leave the Linux environment unchanged.
- Non-goals:
  - Changing which targets the coverage lane builds.
  - Shortening the workspace path for every job.
  - Introducing a Windows-specific coverage tool.

## Known risks and limitations

The projection assumes the hosted runner's temporary directory is `D:\a\_temp`,
which is where the measured run placed it. A different temporary root lengthens
the projection by the same number of characters, and the contract test's
headroom assertion is what makes that visible.

The margin also assumes the number of root occurrences stays near the measured
251. A large increase in rustc invocations per compile could consume it; the
recorded 4,000-character floor is the deliberate buffer for that growth.

## Architectural rationale

The decision keeps a platform-specific constraint inside the platform-specific
leg. Linux keeps the behaviour its measurements justify, Windows gets the
shortest root the runner offers, and neither leg carries the other's
workaround. Expressing the rule as a workflow contract means a later edit that
moves a coverage action outside the guarded pair, or that reintroduces a
workflow-level environment override, fails the lane rather than silently
reinstating the defect.
