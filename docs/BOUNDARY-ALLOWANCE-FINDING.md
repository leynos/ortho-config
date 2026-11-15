# Finding: the localization boundary contract fails on wall-clock deadlines

Status: reproduced on unmodified `origin/main`; pre-existing and
load-sensitive. Raised 2026-11-13 during the Clippy beta-lint repair tracked as
issue #674.

## Claim

`ortho_l10n`'s `boundary` integration test fails whenever nested Cargo
invocations cannot complete inside fixed wall-clock deadlines. It is not caused
by any of that day's source changes, and it is not new.

## Evidence

Runs 1-3 were taken on a working branch that additionally carried the one-file
`map_or` repair for issue #674. Run 4 is the control: an unmodified
`origin/main` in a detached worktree. That repair was later superseded by an
identical change on `main` (PR #616), so none of this document depends on it.
The runs are retained with their original counts because they are the record of
what was observed.

| Run | Tree                                              | Command                                    | Result                                                                                                                                                                                                                          |
| --- | ------------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | repair branch                                     | `make test`                                | `identifier_artefact_lease_cargo_build_tests::a_real_cargo_build_keeps_the_lease_after_its_helper_dies` FAILED: "timed out waiting for the second test process to write `build_started`". The same test passes alone in 78.21s. |
| 2   | repair branch                                     | `make test` (rerun)                        | `ortho_l10n --test boundary`: 26 passed / 18 failed, 909.06s                                                                                                                                                                    |
| 3   | repair branch                                     | `cargo test -p ortho_l10n --test boundary` | 32 passed / 12 failed, 909.05s                                                                                                                                                                                                  |
| 4   | `origin/main` `9be62487`, detached clean worktree | `cargo test -p ortho_l10n --test boundary` | 32 passed / 12 failed, 909.06s                                                                                                                                                                                                  |

Run 4 decides it. That worktree contains none of the repair branch's changes,
and it reproduces the same failing set with the same counts and the same
elapsed time.

Run 1 and runs 2-3 fail differently, which is itself informative: run 1's
failure is a different test, in a different crate, timing out on the same
shared host. Both shapes are load-domain failures rather than assertion
failures.

## Mechanism

`ortho_l10n/tests/boundary/fixture_cargo/budget.rs` defines:

```rust
pub(crate) const SCENARIO_ALLOWANCE: Duration = Duration::from_secs(900);
```

Each contract receives a shared 900s allowance; individual nested Cargo calls
also carry their own caps (70s, 262s, 8s, 1s observed). Every failure has one
of two shapes:

```text
(bounded by the boundary contract's shared allowance) exceeded the N second
deadline; its process tree was killed
```

```text
the boundary contract's 900 second allowance was exhausted while <command>
waited for a sibling nested Cargo invocation to finish
```

The suite runs `cargo package`, `cargo metadata`, and `cargo test` in foreign
fixture roots. On this host those queue behind compile admission
(`build-limits-rustc`, a shared pool of 4) alongside every other agent,
repository and worktree. Observed during these runs: load average 10.6-16.3 on
24 cores, with more than 200 compiler requests queued at peak.

The elapsed time is pinned at 909.0s in both the branch run and the clean-main
run. That is the signature of a suite measuring wall-clock time the machine
decides, rather than a property of the code under test.

## Why it matters beyond a flake

1. It is not intermittent in the usual sense. Every recorded run failed, and
   the failures track host load rather than code changes; the observed elapsed
   times sit just past the 900s allowance rather than varying at random. That
   makes it a plausible signal of host load and an unreliable signal of
   correctness. The complementary claim — that the suite **passes** on an idle
   host — is a hypothesis, not a recorded result: no idle-host run appears in
   this document, and validating it is outstanding work.
2. `make test` runs `cargo test` without `--no-fail-fast`. The first failing
   binary aborts the rest, so later targets go uncertified. On run 1 that hid
   roughly 43 binaries.
3. The assertions concern package closure and dependency edges, not timing. A
   deadline kill destroys exactly the evidence the test exists to produce.

## Suggested direction (not implemented here)

Separate the two concerns. Keep the closure and dependency-edge assertions,
which are the real contract, and make their failure mode independent of host
load: run the nested builds against pre-warmed fixture roots, budget on
progress signals from the nested process instead of elapsed time, or move the
wall-clock part into a dedicated serialized lane with a generous budget on an
idle runner.

Independently, `make test` should pass `--no-fail-fast` so one red binary
cannot silently de-certify every later target.

## Reproduction

The control revision is pinned. `origin/main` advances, and a moving reference
would silently test a different tree from the one recorded above.

```sh
git worktree add /tmp/baseline 9be62487 --detach
cd /tmp/baseline
cargo test -p ortho_l10n --test boundary
# test result: FAILED. 32 passed; 12 failed; 10 ignored; ... 909.0s
```
