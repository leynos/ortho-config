# Architectural decision record (ADR) 010: Opt-in identifier artefact emission

## Status

Accepted.

## Date

2026-09-07.

## Context and Problem Statement

Procedural macros must not perform ambient writes during ordinary compilation.
When explicitly requested with `ORTHO_CONFIG_EMIT_IDENTIFIERS=1`, the derive
macro writes a provisional, schema-versioned JSON artefact below
`${OUT_DIR}/ortho-config`. It records standalone derived identifiers only;
mounted subcommand identifiers remain owned by the compiled documentation IR.

Each expansion writes an atomic fragment and merges fragments
deterministically. The merged document splits at 1 MiB and supplies an index
when needed. The schema is provisional until a consumer exists and marks every
entry with `path_scope: "standalone"`.

`OUT_DIR` is scoped to the package rather than to individual compilation
targets, so one consuming package can have several compiler processes
publishing into one artefact directory. Each writer therefore takes an
exclusive lock (`cli-identifiers.lock`) across the whole fragment-write, merge,
and rendered-file publication sequence. Atomic fragment replacement alone is
insufficient: it protects a single file, but not the read-modify-write of the
merged inventory, so a writer that merged before a peer's fragment landed would
replace the peer's complete inventory with that stale rendering. Locking the
complete sequence makes the publication a serialised read-modify-write per
`OUT_DIR`.

Cargo does not fingerprint proc-macro environment reads. After changing the
opt-in variable or source, force a fresh expansion with
`cargo clean -p <package> && ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build -p <package>`.
Do not export the variable in shell profiles or CI-wide environment blocks.

Fragments are pruned at merge when their recorded source file no longer exists,
which covers deleted files and moves between files. A fragment is named for the
deriving type plus the expansion's line, column, and source-path hash, so
renaming that type in place — or moving it to another line — writes a *new*
fragment and leaves the previous one behind. Because the source file still
exists, the orphan is not pruned, and its entries keep contributing until the
next forced refresh. The limitation is accepted because the documented
invocation above discards the whole artefact directory and regenerates it, and
because the artefact is a standalone declaration inventory whose authoritative
consumption path is the compiled docs IR.

## Consequences

Normal builds do not write artefacts. Explicit emission can fail for filesystem
permissions and reports a derive diagnostic with remediation. Future translator
tooling must join standalone entries with mounted documentation IR rather than
pretending the artefact enumerates full command trees.
