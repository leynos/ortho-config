# Separate the boundary suite's assertions from its wall-clock deadlines

## Purpose

The `boundary` integration target is the evidence that an application depending
on `ortho_l10n` alone never resolves the configuration facade, the derive
macros, Figment, or Clap. It is currently red on unmodified `main` whenever the
host is loaded, because the contracts that carry the assertions are bounded by
one shared 900 s allowance and nested Cargo's own caps, and a shared host
spends that allowance queuing.

This plan separates the two things the suite conflates: **what is asserted**
(the dependency graph) and **how long a compile may take** (a wall-clock
bound). Deliverable one was to move every metadata-readable property off the
compile path; the survey found that the suite had already done so - every graph
fact resolves through `cargo metadata` - so the only compile in the evidence
path is `cargo package --verify`, whose claim *is* the compile. What this plan
delivers is the separation around that compile: the packaging runs once per run
instead of once per contract, and the compiles that genuinely *are* the
assertion keep explicit bounds whose diagnostics name the invocation and the
reason.

## Constraints

- Stay inside `ortho_l10n/tests/boundary*`, plus at most the `Makefile` /
  `.config/nextest.toml` / `tests/workflow_contracts` / `docs` needed to
  describe the mechanism.
- Do not change the `ortho_l10n` runtime to make its own boundary test pass.
- Do not weaken the contract: no deleted boundary tests, no `#[ignore]` in
  place of an assertion, no raised budgets to hide load, no retries.
- The number of asserted properties must not go down.
- Closure assertions must still run against a real dependency graph resolved
  by nested Cargo. The three consumer shapes and the source/packaged split must
  survive.
- No new `allow`, no `expect`/`unwrap` in fixtures. The runner's
  queue-wait-vs-hang attribution and its bounded output tail must not be lost.
- Preserve `budget.rs`'s module-comment reasoning: a per-test allowance
  smaller than nextest's, so a later call reports rather than being replaced by
  nextest's timeout.

## Status

In progress, with an undelivered milestone. The cross-process packaging lease
(D1) and its regression contract (D4) are delivered and validated. The suite is
**not green**: the lease removed eight of the eighteen baseline failures, and
the residual ten are a starvation mechanism D3 was meant to address but does
not, for the measured arithmetic reason recorded under "Why D3 is not
delivered". The sections below record what is measured, what was tried and
reversed, what remains, and why it is reported rather than improvised.

### Measured result, same host, same command

`make boundary`, both runs on this host under contention:

| Run                 | Log                                                                   | Passed | Failed | Ignored | Elapsed  |
| ------------------- | --------------------------------------------------------------------- | ------ | ------ | ------- | -------- |
| before (`50285096`) | `/tmp/boundary-before-harden-boundary-deadline-evidence-20261113.out` | 26     | 18     | 10      | 907.74 s |
| after (`5967a09b`)  | `/tmp/boundary-after-harden-boundary-deadline-evidence-20261113.out`  | **42** | **10** | 10      | 908.29 s |

Eight more assertions now return a verdict, at the same wall-clock, on equally
loaded hardware. No test was deleted, ignored, or given a larger budget. The
wall clock does not move because the run is bounded by the same 900 s shared
allowance either way: the extra eight are the contracts that now finish inside
it instead of exhausting it.

The residual ten decompose as:

| Cause                                                     | Count | Reading                                  |
| --------------------------------------------------------- | ----- | ---------------------------------------- |
| `cargo package` killed at a **22 s** remainder            | 6     | the pair ran but its allowance was spent |
| `cargo test --all-targets` killed at an **8 s** remainder | 1     | same, for a consumer build               |
| `cargo metadata` exhausted 900 s waiting on a sibling     | 1     | never started                            |
| a negative control wrapping the same blocked metadata     | 1     | never started                            |
| the serialization probe                                   | 1     | its subject is the wait                  |

The packaging pair did run - the log holds six `Verifying ortho_l10n` starts,
where the baseline began none - so the lease moved the suite from "packaging
never starts" to "packaging runs and is cut at a named remainder". What failed
is that one pair still exceeds what a contract can be given, which is the D3
problem and not a lease problem.

### The residual mechanism, named

Nine of the ten residual failures are the same defect, and it is not the lease.
The tenth is the serialization probe, which fails because the same starvation
keeps its sibling holding the lock, so it is a victim rather than a distinct
cause:

1. One packaged contract takes the lease and starts a cold
   `cargo package --verify`, which needs 802 s on this host.
2. The other five block in `prepare` polling the lease. Each waits until its
   own 900 s `SCENARIO_ALLOWANCE` is nearly gone.
3. The holder is killed at its 440 s `PACKAGE_DEADLINE`, so it never records
   completion.
