//! Profile field classification and the disclosure projection.
//!
//! Roadmap 9.1.2 lets an author classify a field as `secret` or
//! `reference_only`. Both labels mean the field's value-bearing metadata must
//! never reach a consumer artefact; only the field's public descriptor and its
//! label survive. The label distinguishes *why* the value is withheld: a
//! secret field holds confidential material, while a reference-only field
//! names material the application resolves elsewhere, and the locator itself is
//! treated as sensitive.
//!
//! This module owns the single implementation of that policy. The derive macro
//! emits [`DocMetadata::redacted_for_output`] output, and every public
//! conversion boundary applies it again so handwritten or deserialized
//! metadata cannot bypass it. Renderers must not grow competing classification
//! rules; they consume the already-projected tree.
//!
//! The contract is structural: it removes the modelled value slots (defaults,
//! field examples, CLI possible values, and recursively nested enum members).
//! It is not a scanner for arbitrary confidential strings. Help text,
//! identifiers, links, notes, and custom type names are author-controlled
//! public descriptions, and authors must not embed secrets there.
//!
//! # Reuse policy
//!
//! These functions are crate-private except for [`ProfileFieldExposure`] and
//! the two projection methods on [`DocMetadata`]. Every public conversion
//! boundary applies one of them exactly once, over the whole document it
//! returns; private recursion must not call it again for a subtree, because
//! the global command-example rule is computed from the whole document.
//!
//! The derive's generated methods are separate boundaries that compose: a
//! parent calls each child's `OrthoConfigDocs`/`OrthoConfigSubcommandDocs`
//! method, which projects that child's subtree before returning it. The
//! repetition that composition implies cannot be removed without either
//! leaving a public method unprojected or breaking the handwritten
//! implementations those traits document as supported. What *can* be avoided
//! is cloning: generated code owns the tree it has just built, so it calls
//! [`DocMetadata::into_redacted_for_output`] rather than
//! [`DocMetadata::redacted_for_output`].

use serde::{Deserialize, Serialize};

use super::ir::{DocMetadata, FieldMetadata, ValueType};

/// A field's declared non-disclosure classification.
///
/// The wire spellings are `secret` and `reference_only`, matching the labels
/// accepted by `#[ortho_config(profile_field = "...")]`. Deserializing any
/// other string fails, so an unrecognised label can never silently degrade to
/// an ordinary field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileFieldExposure {
    /// Confidential material whose value must never be published.
    Secret,
    /// A reference the application resolves elsewhere; the reference itself is
    /// sensitive and is never published.
    ReferenceOnly,
}

impl ProfileFieldExposure {
    /// Returns the wire spelling used in documentation IR and agent context.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::docs::ProfileFieldExposure;
    ///
    /// assert_eq!(
    ///     ProfileFieldExposure::Secret.as_str(),
    ///     "secret",
    ///     "secret must keep its canonical wire spelling"
    /// );
    /// assert_eq!(
    ///     ProfileFieldExposure::ReferenceOnly.as_str(),
    ///     "reference_only",
    ///     "reference-only must keep its canonical wire spelling"
    /// );
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Secret => "secret",
            Self::ReferenceOnly => "reference_only",
        }
    }
}

