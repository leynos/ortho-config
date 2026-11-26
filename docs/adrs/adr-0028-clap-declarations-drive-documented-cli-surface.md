# Architectural decision record (ADR) 0028: Clap declarations drive the documented CLI surface

Renumbered from 0027 on 2026-11-27 (duplicate number).

Status: Accepted.

Date: 2026-11-26.

## Context and problem statement

A subcommand argument struct often derives both `clap::Parser` (or
`clap::Args`) and `OrthoConfig`. Those derives describe two different command
lines:

- The user's clap declaration is what the application parses and what
  `SubcmdConfigMerge` reads through `T::command()`.
- The `OrthoConfig` derive's hidden `clap::Parser` struct is what
  `OrthoConfig::load_from_iter` parses when the type is loaded as a
  configuration root.

The documentation derive described only the second surface. A field declared as
`#[arg(value_name = "PROPOSAL")]` became a long flag `--proposal`, its value
name was dropped, and a boolean `#[arg(long)]` switch became an optional-value
flag. The generated man page therefore advertised a flag the binary rejects.

Subcommand metadata had two further mismatches. Environment variables and file
keys were reported in root form (`SPLITTERS_CHECK`, `check`) rather than the
`SPLITTERS_CMDS_VALIDATE_CHECK` and `cmds.validate.check` names the subcommand
merge reads. `OrthoConfigSubcommandDocs` also rebuilt the `about_id` from the
command label, so a struct declaring
`localization_base = "splitters.cli.validate"` reported `validate-about` while
its own `ABOUT_ID` was `splitters-cli-validate-about`.

## Decision drivers

- Documentation must describe the command line that executes, not an internal
  parser the caller never invokes.
- A type with no clap declaration must keep describing the generated parser,
  because `OrthoConfig::load_from_iter` really does parse that surface. A type
  that declares `#[arg(...)]` also derives a clap trait, so its explicit
  declaration denotes a real command line; a dual-surface type is documented
  from the declaration, and the hidden loader remains flag-based.
- `OrthoConfigSubcommandDocs` accepts payloads deriving `clap::Args`, which do
  not implement `clap::CommandFactory`; a fix cannot require that trait
  everywhere.
- Localization identifiers must agree with `ABOUT_ID` and `ARG_IDS`, or with
  the path `LocalizeCmd::with_base(..).localize(..)` resolves.
- The IR is a versioned public contract. Positional metadata is additive and
  omitted when absent.

## Options considered

### Option A: Read the user's clap `Command` at run time

The docs implementation would call `T::command().get_arguments()` and mirror
every argument exactly.

This is rejected for the general case. `clap::Args` payloads do not implement
`CommandFactory`, and a configuration root that derives both traits would
suddenly describe the application's parser rather than the generated parser that
`load_from_iter` actually runs. The derive cannot detect which use-site
applies when metadata is built.

### Option B: Explicit clap declarations win

A field carrying an `#[arg(...)]` or `#[clap(...)]` declaration is described
from that declaration. Fields without one keep the generated parser's long and
short flags. This is the accepted option.

The rule is decided per field at expansion time, so it works for `Args`
payloads and mixed structs without requiring a new trait bound. A field cannot
carry `#[arg(...)]` unless a clap derive has registered the helper attribute,
so an explicit declaration always denotes a real clap command line. A
configuration root with no declaration is unaffected and keeps the generated
surface. Non-literal clap values that cannot be read statically fall back to
the generated surface rather than rejecting valid clap code.

### Option C: Keep the generated surface and add an opt-out

Fields could opt out of generated long or short flags with a new attribute.

This was rejected as the primary fix. It leaves the common case wrong until
every author adds attributes, and it still cannot express a positional
operand's index, value name, or variadic settings. The opt-out remains useful
for configuration-only fields, but it is not a substitute for reading the clap
declaration that already exists.

### Option D: A standalone docs-only derive

Roadmap 12.3.1 plans an `OrthoConfigDocs` derive that reads clap attributes
without requiring `Deserialize`, `Default`, or loader machinery.

This is the complete long-term answer for clap-only argument structs, but it is
a new derive rather than a correction to the metadata the existing derive
emits. The positional model and the projection rules introduced here are shared
with that future derive.

## Decision outcome

The documentation IR distinguishes two surfaces per field:

- A field with an explicit clap argument declaration is documented from it.
  The parser records long and short spellings, the value name, positional index,
  `num_args`, `trailing_var_arg`, `last`, value consumption, visibility, and
  requiredness. Bare `long` and `short` spellings follow the container's
  `#[command(rename_all = "...")]` casing; literal spellings and an assigned
  `id` stay verbatim. Requiredness follows clap: a declared default or a
  value-less switch makes the argument non-required, while a value-taking
  argument with no clap default is required even when Serde supplies a
  `#[serde(default)]` for configuration loading. `CliMetadata.long` and
  `CliMetadata.short` are absent for a positional operand, and
  `CliMetadata.positional: Option<PositionalMetadata>` carries the one-based
  index and the variadic, trailing-var-arg, and `--`-only flags.
