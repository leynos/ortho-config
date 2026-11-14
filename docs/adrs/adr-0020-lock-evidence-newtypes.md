# Architectural decision record (ADR) 0020: Newtypes for lock evidence

## Status

Accepted for issue #606.

## Date

2026-11-01.

## Context and problem statement

The test-only artefact-lock diagnostics in `ortho_config_macros` turn a held
lock into a report naming its holder. That report is assembled from kernel
evidence: a `/proc/locks` record carries a pid, a device number in two halves,
and an inode; the same three quantities are read again from the lock file's own
metadata to decide whether a record describes *this* file.

All three are integers, and two pairs of them share a width: the pid and the
descriptor are both `u32`, and the inode and the raw `dev_t` are both `u64`.
The functions that carry them take several at once — `inspect_holder(pid, ..)`,
`append_owner(lines, pid, descriptors, own_pid)`, `parse_identity(field)` — so
a transposed argument compiles, passes review, and quietly reports a descriptor
as a process. The failure it produces is the one this module exists to prevent:
a report that names the wrong holder.

The same code also drew a CodeScene Primitive Obsession finding on every
function it touched. That finding is a symptom of the defect above rather than
an unrelated style objection, so the two have one remedy.

## Decision

Model the evidence vocabulary as newtypes, in `artefact_lock_evidence.rs`:

- `ProcessId(u32)` — a pid. Rendered as a bare number, because that is how the
  kernel writes one.
- `Descriptor(u32)` — an open file descriptor. Rendered as a bare number.
- `Inode(u64)` — an inode number.
- `DeviceNumber { major, minor }` — a device, constructed either from its two
  glibc halves or from a raw `st_dev` through the documented bit layout.
- `LockFileIdentity { device, inode }` — the pair that names one file.

`LockRecord` stores a `DeviceNumber` whole rather than a split `major`/`minor`
pair. Comparing a record to a file is therefore `LockFileIdentity::holds`,
which lives beside the fields it compares, instead of every call site
recombining the halves itself and being free to recombine them wrongly.

Each newtype exposes only the constructors and accessors its callers use. In
particular none of them forwards arithmetic, ordering, or conversion to the
inner integer, so a value cannot drift back into integer position by accident.

The newtypes stop at the artefact-lock diagnostics. They describe kernel
evidence for one test-only report, not a domain model, and promoting them to
the crate's public surface would impose this vocabulary on callers who have no
`/proc` to speak to.

## Decision drivers

- A report that names the wrong holder is worse than no report, because it
  sends a maintainer after an innocent process.
- Issue #606's failures are intermittent and post-release, so every report is
  read once and acted on; there is no second observation to correct a
  plausible-looking wrong answer.
- The diagnostics are test support inside a procedural-macro crate, where an
  extra type costs nothing at run time.

## Options considered

### Option A: Named newtypes per quantity

Each integer quantity gets its own type, with constructors that encode how the
value is obtained. Transpositions are compile errors.

### Option B: `newt-hype` type aliases

`newt-hype`'s `newtype!(Name, Inner)` emits a **type alias** for one generic
base. `ProcessId(u32)` and `Descriptor(u32)` would therefore be the same type,
which removes exactly the distinction this decision exists to create; the
device and inode pair would collapse likewise.

### Option C: `the-newtype` blanket trait

`the-newtype` forwards traits through one blanket implementation over a
`Newtype` trait requiring `AsRef` and `Deref`. Both re-expose the inner
integer, so a `Descriptor` still coerces into integer position. The crate's own
README says to implement traits manually and avoid the `Newtype` trait when
specific traits are needed, which is the case here.

### Option D: Leave the vocabulary as integers

Retain bare `u32`/`u64` parameters and rely on argument naming and review.

| Topic                     | A: Newtypes      | B: `newt-hype` | C: `the-newtype` | D: Integers |
| ------------------------- | ---------------- | -------------- | ---------------- | ----------- |
| Transposition is an error | Yes              | No             | No               | No          |
| Inner integer reachable   | Only as intended | Yes            | Yes              | Yes         |
| New dependency            | No               | Yes            | Yes              | No          |
| Primitive Obsession score | Clear            | Likely clear   | Likely clear     | Flagged     |

*Table 1: Comparison of the vocabulary options.*

## Consequences and validation

`artefact_lock_evidence_tests.rs` builds its fixtures from the newtypes too, so
a swapped pair fails to compile in the tests as well as in the production
module. The three tables whose every argument was a primitive — the device
splits, the device renderings, and the identity matches — became named `const`
tables iterated by plain `#[test]` functions, carrying each row's descriptive
name as the leading `&str` so a failure still names the case that broke rather
than an anonymous tuple. The remaining `#[rstest]` tables take a string or an
already-typed `Option<ProcessId>`, which carries its own name, so they stay as
they are.