impl DocMetadata {
    /// Returns an owned copy with every protected value-bearing slot removed.
    ///
    /// Protected fields keep their name, help identifiers, CLI spelling,
    /// environment variable, file key, requiredness, and structural value type.
    /// Their default becomes absent, field examples become empty, CLI
    /// `possible_values` become empty, and every nested enum variant list
    /// becomes empty. When *any* field in the tree is protected, every
    /// command-level example in the tree is removed as well: an unstructured
    /// command example cannot be mapped back to the fields it exercises, so a
    /// parent or sibling example could disclose a protected child's value.
    ///
    /// The projection is idempotent and performs a constant number of linear
    /// traversals: one to decide whether any protected field exists, then one
    /// clone-and-project pass over the whole tree.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::docs::{
    ///     CliMetadata, DefaultValue, DocMetadata, FieldMetadata, HeadingIds,
    ///     ProfileFieldExposure, SectionsMetadata, ValueType,
    /// };
    ///
    /// fn sections() -> SectionsMetadata {
    ///     SectionsMetadata {
    ///         headings_ids: HeadingIds {
    ///             name: "n".into(),
    ///             synopsis: "s".into(),
    ///             description: "d".into(),
    ///             options: "o".into(),
    ///             environment: "e".into(),
    ///             files: "f".into(),
    ///             precedence: "p".into(),
    ///             exit_status: "x".into(),
    ///             examples: "ex".into(),
    ///             see_also: "sa".into(),
    ///             commands: None,
    ///         },
    ///         discovery: None,
    ///         precedence: None,
    ///         examples: Vec::new(),
    ///         links: Vec::new(),
    ///         notes: Vec::new(),
    ///     }
    /// }
    ///
    /// let metadata = DocMetadata {
    ///     ir_version: "2.0".into(),
    ///     app_name: "demo".into(),
    ///     bin_name: None,
    ///     about_id: "demo.about".into(),
    ///     synopsis_id: None,
    ///     sections: sections(),
    ///     fields: vec![FieldMetadata {
    ///         name: "token".into(),
    ///         help_id: "help.token".into(),
    ///         long_help_id: None,
    ///         value: Some(ValueType::String),
    ///         default: Some(DefaultValue { display: "hunter2".into() }),
    ///         required: false,
    ///         deprecated: None,
    ///         cli: Some(CliMetadata {
    ///             long: Some("token".into()),
    ///             short: None,
    ///             value_name: None,
    ///             multiple: false,
    ///             takes_value: true,
    ///             value_optional: false,
    ///             possible_values: vec!["hunter2".into()],
    ///             hide_in_help: false,
    ///         }),
    ///         env: None,
    ///         file: None,
    ///         examples: Vec::new(),
    ///         links: Vec::new(),
    ///         notes: Vec::new(),
    ///         profile_field: Some(ProfileFieldExposure::Secret),
    ///     }],
    ///     subcommands: Vec::new(),
    ///     windows: None,
    ///     behaviour: None,
    ///     renderer: None,
    ///     output: None,
    ///     profiles: None,
    /// };
    ///
    /// let redacted = metadata.redacted_for_output();
    /// assert_eq!(
    ///     redacted.fields[0].default,
    ///     None,
    ///     "a protected field must not disclose its default"
    /// );
    /// assert_eq!(
    ///     redacted.fields[0].profile_field,
    ///     Some(ProfileFieldExposure::Secret),
    ///     "the disclosure label must survive projection"
    /// );
    /// ```
    #[must_use]
    pub fn redacted_for_output(&self) -> Self {
        self.clone().into_redacted_for_output()
    }

    /// Returns the same value with every protected value-bearing slot removed.
    ///
    /// This is the form to call when the caller already owns the document, as
    /// the derive's generated methods do with the tree they just built. It
    /// projects in place instead of cloning, so composing a parent from
    /// already-owned children costs a traversal per public boundary rather
    /// than a full-tree clone as well.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::docs::DocMetadata;
    ///
    /// # fn demo(metadata: DocMetadata) {
    /// let projected = metadata.into_redacted_for_output();
    /// assert!(
    ///     projected.subcommands.iter().all(|child| child.fields.iter()
    ///         .all(|field| field.profile_field.is_none() || field.default.is_none())),
    ///     "projected children must not retain protected defaults"
    /// );
    /// # }
    /// ```
    #[must_use]
    pub fn into_redacted_for_output(mut self) -> Self {
        let has_protected = self.contains_protected_field();
        self.redact_node(has_protected);
        self
    }

