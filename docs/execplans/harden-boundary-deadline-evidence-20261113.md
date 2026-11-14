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
bound). Every property that can be read from resolved metadata is moved off the
compile path. The compiles that genuinely *are* the assertion keep an explicit,
generous per-test bound, and their diagnostics name the invocation and the
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

In progress. The cross-process packaging lease (D1) and its regression contract
(D4) are delivered and validated in isolation. The suite is **not yet green**:
the lease removes a six-fold repetition of the packaging work, and the
remaining starvation is a separate cause that D3 addresses. The sections below
record what is measured, what was tried and reversed, and what remains.

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

### D2. Deleted: `--no-verify` plus a hand-written unpacker

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
- **`closure::*` workspace contracts.** Assertions over resolved metadata; the
  compile is corroboration. These are the ones where a metadata-only path is
  available.

### D4. Regression contract (implemented)

`lease_tests.rs` pins, as 8 `#[rstest]` cases:

- a record for this run is reused; a record for another run is not;
- with no run scope, no record is written and none is trusted;
- a held lease is reported as a wait naming the lease, never taken twice;
- a cleared record is not reused;
- the suite enters the packaging boundary from exactly one call site in
  `roots`, and performs at most two invocations per run - read from the source
  because the property is about the *shape* of the code, and no single
  contract's runtime observation can see a second call site in another module;
- the lease protects the set the contracts actually read;
- the poll interval stays a hand-off rather than a stall.

## Progress

- [x] Baseline established; decomposed into the starvation mechanisms above.
- [x] D1 lease implemented, wired through `roots`/`mod`/`closure`, exempted in
      `dylint.toml`, and covered by 8 regressions.
- [x] D2 (`--no-verify` + unpacker) designed, implemented, and deleted against
      the developers' guide.
- [ ] D3: move the metadata-only workspace closure assertions off the compile.
- [ ] After-run on this loaded host with contention present in the log.
- [ ] Gates, sequentially.
- [ ] Documentation of the lease in the developers' guide.

## Surprises and discoveries

- **The suite's red is mostly the mutex, not the packaging.** Nine of the
  baseline's 18 failures are `cargo metadata` never starting because a sibling
  held `NESTED_CARGO`. Nine is more than the six packaging failures, so a
  packaging-only fix cannot turn the suite green even with unlimited budget.
- **`--verify` is far more expensive than the plan assumed.** 802 s for the
  first invocation alone, measured with admission bypassed, against the earlier
  216 s figure taken with a warm cache.
- **The four `packaged_*` manifest contracts compile nothing yet pay the whole
  packaging cost**, because they each enter `prepare_packaged_set` in their own
  process to reach a file that `cargo package` has already written.
- The packaged lock resolves older crates than the workspace's (`displaydoc`
  0.2.5 against 0.2.7, `memchr` 2.7.6 against 2.8.3), so `--verify` compiles a
  *different* graph. This was not changed; it is recorded because it makes the
  verify's cost data-dependent and not representative of the workspace build.

## Decision log

- 2026-11-13: designed a hand-written tar/gzip reader over `tar` + `flate2`
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

## Risks

- **The lease does not make the suite green on its own.** Stated plainly here
  so the reduction is not mistaken for a fix. D3 and the allowance accounting
  are what address the remaining starvation.
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

The lease is delivered and validated in isolation (8 regressions green, target
clippy-clean). The suite-level after-run and the remaining D3 work are recorded
as incomplete rather than claimed.