The device split is still pinned by the cases that motivated it, including one
where the major's high bits sit above the low 32 bits and one where the minor's
high bits run past bit 32, into the 32..44 range that only the widest minor
field reaches. Those cases are what a naive two-mask split passes and then
fails against a real filesystem.

The diagnostics split across `artefact_lock_diagnostics.rs` (what a hold means),
`artefact_lock_evidence.rs` (the evidence vocabulary and parser),
`artefact_lock_holder.rs` (the descriptor scan), and `artefact_lock_process.rs`
(reading `/proc/<pid>/stat`). The split is what keeps each file within the
400-line module cap, and it puts the reading of a process name beside neither
the parsing of lock records nor the deciding of what to report.

The cap counts declarations, not just code. Registering the descriptor scan in
`artefact/mod.rs` pushed that file from exactly 400 lines to 406, and the
`module_max_lines` lint reports the overage against the *declaration* of the
module rather than against anything the change added. The prune moved to
`artefact_prune.rs` to pay for it: it answers a self-contained question, and it
already held the only test-injection point in the prune. `artefact` re-imports
the three names `publish` and `prune_tests` use, so neither had to learn a new
path.

The descriptor scan is separated for a second reason beyond the cap. A scan
that fails and a scan that finds nothing are different evidence, and only the
second says anything about absence. The scan returns a `DescriptorScan` that
keeps `Complete` and `Incomplete` apart, so a read failure cannot be mistaken
for an empty descriptor table. This is a correction to an earlier draft that
folded both failures into "no descriptor".

A complete scan that finds nothing does not, by itself, establish inheritance.
The descriptor scan and the `/proc/locks` record are separate observations
taken at different moments, so an empty scan is equally explained by the
recorded acquirer having released the lock in between. The report names both
possibilities instead of asserting the one that accuses a process. That is a
correction to an earlier draft of this same section, which made the scan's
silence into a conclusion the observation cannot support.

An incomplete scan keeps the descriptors it matched before the failure rather
than discarding them. An unreadable entry inside an otherwise readable
directory is ordinary — `/proc/<pid>/fd` holds every descriptor in the process,
and another thread closing an unrelated one between enumeration and the
metadata read fails an entry that has nothing to do with the lock. Discarding
the matches there would report the lock unexplained while the guard was still
open, which is exactly the intermittent failure this work exists to make
diagnosable; the entry is still reported as making the scan incomplete, so no
claim of absence is made. The scan is sorted so a failure leaves the same match
list rather than depending on `read_dir` order.

`artefact_lock_report_tests.rs` pins both settle outcomes against a *separate*
holder process: one test releases the lock part-way through the window and
asserts the settle returns `Ok` once the release is observed, and one leaves it
held and asserts the settle reports the live owner and its open descriptor. The
holder has to be a separate process. `flock` belongs to the open file
description, and the suites in this binary fork helper processes, each fork
duplicating the whole descriptor table; an in-process guard would therefore
stay held under this process's name until the child's `exec`, and the tests
would fail intermittently for the reason they exist to diagnose.

The release is ordered by what the settle *observed*, not by a timer. An
earlier draft released from a sleeping thread and required the settle to
outlast the sleep, which did not establish the probe order it claimed: a
releaser scheduled first could finish before the first probe, so a correct
settle would return early and fail. `ensure_settles_free_observed` reports each
probe before acting on it, and the test releases from the first observed
`Held`, so the order is a fact about the code — held, released, free — rather
than a race the assertion hopes to win. The same recording catches what the
timer could not: a settle that returned `Ok` on its first held probe fails
because it never records a held probe at all.

The probe *count* is not fixed, and asserting one would reintroduce the same
defect in a smaller place. `probe_lock` drops its acquisition before returning,
so the callback is not a window in which the lock is still held; it establishes
only that this probe saw what it reports. The helper polls for the release on
its own interval, which is longer than the settle's re-probe interval, so a
correct settle sees one or more held probes and then a free one. The test
asserts that shape rather than a number, so it fails on early success or on a
free probe preceding the hold, and does not fail merely because the helper was
slow to answer.

## Alternatives rejected

- `newt-hype` collapses same-inner newtypes to one type, defeating the purpose.
- `the-newtype` forwards `AsRef`/`Deref`, re-exposing the inner integer.
- Bare integers leave the transposition defect in place.
- Extracting the helpers without the newtypes would move the primitive
  parameters rather than remove them.

## References

- [Developers' guide](../developers-guide.md), "Diagnosing a post-release lock
  hold".
- [Issue #606](https://github.com/leynos/ortho-config/issues/606).
- [Artefact-lock diagnostics PR
  #615](https://github.com/leynos/ortho-config/pull/615).
