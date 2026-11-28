# 8.3.3 Evaluate an optional macro attribute for Cargo subcommand wrappers

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (community-of-experts design review applied; awaiting user
approval; no implementation has started)

Branch: `8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute`

Roadmap item: `8.3.3` in `docs/roadmap.md`.

## Purpose / big picture

Cargo runs an installed tool named `cargo-<name>` whenever a user types
`cargo <name> [OPTIONS]`. It does not pass the options through unchanged: it
inserts the subcommand name as the first argument, so the tool receives
`["cargo-<name>", "<name>", OPTIONS...]`. A `clap` parser that models only
`cargo-<name> [OPTIONS]` rejects that injected `<name>` token before any
application code runs. This rule is the *Cargo dispatch contract*.

OrthoConfig already supports the contract in two ways:

- Roadmap item 8.3.1 shipped a helper,
  `ortho_config::cargo::external_subcommand`. It wraps a hand-built
  `clap::Command` in a synthetic `cargo` parent.
- Roadmap item 8.3.2 documented a derive template. A `#[derive(clap::Parser)]`
  struct with `#[command(name = "cargo", bin_name = "cargo-<name>")]` holds a
  single-variant `#[derive(clap::Subcommand)]` enum, which wraps the existing
  options struct.

`docs/design.md` §4.17 also floats a third option, which has never been built:
a derive attribute that would make the macro write the wrapper.

```rust
#[ortho_config(cargo_subcommand = "orthohelp", cargo_bin = "cargo-orthohelp")]
```

This plan answers one question: should that attribute exist? It prototypes the
attribute's syntax and its candidate expansions against the real
OrthoConfig-powered Cargo tools. It scores the results against criteria fixed
before the prototype runs, and records the decision in an architectural
decision record (ADR). The roadmap's success criterion is that "the design
records either a narrow accepted macro surface or a clear reason to keep the
helper and documentation as the only supported abstraction".

When the work is complete, a maintainer reading `docs/design.md` §4.17 finds a
decision rather than an open candidate. A downstream author who types the
floated syntax gets a compiler error naming the supported alternative. Today
that syntax compiles and silently does nothing (see `Surprises & discoveries`).

On the rejection path, nothing changes at run time: no configuration precedence
and no existing API. The new compile error is a source-level change, released
in the next minor version with migration guidance. If the evidence instead
favours a macro, the plan stops. It then returns a specific, narrow surface for
approval before any public API is added.

The reconnaissance already points one way (prediction P1 in `Decision log`).
The plan still runs the prototype and gives every candidate its strongest form,
so the evidence, not the prediction, decides.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- Do not move Cargo dispatch semantics into the `OrthoConfig` trait,
  `OrthoConfig::load`, or the merge pipeline (`docs/design.md` §4.17;
  ADR-0004). Configuration precedence stays: defaults, then files, then the
  selected profile (opt-in), then environment, then explicit command-line
  arguments.
- Preserve the public signature and behaviour of
  `ortho_config::cargo::external_subcommand`
  (`ortho_config/src/cargo/mod.rs:105`).
- Preserve the documented templates and their tested examples:
  `docs/users-guide.md` §"Derive-based template" and `README.md` §"Cargo
  subcommands". Changing either template needs explicit approval (question Q3).
- Do not edit ADR-0004. Its decision stands on every path. Any change to it
  goes through the amendment process in `docs/documentation-style-guide.md` and
  requires escalation.
- Add no production dependency, and no dev-dependency to any crate.
  `ortho_config` already has `rstest`, `rstest-bdd`, `googletest`,
  `pretty_assertions`, `insta`, `trybuild`, and `proptest`
  (`ortho_config/Cargo.toml:56-73`). `ortho_config_macros` has `rstest`,
  `trybuild`, and `proptest`, but not `googletest` or `pretty_assertions`. Its
  tests therefore use the crate's existing `anyhow::ensure!` style.
- On the rejection path, exactly one behavioural change is permitted: a
  targeted compile error for the struct-level keys `cargo_subcommand` and
  `cargo_bin` (decision D6). It reaches the two derives that parse struct-level
  attributes through `parse_input`: `OrthoConfig` and `RequiredConfigSchema`
  (`ortho_config_macros/src/lib.rs:46,129`). Every other unknown struct-level
  key stays silently discarded, as `parse_struct_attrs` documents
  (`ortho_config_macros/src/derive/parse/struct_attrs.rs:309-316`).
- No Rust source file may exceed 400 lines (`AGENTS.md`).
  `ortho_config_macros/src/derive/parse/struct_attrs.rs` is at 397 lines, so it
  may grow by at most 3 lines.
- Follow the testing rules in `AGENTS.md`:
  - every assertion carries a diagnostic message;
  - the environment is injected (`MapEnv`) and never mutated in-process;
  - only a child process may set environment variables.
- The prototype is evidence, not product. It lives in one auto-discovered
  integration-test target. On the rejection path it is deleted, not promoted.
  It must pass every gate while it exists.
- Completed ExecPlans (`8-3-1-…`, `8-3-2-…`) are historical and must not be
  edited.
- Gates run in sequence through a `scrutineer` agent, with `tee` logs under
  `/tmp`. Respect compile admission: never bypass `RUSTC_WRAPPER`, never use an
  isolated Cargo cache, and never build under `/tmp`.
- `typos.toml` is generated. Spelling exceptions go only in
  `typos.local.toml`. The spelling standard is en-GB-oxendict, with `-ize`
  endings.

## Tolerances (exception triggers)

ExecPlan files are excluded from every count below, including this file.

### Survey basis

The 8.3.1 plan set one numeric budget: about 150 net production lines and at
most 10 files. Its outcomes record about 120 production lines, and no increase
was needed.

The 8.3.2 plan set only qualitative triggers. Its scope then grew through
milestones M3–M5 (recipes, a merge example, a build test) without any recorded
tolerance breach. Its recipes, pinned to 0.10.0, also broke on the 0.11.0 merge
commit.

Two lessons follow. Each category here gets a numeric limit, so unrecorded
growth trips an escalation. No documentation example hard-codes a release
version.

### Counting rule

Count only at a commit, never against the working tree. `git diff` does not see
untracked files. The comparison base is the merge base with `origin/main`.

```bash
base=$(git merge-base HEAD origin/main)
git diff --numstat "$base" HEAD -- . \
  ':!docs/execplans/8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute.md' \
  ':!typos.toml' |
awk '
  { path=$3; net=$1-$2 }
  path ~ /(^|\/)(Cargo\.toml|Cargo\.lock|Makefile|typos\.local\.toml)$/ \
    || path ~ /^(\.config|\.github)\// { cfg+=net; nf++; next }
  path ~ /\.md$/ { doc+=net; nf++; next }
  { code+=net; nf++ }   # .rs, .stderr, .feature, .snap, .py and anything else
  END { printf "files=%d code=%d config=%d docs=%d\n", nf, code, cfg, doc }'
```

Classification is by whole file:

- Configuration: manifests, lockfiles, `Makefile`, `.config/**`,
  `.github/**`, and `typos.local.toml`.
- Documentation: every other `*.md` file.
- Code: everything else, including `.rs` and trybuild `.stderr` files. This
  catch-all means no path goes uncounted.
- `typos.toml` is generated and excluded. If it changes, commit it separately
  as `AGENTS.md` §Spelling requires.

### Limits

- Files: escalate if the final diff touches more than 15 non-ExecPlan files.
- Code (net, at the delivery commit): escalate above 200. Expected use is
  about 110: the diagnostic module, its unit and property tests, and one
  trybuild pair.
- Prototype (transient code, measured at its own commit): escalate above 650
  lines across the prototype target, or if any single file exceeds 400 lines.
  The prototype is deleted before delivery, so it does not count towards the
  final code budget.
- Configuration: escalate above 0 net lines. The prototype target is
  auto-discovered (`ortho_config` does not set `autotests = false`), so it
  needs no `[[test]]` stanza.
- Documentation (net): escalate above 480. Expected use is about 350:

  | Document                          | Approximate lines |
  | --------------------------------- | ----------------- |
  | ADR                               | 200               |
  | Design §4.17                      | 25                |
  | User's guide                      | 25                |
  | Developers' guide                 | 30                |
  | `docs/v0-12-0-migration-guide.md` | 25                |
  | CHANGELOG                         | 12                |
  | `docs/contents.md`                | 6                 |
  | Roadmap                           | 12                |

