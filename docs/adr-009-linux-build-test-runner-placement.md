# ADR-009: Place the Linux build-test leg on Ubicloud

Status: Accepted.

Date: 2026-09-29.

## Context and problem statement

The Linux leg of `ci.yml`'s `build-test` ran on GitHub-hosted `ubuntu-latest`.
Over the ten runs before the move its queue wait had a 10-min median and
reached 29 min, against a 27-min median wall, which made it the worst
hosted-queue case in the estate. The Windows leg and the packaging legs did not
show the same cost.

Moving the leg raises four questions: which runner size, what a pull request
from a fork runs on, which check names a branch-protection rule can require,
and how the leg stays inside the disk of the size chosen.

## Decision drivers

- A required check must exist under one name for every event, so a fork's pull
  request and a branch's pull request must report the same contexts.
- A fork cannot obtain an Ubicloud runner, so its leg must fall back to a
  hosted one.
- The cache proxy credentials exist only on Ubicloud runners, and the step that
  exports them fails closed elsewhere.
- Sizing is measured, not assumed: two runs on `ubicloud-standard-2` died of a
  full disk during coverage, the second falling to 791 MB free.

## Options considered

- Stay on `ubuntu-latest`: no change, and the queue wait stays.
- `ubicloud-standard-2`: the estate's starting shape, but its disk is 75 GB and
  the leg exhausted it in both measured runs, even with the lint trees
  discarded before coverage.
- `ubicloud-standard-4`: 150 GB, measured at 18 s queue wait and 19 min 26 s
  wall on its first run.

## Decision outcome

The Linux leg runs on `ubicloud-standard-4`, chosen for disk rather than wall
time. Runner placement is held by `runner_placement_test.py`:

- `runs-on` is a fork fallback read from the matrix, with the fork arm first:
  a fork runs on `ubuntu-latest`, everything else on the owned arm.
- The job is named `build-test (${{ matrix.platform }})`, so the required
  contexts are `build-test (linux)` and `build-test (windows)` on every event.
  Every step keys on `matrix.platform`, never on a runner label.
- The credentials step is guarded on `runner.environment == 'self-hosted'` and
  runs before Setup Rust, which starts the sccache server.
- Before coverage the Linux leg runs `scripts/discard_build_trees.py` to remove
  the lint build trees. The script refuses any name that is not a single
  relative component naming a direct child of the target directory, and reports
  every filesystem failure as one diagnostic with a non-zero status.

## Consequences

- The ruleset's required contexts had to be renamed when the leg merged.
- The size is a contract: changing it means changing `LINUX_RUNNER` in the
  contract, so a resize is a reviewed decision with measurements.
- Ubicloud minutes are billed until the job ends, so the job carries a
  `timeout-minutes` ceiling that the contract requires.
- A fork's Linux leg runs on a hosted runner without the cache proxy; its
  timings are not comparable with an owned run.
- The Windows leg and the packaging legs are unchanged and may move later on
  their own measurements.