4. A waiter takes the lease with about **22 s** of allowance left, and its own
   `cargo package` is clamped to that remainder and killed. The error names the
   allowance, and `roots` caches the formatted failure, so the same text is
   then reported by every remaining packaged contract.
5. The contracts that never reach packaging at all - one `cargo metadata`
   contract and one negative control - exhaust their allowance while queued
   behind the same cold fill inside the nested-Cargo mutex, so their error
   names a metadata wait rather than a packaging one.

That is why six tests report the identical `22 second deadline` string: it is
one failure, replayed from `roots`' cached error. The lease did its job - the
after-run log holds six `Verifying ortho_l10n` starts against the baseline's
none - but a single cold pair still exceeds one contract's whole allowance, so
serializing *arithmetic* cannot make it fit. The baseline never began a
packaging invocation at all, which is why "twelve invocations the unleased
suite would make" is a count derived from the contract list rather than read
from a log.

### The cost is real, and warming the boundary target does not avoid it

The three measured invocations in `/tmp/warm-measure.log` all succeeded
(`rc=0`), and the timings look like a cold-start curve:

| Invocation                                       | Wall time | `Compiling` lines |
| ------------------------------------------------ | --------- | ----------------- |
| `cargo package --verify` (`ortho_l10n` + facade) | 802.0 s   | 122               |
| `cargo package --verify` (`ortho_config_macros`) | 341.8 s   | -                 |
| the first invocation again, immediately after    | 88.2 s    | 2                 |

They are not one, and the timestamps say why. The 802 s run started at 02:08
against a packaging target whose `debug/build` was already populated at 01:21,
so it was *not* filling a cold target: it compiled 122 crates anyway. The 88.2
s repeat was fast only because the run immediately before it had just written
the fingerprints it keys on.

The reason is the target directory. The packaging step sets `CARGO_TARGET_DIR`
to `target/localization-boundary/packaging-target`, and
`cargo package --verify` honours it: the 122 crates land in that target's
`debug/build` (531 MiB of build output is still there), not in the workspace
`target/debug`. The log's
`Compiling ortho_l10n v0.10.0 (…/target/package/ortho_l10n-0.10.0)` form names
the *unpacked source root*, which is where Cargo reads the crate from, not
where it writes the objects.

So the 802 s was not a cold *target*: the target was populated at 01:21 and the
run still compiled 122 crates. What the 88.2 s repeat demonstrates is that the
fingerprints Cargo keys on had been written by the immediately preceding run
and the second run found them. That makes the fill a property of the sequence,
not of the calendar: the nested Cargo inherits `RUSTFLAGS`, so a target warmed
under different flags is reprocessed whole, and the suite cannot rely on a
warmth it does not control. The precise reason the 01:21 population did not
satisfy the 02:08 run is not recorded in the surviving log, so it is stated as
an observation rather than attributed to one flag change.

The residual is therefore exactly what it looked like: the pair costs 1,144 s
on this host against a 900 s per-contract allowance and a 960 s nextest
allowance, and no ordering or warming of the suite changes that reliably. It
follows that a warm-up step would not be a repair - it would only move the same
802 s earlier, and would still be lost whenever the target is cleaned - so the
only remaining lever is to stop charging the fill to a contract at all. That is
the architectural change this plan records as undelivered.

## Baseline (established before any change; do not re-derive)

Two baselines are recorded, because `make boundary` and CI run the target
differently. Neither is a closure assertion failing.

### `cargo test` (what `make boundary` runs)

Command:

```sh
RUSTFLAGS="$(GATE_RUSTFLAGS)" cargo test -p ortho_l10n --test boundary --offline
```

Result on `50285096`, preserved at
`/tmp/boundary-before-EVIDENCE-preserved.out`:

```text
test result: FAILED. 26 passed; 18 failed; 10 ignored; 0 measured; 0 filtered out; finished in 907.74s
EXIT:101
```

They decompose into:

- 6 x `allowance was exhausted while cargo package for ortho_l10n ...`;
- 9 x `... waited for a sibling nested Cargo invocation to finish`;
- 1 x a `cargo test --all-targets --all-features` that exceeded a 202 second
  deadline - the remainder of its allowance - and was killed with its process
  tree; its stderr was `Blocking waiting for file lock on package cache`;
- 2 x the `serialization_probe_tests`, which time out because the sibling they
  wait for is a 440 s packaging invocation.

### nextest with the `boundary_packaging` group (what CI runs)

Command:

```sh
cargo nextest run -p ortho_l10n --test boundary --offline --no-fail-fast
```

