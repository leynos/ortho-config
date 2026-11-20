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
//! reason about. Every production slot has one model counterpart, and the
//! correspondence between them is discharged by the differential property tests
//! in `ortho_config/src/docs/profile_redaction/proptests.rs`, not by this file.
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
//! Two production slots are deliberately absent. Static sections, renderer
//! declarations, and behaviour metadata carry no value derived from a field, so
//! projecting them is the identity and including them would only enlarge the
//! state space. `Field::help_id` collapses the two help identifiers and the
//! public `links`/`notes` lists, because the projection treats all of them
//! alike: preserved verbatim.
//!
//! # Tree encoding
//!
//! Production stores the command tree as nested `subcommands`. The model stores
//! it as a preorder sequence of commands, each naming its parent by index, and
//! [`is_tree_like`] states the invariant that makes that encoding a genuine
//! rooted tree: a single root at index zero, and every other command's parent
//! strictly earlier than itself, so following parents always terminates at the
//! root. Depth is therefore unbounded, and the parent field is what a
//! topology-preservation claim is about.
//!
//! The encoding is forced, not preferred. Verus rejects structural recursion
//! through a `Seq` of nodes: a recursive call inside a `Seq::new` closure fails
//! its termination check, a mutual pair of spec functions is rejected outright,
//! and an explicit size measure crashes the verifier with ill-typed AIR. The
//! recursive traversal itself is therefore covered by the differential property
//! tests, whose model walks the nested production type; the division is recorded
//! in ADR-0024 as a residual testing boundary.

use vstd::prelude::*;

verus! {
mod profile_redaction_model;
mod profile_redaction_integrity;
mod profile_redaction_noninterference;

fn main() {}

}
