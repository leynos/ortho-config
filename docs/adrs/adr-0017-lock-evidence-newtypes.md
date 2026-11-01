# Architectural decision record (ADR) 0017: Newtypes for lock evidence

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

| Topic                     | A: Newtypes | B: `newt-hype` | C: `the-newtype` | D: Integers |
| ------------------------- | ----------- | -------------- | ---------------- | ----------- |
| Transposition is an error | Yes         | No             | No               | No          |
| Inner integer reachable   | Only where  | Yes            | Yes              | Yes         |
|                           | intended    |                |                  |             |
| New dependency            | No          | Yes            | Yes              | No          |
| Primitive Obsession score | Clear       | Likely clear   | Likely clear     | Flagged     |

*Table 1: Comparison of the vocabulary options.*

## Consequences and validation

`artefact_lock_evidence_tests.rs` builds its fixtures from the newtypes too, so
a swapped pair fails to compile in the tests as well as in the production
module. The tests keep their named cases: the all-primitive `#[rstest]` tables
became named `const` tables iterated by plain `#[test]` functions, and each row
keeps its descriptive name as the leading `&str`, so a failing row still names
the case that broke rather than an anonymous tuple.

The device split is still pinned by the seven cases that motivated it,
including the two where the major's high bits and the minor's high bits sit
above the low 32 bits. Those cases are what a naive two-mask split passes and
then fails against a real filesystem.

The diagnostics split across `artefact_lock_diagnostics.rs` (what a hold means),
`artefact_lock_evidence.rs` (the evidence vocabulary and parser), and
`artefact_lock_process.rs` (reading `/proc/<pid>/stat`). The split is what
keeps each file within the 400-line module cap, and it puts the reading of a
process name beside neither the parsing of lock records nor the deciding of
what to report.

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
- [Artefact-lock diagnostics PR #615](https://github.com/leynos/ortho-config/pull/615).