- `struct_attrs.rs`: escalate if it would exceed 400 lines.

### Other triggers

- Interface: escalate if any public item, other than the new diagnostic,
  must change.
- Architecture: escalate and set `BLOCKED` in either of these cases:
  - the evidence favours candidate D, so `OrthoConfig::load` would accept
    the injected token;
  - any candidate passes the acceptance rubric.

  Either outcome changes the boundary that ADR-0004 protects, or adds a public
  macro surface.
- Iterations: escalate after three unsuccessful repairs of the same gate.
- Infrastructure: escalate on disk exhaustion or a `build-limits:` refusal.
  Waiting for an admitted compile slot is normal and does not justify a retry.
- Ambiguity: escalate if two consumers' shapes support conflicting readings
  of a rubric gate. Present both readings.

## Risks

### Confirmation bias

- **Risk:** the reconnaissance already predicts rejection (P1), so a weak
  prototype could "confirm" it.
- **Severity:** high. **Likelihood:** medium.
- **Mitigation:**
  - The rubric (D2, revised in D11) is fixed before EP-M1, and its
    thresholds come from measured baselines.
  - Gate G1 is judged against the whole `#[ortho_config]` namespace, not
    only the `OrthoConfig` derive.
  - Each candidate is prototyped in its strongest form, including candidate
    E (D12).
  - A read-only `wyvern` agent that is not shown P1 recounts the census
    independently.

### ADR number collision