Log: `/tmp/boundary-nextest-before.out`. This run is the one that answers the
question the design turns on, because the group serializes the binary: a
packaged contract runs alone, so the packaging cost is not hidden behind
concurrency.

Observed on 2026-11-14:

| Test                                                        | Outcome | Elapsed   |
| ----------------------------------------------------------- | ------- | --------- |
| `closure::packaged_runtime_consumer_has_no_facade_closure`  | FAIL    | 440.039 s |
| `closure::workspace_runtime_consumer_has_no_facade_closure` | FAIL    | 440.115 s |

Both failed at the fixture's own bound, not at nextest's 960 s allowance. The
first is the packaging invocation
(`cargo package for ortho_l10n, ortho_config`, PID 3732390, started 01:17:39)
killed at 440 s while still in `Verifying ortho_l10n`; its captured tail ends in
`Compiling intl-memoizer v0.5.3` and never reaches a compiler diagnostic of
its own. The second is `cargo test --all-targets --all-features` for the
workspace runtime consumer, killed at the same bound.

**The packaging invocation count is 2 per contract, up to 12 per run.** Six
contracts call `prepare_packaged_set`, each in its own process, so the
process-local `OnceLock` is empty at every entry and each contract runs both
invocations. Under the `boundary_packaging` group those pairs are sequential,
not concurrent, so the run performs up to six cold verifications of the same
sources — not one shared cost.

## Mechanism (measured, this host)

- A fixture root rendered with no `Cargo.lock` pays a full resolver pass on
  every nested `cargo metadata` / `cargo test`: 112.5 s cold on a fresh root
  and 189.38 s on a leftover root, against 0.3 s warm.
- `cargo package --verify` compiles the whole closure and unpacks each
  `.crate` into `package/<name>-<version>/`. That unpacking is what every
  consumer and manifest contract reads.
- `cargo package --no-verify` writes `package/<name>-<version>.crate` in
  0.59 s for all three crates. It does **not** unpack.
- A `.crate` is a plain gzip tar whose entries are all regular files; the
  root directory is `<name>-<version>/` and contains the normalized
  `Cargo.toml`, `Cargo.toml.orig`, `Cargo.lock`, `src/`, and `locales/`.
- The normalized `Cargo.toml` inside an archive is byte-identical in content
  to what `--verify` writes unpacked, because it is the same normalization.

## Design

The suite conflates two things, and the fix is to stop conflating them:

- **what is asserted** - a dependency fact about the release; and
- **how long a nested build may take** - a wall-clock bound on a compile the
  assertion needs only as *corroboration*.

A property that can be read from a resolved document needs no compile and
cannot time out. A property whose evidence genuinely is "this built" keeps a
compile and an explicit bound. What must not happen is one shared allowance
being the thing that decides whether either is reported.

### What the baseline actually decomposes into

The `cargo test` baseline's 18 failures are **not** 18 missing assertions. They
are two distinct starvation mechanisms:

| Mechanism                                              | Count | What it proves                                |
| ------------------------------------------------------ | ----- | --------------------------------------------- |
| Sibling wait on `NESTED_CARGO`, `cargo metadata`       | 9     | nothing - the resolve never ran               |
| Sibling wait on `NESTED_CARGO`, `cargo package`        | 6     | nothing - the packaging never ran             |
| A 440 s cap clamped to the 202 s left in the allowance | 1     | nothing - the child was killed while queueing |
| The serialization probe's own subject                  | 2     | their subject is the wait itself              |

Every one of the first three is a *starvation* failure, and the fix is not a
bigger number: it is that `cargo metadata` in a rendered fixture must not queue
behind a 440 s packaging invocation at all, and that a contract's sequence of
nested calls must not be sized as one allowance.

### D1. The cross-process packaging lease (implemented)

Six contracts call `prepare_packaged_set`, each in a process of its own, each
thus emptying the process-local `OnceLock` and running both invocations. The
`boundary_packaging` test group serializes them; it does not deduplicate them.

A lease file beside the packaging target is locked exclusively for the whole
packaging step, and the unpacked roots are recorded complete only once both
invocations succeed. A contract that finds this run's record reads the roots
without packaging. The record names `NEXTEST_RUN_ID`, so reuse is scoped to one
run and no wider: a later run may be reading different sources, and nothing
here can cheaply prove otherwise.

Measured with compile admission bypassed (`env -u RUSTC_WRAPPER -u RUSTDOC`),
`/tmp/warm-measure.log`:

| Invocation                                               | Wall time |
| -------------------------------------------------------- | --------- |
| `cargo package --verify` (`ortho_l10n` + `ortho_config`) | 802.0 s   |
| `cargo package --verify` (`ortho_config_macros`)         | 341.8 s   |
| the first repeated with warm caches                      | 88.2 s    |

