# Architectural decision record (ADR) 0021: Test seams for the lock diagnostics

## Status

Accepted for issue #606.

## Date

2026-11-14.

## Context and problem statement

[ADR-0020](adr-0020-lock-evidence-newtypes.md) gave the artefact-lock
diagnostics a newtype vocabulary, so a transposed pid, descriptor, device, or
inode is a compile error. That closed one class of defect and left another
untouched: the vocabulary says what the evidence *is*, not where it comes from.

Every observation the report makes was an inline `std::fs` call on `/proc`, and
every wait was an inline `std::thread::sleep`. That had two consequences, both
of which showed up as missing tests rather than as bugs:

- The report's failure branches — a descriptor table that fails part way
  through a scan, a `/proc/<pid>/stat` that cannot be read, a lock table that
  cannot be listed — could be reached only by arranging filesystem topology. In
  practice they were reached by nothing, so the paths a maintainer most needs a
  good sentence from were the least exercised.
- The settle's deadline was untestable in principle. A real clock reaches
  `SETTLE_BUDGET` only by waiting the budget out, which is two seconds per
  assertion and, worse, makes the test about wall-clock time.

Both are the same defect in different clothes: the code decided *and* fetched,
so a test could only act on the decision by controlling the world.

## Decision

Put every kernel read behind a `ProcFs` trait in `artefact_lock_procfs.rs`, and
every wait and deadline behind a `Clock` trait in `artefact_lock_clock.rs`.

- `ProcFs` has one method per observation: `lock_table`,
  `descriptor_entries`, `descriptor_identity`, `process_stat`.
- `Clock` has `now` and `sleep`, and nothing else.
- `RealProcFs` and `RealClock` are the only production implementations, and
  `Boundaries::real` in `artefact_lock_diagnostics.rs` is the only place either
  is selected.
- The two travel together as a `Boundaries<'_>` value, because every entry point
  that takes one takes the other.

Both traits are `pub(super)`, both are confined to the test-only diagnostics in
`ortho_config_macros`, and neither is exported from the crate.

## Decision drivers

- Issue #606's failures are intermittent and post-release. A report is read once
  and acted on, so a wrong or unexercised sentence is expensive.
- The two branches that matter most — "the scan failed" and "the scan found
  nothing" — lead to opposite conclusions, and the code already distinguishes
  them with `DescriptorScan`. A seam is what lets a test assert the distinction
  rather than the code that happens to produce it.
- A deadline can only be asserted against a clock the test owns.
- The diagnostics are test support inside a procedural-macro crate, so an extra
  trait costs nothing at run time.

## Options considered

### Option A: One trait per observation set, as decided

Four `ProcFs` methods and a two-method `Clock`. Each failure branch is
reachable by failing exactly one method and leaving the others answering.

### Option B: A single `ProcFs::read(path)` method

Simplest to implement, and useless for the purpose: with one read method,
failing "the lock table" and failing "a descriptor" are the same event in a
fake, so the report's branches become indistinguishable from each other and the
seam tests nothing that the real code does not already merge.

### Option C: `mockall` or `mockable`

Neither is a dependency of `ortho_config_macros`, and neither removes the need
for the trait. `mockall` would generate the same set of methods behind a macro,
at the cost of a new dependency and a mock whose failure modes are less legible
than an explicit fake in the same module tree. The doubles are small (a map, a
`Cell`, a `RefCell`), so the explicit form is cheaper to read.

### Option D: Keep the inline reads, extract only the pure decisions

This is what ADR-0020 did. It works for parsing and matching — those can be
pure functions of their inputs — and cannot work for the deadline, which has no
input other than the clock.

| Topic                        | A: Traits | B: One read | C: `mockall` | D: Inline |
| ---------------------------- | --------- | ----------- | ------------ | --------- |
| Branches separately testable | Yes       | No          | Yes          | No        |
| Deadline testable            | Yes       | Yes         | Yes          | No        |
| New dependency               | No        | No          | Yes          | No        |
| Production implementation(s) | Exactly 1 | Exactly 1   | Exactly 1    | n/a       |

