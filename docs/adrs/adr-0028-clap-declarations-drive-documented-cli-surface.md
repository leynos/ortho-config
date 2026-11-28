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
from that declaration. A clap helper attribute only compiles when a clap derive
registered it, so one declaration proves the whole struct is a clap command:
every field without its own declaration is then documented from clap's derived
argument — a positional operand — rather than from the generated parser. A
struct with no declaration at all keeps the generated parser's long and short
flags. This is the accepted option.

The rule is decided at expansion time, so it works for `Args` payloads and
mixed structs without requiring a new trait bound. A bare `bool` without a
declaration is excluded because clap derives a value-less switch for it and
then rejects a positional switch; such a struct cannot be a valid clap command,
so the generated surface remains. A declaration whose value cannot be read
statically — a non-literal `value_name`, `id`, or `num_args` expression — is
documented without CLI metadata rather than falling back to a synthesized flag.

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
  requiredness. A declaration the derive cannot read statically is documented
  without CLI metadata: clap owns that argument, so advertising the generated
  parser's flag in its place would describe an invocation the parser rejects.
  The field keeps its environment and file metadata until a readable
  declaration or the standalone derive in roadmap 12.3.1 supplies the surface.
  Bare `long` and `short` spellings follow the container's
  `#[command(rename_all = "...")]` casing; literal spellings and an assigned
  `id` stay verbatim. Requiredness follows clap's inference: a declared default
  or a value-less switch makes the argument non-required, and clap treats
  `Option<T>` and `Vec<T>` as non-required. A value-taking scalar argument with
  no optional wrapper and no clap default is required by default, even when
  Serde supplies a `#[serde(default)]` for configuration loading; an explicit
  `required` setting overrides that inference either way. `CliMetadata.long` and
  `CliMetadata.short` are absent for a positional operand, and
  `CliMetadata.positional: Option<PositionalMetadata>` carries the one-based
  index and the variadic, trailing-var-arg, and `--`-only flags.
- In a struct with no clap argument declaration, a field keeps the generated
  long and short flags, because the configuration loader's hidden parser is the
  surface it exposes. In a struct with at least one declaration, every field
  without its own declaration is documented as clap's derived positional
  operand with the same one-based index. Such a configuration root still has
  two surfaces: the documentation describes the clap command line, and
  `OrthoConfig::load_from_iter` continues to accept the hidden parser's
  generated flag form.

`ORTHO_DOCS_IR_VERSION` becomes "2.1". Omitting `positional` preserves the
existing object shape, because it is
`#[serde(default, skip_serializing_if = "Option::is_none")]`, and readers that
predate the field ignore it. Version 2.1 is nevertheless a behavioural
correction rather than a byte-identical output: a clap boolean flag is now a
switch instead of an optional-value flag, and requiredness follows clap, so
values on existing clap-declared fields can change.

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
is the payload's clap command name when it implements `CommandFactory`: the
declared `#[command(name = "...")]`, or clap's package-name default. A docs-only
`Args` payload has no command of its own and keeps the selector's command
label. Explicit `env(name = ...)` and `file(key_path = ...)` documentation
overrides remain verbatim, because they are absolute declarations rather than
generated names. The generated `<FIELD>` suffix and `<key>` use Serde's
*deserialization* name — a field-level `#[serde(rename = "...")]` or
`#[serde(rename(deserialize = "..."))]`, plus the container `rename_all` rule —
because the loader deserializes those layers. A serialization-only rename does
not move the input name. A generated environment name is published only when
replaying the provider's prefix strip, uppercase, `__` split, and lowercase
transforms reproduces the key exactly. A camel-case, dotted, or
double-underscore key keeps its file key but receives no environment route,
because advertising a variable that populates a different key would mislead
callers.

The generated selector no longer rebuilds identifiers. It calls
`get_doc_metadata_for_subcommand`, so handwritten `OrthoConfigDocs` payloads
retain the provided fallback.

The roff renderer orders positional operands by index in SYNOPSIS and renders
their value names. An operand that consumes the rest of the command line
(`trailing_var_arg`) or is reachable only after `--` (`last`) follows the named
flags, because a flag placed after it would be parsed as operand data; a `last`
operand is also preceded by an emitted `--` delimiter. The PowerShell MAML
`position` attribute is one-based, matching clap's positional index, so the
renderer emits the index unchanged, displays the value name, and orders syntax
items by index rather than declaration order. Agent context lists a positional
as an input without a long flag, orders operands first by their one-based
index, and carries both the index and the value name. Those additions are
optional wire fields, omitted when absent, so the schema v1 contract remains
additive.

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

## Known risks and limitations

- A struct that derives a clap trait but declares no clap argument anywhere is
  indistinguishable from a configuration-only type. Its fields keep the
  generated long flags even though clap parses them positionally. Declaring any
  one argument, or an explicit `#[arg(skip)]`, makes the whole surface
  readable; the standalone derive in roadmap 12.3.1 removes the ambiguity.
- A bare `bool` field without a declaration is left on the generated surface:
  clap derives a value-less switch for it and rejects a positional switch, so
  the struct cannot be a valid clap command.
- A flattened container is not expanded into descendant field metadata. It
  keeps the generated surface as before, and roadmap 12.3.1 covers the
  expansion.
- A clap key whose value is a non-literal expression is not modelled. That
  field is documented without CLI metadata rather than with a synthesized flag
  that clap would reject.
- The `about_id` uses the declared base only when no parent path is supplied.
  A caller that mounts the struct somewhere else receives the mounted path,
  matching `LocalizeCmd` rather than the standalone constant.
- A configuration root that both derives a clap trait and is loaded through
  `OrthoConfig::load_from_iter` exposes two parsers. Documentation describes
  the declared clap command line; the hidden loader keeps its generated flag
  form, so a root should not rely on a clap-only positional when it is also
  loaded from an argument iterator.
- Duplicate environment names and file keys are validated in the standalone
  namespace. The mounted namespace is resolved at runtime from the payload's
  clap command name, or from the selector label for a docs-only `Args` payload,
  so a projected default can coincide with a verbatim `env(name = ...)` or
  `file(key_path = ...)` override only for that mount; the derive cannot
  compare the mounted names at expansion time.
- Agent-context ordering follows `CliMetadata.positional.index`. A field the
  derive did not classify as a positional keeps its generated flag and is
  ordered with the named flags.

## References

- [Issue 717](https://github.com/leynos/ortho-config/issues/717).
- [ADR-0003](adr-0003-define-schema-ownership-for-agent-native-contracts.md).
- [ADR-0005](adr-0005-subcommand-docs-companion-trait.md).
- [cargo-orthohelp design](../cargo-orthohelp-design.md) §2.2, §3.5, §12.
- [Roadmap](../roadmap.md) §12.2 and §12.3.
