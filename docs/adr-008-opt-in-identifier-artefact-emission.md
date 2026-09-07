# Architectural decision record (ADR) 008: Opt-in identifier artefact emission

## Status

Accepted.

## Context and decision

Procedural macros must not perform ambient writes during ordinary compilation.
When explicitly requested with `ORTHO_CONFIG_EMIT_IDENTIFIERS=1`, the derive
macro writes a provisional, schema-versioned JSON artefact below
`${OUT_DIR}/ortho-config`. It records standalone derived identifiers only;
mounted subcommand identifiers remain owned by the compiled documentation IR.

Each expansion writes an atomic fragment and merges fragments
deterministically. The merged document splits at 1 MiB and supplies an index
when needed. The schema is provisional until a consumer exists and marks every
entry with `path_scope: "standalone"`.

Cargo does not fingerprint proc-macro environment reads. After changing the
opt-in variable or source, force a fresh expansion with
`cargo clean -p <package> && ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build -p <package>`.
Do not export the variable in shell profiles or CI-wide environment blocks.

## Consequences

Normal builds do not write artefacts. Explicit emission can fail for filesystem
permissions and reports a derive diagnostic with remediation. Future translator
tooling must join standalone entries with mounted documentation IR rather than
pretending the artefact enumerates full command trees.
