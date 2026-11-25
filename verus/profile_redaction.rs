//! Verus model and proofs for the profile-field disclosure projection.
//!
//! Roadmap 9.1.2 removes a protected field's value-bearing metadata at every
//! public conversion boundary. `ortho_config::docs` owns that projection; this
//! file proves the two semantic obligations that no test can discharge over
//! unbounded input:
//!
//! - V1, noninterference: two documents that agree on everything the policy is
//!   allowed to reveal project to equal results, however their protected value
//!   slots differ.
//! - V2, projection integrity: the projection preserves public descriptors,
//!   ordinary fields, and command topology; empties exactly the protected slots;
//!   suppresses every command-level example once any field is protected; and is
//!   idempotent.
//!
//! # Model-to-production mapping
//!
//! The model mirrors the production kernel rather than calling it: the real
//! types carry `String`s, `Vec`s, and serde attributes, none of which Verus can
//! reason about. Every production slot the projection can change has a model
//! counterpart; slots the policy preserves verbatim are collapsed, because
//! projecting them is the identity (the mapping table below shows which
//! production slots each model slot stands for). This file proves the
//! obligations over the model only; it does not prove that the production
//! traversal implements the model. The differential property tests in
//! `ortho_config/src/docs/profile_redaction/proptests.rs` are bounded evidence
//! for that correspondence, not a proof of it.
//!
//! | model                       | production                                     |
//! | --------------------------- | ---------------------------------------------- |
//! | `Exposure::Ordinary`        | `FieldMetadata::profile_field == None`          |
//! | `Exposure::Secret`          | `ProfileFieldExposure::Secret`                  |
//! | `Exposure::ReferenceOnly`   | `ProfileFieldExposure::ReferenceOnly`           |
//! | `Field::name`               | `FieldMetadata::name`                           |
//! | `Field::help_id`            | `help_id`, `long_help_id`, `links`, `notes`     |
//! | `Field::cli`                | `CliMetadata` minus `possible_values`           |
//! | `Field::env`, `Field::file` | `env`, `file`                                   |
//! | `Field::default`            | `DefaultValue`, collapsed to its literal        |
//! | `Field::examples`           | `FieldMetadata::examples`                       |
//! | `Field::possible_values`    | `CliMetadata::possible_values`                  |
//! | `Field::value`              | `ValueType`                                     |
//! | `Command::sections_examples`| `SectionsMetadata::examples`                    |
//! | `Command::fields`           | `DocMetadata::fields`                           |
//! | `Command::parent`           | the `DocMetadata::subcommands` edge             |
//!
//! Some production slots are deliberately collapsed rather than modelled one
//! for one. Static sections, renderer declarations, and behaviour metadata
//! carry no value derived from a field, so projecting them is the identity and
//! including them would only enlarge the state space. `Field::help_id`
//! collapses the two help identifiers and the public `links`/`notes` lists,
//! because the projection treats all of them alike: preserved verbatim. The
//! model's `ValueType::Scalar(tag)` collapses production's ten literal-free
//! scalar variants, but keeps an opaque `tag` for which one it is, so a
//! preservation claim cannot equate two different scalar kinds by reducing them
//! to one constant. `Enum`, `List`, and `Map` are modelled structure for
//! structure.
//!
//! # Tree encoding
//!
//! Production stores the command tree as nested `subcommands`. The model stores
//! it as a parent-before-child sequence of commands, each naming its parent by
//! index, and [`is_tree_like`] states the invariant that makes that encoding a
//! genuine rooted tree: a single root at index zero, and every other command's
//! parent strictly earlier than itself, so following parents always terminates
//! at the root. That invariant admits any topological linearization, not only
//! production's preorder one: a sibling may precede the descendants of an
//! earlier sibling. Nothing in the proofs needs more, and nothing here lets a
//! node be its own ancestor. Depth is therefore unbounded, and the parent field
//! is what a topology-preservation claim is about.
//!
//! The encoding is forced, not preferred. Verus rejects structural recursion
//! through a `Seq` of nodes: a recursive call inside a `Seq::new` closure fails
//! its termination check, a mutual pair of spec functions is rejected outright,
//! and an explicit size measure crashes the verifier with ill-typed AIR. The
//! recursive traversal itself is therefore covered by the differential property
//! tests, not by this file: `proptest_model.rs` rebuilds the nested production
//! tree slot by slot, and `proptests.rs` asserts that rebuilding equals the
//! production projection.
//!
//! That oracle is a second, differently shaped implementation rather than an
//! independent specification, so a mistake made identically in both would
//! agree. `proptest_assertions.rs` compares each original node with its
//! projected counterpart group by group, which catches such a mistake where
//! the two agree about a preserved or cleared slot. It does not catch every
//! one: the command-example expectation comes from the oracle's own
//! `model_has_protected` decision, so a protected-field decision shared by
//! production and the oracle can escape both checks. The oracle's reach is
//! bounded by the generator in `proptests_support.rs`: depth 4, at most 32
//! command nodes, at most 64 fields, and at most 8 children or fields per node.
//! Inside that bound the corpus is adversarial — every protected literal is
//! drawn from a `SECRET-` family — but no claim is made outside it. The
//! division is recorded in ADR-0026 as a residual testing boundary.
//!
//! The model specifies one document, so the sibling-group boundary has no
//! separate theorem here. `DocMetadata::redacted_group_for_output` does not add
//! a second projection rule: it decides the command-example flag over the group
//! and then projects each member with the same rule this file proves. What it
//! adds beyond the model is that a member's examples can be removed by another
//! member's field, which the group-wide flag expresses; the sibling regressions
//! in `ortho_config/tests/subcommand_docs.rs` are the evidence for that
//! boundary, not a refinement proof.

use vstd::prelude::*;

verus! {
mod profile_redaction_model;
mod profile_redaction_integrity;
mod profile_redaction_noninterference;

fn main() {}

}