- **Risk:** `adr-0028-*` already exists on two open branches:
  - `origin/11-2-1-ship-en-us-translations-for-clap-stable-error-matrix`
    (PR #588);
  - the `issue-717` branch (PR #753).
- **Severity:** medium. **Likelihood:** high.
- **Mitigation:**
  - Allocate the ADR number at EP-M2, as the next number free on
    `origin/main` and on every open-PR branch, using
    `git ls-tree -r --name-only <ref> docs/adrs`.
  - Recheck the number at EP-M5, just before the PR leaves draft.
  - Follow the renumbering rule in the style guide's naming section.

### Estate drift

- **Risk:** the census reads other repositories, which keep moving.
- **Severity:** medium. **Likelihood:** high.
- **Mitigation:** pin each repository to a commit printed by `gh api`. Cite
  pinned line ranges, and quote permalinked excerpts in the ADR.

### Incomplete census

- **Risk:** only repositories owned by `leynos` were searched.
- **Severity:** low. **Likelihood:** medium.
- **Mitigation:** the ADR states the search command and its scope, and
  records a revisit trigger (D13).

### Source-breaking diagnostic

- **Risk:** the new compile error breaks a downstream crate that already
  wrote the floated syntax.
- **Severity:** low. **Likelihood:** low.
- **Mitigation:**
  - A search on 2026-11-28 found no attribute-form use
    (`Surprises & discoveries`).
  - The change ships in the next minor release (0.12.0), not a 0.11.x patch.
  - It carries a `**Breaking:**` CHANGELOG entry and a migration-guide row.
  - The crates are pre-1.0, so no compatibility shim is warranted.

### trybuild `.stderr` drift

- **Risk:** a trybuild `.stderr` file captures rustc's snippet rendering,
  which changes when the nightly pin moves.
- **Severity:** low. **Likelihood:** medium.
- **Mitigation:**
  - The unit and property tests pin the message text, so the `.stderr` is a
    secondary check.
  - The fixture is minimal: one derive and one field.
  - The ADR states that regenerating the `.stderr` on a toolchain bump is
    expected maintenance.

### Stale guide anchor in the error text

- **Risk:** the compile error names a guide heading that is later renamed.
- **Severity:** low. **Likelihood:** medium.
- **Mitigation:** the message names only the stable Rust path
  `ortho_config::cargo::external_subcommand` and "the user's guide". It quotes
  no heading.

### Line budget in `struct_attrs.rs`

- **Risk:** the file is at 397 lines, and the call site plus a doc-comment
  rewrite could push it past 400.
- **Severity:** low. **Likelihood:** medium.
- **Mitigation:**
  - The new module exposes a single query function, called with a fully
    qualified path. That avoids adding an import.
  - The doc comment is rewritten line-neutrally.
  - `wc -l` at 400 or below is an acceptance check.

### Prototype build cost

- **Risk:** the prototype target adds a cold compile and link to `make test`
  while it exists.
- **Severity:** low. **Likelihood:** high.
- **Mitigation:**
  - Use a single target with `#[path]` submodules.
  - It spawns no child `cargo` and runs no trybuild, so it falls under
    nextest's default allowance.
  - It is deleted in EP-M2.

## Progress

- [x] (2026-11-28) Renamed the branch and set the Lody session title.
- [x] (2026-11-28) Wyvern reconnaissance covered:
  - the helper and its consumers;
  - the derive attribute machinery;
  - the documentation and prior plans;
  - the test tooling.
- [x] (2026-11-28) Prior-art research (the Cargo book, clap's cookbook,
  `clap-cargo`) and an estate census of OrthoConfig-powered `cargo-*` tools.
- [x] (2026-11-28) Wrote the initial draft and committed it.
- [x] (2026-11-28) Ran a community-of-experts design review with four panels:
  - contracts and structure;
  - alternatives and rubric;
  - failure modes and verification;
  - viability and documentation governance.

  Findings were applied as D10–D15 and the revisions below.
- [ ] Plan approved by the user, with Q1–Q3 answered.
- [ ] `EP-M1`: prototype, `macro_rules!` syntax stand-in, and census evidence
  committed. All gates green.
- [ ] `EP-M2`: decision gate scored and the ADR drafted as `Proposed`.
  Then, depending on the result:
  - rejection path: prototype retired;
  - acceptance path: plan set to `BLOCKED`.
- [ ] `EP-M3`: targeted diagnostic for the rejected keys (rejection path).
- [ ] `EP-M4`:
  - ADR `Accepted`;
  - design document, guides, migration guide, CHANGELOG and index updated;
  - roadmap 8.3.3 marked done.
- [ ] `EP-M5`: ADR number rechecked, PR ready for review, CI green.

## Surprises & discoveries

### The floated syntax compiles today and does nothing

- **Evidence:** `parse_struct_attrs` sends unrecognized struct-level keys to
  `discard_unknown` (`ortho_config_macros/src/derive/parse/struct_attrs.rs`,
  lines 365-373 and 43-52). The unit test
  `parse/tests/ortho_attrs_unknown_keys.rs:9-19` asserts that
  `unknown = "ignored"` is accepted.
- **Impact:** someone who reads design §4.17 can write
  `#[ortho_config(cargo_subcommand = "x")]` today and believe a wrapper was
  generated. They find out otherwise only when Cargo dispatch fails at run
  time. This motivates D6.

### Diagnostic coverage differs by placement

- **Evidence:**
  - Two enum derives already reject every key except `crate`, with
    "unsupported ortho_config option on enum": `OrthoConfigSubcommandDocs`
    (`subcommand_docs.rs:22-24`) and `SelectedSubcommandMerge`
    (`selected_subcommand_merge.rs:58-60`).
  - At field level, unknown keys are discarded (`field_attrs.rs:84-90`).
  - `RequiredConfigSchema` shares the struct-level parser
    (`ortho_config_macros/src/lib.rs:129`, then `parse_input`).
- **Impact:**
  - D6 covers struct level on `OrthoConfig` and `RequiredConfigSchema`.
  - Enum placement already fails, though without naming the alternative.
  - Field level stays out of scope, because the syntax was only ever
    proposed at struct level.

### Repository precedents for targeted rejection

- **Evidence:** two targeted rejections already exist, each inside its own
  feature module:
  - `behavior` in `doc_attrs.rs:35,47-55`;
  - `localized_default` in `localization.rs` (`reject_localized_default`).
- **Impact:** the new rejection lives in a feature-named module (D10), not a
  general registry.

### No consumer's wrapped type derives `OrthoConfig`

- **Evidence:**
  - `cargo-orthohelp`'s `Args` derives only `ClapArgs` and `Clone`
    (`cargo-orthohelp/src/cli/mod.rs:73-74`).
  - `cargo-bdd` wraps `Cli`, which derives only `Parser`.
  - `cargo-theorem` wraps `TheoremArgs`, which derives only `Args` and
    implements `OrthoConfigDocs` by hand.
  - Each tool keeps `OrthoConfig` on leaf argument types merged per selected
    subcommand. One example is `cargo-theorem`'s `ContextArgs`.
  - `cargo-theorem`'s wrapper enum `CargoCommand` does derive
    `OrthoConfigSubcommandDocs`, so it already carries the `#[ortho_config]`
    namespace.
- **Impact:** gate G1 must be scored over every derive that accepts
  `#[ortho_config]` (D11), or it decides the outcome by definition.

### The consumers disagree on the shape a macro would emit

- **Evidence:**
  - `cargo-orthohelp` and `cargo-theorem` use `bin_name = "cargo"`. The
    documented derive template uses `bin_name = "cargo-<name>"`.
  - `cargo-bdd` parses both the Cargo-injected form and a flat direct form,
    and records which one it saw.
  - `cargo-theorem` puts `version` on both levels, derives
    `OrthoConfigSubcommandDocs` on the wrapper enum, and parses through
    `parse_localized_command`.
- **Impact:** see gate G3. Only the variance points that change the
  expansion count against a candidate (D11).

### Documentation metadata is the one repetition clap does not already shorten

- **Evidence:** `cargo-theorem`'s `command_metadata()` patches a `DocMetadata`
  by hand to describe the `cargo-theorem` → `theorem` → `context` tree for
  agent-context output (pinned lines 117-127). `cargo-orthohelp`'s agent
  context builds the command path from `bin_name`
  (`cargo-orthohelp/src/agent_context/mod.rs:316-321`).
- **Impact:** candidate C is about correct documentation output, not line
  count. A plain function may serve it better than an attribute (D9).

### clap's cookbook writes the whole wrapper as one derived enum

- **Evidence:** clap's `cargo_example_derive` cookbook uses a single
  `#[derive(Parser)]` enum with
  `#[command(name = "cargo", bin_name = "cargo")]`, unpacked by an irrefutable
  `let`.
- **Impact:** that form sets a lower floor than the 8.3.2 template's
  struct-plus-enum spelling. It is scored as option F, and changing the
  documented template needs approval (Q3).

### No downstream code uses the floated syntax

- **Evidence:** a search on 2026-11-28 using
  `gh search code 'cargo_subcommand' --owner leynos` matched only identifier
  names, such as test-support helpers in `leynos/theoremc`. It found no
  `#[ortho_config(` use. Phrase searches for
  `"cargo_subcommand =" ortho_config` and `"cargo_bin =" ortho_config` returned
  nothing. GitHub code search tokenizes punctuation, so the pinned consumers
  were also read directly.
- **Impact:** D6 is safe to ship in the next minor release.

### ADR-0028 is already claimed on two open branches

- **Evidence:** listed under the ADR-number-collision risk.
- **Impact:** the ADR number is allocated at EP-M2 (D5).

## Decision log

### P1. Pre-registered prediction (not a decision)

Every candidate fails at least one rubric gate. The outcome keeps the helper
and the documented templates as the only supported abstraction. The plan treats
this as a hypothesis that EP-M1 can falsify.

### D1. Prototype the syntax with a `macro_rules!` stand-in and handwritten expansions

Do not implement a proc-macro (a compiler plug-in that generates code from
attributes) until the evidence supports it.

The prototype lives in one disposable integration-test target:
`ortho_config/tests/cargo_wrapper_prototype.rs`, with submodules under
`ortho_config/tests/cargo_wrapper_prototype/`. Inside it, a test-local
`macro_rules!` named `cargo_wrapper_prototype!` takes the candidate key syntax:

```rust
cargo_wrapper_prototype! {
    #[derive(...)] #[command(...)]   // pass-through attributes
    for Options;
    cargo_subcommand = "demo", cargo_bin = "cargo-demo"
    $(, knob = value)*
}
```

The stand-in emits candidates A and B for the given options type. It also checks
`cargo_bin == concat!("cargo-", cargo_subcommand)` with a `const` assertion.
Candidate C's metadata and the `DocMetadata` it implies are written by hand.

Rationale:

- The open questions concern the expansion and how well it fits the
  consumers, not whether `syn` can parse two string keys. Comparable keys are
  already parsed in `struct_attrs.rs` and `doc_attrs.rs`.
- The stand-in proves that the two keys, plus any knobs, are enough input.
  It makes each knob's syntax concrete. It needs no proc-macro crate, no
  dependency and no extra compile target.
- If the decision is to accept, the real proc-macro is implemented under a
  revised plan.

### D2. Accept a candidate only if it passes all five rubric gates (revised by D11)

The gates are G1 legal host, G2 net savings, G3 variance coverage, G4 a visible
dispatch contract, and G5 conformance with ADR-0004.

Rationale: the roadmap's rejection rule is "unless it removes real repeated
boilerplate across multiple OrthoConfig-powered Cargo tools without hiding the
Cargo dispatch contract".

- G1–G3 make "real repeated boilerplate across multiple tools" testable.
- G4 and G5 make "without hiding the contract" testable.

### D3. The evidence base is the pinned estate census

The census covers every Cargo tool owned by `leynos` that depends on
`ortho_config`, pinned to printed commits. Three tools qualify:

- `cargo-orthohelp` (this repository);
- `cargo-bdd` (`leynos/rstest-bdd`);
- `cargo-theorem` (`leynos/theoremc`).

The documentation-harness `cargo-demo` binaries are templates, not independent
tools. They are reported separately and do not count towards "multiple tools".

Rationale: the roadmap asks for repetition *across tools*, and templates repeat
by construction.

### D4. Candidate D is scored on paper only

Candidate D would make the generated `load()` accept the injected token, by
wrapping the hidden `__<Name>Cli` parser.

Rationale: design §4.17 and ADR-0004 place dispatch at the command boundary,
outside `OrthoConfig::load`. If D scores as the only viable candidate, that is
an architecture deviation: set `BLOCKED` and escalate.

### D5. Record the outcome in a new ADR; allocate its number at EP-M2

The ADR is titled
`# Architectural decision record (ADR) nnnn: Cargo subcommand wrapper attribute`.
It is referenced from design §4.17. ADR-0004 is not edited.

Rationale:

- Rejecting, or narrowly accepting, a new macro surface is a decision in its
  own right.
- The style guide reserves amendment for "a targeted correction", and ADR-0004
  needs no correction.
- ADR-0004's own dated in-place "Amendment" and "Clarification" sections
  predate the banner convention. They are context, not a template.

The ADR is `Proposed` at EP-M2 and `Accepted` at EP-M4, under the style guide's
status rules.

### D6. Make `cargo_subcommand` and `cargo_bin` at struct level a targeted compile error (rejection path)

Every other unknown key stays silently discarded.

Rationale: design §4.17 publicized this exact syntax, and today it fails
silently. That is the worst outcome for a Cargo tool, because the failure only
appears under real Cargo dispatch. The repository already rejects specific
known-wrong keys inside their feature modules (`behavior`, `localized_default`)
while keeping other keys forward-compatible.

Alternative considered: documentation only. Rejected, because documentation
cannot reach a user who has already copied the syntax from design §4.17.

Wording: "not supported", not "does not generate", so that a later,
backward-compatible acceptance stays open (D13).

### D7. No Verus or Kani obligation on either path

See `Verification plan` §"Formal verification decision".

### D8. Commit the prototype, then delete it in a separate commit

The prototype is committed in EP-M1, gated, scored, and deleted in its own
commit in EP-M2. Its measured results and key expansions are reproduced in the
ADR's evidence subsections, so the decision survives a squash merge.

### D9. Documentation metadata is the only defensible target, and a function serves it better than an attribute

Candidate C's motivating case is `cargo-theorem`'s hand-built `DocMetadata`.
Before proposing anything new, the prototype measures how much of that patch
the existing struct-level `bin_name` key already removes.

The ADR's "Decision outcome / proposed direction" states that documentation
metadata is the only target the evidence supports for this surface. It also
states that a plain generic function serves it better than an attribute. One
candidate signature is:

```rust
ortho_config::cargo::entry_doc_metadata::<E: OrthoConfigSubcommandDocs>(installed_bin, base)
```

That function is deferred, not implemented: it has one consumer, so it fails
the "multiple tools" test as well. Whether to add a roadmap item now is
question Q2.

### D10. Abstraction sweep, and where the diagnostic lives

The `AGENTS.md` abstraction policy requires a sweep for existing equivalent
behaviour. The sweep found `misspelt_behaviour` in `doc_attrs.rs` and
`reject_localized_default` in `localization.rs`. Both are inline match arms
inside feature-scoped key families.

Neither existing module is a fit:

- `doc_attrs.rs` holds documentation keys, and these keys are not
  documentation keys.
- `struct_attrs.rs` cannot take another match arm within its line cap.

The rejection therefore goes in a new feature-named module,
`ortho_config_macros/src/derive/parse/cargo_keys.rs`. It exposes one pure query:

```rust
pub(super) fn rejected_cargo_key_error(meta: &syn::meta::ParseNestedMeta) -> Option<syn::Error>
```

It is called from the `_` arm of `parse_struct_attrs` as:

```rust
cargo_keys::rejected_cargo_key_error(meta).map_or_else(|| discard_unknown(meta), Err)
```

Reuse policy:

- The module owns only the Cargo wrapper keys rejected by the ADR.
- Its only permitted caller is `parse_struct_attrs`.
- Other rejected keys stay with their own key families.

### D11. Rubric revisions from the design review

**G1.** A legal host must exist *within the `#[ortho_config]` attribute
namespace*, on a type the consumer already writes. G1 is scored for all four
derives that accept the namespace:

- `OrthoConfig`;
- `RequiredConfigSchema`;
- `SelectedSubcommandMerge`;
- `OrthoConfigSubcommandDocs`.

The score depends on the evidence, not on G1's wording.

**G2.** The threshold is set by rule, not picked in advance. A candidate must
save, net, at least half the smallest measured consumer baseline (rounded up),
in at least two consumers. Repeated name strings are reported as a secondary
metric. That metric can explain a result but cannot pass G2 on its own.

The reviewers' preliminary counts were:

- `cargo-orthohelp`: about 16;
- `cargo-theorem`: about 17;
- `cargo-bdd`: about 12.

That suggests a threshold of 6. Final numbers come from the EP-M1 census.

**G3.** Each variance point is classified as either *expansion-changing* or
*orthogonal*:

- Expansion-changing: v1 usage spelling, v2 `version` placement, v3 extra
  derives on the wrapper, v7 `subcommand_required`.
- Orthogonal: v4 dual-form parsing, v5 localized parsing, v6 hint errors. A
  generated `Parser` type works with these unchanged.

Only expansion-changing points count against a candidate. The limit is at most
two knobs beyond `cargo_subcommand` and `cargo_bin`. Each knob re-exposes a clap
`#[command]` setting, so more than two means the macro is re-implementing
clap's attribute surface.

**Candidates A and B** require the host to derive both `OrthoConfig` and
`clap::Parser`. The derive emits only a private `__<Name>Cli`, so `<Name>`
otherwise has neither `CommandFactory` nor `Args`. A derive also cannot see its
sibling derives. The error a user sees when `Parser` is missing is recorded as
G4 evidence.

Candidate A in its strongest form keeps the inner `ArgMatches`, which
`load_and_merge_with_matches` needs. It therefore exposes
`into_parts(self, &ArgMatches) -> (<Name>, &ArgMatches)` rather than only
`into_inner`.

### D12. Add candidate E (library `macro_rules!` entry macro) and option F (clap's one-enum cookbook form)

E is prototyped through the same stand-in machinery and scored on every gate.
It lies outside the roadmap's `#[ortho_config]` syntax. If that is its only
failing gate, the ADR lists it as *deferred*, not *rejected*.

F is scored too. Adopting it would change the documented templates, which needs
approval (Q3).

### D13. Rejection is reversible only by superseding the ADR, and the ADR records a revisit trigger

The trigger is two or more consumers that have a legal host, or a second
consumer of documentation metadata. The ADR records:

- the exact census commands;
- the pinned commits;
- permalinked excerpts.

### D14. Changelog and migration

The diagnostic ships in the next minor release, 0.12.0. It gets:

- a `### Changed (derive attributes)` CHANGELOG entry that opens with
  `**Breaking:**`;
- a row in the "Impact at a glance" table of
  `docs/v0-12-0-migration-guide.md`;
- a short section in that guide quoting the new error.

The guide currently says "No code changes are required". That sentence is
qualified, because the change only affects code that used a syntax which never
worked.

### D15. Commit plan

Red tests cannot pass the gates, so each EP-M3 red step is recorded as a `tee`
transcript, and the red and green states land in one commit. The commits are:

| Milestone | Commits                                                                                              |
| --------- | ---------------------------------------------------------------------------------------------------- |
| EP-M1     | The prototype and the plan's evidence, together                                                      |
| EP-M2     | (a) Proposed ADR and plan scoring; (b) prototype retirement                                          |
| EP-M3     | Diagnostic, tests and `.stderr`                                                                      |
| EP-M4     | (a) ADR accepted plus design §4.17; (b) guides and migration guide; (c) CHANGELOG, index and roadmap |
| EP-M5     | Review fixes, one commit per finding cluster                                                         |

## Outcomes & retrospective

Not started. Complete this at EP-M5 with:

- the decision;
- the rubric scores;
- final tolerance use per category;
- lessons learned.

## Context and orientation

The repository is a Cargo workspace (root `Cargo.toml:2-14`). The parts that
matter here are described below.

### The helper

`ortho_config/src/cargo/mod.rs` (135 lines) defines
`external_subcommand(installed_bin_name, subcommand_name, command)`.

- It returns
  `Command::new("cargo").bin_name("cargo").subcommand_required(true)`, with the
  renamed inner command as the only subcommand.
- The inner command's `bin_name` is reset, and its `display_name` is set to
  the installed name.
- Debug assertions require:
  - a non-empty name;
  - a name other than `help`;
  - `installed_bin_name == "cargo-<name>"`.
- The helper does no tracing and reads no environment.

The helper's tests are:

- unit tests in `ortho_config/src/cargo/tests.rs`;
- the property `wrapper_preserves_generated_leaf_command_matches` at
  `ortho_config/src/cargo/proptests.rs:125`;
- two rstest-bdd scenarios in
  `ortho_config/tests/features/cargo_entry_point.feature`, wired at
  `ortho_config/tests/rstest_bdd_support/behaviour/scenarios.rs:17`;
- three insta snapshots in `ortho_config/tests/cargo_entry_point.rs`.

### The documented templates

The templates live in `docs/users-guide.md` §"Cargo external-subcommand entry
points" (line 780 onwards), with these tested examples:

- `guide-cargo-external-subcommand`;
- `guide-cargo-derive-subcommand`;
- `guide-cargo-derive-merge`;
- `guide-cargo-error-hints`.

`README.md` §"Cargo subcommands" (lines 158-244) has a copy. The harness tests
are `ortho_config/tests/documentation_examples/cargo_derive.rs` and
`cargo_flows.rs`. They run real `cargo <name>` through `PATH`.

### Derive attribute parsing

The code is in `ortho_config_macros/src/derive/parse/`:

- `StructAttrs` is defined in `mod.rs:82-103`.
- `parse_struct_attrs` is in `struct_attrs.rs:320-387`. The file is 397
  lines long.
- Struct-level documentation keys, and the `behavior` precedent, are in
  `doc_attrs.rs` (321 lines).
- `localization.rs` holds `reject_localized_default`.
- Parser unit tests are registered in `parse/tests/mod.rs`.

Compile-fail cases are `.rs`/`.stderr` pairs in `ortho_config/tests/ui/`, run
by glob from `ortho_config/tests/compile_fail.rs`. To regenerate a `.stderr`,
run `TRYBUILD=overwrite cargo test -p ortho_config --test compile_fail`, then
run again without `TRYBUILD` (`docs/developers-guide.md:3729-3734`).

### Derives that accept `#[ortho_config]`

All four are in `ortho_config_macros/src/lib.rs`:

| Derive                      | Line |
| --------------------------- | ---- |
| `OrthoConfig`               | 46   |
| `RequiredConfigSchema`      | 129  |
| `SelectedSubcommandMerge`   | 166  |
| `OrthoConfigSubcommandDocs` | 179  |

Only the first two parse struct-level keys through `parse_struct_attrs`. The
derive already emits a hidden `#[derive(clap::Parser)]` struct named
`__<Name>Cli` (`generate/structs.rs:50-69`). No `clap::Subcommand` type is
generated anywhere.

### Documentation IR

`ortho_config/src/docs/ir/mod.rs:36-80` defines `DocMetadata`. It has
`app_name` and `bin_name: Option<String>`, and no Cargo field.
`cargo-orthohelp` reads `bin_name` in `src/generation.rs:217-228` and
`src/agent_context/mod.rs:316-321`.

### Estate census

The census is pinned to the commits that `gh api` printed on 2026-11-28. To
print them again:

```bash
gh api repos/leynos/theoremc/commits/HEAD --jq .sha
gh api repos/leynos/rstest-bdd/commits/HEAD --jq .sha
git rev-parse origin/main
```

When the plan was drafted, they were:

- theoremc: `cd2cffa13614555bb2aaf369a0714782b3e9d256`;
- rstest-bdd: `470c20ae27ef6ed9e97d361c738666a8cad22175`;
- this repository's `origin/main`: `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3`.

The census found the following.

`cargo-orthohelp` (`cargo-orthohelp/src/cli/mod.rs:53-70`):

- a derived parent with `bin_name = "cargo"` and `version`;
- a single-variant enum, `Orthohelp(Args)`;
- `src/main.rs:76-83` parses through `try_get_matches` and
  `from_arg_matches`, so that it can add invocation hints (`main.rs:85-111`);
- `main.rs:138-141` destructures the variant and emits the
  dispatch-boundary debug event.

`cargo-bdd` (`crates/cargo-bdd/src/cli/invocation.rs:43-78`):

- it calls `external_subcommand("cargo-bdd", "bdd", Cli::command())` only
  when `argv[1] == "bdd"`, and otherwise parses the flat direct form;
- it records an `InvocationForm`, which later error paths reuse;
- the wrapper-specific glue, `subcommand_matches` plus a "lost `bdd`"
  error, is about 12 lines;
- `Cli` has subcommands (`Steps`, `Unused`, `Duplicates`, `Skipped`), so a
  flat parser given the injected `bdd` fails with `InvalidSubcommand`.

`cargo-theorem` (`crates/cargo-theorem/src/cli.rs:98-127` and line 186):

- a derived parent `CargoEntry`, with `bin_name = "cargo"`, `version` and
  `subcommand_required = true`;
- an enum `CargoCommand` deriving `OrthoConfigSubcommandDocs`, with
  `#[command(name = "theorem", version)]` on its variant;
- it parses through `parse_localized_command::<CargoEntry, _, _>`;
- `command_metadata()` patches `DocMetadata` by hand;
- the crate also has its own `cargo theorem` entry-point tests
  (`crates/cargo-theorem/tests/entry_points.rs`).

### Prior art

Researched with Firecrawl on 2026-11-28:

- The Cargo book's "External tools" chapter, under "Custom subcommands",
  states the contract: the first argument is the binary's file name, then the
  subcommand name, then the forwarded arguments. `cargo help <name>` runs
  `cargo-<name> <name> --help`, and Cargo sets `CARGO` in the environment.
- clap 4.6's cookbook entry `_derive::_cookbook::cargo_example_derive` uses a
  single `#[derive(Parser)]` enum as the whole wrapper.
- `clap-cargo` 0.19 provides reusable `Manifest`, `Workspace` and `Features`
  flag structs and a `style` module. It does not generate a wrapper.
- No crate was found that generates a Cargo external-subcommand wrapper from
  an attribute.

### Documentation to read before implementing

- `docs/design.md` §4.17;
- `docs/adrs/adr-0004-cargo-external-subcommand-entry-point.md`;
- `docs/users-guide.md` §"Cargo external-subcommand entry points";
- `docs/developers-guide.md`:
  - "Subcommand dispatch changes" (line 503);
  - "Derive expansion ownership" (line 519);
  - "Testing these diagnostics" (line 3707);
- `docs/documentation-style-guide.md` §ADRs (line 520 onwards);
- `docs/v0-12-0-migration-guide.md`;
- `docs/rust-testing-with-rstest-fixtures.md`;
- `docs/rstest-bdd-users-guide.md`;
- `docs/rust-doctest-dry-guide.md`;
- `docs/reliable-testing-in-rust-via-dependency-injection.md` (the `MapEnv`
  injection used by the probe);
- `docs/complexity-antipatterns-and-refactoring-strategies.md` (keep the new
  query flat and single-purpose);
- `docs/clap-dispatch-and-ortho-config-integration.md`;
- `docs/cargo-orthohelp-design.md` §6;
- `docs/agent-native-cli-design.md` (consumers of `DocMetadata` in agent
  context, relevant to D9);
- `docs/improved-error-message-design.md` (diagnostic wording);
- `docs/cli-localization-design.md` and
  `docs/localizable-rust-libraries-with-fluent.md` (`cargo-theorem`'s localized
  parse path, variance point v5);
- `docs/scripting-standards.md` (only if a counting script is added; none is
  planned beyond the inline `awk`).

### Skills

- `rust-router`, which routes to `rust-types-and-apis` (the surface
  decision), `rust-unit-testing` (the `rstest` and trybuild tables) and
  `proptest` (the near-miss property);
- `arch-decision-records` for the ADR;
- `codegraph-mcp` for call-site analysis;
- `execplans`;
- `en-gb-oxendict-style` for prose.

## Plan of work

### EP-M0: approval gate (no code)

Present the plan for approval, with questions Q1–Q3 answered, before EP-M1
begins.

### EP-M1: prototype and census (prototyping milestone)

Create the target `ortho_config/tests/cargo_wrapper_prototype.rs`. Its module
comment states three things: it is a disposable 8.3.3 prototype; it is governed
by this plan; and EP-M2 deletes it. Submodules live under
`ortho_config/tests/cargo_wrapper_prototype/` and are attached with `#[path]`.

#### Stand-in

`stand_in.rs` holds the `cargo_wrapper_prototype!` `macro_rules!` described in
D1. It emits two items:

- **Candidate A:** a public, documented `<Name>Cargo` enum. It derives
  `clap::Parser` with `#[command(name = "cargo", bin_name = …)]` plus any
  pass-through attributes. Its single variant wraps the options type. It also
  provides `into_parts`.
- **Candidate B:** `<Name>::cargo_command() -> clap::Command`, which returns
  `external_subcommand(cargo_bin, cargo_subcommand, <Name>::command())`. It
  also provides
  `<Name>::cargo_matches(&ArgMatches) -> Result<&ArgMatches, clap::Error>` for
  the inner-matches extraction that `cargo-bdd` and the merge example repeat.

A `const` assertion enforces name consistency.

A second arm emits candidate E, the library-style entry macro from D12, with
pass-through attributes.

#### Consumer reductions

`orthohelp.rs`, `bdd.rs` and `theorem.rs` are minimal but faithful copies of
each tool's entry shape:

- They keep every variance point (v1–v7) and drop business logic.
- Each holds its baseline shape exactly as the consumer writes it today,
  plus one adoption per applicable candidate (A, B, E, F).
- No reduction exposes a positional argument at the injected-token position,
  so that the negative control cannot be satisfied by accident.

#### Metadata

`metadata.rs` holds candidate C. It hand-writes the `DocMetadata` the
`cargo-theorem` reduction needs. It then measures how much of that the existing
`#[ortho_config(bin_name = …)]` key already supplies, and how much a generic
`entry_doc_metadata` function would remove (D9).

#### Tables

For each reduction, one `#[rstest]` table runs the baseline and each candidate
over the same argv cases:

| Case | argv                                  |
| ---- | ------------------------------------- |
| 1    | Cargo-injected: `["cargo-x", "x", …]` |
| 2    | `help`                                |
| 3    | `--version` at each level             |
| 4    | Flat direct: `["cargo-x", …]`         |

Case 4 is the only one that separates v4. Each case declares an expected
outcome, `Same` or `Diverges(reason)`:

- `Same` asserts equal parse results and byte-equal rendered usage, using
  `pretty_assertions::assert_eq!`.
- `Diverges` asserts inequality and names the variance point.

Every gate failure therefore cites a test that ran. Matchers come from
`googletest`.

#### Negative control

The flat options parser alone is given the Cargo-injected argv. The table fixes
the expected `ErrorKind` per reduction:

- `InvalidSubcommand` where the flat parser has subcommands (`cargo-bdd`,
  `cargo-theorem`);
- `UnknownArgument` where it has neither subcommands nor positionals
  (`cargo-orthohelp`).

The test also asserts that the error's context value
(`ContextKind::InvalidSubcommand` or `ContextKind::InvalidArg`) equals the
injected token. This proves the token caused the failure, and therefore that
the tables detect a missing wrapper.

#### Probe

`cargo_attribute_is_silently_ignored_today` declares two identical structs. One
carries `#[ortho_config(cargo_subcommand = "demo", cargo_bin = "cargo-demo")]`;
the other carries no Cargo keys. Each has a `port: u16` field.

Both load through `load_from_iter_with_env` with an injected `MapEnv` whose
current directory does not exist, so no ambient file or environment can
interfere.

- **Flat row:** both structs, given `["demo", "--port", "7"]`, return
  `Ok(port == 7)`, with equal values.
- **Decisive row:** the attributed struct, given
  `["cargo-demo", "demo", "--port", "7"]`, still fails. This is the silent
  failure D6 addresses, observed directly. The expected error kind is taken
  from the generated parser's shape and recorded.

#### Census and recount

Count wrapper lines with the rule in obligation E2, and record a table under
`Artefacts and notes`. A `wyvern` agent, given only the counting rule and the
pinned paths, recounts independently. Reconcile any difference larger than one
line per cell before EP-M2.

#### Acceptance

- The focused run passes.
- `make check-fmt`, `make lint`, `make test` and `make markdownlint` pass.
- The milestone is one commit (D15).
- Recovery: revert the commit; nothing depends on it.

### EP-M2: decision gate

1. Score each candidate (A, B, C, D, E, and option F) against G1–G5 in a
   table under `Decision log`. Cite prototype test names and census rows.
2. Allocate the ADR number (D5).
3. Draft the ADR with status `Proposed`, using the style guide's section
   order:
   - Status;
   - Date;
   - Context and problem statement;
   - Decision drivers;
   - Options considered: A–F, plus "helper plus templates";
   - Decision outcome / proposed direction. Its `###` subsections are
     "Evidence: estate census", "Evidence: prototype scoring" and "Evidence
     and verification". Each table carries an `_Table n: …_` caption.
   - Consequences;
   - Known risks and limitations: the breaking diagnostic, `.stderr`
     maintenance, and the revisit trigger;
   - Outstanding decisions: only while the ADR is `Proposed`. Resolve them
     before EP-M4, because the style guide reserves this section for
     `Proposed` ADRs.
4. Run `make markdownlint` and `make check-fmt`, then commit (D15, 2a).

Then take one of three paths.

**Rejection path** (every candidate fails at least one gate). Delete the
prototype target in commit 2b, titled "Retire the 8.3.3 Cargo wrapper
prototype". Its message cites the prototype commit and the ADR. Run the full
gates, then continue to EP-M3.

**Acceptance path** (some candidate passes every gate):

- Set this plan to `BLOCKED`.
- In `Decision log`, write the narrow surface: the exact keys, the generated
  item names and their visibility, and the knobs.
- Add the verification additions from "Acceptance-path obligations".
- Wait for approval. The prototype stays as the red baseline for the real
  macro.

**Candidate D only.** Set the plan to `BLOCKED` and escalate under D4.

### EP-M3: diagnostic for the rejected keys (rejection path)

#### Red

1. Create `ortho_config_macros/src/derive/parse/tests/cargo_keys.rs` and
   register it in `parse/tests/mod.rs`. It holds two tests.

   An `rstest` table. Each case asserts that `parse_struct_attrs` returns an
   error whose text names the offending key and
   `ortho_config::cargo::external_subcommand`. The cases are:
   - `cargo_subcommand = "demo"`;
   - `cargo_bin = "cargo-demo"`;
   - both keys together;
   - the list form `cargo_subcommand("demo")`;
   - the bare path `cargo_subcommand`.

   The `cargo_bin` case must also mention `bin_name`, and must not name
   `cargo_subcommand`.

   A `proptest` property: for any identifier other than the two keys, a struct
   carrying that key still parses `Ok`. The identifiers come from
   `"[a-z_][a-z0-9_]{0,24}"`, filtered through `syn::parse_str::<syn::Ident>`
   so keywords are excluded, and must not belong to the set of known keys. The
   generator explicitly mixes in near misses: `cargo`, `cargo_bins`,
   `cargo_subcommands`, `my_cargo_bin`, and `CARGO_BIN` as written.

2. Run the focused test:

   ```bash
   cargo test -p ortho_config_macros cargo_keys 2>&1 \
     | tee /tmp/red-cargo-keys-ortho-config-8-3-3.out
   ```

   It must fail because parsing succeeds. Check the log for the `rstest` case
   failures. The property passes on red, which is expected: it guards against
   over-rejection.

3. Add `ortho_config/tests/ui/cargo_subcommand_attribute_rejected.rs`, with no
   `.stderr` yet. Run the compile-fail target:

   ```bash
   cargo test -p ortho_config --test compile_fail 2>&1 \
     | tee /tmp/red-compile-fail-ortho-config-8-3-3.out
   grep -F 'Expected test case to fail to compile, but it succeeded' \
     /tmp/red-compile-fail-ortho-config-8-3-3.out
   ```

   The `grep` must match. A missing `.stderr` alone would also fail the test,
   and that is not the red evidence required.

#### Green

Add `ortho_config_macros/src/derive/parse/cargo_keys.rs`. Its module comment
explains why the keys are rejected and cites the ADR. It defines
`rejected_cargo_key_error` (D10):

- The error is spanned on `meta.path`, as `misspelt_behaviour` is.
- The text interpolates the actual key, through a small fixed `concat!`
  template.

In `struct_attrs.rs`, the `_` arm's final call becomes the D10 expression.
Rewrite the `parse_struct_attrs` doc comment line-neutrally, so that it names
the rejected keys as the deliberate exception and drops the "custom
`compile_error!` guard" suggestion. Then:

1. Check that `wc -l` reports 400 or fewer.
2. Generate the `.stderr` with `TRYBUILD=overwrite` and inspect it.
3. Run again without `TRYBUILD`.
4. Confirm that `ortho_attrs_unknown_keys` still passes, unchanged.

Proposed message text, to be finalized against
`docs/improved-error-message-design.md`:

```plaintext
unsupported attribute `cargo_subcommand`: OrthoConfig does not support
Cargo subcommand wrapper attributes; wrap the parser with
`ortho_config::cargo::external_subcommand` or use the derive template in
the user's guide
```

For `cargo_bin`, append "to set the documented binary name, use `bin_name`".

#### Refactor

Doc comments only. Commit once, with the red transcripts summarized in the
message (D15).

### EP-M4: documentation and the decision record

- **Design §4.17.** Replace the paragraph beginning "A macro extension may be
  useful later…" and the candidate-syntax block with three things: the
  decision, a one-sentence reason, and a link to the ADR. Keep the
  regression-coverage paragraph. If D9 applies, record the
  documentation-metadata direction here as well, so that it is not held only in
  the ADR.
- **ADR.**
  - Set the status to `Accepted`, with the date and a one-line summary.
  - Move every resolved outstanding decision into "Consequences" or "Known
    risks and limitations".
- **`docs/users-guide.md`**, §"Cargo external-subcommand entry points". Add
  one prose paragraph with no fenced Rust block, so the harness gains no cold
  build. It states:
  - no attribute generates the wrapper;
  - struct-level `cargo_subcommand` and `cargo_bin` are compile errors on
    `OrthoConfig` and `RequiredConfigSchema`;
  - the enum derives already reject them;
  - the helper and the templates are the supported routes.
- **`docs/v0-12-0-migration-guide.md`.** Add an "Impact at a glance" row and a
  short section quoting the error (D14). Qualify the sentence "No code changes
  are required".
- **`docs/developers-guide.md`.**
  - "Derive expansion ownership": `cargo_keys.rs`, its sole caller, and its
    reuse policy.
  - Beside "Testing these diagnostics": the convention that a publicly
    rejected key is diagnosed inside its own feature module, while other
    unknown keys stay discarded. Name all three precedents: `behavior`,
    `localized_default`, and the Cargo keys.
  - The "Subcommand dispatch changes" checklist: one bullet linking the ADR.
- **`CHANGELOG.md`**, under Unreleased: `### Changed (derive attributes)`,
  with a `**Breaking:**` lead (D14).
- **`docs/contents.md`.** Index the ADR after the ADR-0027 entry. This
  ExecPlan was indexed after the 8.3.2 entry when the draft was published;
  update that entry's summary to state the outcome.
- **`docs/roadmap.md`.** Tick the four 8.3.3 sub-items and mark 8.3.3 `[x]`.
  Following the 8.3.1 pattern, add a short note that states the outcome and
  links the ADR.
- **Gates.** Run `make fmt`, then `make check-fmt` and `make markdownlint`.
  `make lint` and `make test` must pass on the final EP-M4 commit.
- `docs/repository-layout.md` needs no change. It describes macro parsing
  only at the directory level, and the prototype is gone by this point.

### EP-M5: delivery

1. Recheck the ADR number against `origin/main` and the open PR branches.
2. Mark the PR ready for review.
3. Run the review loop (CodeRabbit through comenq) to equilibrium, handling
   findings within the plan's tolerances.
4. Complete `Outcomes & retrospective`.

### Approval questions

- **Q1.** Approve D6: a targeted compile error for struct-level
  `cargo_subcommand` and `cargo_bin`, released in 0.12.0 as a breaking
  CHANGELOG entry with migration guidance? The alternative is documentation
  only, which leaves the floated syntax silently accepted.
- **Q2.** On the rejection path, should a roadmap item be added now for the
  `DocMetadata` Cargo-entry function (D9)? It would be conditional on a second
  consumer. Or should the ADR record only the direction and its revisit trigger?
- **Q3.** If option F (clap's one-enum cookbook form) scores better than the
  current template, may EP-M4 revise the documented derive template and its
  tested examples? If not, F is recorded in the ADR only.

## Concrete steps

Run every command from the worktree root. Name gate logs
`/tmp/$ACTION-ortho-config-8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute.out`.

```bash
L=ortho-config-8-3-3-evaluate-cargo-subcommand-wrappers-macro-attribute
# EP-M1 focused run
cargo test -p ortho_config --test cargo_wrapper_prototype 2>&1 | tee /tmp/test-prototype-$L.out
# EP-M3 focused runs
cargo test -p ortho_config_macros cargo_keys 2>&1 | tee /tmp/test-cargo-keys-$L.out
cargo test -p ortho_config --test compile_fail 2>&1 | tee /tmp/test-compile-fail-$L.out
wc -l ortho_config_macros/src/derive/parse/struct_attrs.rs   # must be <= 400
# Milestone gates (delegated to scrutineer, run in sequence)
make check-fmt
make lint
make test
make markdownlint   # also runs the spelling gate
# Tolerance count, at each milestone commit: see "Counting rule"
```

`make nixie` runs only if a Mermaid diagram is added. None is planned.

Expected excerpt from the EP-M3 green run:

```plaintext
test derive::parse::tests::cargo_keys::rejects_cargo_wrapper_keys::case_1_subcommand_value ... ok
test derive::parse::tests::cargo_keys::other_unknown_keys_are_still_discarded ... ok
test tests/ui/cargo_subcommand_attribute_rejected.rs ... ok
```

## Validation and acceptance

**EP-M1.**

- The focused prototype run passes.
- Every `Same` row agrees, and every `Diverges` row names its variance
  point.
- The negative control fails, for each reduction, with the declared
  `ErrorKind` and the injected token in its context.
- The probe's decisive row fails as recorded.
- All four gates pass at the commit.

**EP-M2.**

- The ADR exists with status `Proposed` and a scored G1–G5 table.
- On the rejection path, `ortho_config/tests/cargo_wrapper_prototype*` no
  longer exists, and all four gates pass.

**EP-M3.**

- The red transcripts show:
  - the `rstest` cases failing;
  - the trybuild "succeeded" message.
- After the change:
  - all of them pass;
  - `ortho_attrs_unknown_keys` passes unchanged;
  - `struct_attrs.rs` is at 400 lines or fewer;
  - all four gates pass.

**EP-M4.**

- `make check-fmt`, `make markdownlint`, `make lint` and `make test` pass.
- Design §4.17 no longer presents the attribute as an open candidate.
- The ADR is `Accepted`, with no outstanding decisions.
- Roadmap 8.3.3 is `[x]`.

**Overall.** `make check-fmt`, `make lint` and `make test` succeed after every
milestone. `make markdownlint` succeeds after every milestone too, because
every milestone updates this plan.

### Why no rstest-bdd, insta or end-to-end tests on the rejection path

- **rstest-bdd:** no feature is added. The only behavioural change is a
  compile-time diagnostic, which runtime scenarios cannot observe.
  `ortho_config/tests/features/cargo_entry_point.feature` continues to specify
  the supported helper.
- **insta:** no snapshot is added. The trybuild `.stderr` already serves as
  the snapshot of the diagnostic.
- **End-to-end:** no new tests are added. Cargo dispatch is unchanged, and
  the existing harness tests already exercise it through `PATH`.

The acceptance path adds all three; see `Verification plan`.

## Verification plan

### Evaluation obligations (EP-M1 and EP-M2)

#### E1 (G1, legal host)

**Obligation.** For each consumer, consider every derive that accepts
`#[ortho_config]`. Is any of them already on a type the consumer writes, and
able to host the keys without adding configuration behaviour the tool does not
want?

**Method.** Census facts with pinned citations, plus a stand-in invocation in
each reduction.

**Pass.** At least two of the three consumers.

**Non-vacuity.** `cargo-theorem`'s `CargoCommand` is a real witness: it already
derives `OrthoConfigSubcommandDocs`. So G1 can pass on evidence.

#### E2 (G2, net savings)

**Obligation.** Wrapper lines removed, minus lines added, must meet the rule in
D11 in at least two consumers. Lines added are the invocation, its knobs and
any adoption glue.

**Counting rule.** Count non-blank, non-comment lines in items that exist only
to give the program its Cargo entry shape:

- the parent struct or enum;
- the single-variant enum;
- variant destructuring;
- inner-matches extraction and its "lost token" error;
- wrapper `DocMetadata` patching.

Exclude:

- tracing initialization;
- error-hint rendering;
- business logic.

These excluded items are application policy, and no candidate removes them.
Repeated name strings are reported separately.

**Method.** Manual count plus an independent `wyvern` recount.

**Non-vacuity.** Each consumer must have a non-zero baseline. Otherwise the
gate is void for that consumer and is reported as void.

#### E3 (G3, variance coverage)

**Obligation.** Each expansion-changing variance point (v1, v2, v3, v7) is
either reproduced or listed as a knob. Total knobs must not exceed two.
Orthogonal points (v4, v5, v6) are shown to compose unchanged.

**Method.** The `Same`/`Diverges` tables. Each `Diverges` row names its
variance point and asserts inequality.

**Non-vacuity.**

- The negative control: per-reduction error kinds, with the injected token
  in the error context.
- The flat-direct row, which separates v4.

#### E4 (G4, visible contract)

**Obligation.** In the adopting source, a reader can tell that `argv[1]` is
Cargo's injected token without reading macro output. The generated items are
public, documented, and not named with a `__` prefix. Help output is
byte-identical to the baseline's, or diverges only on a recorded variance
point. The error a user sees when `clap::Parser` is missing on the host for
candidates A and B is recorded and judged.

**Method.** The usage rows in the tables, review of the reduction source, and a
recorded compile error from a deliberately broken stand-in use (recorded but
not committed).

#### E5 (G5, ADR-0004 conformance)

**Obligation.** The expansion touches neither `OrthoConfig::load` nor the merge
pipeline.

**Method.** Inspection. Candidate D fails by construction.

### Behavioural obligations (rejection path)

#### O1. The rejected keys produce a targeted compile error

**Obligation.** Each rejected key, in any of these forms, produces a compile
error. The error is spanned on the key and names that key and the supported
alternative.

- Name-value form.
- List form.
- Bare form.
- Alone or together.

**Scope.** Struct level, on `OrthoConfig` and `RequiredConfigSchema`.

**Method.** The `rstest` table in `parse/tests/cargo_keys.rs`, plus the
trybuild case and its `.stderr`.

**Non-vacuity.** Red transcripts show the cases failing and the trybuild
"succeeded" message. The EP-M1 probe's decisive row shows the harm today.

**Wrong implementations rejected.** Reporting `cargo_subcommand` for a
`cargo_bin` input fails the key-naming assertion.

#### O2. Forward compatibility is preserved

**Obligation.** Every other unknown struct-level key is still discarded.

**Method.** Three pieces of evidence:

- the existing `parse/tests/ortho_attrs_unknown_keys.rs` table, unchanged;
- the near-miss property in `cargo_keys.rs`, whose generator mixes in
  `cargo`, `cargo_bins`, `cargo_subcommands`, `my_cargo_bin` and `CARGO_BIN`;
- recorded counts of how many generated cases were near misses, as
  classification evidence.

**Wrong implementations rejected.** Rejecting all unknown keys, prefix
matching, substring matching and case folding each fail the property.

#### O3. Runtime behaviour is unchanged

**Method.** The existing suites stay green:

- the helper's unit, property, rstest-bdd and snapshot tests;
- the documentation-harness Cargo dispatch tests, which run real
  `cargo <name>` through `PATH`.

#### Out of scope

- Field-level keys, which stay discarded.
- The enum derives, which already reject the keys. This is recorded, not
  changed.

### Axioms (trusted, not verified here)

- Cargo's external-subcommand argument contract, as documented in the Cargo
  book.
- clap 4's semantics:
  - derive behaviour for `name`, `bin_name`, `version` and
    `subcommand_required`;
  - single-variant enums;
  - error kinds and context, as observed in clap 4.6
    (`clap_builder/src/parser/parser.rs:533-579`).
- `syn`'s `ParseNestedMeta` behaviour for name-value, list and bare items,
  which `discard_unknown` already relies on.

### Formal verification decision

**Verus is not warranted on either path.**

- On the rejection path, the work introduces no lemma over an unbounded
  domain and no contractual business logic. The decision rests on evidence, and
  the only executable change is a two-key diagnostic. A table and a property
  cover it.
- On the acceptance path, the strongest invariant is I1 (argv equivalence
  for every argv tail). Its truth rests on clap's parser semantics. Those are
  third-party axioms, listed above, not a model the project owns. A Verus proof
  would have to assume the very parser behaviour it claims to establish. A
  property test against the real clap is the proportionate evidence, as in
  8.3.1.

**Kani is not warranted either.** There is no `unsafe` code, no arithmetic and
no bounded state machine.

**Property testing.** The helper's existing
`wrapper_preserves_generated_leaf_command_matches` remains the argv equivalence
evidence for the supported abstraction, unchanged. The new near-miss property
protects O2.

**Where this is recorded.** These choices go in the ADR's "Evidence and
verification" subsection, so the design documentation carries them, not only
this plan.

### Acceptance-path obligations (only if EP-M2 accepts; requires re-approval)

These are sketched now so the acceptance path is costed. Approval of this plan
does not authorize them.

- **I1, argv equivalence.** For every argv tail `T`, the generated wrapper
  accepts `[bin, name, T…]` exactly when the inner parser accepts `[name, T…]`,
  and the matches are equal. Method: reuse the strategy in
  `ortho_config/src/cargo/proptests.rs`. Non-vacuity: record classification
  counts for flag, option and positional shapes.
- **I2, name consistency.** A `cargo_bin` other than
  `cargo-<cargo_subcommand>` is a compile error. Method: a trybuild
  compile-fail case.
- **I3, help stability.** insta snapshots of top-level and inner help for
  both invocation forms.
- **I4, end-to-end.** A fixture binary is run as `cargo <name> --help`
  through `PATH`, reusing `cargo_runner::cargo_dispatch_path`.

Proposed feature, `ortho_config/tests/features/cargo_wrapper_attribute.feature`:

```gherkin
Feature: Generated Cargo subcommand wrapper
  Scenario: Cargo dispatch reaches the options struct
    Given a configuration struct declaring Cargo subcommand "demo"
    When the generated wrapper parses "cargo-demo demo --verbose"
    Then the options struct sees verbose enabled

  Scenario: Flat invocation is rejected
    Given a configuration struct declaring Cargo subcommand "demo"
    When the generated wrapper parses "cargo-demo --verbose"
    Then parsing fails because the subcommand token is missing
```

## Conformance basis

### Sources

- `docs/roadmap.md` item 8.3.3, at `origin/main`
  `de2e7ae1d0ac42dfe8adf02cd9dad944283970d3`. Its sub-items are:
  - RM-a: prototype the syntax;
  - RM-b: decide the form;
  - RM-c: reject unless there is real repetition that does not hide the
    contract;
  - RM-d: success, meaning the design records the outcome.
- `docs/design.md` §4.17 (DES-4.17), the paragraph on the candidate syntax.
- ADR-0004: accepted 2026-05-21, amended 2026-08-09, clarified 2026-11-11.
- `AGENTS.md`: the file-size cap, testing rules, documentation rules and
  abstraction policy.

This phase has no Terms of Reference document.

### Trace

| Source                                             | Path                                                                            |
| -------------------------------------------------- | ------------------------------------------------------------------------------- |
| RM-a                                               | D1 → EP-M1 → `cargo_wrapper_prototype!` stand-in, tables and probe              |
| RM-b                                               | D2, D11, E1–E5 → EP-M2 → ADR "Decision outcome / proposed direction"            |
| RM-c                                               | G1–G4 → EP-M2 scoring table                                                     |
| RM-d                                               | D5 → EP-M4 → design §4.17 and the `Accepted` ADR                                |
| DES-4.17, ADR-0004                                 | G5 and D4 → Constraints                                                         |
| Discovery that unknown keys are discarded silently | D6, D10, D14 → EP-M3 and EP-M4 → O1 and O2 tests; CHANGELOG and migration guide |

## Idempotence and recovery

Every milestone ends in gated commits, so re-running a focused test is
harmless. To abandon the prototype, revert its commit.

If `TRYBUILD=overwrite` writes an unexpected `.stderr`, first copy it aside,
then discard it with `git restore --source=HEAD -- <path>`. Do not use
`git checkout --`: it reverts to the index and destroys unstaged work.

The ADR and documentation edits are plain text and can be reverted commit by
commit. If the ADR number collides after allocation, rename the file and its
references in a single commit.

## Artefacts and notes

The census table, the independent recount, the probe results, and the rubric
scores are recorded here during EP-M1 and EP-M2.

## Interfaces and dependencies

The rejection path adds no public interface. It adds one new crate-private item:

```rust
// ortho_config_macros/src/derive/parse/cargo_keys.rs
pub(super) fn rejected_cargo_key_error(meta: &syn::meta::ParseNestedMeta) -> Option<syn::Error>;
```

Its only permitted caller is the `_` arm of `parse_struct_attrs`. No dependency
is added.

The prototype uses only existing `ortho_config` dev-dependencies (`rstest`,
`googletest` and `pretty_assertions`), plus `clap`. The new macro-crate tests
use the existing `rstest`, `proptest` and `anyhow`.

## Revision note

- 2026-11-28, initial draft. Built from the Wyvern reconnaissance, the estate
  census and the prior-art research.
- 2026-11-28, revision 1: applied the community-of-experts design review.

  Rubric and candidates:
  - G1 is now judged over the whole `#[ortho_config]` namespace.
  - The G2 threshold is set by rule from measured baselines.
  - G3 variance points are classified as expansion-changing or orthogonal.
  - Candidate E and option F are added.
  - A `macro_rules!` stand-in replaces the doc-comment syntax sketch, so RM-a
    is met.
  - Candidates A and B now state their `clap::Parser` precondition, and A
    keeps the inner matches.

  Test controls:
  - The negative control now expects a per-reduction `ErrorKind` and checks
    that the injected token caused the failure.
  - The probe now injects `MapEnv` and includes a decisive Cargo-form row.
  - The trybuild red step now looks for trybuild's "succeeded" text.
  - The counting rule now runs at `HEAD`, uses `awk` sums per category, and
    puts anything unclassified under code.
  - A near-miss property is added.

  Diagnostic and documentation:
  - The ADR number is allocated at EP-M2, because ADR-0028 collides on
    PR #588 and PR #753.
  - The ADR outline now follows the style guide.
  - D6's scope now names `RequiredConfigSchema`, the enum derives and fields.
  - The diagnostic module is now feature-named (`cargo_keys.rs`), with a
    pure query interface (CQRS).
  - D14 adds the breaking-change CHANGELOG entry and migration guidance.
  - The developers'-guide placement is now explicit.
  - D15 adds the commit plan, and `make markdownlint` is now part of every
    milestone's acceptance.

  Corrections:
  - The request's `docs/rtest-bdd-users-guide.md` is read as the existing
    `docs/rstest-bdd-users-guide.md`.
  - The stale "or an older guide" claim is removed.

  Effect on remaining work: the evaluation is more expensive, because of
  candidate E, the stand-in and the near-miss property. It is also harder to
  bias. The documentation budget rises by 30 lines, for the migration guide.