    /// Returns a group of sibling documents projected as one unit.
    ///
    /// A public boundary that returns several documents at once — the
    /// generated `OrthoConfigSubcommandDocs` methods return one document per
    /// enum variant — must decide the command-example rule over the group, not
    /// per document. When any member declares a protected field, every
    /// member's command-level examples are suppressed; when none does, every
    /// command-level example is kept.
    ///
    /// The scan that decides the rule stops at the first member declaring a
    /// protected field, so it may inspect only part of the group. The
    /// projection pass that follows visits every member and every descendant,
    /// clearing protected value slots and applying the group decision. It
    /// cannot be limited to the members the scan inspected: an ordinary
    /// member's examples must still go when a later sibling is protected.
    ///
    /// The projection is idempotent, so a document a child boundary already
    /// projected keeps the result of its own slots. Its command examples are
    /// not exempt: a single-document projection keeps them when that document
    /// is clean, and this group decision removes them when any sibling is
    /// protected. Members are projected in the caller's order, and the
    /// projection pass costs one linear traversal of each member's subtree, so
    /// members a child boundary already projected are walked again here.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::docs::DocMetadata;
    ///
    /// # fn demo(documents: Vec<DocMetadata>) {
    /// let projected = DocMetadata::redacted_group_for_output(documents);
    /// assert!(
    ///     projected.iter().all(|doc| doc.fields.iter()
    ///         .all(|field| field.profile_field.is_none() || field.default.is_none())),
    ///     "every protected field in the group must lose its default"
    /// );
    /// // A protected field keeps its label, so a surviving label means the
    /// // group decision applied: every member's command examples are gone,
    /// // including those of a clean sibling that was projected alone earlier.
    /// assert!(
    ///     !projected.iter().any(|doc| doc.fields.iter()
    ///         .any(|field| field.profile_field.is_some()))
    ///         || projected.iter().all(|doc| doc.sections.examples.is_empty()),
    ///     "a protected member removes every member's command examples"
    /// );
    /// # }
    /// ```
    #[must_use]
    pub fn redacted_group_for_output(mut documents: Vec<Self>) -> Vec<Self> {
        let has_protected = documents.iter().any(Self::contains_protected_field);
        for document in &mut documents {
            document.redact_node(has_protected);
        }
        documents
    }

    /// Reports whether this subtree declares any protected field.
    fn contains_protected_field(&self) -> bool {
        #[cfg(test)]
        traversal_probe::record_node();
        self.fields
            .iter()
            .any(|field| field.profile_field.is_some())
            || self.subcommands.iter().any(Self::contains_protected_field)
    }

    /// Projects this node and its descendants in place.
    ///
    /// `has_protected` is decided once for the whole tree so that command
    /// examples are suppressed globally rather than per command.
    fn redact_node(&mut self, has_protected: bool) {
        #[cfg(test)]
        traversal_probe::record_node();
        if has_protected {
            self.sections.examples.clear();
        }
        for field in &mut self.fields {
            field.redact_protected_values();
        }
        for child in &mut self.subcommands {
            child.redact_node(has_protected);
        }
    }
}

impl FieldMetadata {
    /// Clears the value-bearing slots of one protected field.
    fn redact_protected_values(&mut self) {
        if self.profile_field.is_none() {
            return;
        }
        self.default = None;
        self.examples.clear();
        if let Some(cli) = self.cli.as_mut() {
            cli.possible_values.clear();
        }
        if let Some(value) = self.value.as_mut() {
            redact_value_type(value);
        }
    }
}

/// Removes every literal carried by a value type while keeping its shape.
fn redact_value_type(value: &mut ValueType) {
    match value {
        ValueType::Enum { variants } => variants.clear(),
        ValueType::List { of } | ValueType::Map { of } => redact_value_type(of),
        _ => {}
    }
}

#[cfg(test)]
mod derive_tests;
#[cfg(test)]
mod proptests;
#[cfg(test)]
mod test_fixtures;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod traversal_probe {
    //! Counts command-node visits so tests can pin the projection to a
    //! constant number of linear traversals instead of one projection per
    //! subtree.
    //!
    //! The counter is thread-local: tests run concurrently, and a shared
    //! counter would count another test's projections as well.

    use std::cell::Cell;

    thread_local! {
        static NODE_VISITS: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn reset() {
        NODE_VISITS.with(|visits| visits.set(0));
    }

    pub(crate) fn visits() -> usize {
        NODE_VISITS.with(Cell::get)
    }

    pub(crate) fn record_node() {
        NODE_VISITS.with(|visits| visits.set(visits.get() + 1));
    }
}