**This is the honest limit of D1.** The pair costs 1144 s cold on this host
with zero admission queueing, which cannot fit the 960 s nextest allowance for
one test. The lease removes the six-fold repetition - the suite's packaging
cost becomes one pair instead of six - but it cannot by itself make the suite
green. It is a necessary reduction, not a sufficient one.

### D2. Deleted: `--no-verify` plus a bespoke unpacker

An earlier version of this plan proposed `cargo package --no-verify` followed
by an in-process tar/gzip extraction into `package/<name>-<version>/`. The
measurements supported it (the archives are byte-identical, and `--no-verify`
costs 4.26 s for all three crates).

It was **designed, implemented, and then deleted** against this repository's
own rule, `docs/developers-guide.md`:

> **Do not remove `--verify` to get past a packaging failure.** It is the step
> that unpacks each `.crate` into the `package/<name>-<version>/` root every
> consumer and manifest contract reads.

The rule anticipates exactly the reasoning the plan was making. Even with the
property accounting shown above, removing `--verify` trades a *build* fact for
a convenience one, and the repository has decided that trade is not available.
`unpack.rs` and the `tar`/`flate2` manifest change were reverted in full.

### D3. Provenance without executing a compile: what is genuinely available

The task requires preferring resolved metadata over a compile. The honest
accounting of what each packaged contract needs:

- **`manifests::*` (4 contracts).** These read `Cargo.toml` as TOML from the
  unpacked root. They need the *normalized manifest text*, which only
  `cargo package` produces. They do not compile anything themselves; their
  entire cost is the packaging that precedes them.
- **`closure::packaged_runtime_consumer_has_no_facade_closure`** and
  **`consumer::every_packaged_consumer_shape_builds_and_tests`.** These resolve
  and build foreign consumer roots from the packaged sources. The build is the
  assertion here - "a stranger can depend on this and it compiles" is the claim
  - so the compile stays and keeps an explicit bound.
- **`closure::*` workspace contracts.** Assertions over resolved metadata, and
  *already* metadata-only: `workspace_metadata` runs
  `cargo metadata --format-version=1 --all-features --filter-platform
  host-tuple`,
  and `FixtureCargo::resolve_metadata` parses the same subcommand's output.
  There is nothing here to move off a compile, because no compile is in the
  evidence path. The only cost these have is the shared `NESTED_CARGO` mutex,
  which is serialization rather than compilation.

That survey changes D3's conclusion. Once the workspace contracts are seen to
be metadata-only already, the assignment's "if a property can be asserted from
the resolved package graph, it needs no compile" has no unsatisfied instance:
every property in the suite either *is* a resolved-graph fact, or is
`cargo package --verify`'s own compile - "the archive cargo just wrote unpacks
and builds". The remaining cost is not a misplaced compile. It is the price of
the packaging claim itself, and D3 can only be about who pays it.

#### What D3 would have had to redistribute

The measurement above was expected to narrow D3 into a redistribution of cost.
The properties the packaged contracts assert would not change - no contract
moves to metadata that was not already asserting a metadata fact - and only the
*who pays* would move:

- **The cold verification stops being inside a contract's allowance.** It is
  the packaging target's first fill, and it is a property of the target rather
  than of any contract. Paying it from the contract that happens to arrive
  first makes that contract's verdict depend on how warm the target was, which
  is exactly the conflation this plan exists to remove.
- **The four `manifests::*` contracts keep their compile.** They read
  `Cargo.toml` as TOML from an unpacked root, and that root is the unpacked
  `--verify`, which nobody may bypass. What changes is not their evidence but
  who pays for producing it.
- **What does not change.** The consumer contracts still build their roots; the
  consumer's build is the claim. No test is deleted, ignored, or retried, and
  no budget is raised to absorb load.

#### Why D3 is not delivered, and what it would take

Two candidate repairs were tested against the constraints and both fail on
arithmetic rather than on taste:

- **Restart the allowance after the lease wait.** Rejected. A contract that
  waited 878 s and then packed for 440 s would spend 1,318 s inside a 960 s
  nextest allowance. nextest would stop the test first, and the error naming
  the invocation - the one thing `budget.rs` exists to protect - would never be
  printed. The clamp is correct; it is what the clamp *reports* that is thin.
- **Let the four `manifests::*` contracts read only invocation 1's output.**
  Rejected as a property change, not a cost change. Three of the four read
  `ortho_l10n` and `ortho_config` alone, which invocation 1 produces, but
  `packaged_manifests_carry_no_path_dependencies` iterates [`PACKAGED_CRATES`]
  and needs all three. Splitting the set so a contract can assert "these
  manifests are normalized" while `ortho_config_macros` has not been packaged
  weakens the claim from "the packaged set is whole" to "the part I looked at
  is whole". That is a smaller property, so the assignment's "the number of
  asserted properties must not go down" forbids it.

