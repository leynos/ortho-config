# Architectural decision record (ADR) 0027: Agent-context summary locale selection

## Status

Accepted.

Date: 2026-11-26.

## Context and problem statement

`cargo-orthohelp` writes one non-locale-scoped `agent-context.json` document.
Its command summaries were resolved from a localizer hard-coded to `en-US`, so
a package that ships its catalogue only under another English locale — for
example `locales/en-GB/messages.ftl` — received no summaries even when invoked
with `cargo orthohelp --locale en-GB`. The only signal was the warning
`no en-US localizer; agent-context summaries omitted`.

The existing behaviour is documented, so this is a feature request rather than
a defect (issue #719). British-English projects could duplicate their catalogue
as `en-US`, relabel British text as `en-US`, or ship no summaries; all three
options are poor. The agent-context schema already carries an optional,
language-neutral `summary`, and the document layout deliberately stays outside
locale directories.

## Decision drivers

- Let a package ship one catalogue under its own English variant.
- Keep exactly one `agent-context.json`, at the output root, with schema
  version 1 unchanged.
- Keep the schema and output layout identical, and keep summary text
  identical for runs that still select the same catalogue.
- Make a fallback deterministic and observable without adding a wire field.
- Preserve the exact-identifier reuse that lets localized output share the
  summary localizer.
- Treat a summary as cosmetic: a missing or malformed catalogue degrades to the
  next candidate rather than failing the run.

## Options considered

### Option A: Keep the `en-US`-only localizer

Rejected. It forces the workarounds the issue records and leaves an
English-variant package without agent-facing summaries.

### Option B: Locale-scope agent context

Rejected. Writing `agent-context.json` per locale would change the output
layout, multiply a document that exists to describe the command surface once,
and break consumers that read a single document.

### Option C: Ordered single-catalogue selection (accepted)

Use the first requested or metadata locale, then `en-US`, then a discovered
locale matching an earlier candidate's language, then the embedded `en-US`
defaults. Keep one document and one localizer.

### Option D: Record the chosen locale in the JSON schema

Rejected. The choice is diagnostic, not part of the invocation contract, and
adding a field would change schema version 1 for information an operator reads
once. Structured `tracing` on stderr carries the locale and its source without
touching the wire format.

| Topic                      | A: `en-US` only | B: locale-scoped | C: ordered choice | D: schema field |
| -------------------------- | --------------- | ---------------- | ----------------- | --------------- |
| `en-GB`-only package works | No              | Yes              | Yes               | Yes             |
| One document at the root   | Yes             | No               | Yes               | Yes             |
| Schema v1 unchanged        | Yes             | No               | Yes               | No              |
| Deterministic, observable  | Yes             | Yes              | Yes               | Yes             |
| No duplicate catalogue     | No              | Yes              | Yes               | Yes             |

_Table 1: Comparison of summary-locale placement options._

## Decision outcome / proposed direction

### Preference order

The summary localizer is selected from this ordered candidate list:

1. the first parseable value of `--locale`;
2. otherwise, when no `--locale` value is given, the first parseable locale
   declared in `[package.metadata.ortho_config] locales`, whether or not
   `--all-locales` was given. The flag governs which locales are generated, not
   which locale describes the command surface;
3. `en-US`, the historical fallback;
4. a locale discovered under `locales/` that shares the language subtag of an
   earlier candidate, ordered by candidate and then by sorted discovery;
5. the embedded `en-US` defaults.

The plan's "first explicit locale" is read as the first parseable explicit
value: an unparsable entry is not a locale, is reported, and yields the next
parseable explicit value. A list with no parseable value yields no preferred
candidate and never surrenders the slot to metadata. Duplicate locales collapse
to the highest-preference source.

### Candidate acceptance

A candidate counts only when the package ships at least one `.ftl` resource for
it and the localizer builds. An empty resource directory is skipped rather than
accepted from embedded defaults, because embedded defaults resolve library
messages rather than the package's command descriptions. A load or build
failure is reported with the locale and error and the next candidate is tried.
When no candidate qualifies, `en-US` is built from embedded defaults only; that
can still resolve library message identifiers, and when it also fails the
existing "summaries omitted" behaviour remains. Once a catalogue is selected,
each summary is looked up only there: a missing identifier or blank resolution
omits that command's summary without trying another catalogue.

### Same-language fallback

Discovery reuses the sorted `locales/` enumeration used by `--all-locales`.
Matching compares the `unic-langid` language subtag, so `en-GB` matches an
`en-US` candidate, `fr-FR` matches `fr-CA`, and `zh-Hans` matches `zh`. A
discovered name carrying a variant subtag, such as `en-backup`, is rejected
because it is usually a backup or scratch directory rather than a locale.
Locales already present as preferences are excluded, and a discovery failure is
reported and treated as no fallbacks. Because `en-US` precedes same-language
discovery, a package whose metadata requests only `en-US` and which ships
`locales/en-GB/messages.ftl` but no `locales/en-US` directory resolves its
summaries from `en-GB` only after the absent `en-US` candidate is skipped.

### Observability

The selected locale and source are emitted at `info` as
`agent-context summary locale selected`, with `source` set to `requested`,
`metadata`, `en-US`, `same-language`, or `embedded-defaults`. A choice that
differs from the first preference emits a warning naming the first preference,
the tried candidates, and the chosen locale. Reaching the embedded defaults
emits a warning listing the candidates, and a total build failure warns that
summaries are omitted. No schema field is added, and `agent-context.json` keeps
schema version 1.

### Reuse and ownership

The selected localizer is shared with localized output only when a requested
locale is the exact same identifier and the localizer was built from package
resources. An `en-GB` summary therefore never stands in for a requested `fr-FR`
translation, and an embedded-defaults fallback never masks a missing or
malformed catalogue during human-document generation. `SummaryLocalizer`
carries the provenance flag, and reuse is a single predicate over it. The
selection policy lives in `cargo-orthohelp/src/summary_locale/`, separate from
the general locale resolver and from the Fluent adapter in `locale.rs`. The
split keeps the domain policy testable without a compiled bridge and keeps each
file within the repository's length limit.

## Goals and non-goals

- Goals:
  - Resolve summaries from a requested or metadata locale that ships a
    catalogue.
  - Fall back deterministically through `en-US`, same-language discovery, and
    embedded defaults.
  - Keep one document, schema version 1, and existing output order.
  - Report the choice and every fallback on stderr.
- Non-goals:
  - Translate command summaries that no catalogue supplies.
  - Emit per-locale agent-context documents.
  - Add a locale field to the agent-context schema.
  - Change the `--locale` or `--all-locales` resolution for IR, man pages, or
    PowerShell output.

## Migration plan

- The behaviour ships in v0.11.0 and is recorded in the v0.10.0-to-v0.11.0
  migration guide.
- The schema and output layout are unchanged for every run. Summary text is
  unchanged for runs that still select the same catalogue: packages without
  catalogues keep the embedded-defaults text, and a package whose first
  preference resolves keeps that text. A package that ships only a fallback
  catalogue gains summaries, or different summaries, because the new rule
  selects a catalogue the previous implementation never consulted.
- No consumer code change is required, and no cache salt changes: the bridge
  IR is unaffected, and the summary localizer is rebuilt per invocation.
- Reverting the change restores `en-US`-only resolution without touching the
  agent-context schema or output layout.

## Known risks and limitations

- A same-language fallback can choose a regional variant the caller did not
  name, such as `en-GB` for an `en-US` request. The warning makes the choice
  visible, and the alternative is no summary at all.
- `en-US` outranks same-language discovery. A package that ships both `en-US`
  and `fr-FR`, invoked with `--locale fr-CA`, receives English summaries rather
  than French ones. The documented preference order favours the historical
  English fallback; changing that ranking would need a separate decision.
- A malformed preferred catalogue falls through to the next candidate, and a
  wholly malformed package can reach embedded defaults. Summaries then resolve
  only where a message identifier happens to exist in the library catalogue.
- Summary discovery skips a directory name that does not parse as a language
  identifier. The general `--all-locales` path instead reports an invalid
  locale, so the two discovery policies deliberately differ.
- One localizer serves the whole document, so a mixed-language package cannot
  give different commands different summary locales.

## Architectural rationale

The selection is domain policy over a driven Fluent adapter: it decides which
catalogue to ask for, while `locale.rs` owns reading resources and building a
localizer. Keeping the decision in its own module makes the preference order a
pure, testable function and keeps tracing labels and fallback warnings beside
the rule they describe. A single document with a diagnostic locale choice
preserves the agent-context contract and avoids turning a cosmetic summary into
a schema commitment.

## References

- [Issue #719: Resolve agent-context summaries from the requested locale](https://github.com/leynos/ortho-config/issues/719).
- [cargo-orthohelp design](../cargo-orthohelp-design.md) §6.3.1.1.
- [Users' guide: output layout](../users-guide.md#output-layout).
- [v0.11.0 migration guide](../v0-11-0-migration-guide.md#16-agent-context-summaries-follow-the-requested-locale).
- [ADR-0003: Define schema ownership for agent-native contracts](adr-0003-define-schema-ownership-for-agent-native-contracts.md).