*Table 1: Comparison of the seam options.*

## Consequences and validation

The traits answer in their *callers'* vocabulary rather than the kernel's,
which is what makes the distinction the diagnostics exist to draw assertable:

- `descriptor_entries` returns the descriptor numbers it parsed **and** the
  names it could not, because "there is no descriptor here" and "I could not
  tell what this was" are different answers. A listing that stopped part-way
  keeps the entries it yielded and names its own fault, and it hands back the
  open iterator it was read through: dropping that iterator first would close a
  descriptor the listing named, so a self-scan would report an incomplete scan
  it caused itself.
- The retention rule is a seam of its own. `collect_until_fault` is generic
  over the item and error type precisely so it can be driven from a scripted
  list; a fake can only hand over a listing that is already complete or already
  partial, so a mutant that discarded the entries before a fault survived every
  test that drove `FakeProcFs`. Testing the rule where it is decided is what
  makes it evidence.
- A failed scan carries whatever it matched before the failure, so an unrelated
  descriptor closing mid-scan cannot discard a live holder.
- `process_stat` answers `Option`, because a process can exit between the record
  that names it and the read that would describe it; the report degrades its
  sentence rather than failing.

`FakeClock` advances only when the code under test sleeps, so
`a_hold_that_outlives_the_budget_fails_with_the_report` reaches the deadline in
as many iterations as the settle has probes. The test was validated by
mutation: moving the settle's sleep before its deadline check makes it fail
with "200 waits for 200 probes", which establishes that the deadline and not
the sleep count ends the loop. The complementary mutant — sleeping after the
final probe — passes, because both orderings produce one fewer wait than probe.
That mutant is not distinguished by this test, and the test's docstring says so
rather than implying a coverage it does not have.

`FakeProcFs` is keyed on the rendered form of each newtype, which is the one
place ADR-0020's lack of a `get()` costs anything. A pid the test did not
describe has no descriptor table, matching an exited process. That default
caught a wrong test rather than a wrong implementation:
`an_undescribable_process_is_named_as_such` first gave its pid an empty table,
which is a *complete* scan whose "no descriptor" answer is correct; only an
unreadable listing produces the sentence the test pins.

The seams stopped at the report and did not reach `probe_lock`, which still
takes its observation through a real file handle. A test that needs a genuinely
held lock needs a genuinely held lock: the tests that exercise the settle hold
it from a helper *process* for the `flock` reason recorded in ADR-0020, and no
fake can stand in for that. The traits make the *decisions* testable, not the
`flock` semantics.

Re-use policy, which `AGENTS.md` requires of a new abstraction: neither trait
is a general-purpose filesystem or time abstraction, neither is public, and
neither leaves this module tree. A new observation the report needs is a new
`ProcFs` method plus its `RealProcFs` implementation, not a `std::fs` call at
the point of use; a new wait is a `Clock` call through the `Boundaries` already
in hand, not a fresh `std::thread::sleep`. `Boundaries::real` remains the only
place a production implementation is chosen. The policy is recorded in the
developers' guide under "Diagnosing a post-release lock hold".

## Alternatives rejected

- A single `read(path)` method collapses the branches it was meant to separate.
- `mockall` adds a dependency to buy a less legible fake.
- Extracting only the pure decisions leaves the deadline untestable, which is
  one of the two problems this decision exists to solve.

## References

- [ADR-0020: Newtypes for lock evidence](adr-0020-lock-evidence-newtypes.md).
- [Developers' guide](../developers-guide.md), "Diagnosing a post-release lock
  hold".
- [Issue #606](https://github.com/leynos/ortho-config/issues/606).
- [Artefact-lock diagnostics PR
  #615](https://github.com/leynos/ortho-config/pull/615).