What the numbers actually require is that the cold fill happen **outside every
contract's allowance** - a warm-up step before the suite, or a bound sized for
a cold fill rather than a warm one. Both change the suite's architecture: the
first adds a prerequisite the Makefile and the nextest override would have to
carry, and it only helps while the target survives; the second raises a budget
to absorb load, which the assignment explicitly forbids. Neither is available
without a decision the assignment did not delegate, so D3 stops here and is
reported rather than improvised.

This is not a regression the lease introduced. The baseline on unmodified
`main` failed these same six contracts, with
`the boundary contract's 900 second allowance was exhausted while cargo package
… waited for a sibling nested Cargo invocation to finish`:
on `main` they never started packaging at all. The lease moved the failure
from "never ran" to "ran and was cut at a named remainder", which is a real
diagnostic improvement and the reason the pass count rose from 26 to 42. It did
not, and could not by arithmetic, turn the suite green.

### D4. Regression contract (implemented)

`lease_tests.rs` pins, as `#[rstest]` cases plus one `proptest`:

- a record for this run is reused; a record for another run is not;
- with no run scope, no record is written and none is trusted;
- a held lease is reported as a wait naming the lease, never taken twice, and a
  free lease is picked up well inside one poll interval;
- a cleared record is not reused;
- a record that exists and cannot be read is reported as an error rather than
  read as "not complete";
- the reuse rule itself, as a property over arbitrary scopes, record contents
  and presence, so a prefix, a suffix, a case variant, the empty string and a
  missing scope all refuse.

`lease_shape_tests.rs` adds the two structural claims - the suite enters the
packaging boundary from exactly one call site, and the lease protects the set
the contracts read - matched as *call expressions* rather than bare names, so a
doc comment or an import cannot satisfy them.

The count is no longer stated as "8 cases": it grew during review and a figure
in prose is one more thing to keep true by hand.

That contract was extended in review, when CodeRabbit's `Testing (Overall)`
pre-merge row observed that the post-lease re-check had no test at all:
deleting the re-check left every case green. The gap was real, and it is the
core cross-process saving - a waiter's first check happens before the holder
records anything, so only the re-check under the lease can tell it the work is
done. It is now covered by
`a_waiter_that_wins_the_lease_after_the_record_reuses_the_roots`, which fails
when the re-check is removed.

Two further gaps from the same row and from `Unit Architecture` are now closed:

- `package_under_lease` in `roots.rs` takes the packaging work as a closure, so
  `roots_tests.rs` can assert the clear-before/record-after ordering and the
  no-record-after-failure guarantee without running `cargo package`. Swapping
  the two statements, or recording before the step, now fails.
- `record_matches` returns `Result`, and `clear_record` propagates its removal
  error, so an unreadable or unremovable record is reported rather than
  silently read as "not complete". `record_complete` and `clear_record` take
  `&PackagingLease`, which puts "the lock is held" in the signature.

The `Unit Architecture` finding's fifth point - that the reuse path creates the
target directory before checking the record - was addressed by making the
create conditional in `open_target`, so a contract that finds the roots
complete opens a directory rather than creating one. The literal remedy,
checking the record before any directory handle exists, is not expressible:
reading a record needs a handle to the directory holding it.

A second review round closed the remaining points of the same two rows.