- A field without such a declaration keeps the generated long and short flags,
  because the configuration loader's hidden parser is the surface it exposes. A
  configuration root that also declares clap arguments has two surfaces; the
  documentation describes the declared clap command line, and
  `OrthoConfig::load_from_iter` continues to accept the hidden parser's
  generated flag form for that root.

`ORTHO_DOCS_IR_VERSION` becomes "2.1". A flag-only command surface serializes
exactly as before apart from the version string, because `positional` is
`#[serde(default, skip_serializing_if = "Option::is_none")]`. Readers that
predate the field ignore it.

`OrthoConfigDocs` gains a provided method
`get_doc_metadata_for_subcommand(parent_path, name)`. The generated override
chooses the identifier path as follows:

- with a non-empty parent path, `parent_path` plus `name`, matching
  `LocalizeCmd`'s mounted walk;
- with an empty parent path and an explicit `localization_base`, the declared
  base, matching the struct's `ABOUT_ID` and `ARG_IDS`;
- with an empty parent path and no declared base, the command label, preserving
  the previous collision-free behaviour.

Environment and file metadata are projected into the namespaces the subcommand
merge reads: `<PREFIX>_CMDS_<NAME>_<FIELD>` and `cmds.<name>.<key>`. `<NAME>`
is the payload's explicit `#[command(name = "...")]` where declared, otherwise
the command label the selector supplies. Explicit `env(name = ...)` and
`file(key_path = ...)` documentation overrides remain verbatim, because they
are absolute declarations rather than generated names.

The generated selector no longer rebuilds identifiers. It calls
`get_doc_metadata_for_subcommand`, so handwritten `OrthoConfigDocs` payloads
retain the provided fallback.

The roff renderer orders positional operands by index in SYNOPSIS and renders
their value names. An operand that consumes the rest of the command line
(`trailing_var_arg`) or is reachable only after `--` (`last`) follows the named
flags, because a flag placed after it would be parsed as operand data; a `last`
operand is also preceded by an emitted `--` delimiter. The PowerShell MAML
renderer maps a positional's one-based clap index to MAML's zero-based
`position` attribute and displays the value name. Agent context continues to
list a positional as an input without a long flag; ordering positionals by
index remains roadmap 12.2.3.

## Goals and non-goals

- Goals:
  - Stop advertising flags that the application's parser rejects.
  - Preserve clap value names and positional information in the IR.
  - Use the subcommand merge's environment and file namespaces.
  - Keep localization identifiers consistent with generated constants.
- Non-goals:
  - Make every configuration field positionally invocable.
  - Replace the generated parser with the user's clap declaration.
  - Model struct-level clap features beyond argument-name casing, such as
    `rename_all_env` or generated help headings.
  - Deliver the standalone docs-only derive from roadmap 12.3.1.
  - Order positional inputs in agent context by index (roadmap 12.2.3).

## Known risks and limitations

- A field that relies on clap's default positional treatment but declares no
  `#[arg(...)]` key at all is indistinguishable from a configuration-only
  field. It keeps the generated long flag. Authors can declare a value name or
  index to make the operand explicit; the standalone derive in roadmap 12.3.1
  removes the ambiguity.
- A flattened container is not expanded into descendant field metadata. It
  keeps the generated surface as before, and roadmap 12.3.1 covers the
  expansion.
- A clap key whose value is a non-literal expression is not modelled. That
  field keeps the generated surface rather than risking a wrong claim.
- The `about_id` uses the declared base only when no parent path is supplied.
  A caller that mounts the struct somewhere else receives the mounted path,
  matching `LocalizeCmd` rather than the standalone constant.
- A configuration root that both derives a clap trait and is loaded through
  `OrthoConfig::load_from_iter` exposes two parsers. Documentation describes
  the declared clap command line; the hidden loader keeps its generated flag
  form, so a root should not rely on a clap-only positional when it is also
  loaded from an argument iterator.
- Duplicate environment names and file keys are validated in the standalone
  namespace. The mounted namespace appends a runtime command name, so a
  projected default can coincide with a verbatim `env(name = ...)` or
  `file(key_path = ...)` override only for the mount that supplies that name;
  the derive cannot see the mount at expansion time.
- Positional operands are listed in agent context but not yet ordered by
  index. Consumers can read the order from `CliMetadata.positional`.

## References

- [Issue 717](https://github.com/leynos/ortho-config/issues/717).
- [ADR-0003](adr-0003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-0005](adr-0005-subcommand-docs-companion-trait.md).
- [cargo-orthohelp design](../cargo-orthohelp-design.md) §2.2, §3.5, §12.
- [Roadmap](../roadmap.md) §12.2 and §12.3.