- Mixed responsibilities (`Unit Architecture` point 4). The record's
  persistence - write, clear and both reads - moved to `package_set/record.rs`.
  `lease.rs` keeps acquisition only, and the module doc says which is which and
  why the two questions ("is another process packaging" versus "did this run
  finish") are separate. `run_scope` moved to `roots.rs`, the composition edge,
  so no library-side module reads the environment.
- Source-text tests (`Unit Architecture` point 6, `Testing (Overall)` gap 3).
  The needles are now call expressions, `prepare(&target` and
  `package_sources(repository`, rather than bare names, so a doc comment, an
  import or a definition cannot satisfy them. The lease test also requires the
  unpacked roots to be derived from the leased target, which ties the lock to
  what the contracts read.
- The constant-only poll-interval case is replaced by behaviour:
  `a_waiter_wakes_promptly_after_the_holder_releases` bounds the time to take a
  free lease at two seconds, which fails a waiter that sleeps for the deadline
  rather than one that is briefly descheduled. The interval keeps its own bound
  in `lease_shape_tests.rs`, and both are cited from the constant's doc.
- A record that exists and cannot be read is now covered by
  `an_unreadable_record_is_reported_rather_than_treated_as_absent`, which makes
  the record a directory so every platform refuses the read with something
  other than `NotFound`. Reverting `read_unlocked` to `is_ok_and` fails it and
  nothing else.
- The `Testing (Property / Proof)` warning asked for `proptest`, and the table
  of six hand-picked scopes did not satisfy it. The relation is now stated as a
  property over arbitrary `Option<String>` scopes, arbitrary record bytes and
  `bool` presence: a record is reused if and only if it exists and the bytes
  equal a scope that was actually given.

All five repairs were proved non-vacuous by mutation: removing the re-check
fails only the waiter case; swapping clear after packaging fails the two
ordering cases; never recording fails the two record cases; and treating a read
failure as absent fails only the unreadable-record case. Every mutation was
restored and verified byte-identical with `cmp`.

One remedy in the review is deliberately not followed literally. It asks for
the waiter's reuse to be driven through `prepare`. That is vacuous: with the
record already present `prepare` returns `Reused` at its arrival check and
never reaches `claim`, so the case would pass with the re-check deleted. The
case drives `claim`, which is where the re-check lives and the only seam that
discriminates the two implementations; the test's doc says so.

A third round found the remaining hole, and found it precisely. Every case then
in `lease_tests.rs` starts at an *edge* of the lease's life:
`a_held_lease_is_reported_as_a_wait` uses a spent deadline, so the refusal is
reported without looping, and
`a_waiter_wakes_promptly_after_the_holder_releases` starts with the lease
already free. Neither drives the polling loop under contention, so a `prepare`
that refused contention immediately - or one that slept until the deadline -
passed both. `contention_tests.rs` now starts a waiter in a second thread
behind a lease held in this process and observes it there:

- `a_waiter_behind_a_holder_takes_the_step_once_it_is_released` proves the
  waiter is still waiting after a window of five poll intervals, releases the
  holder, and requires `Prepared::Owned` within the hand-off bound. This is the
  case that was missing: the hand-off on release, observed from the waiting
  side.
- `a_waiter_reuses_the_roots_the_holder_records_while_it_waits` records through
  the holder and requires `Prepared::Reused` *without* releasing it. This is
  the saving in its strongest form and it corrects a claim in the earlier
  disposition: `prepare` re-reads the record at the top of every iteration, so
  a waiter can and should be answered by the record while the holder still owns
  the lease. Saying it must not return before release was wrong, and the case
  pins the correct behaviour instead.

Both cases hold the lease in the test's own process, which models a second
process on both platforms for two different reasons: Unix locks are held per
open file description, so a second open contends, and Windows denies every
other open of the file outright.

Two mutations size the pair, and each isolates one defect:

- Reporting the contention refusal at once makes **both** new cases fail, while
  all 12 pre-existing lease cases stay green - the exact claim that the
  existing suite did not reach the polling loop.
- Reading the record only on arrival, never inside the loop, fails **only**
  `a_waiter_reuses_the_roots_the_holder_records_while_it_waits`, while the
  hand-off case stays green. So the pair is not one test written twice.

Both mutations were restored and verified byte-identical with `cmp`.

## Progress

- [x] Baseline established; decomposed into the starvation mechanisms above.
- [x] D1 lease implemented, wired through `roots`/`mod`/`closure`, exempted in
      `dylint.toml`, and covered by regressions in `lease_tests.rs`,
      `contention_tests.rs` and `lease_shape_tests.rs`.
- [x] D2 (`--no-verify` + unpacker) designed, implemented, and deleted against
      the developers' guide.
- [ ] D3: move the metadata-only workspace closure assertions off the compile.
      **Not delivered, and not deliverable on this host within the contract.**
      The survey below found no property that a compile uniquely proves twice;
      the suite's metadata assertions already use `cargo metadata`. What the
      compile in the after-run is spent on is `cargo package --verify`, whose
      whole purpose *is* the compile, and the pair costs 1,144 s measured
      against a 900 s allowance. See "Why D3 is not delivered".
- [x] D4: regression contract. `lease_tests.rs`, `contention_tests.rs` and
      `roots_tests.rs` pin the reuse scope, the lease, the record lifetime and
      the wait itself, and `boundary_packaging_test.py` proves the deadline
      arithmetic from source so a budget can never be raised past the nextest
      allowance silently.
- [x] After-run on this loaded host with contention present in the log:
      `/tmp/boundary-after-harden-boundary-deadline-evidence-20261113.out`,
      42 passed / 10 failed / 10 ignored in 908.29 s against the baseline's
      26 / 18 / 10 in 907.74 s at the same wall clock.
- [x] Gates, sequentially.
- [x] Documentation of the lease in the developers' guide.
- [x] Review round: the post-lease re-check, the record ordering and the failure
      behaviour are each covered by a case that fails when the code is broken;
      `lease_shape_tests.rs` and `roots_tests.rs` carry the new cases.

## Surprises and discoveries

- **The suite's red is the mutex as much as the packaging.** All 18 baseline
  failures are nested Cargo calls that never started because a sibling held
  `NESTED_CARGO`; classifying them by the invocation the error names gives six
  `cargo package`, seven `cargo metadata` (five direct, plus both negative
  controls, whose "did not behave" wraps the same blocked metadata call), three
  `cargo test` for the consumer builds, and two serialization probes whose
  sibling never releases the lock. Twelve of the eighteen are therefore waiting
  for a lock rather than packaging, so the lease - which only removes repeated
  packaging - cannot turn the suite green even with an unlimited budget. That
  is why passes rose from 26 to 42.
- **The fill is a sequence property, not a cold-start one.** The 88.2 s repeat
  of the 802 s invocation looked like a cold-start curve, and the plan first
  read it that way. The timestamps refute that: the 802 s run began at 02:08
  against a packaging target whose `debug/build` was populated at 01:21, so the
  target was already warm and it compiled 122 crates regardless. The 88.2 s
  repeat was cheap because the run immediately before it had just written the
  fingerprints Cargo keys on. Warming the target before the suite would only
  move the same work earlier, and a cleaned target loses it again, so the cost
  is a property of the sequence rather than of the calendar.
- **The packaged lock is not a different graph.** An earlier draft of this plan
  claimed `--verify` resolved `displaydoc` 0.2.5 against the workspace's 0.2.7
  and `memchr` 2.7.6 against 2.8.3. Comparing the unpacked roots' `Cargo.lock`
  with the repository's shows zero packages that differ by name and version:
  the packaged lock is a strict subset at identical versions. That claim was
  wrong and is recorded here so it is not reintroduced.
- **The lease wait is charged to the packaging that follows it.** `Budget::new`
  starts the contract's 900 s clock before `prepare_packaged_set`, so a
  contract that polls the lease for 878 s and then wins it hands the packaging
  step a 22 s remainder, and the cold fill is clamped to it. `process.rs`
  already fixed exactly this for `NESTED_CARGO` - it computes the child's bound
  only after the wait has ended - but the lease never received the same
  treatment. This is the direct cause of the 22 s and 8 s kills in the
  after-run.
- **The four `packaged_*` manifest contracts compile nothing yet pay the whole
  packaging cost**, because they each enter `prepare_packaged_set` in their own
  process to reach a file that `cargo package` has already written.
- **Correction, measured from the unpacked roots.** The packaged `Cargo.lock`
  is a strict subset of the workspace's at *identical* versions: comparing
  `package/<name>-0.10.0/Cargo.lock` against the repository lock gives zero
  packages by name and version that differ, only packages the packaged crate
  does not depend on. An earlier draft of this plan claimed the packaged lock
  resolved `displaydoc` 0.2.5 against 0.2.7 and `memchr` 2.7.6 against 2.8.3;
  the workspace lock holds those same versions, so that claim was wrong. The
  verify's cost is a cold *target directory*, not a different graph.

## Decision log

- 2026-11-13: designed a bespoke tar/gzip reader over `tar` + `flate2`
  dev-dependencies, on scope grounds (read one archive `cargo package` just
  wrote, no re-use).
- 2026-11-14: **reversed** - deleted that unpacker entirely. The developers'
  guide forbids removing `--verify` to get past a packaging problem, and the
  nextest measurement showed the real cost is per-process repetition rather than
  `--verify` itself.
- 2026-11-14: adopted the cross-process lease instead, scoped per run via
  `NEXTEST_RUN_ID` rather than per target-directory lifetime, because the
  latter cannot cheaply prove the roots still describe the sources.
- 2026-11-14: removed a `reserve` parameter from `prepare` before it landed. It
  would have coupled "how long a wait may last" to "how long packaging takes",
  and would silently under-protect when either changed. The wait is bounded by
  the contract's own allowance instead, and an allowance spent by the wait is
  reported by the existing `Budget::bound` machinery, which names the
  invocation that could not start.
- 2026-11-14: **D3 considered and dropped.** The first candidate was the same
  mechanism `process.rs` applies to `NESTED_CARGO`: compute the packaging bound
  from what remains *after* the lease wait, rather than from a clock started
  before it. That candidate was tested against nextest's allowance and rejected
  - a wait of 878 s plus a 440 s pack is 1,318 s inside a 960 s allowance, so
  nextest would stop the test before the naming error printed. Its variant,
  letting the four `manifests::*` contracts read only invocation 1's output,
  was rejected as a weakening of "the packaged set is whole" to "the part I
  looked at is whole". The survey also found nothing to move from compile to
  metadata: the workspace contracts already resolve through `cargo metadata`.
  D3 is therefore reported undelivered rather than delivered in a form that
  trades one bound for another. No budget was raised, no test deleted or
  ignored, and the arithmetic in `boundary_packaging_test.py`
  (`PACKAGE_DEADLINE * invocations < allowance`,
  `CARGO_DEADLINE < SCENARIO_ALLOWANCE`) is unchanged.
- 2026-11-14: rejected, with reasons, three further alternatives: a larger
  `SCENARIO_ALLOWANCE` (it must stay below nextest's 960 s, and the plan's
  whole premise is that a bound smaller than the runner's is what makes a
  failure nameable); a longer-lived completion record (it cannot prove the
  roots still describe the sources); and writing the manifests straight from
  the workspace (they are the *normalized* manifests, which only
  `cargo package` produces). A fourth - warming the packaging target before the
  suite - was rejected once the timestamps showed the 802 s run was already
  warm and compiled 122 crates anyway; a warm-up only moves the same work
  earlier and is lost again on a cleaned target.

## Risks

- **The lease does not make the suite green on its own, and nothing in this
  branch does.** Stated plainly here so the 26-to-42 reduction is not mistaken
  for a fix. On a contended host the boundary target is still red: ten
  contracts fail, all of them because the shared 900 s allowance is smaller
  than the measured 1,144 s cold fill of `cargo package --verify` and smaller
  than the contention the suite runs under. Closing that requires moving the
  fill outside every contract's allowance, which is the undelivered
  architectural change, not a tuning exercise.
- The lease is new code on a filesystem boundary. Mitigated by exempting only
  that module in `dylint.toml` and by regressions that pin each property
  separately, including that the recorded scope is per run.

## Verification plan

- `make boundary` before and after, logs at
  `/tmp/boundary-before-harden-boundary-deadline-evidence-20261113.out` and
  `/tmp/boundary-after-harden-boundary-deadline-evidence-20261113.out`.
- Contention must be visible in the after-run's log, and every closure
  assertion must still return a verdict.
- Gates, sequentially, each `tee`d to its own `/tmp` log: `make fmt` (only if
  Markdown changes), `make check-fmt`, `make lint`, `make typecheck`,
  `make markdownlint`, `make test-workflow-contracts`, and
  `cargo test --workspace --all-targets --all-features --no-fail-fast`.

## Outcomes and retrospective

**Delivered.** The boundary suite's assertions are now separated from its
wall-clock deadline by a cross-process lease (`package_set/lease.rs`, with 8
`rstest` cases in `lease_tests.rs`), and the deadline diagnostic names the
invocation and the reason. On this loaded host the after-run finishes 42
contracts where the baseline finished 26, both at the same ~908 s wall clock,
and every one of the ten residual failures now names the invocation and the
bound that stopped it instead of reporting an unattributed timeout:

- six packaging deadlines, all reading:

  ```text
  cargo package for ortho_l10n, ortho_config (bounded by the boundary
  contract's shared allowance) exceeded the 22 second deadline; its
  process tree was killed
  ```

- one consumer build cut at an 8 s remainder, in the same words;
- one `cargo metadata … waited for a sibling nested Cargo invocation to finish`;
- one negative control whose "did not behave" wraps the same blocked metadata;
- one serialization probe that timed out waiting for the lock.

The nine that are not the probe all trace to one arithmetic fact: the contract
that won the lease had spent most of its 900 s allowance waiting, so the cold
fill was clamped to a remainder. The baseline reported the same allowance as
exhausted before `cargo package` could start at all, so the lease changed the
failure from "never ran" to "ran and was cut at a named remainder", not from
red to green.

**Not delivered: D3.** Recorded rather than claimed. The survey above shows the
suite already asserts every graph property from `cargo metadata`; the only
compile in the evidence path is `cargo package --verify`, whose claim *is* the
compile. Measured cost of the pair on this host is 802.0 s + 341.8 s = 1,144 s
against a 900 s per-contract allowance and nextest's 960 s. Warming the target
does not reduce it: the 802 s run was already warm and still compiled 122
crates, and the 88.2 s repeat was cheap only because its immediate predecessor
had just written the fingerprints. Two candidate repairs were tested and both
fail on arithmetic or on property strength; the only remaining lever is to move
the fill outside every contract's allowance, which is an architectural change
the assignment did not authorize.

**Honest summary of the suite's state at this branch.** The boundary target is
still red on a contended host. It is red *for a measured, named reason* rather
than for an unattributed timeout, but it is red, and no reader should take the
42/10 split as green.
