//! Model types, the disclosure projection, and its value-level lemmas.
//!
//! The crate root `profile_redaction.rs` documents the model-to-production
//! mapping and why the command tree is encoded as a parent-before-child
//! sequence. This module holds that encoding verbatim: the
//! `Doc`/`Command`/`Field`/`ValueType` shapes, the two projection traversals,
//! the `is_tree_like` invariant, and the per-field and per-value lemmas the
//! semantic obligations rest on.

use vstd::prelude::*;

verus! {

/// A field's non-disclosure classification. `Ordinary` models "no label".
pub enum Exposure {
    /// No label: every slot is public.
    Ordinary,
    /// Confidential material.
    Secret,
    /// A locator the application resolves elsewhere.
    ReferenceOnly,
}

/// A structural value type. Only `Enum(..)` carries a list of literals.
pub enum ValueType {
    /// A scalar, tagged by which scalar kind it is.
    ///
    /// Production has ten distinct scalar variants (`String`, `Integer {
    /// bits, signed }`, `Float { bits }`, `Bool`, `Duration`, `Path`, `IpAddr`,
    /// `Hostname`, `Url`, `Custom`). The model collapses them into one opaque
    /// tag: no obligation inspects which scalar it is, only that the projection
    /// returns the same one, so the tag carries "same scalar kind" without
    /// enumerating ten cases. Modelled as a carried field rather than as the
    /// unit `Scalar` so that `clear_members` demonstrably preserves it instead
    /// of reconstructing a constant.
    Scalar(int),
    /// An enumeration and its members.
    Enum(Seq<int>),
    /// A list of the inner type.
    List(Box<ValueType>),
    /// A map of the inner type.
    Map(Box<ValueType>),
}

/// CLI metadata other than the possible-value list, which is a value slot.
pub struct Cli {
    /// Long flag spelling.
    pub long: Option<int>,
    /// Short flag spelling.
    pub short: Option<int>,
    /// Whether the flag consumes a value.
    pub takes_value: bool,
}

/// One configuration field.
pub struct Field {
    /// Public field name.
    pub name: int,
    /// Public help identifiers.
    pub help_id: int,
    /// Declared classification.
    pub exposure: Exposure,
    /// Whether the field is required.
    pub required: bool,
    /// Public CLI metadata.
    pub cli: Option<Cli>,
    /// Public environment variable.
    pub env: Option<int>,
    /// Public file key.
    pub file: Option<int>,
    /// Value-bearing slot: the documented default.
    pub default: Option<int>,
    /// Value-bearing slot: field-level examples.
    pub examples: Seq<int>,
    /// Value-bearing slot: CLI possible values.
    pub possible_values: Seq<int>,
    /// Structural value type, whose enum members are value-bearing.
    pub value: Option<ValueType>,
}

/// One command of the documentation tree.
pub struct Command {
    /// Index of the parent command, or `-1` for the root.
    pub parent: int,
    /// Value-bearing slot: command-level examples.
    pub sections_examples: Seq<int>,
    /// Fields declared on this command.
    pub fields: Seq<Field>,
}

/// A documentation document: its commands in any parent-before-child order.
///
/// Production's nested encoding linearizes to preorder, which is one such
/// order; [`is_tree_like`] states only what the proofs use.
pub struct Doc {
    /// Every command of the tree.
    pub commands: Seq<Command>,
}

/// Removes every literal a value type carries while keeping its shape.
pub open spec fn clear_members(value: ValueType) -> ValueType
    decreases value
{
    match value {
        ValueType::Scalar(tag) => ValueType::Scalar(tag),
        ValueType::Enum(_members) => ValueType::Enum(Seq::empty()),
        ValueType::List(inner) => ValueType::List(Box::new(clear_members(*inner))),
        ValueType::Map(inner) => ValueType::Map(Box::new(clear_members(*inner))),
    }
}

/// Applies [`clear_members`] through the optional value type.
pub open spec fn clear_value_shape(value: Option<ValueType>) -> Option<ValueType> {
    match value {
        None => None,
        Some(inner) => Some(clear_members(inner)),
    }
}

/// Reports whether a field is protected, matching `profile_field.is_some()`.
pub open spec fn is_protected(field: Field) -> bool {
    field.exposure != Exposure::Ordinary
}

/// Empties a protected field's value-bearing slots and nothing else.
pub open spec fn redact_field(field: Field) -> Field {
    if is_protected(field) {
        Field {
            default: None,
            examples: Seq::empty(),
            possible_values: Seq::empty(),
            value: clear_value_shape(field.value),
            ..field
        }
    } else {
        field
    }
}

/// Projects every field of one command.
pub open spec fn redact_fields(fields: Seq<Field>) -> Seq<Field> {
    Seq::new(fields.len(), |i: int| redact_field(fields[i]))
}

/// Reports whether one command declares a protected field.
pub open spec fn command_has_protected(command: Command) -> bool {
    exists|j: int| 0 <= j < command.fields.len()
        && is_protected(#[trigger] command.fields[j])
}

/// Reports whether any field anywhere in the document is protected.
pub open spec fn contains_protected(doc: Doc) -> bool {
    exists|i: int| 0 <= i < doc.commands.len()
        && command_has_protected(#[trigger] doc.commands[i])
}

/// Rewrites a document that contains a protected field.
///
/// Every command example goes, because an unstructured example cannot be mapped
/// back to the fields it exercises, so a parent or sibling example could
/// disclose a protected child's value. Every field is passed through
/// [`redact_field`], which only touches protected ones.
pub open spec fn hard_project(doc: Doc) -> Doc {
    Doc {
        commands: Seq::new(doc.commands.len(), |i: int| Command {
            parent: doc.commands[i].parent,
            sections_examples: Seq::empty(),
            fields: redact_fields(doc.commands[i].fields),
        }),
    }
}


/// The disclosure projection `P`.
pub open spec fn project(doc: Doc) -> Doc {
    if contains_protected(doc) {
        hard_project(doc)
    } else {
        doc
    }
}

/// Reports whether an optional scalar slot carries no value.
pub open spec fn is_absent(value: Option<int>) -> bool {
    match value {
        None => true,
        Some(_) => false,
    }
}

// ---------------------------------------------------------------------------
// The tree invariant.
// ---------------------------------------------------------------------------

/// The encoding is a genuine rooted tree: one root, and every other command's
/// parent strictly earlier than itself.
pub open spec fn is_tree_like(doc: Doc) -> bool {
    doc.commands.len() > 0
        && doc.commands[0].parent == -1
        && (forall|i: int| 1 <= i < doc.commands.len()
            ==> 0 <= #[trigger] doc.commands[i].parent < i)
}

// ---------------------------------------------------------------------------
// Model lemmas: the value-type and field projections.
// ---------------------------------------------------------------------------

pub proof fn clear_members_idempotent(value: ValueType)
    ensures clear_members(clear_members(value)) == clear_members(value),
    decreases value,
{
    match value {
        ValueType::Scalar(_tag) => {},
        ValueType::Enum(_members) => {},
        ValueType::List(inner) => clear_members_idempotent(*inner),
        ValueType::Map(inner) => clear_members_idempotent(*inner),
    }
}

pub proof fn clear_value_shape_idempotent(value: Option<ValueType>)
    ensures clear_value_shape(clear_value_shape(value)) == clear_value_shape(value),
{
    match value {
        None => {},
        Some(inner) => clear_members_idempotent(inner),
    }
}

pub proof fn redact_field_keeps_exposure(field: Field)
    ensures redact_field(field).exposure == field.exposure,
{
}

pub proof fn redact_fields_keep_exposure(fields: Seq<Field>)
    ensures forall|i: int| 0 <= i < fields.len()
            ==> is_protected(#[trigger] redact_fields(fields)[i]) == is_protected(fields[i]),
{
    assert forall|i: int| 0 <= i < fields.len() implies
        is_protected(#[trigger] redact_fields(fields)[i]) == is_protected(fields[i]) by {
        redact_field_keeps_exposure(fields[i]);
    };
}

pub proof fn redact_field_idempotent(field: Field)
    ensures redact_field(redact_field(field)) == redact_field(field),
{
    if is_protected(field) {
        clear_value_shape_idempotent(field.value);
        redact_field_keeps_exposure(field);
    }
}

pub proof fn redact_fields_idempotent(fields: Seq<Field>)
    ensures redact_fields(redact_fields(fields)) == redact_fields(fields),
{
    assert forall|i: int| 0 <= i < fields.len() implies
        redact_field(#[trigger] redact_fields(fields)[i]) == redact_fields(fields)[i] by {
        redact_field_idempotent(fields[i]);
    };
}

/// Every literal a value type carries is gone.
pub open spec fn value_literals_cleared(value: ValueType) -> bool
    decreases value
{
    match value {
        ValueType::Scalar(_tag) => true,
        ValueType::Enum(members) => members.len() == 0,
        ValueType::List(inner) | ValueType::Map(inner) => value_literals_cleared(*inner),
    }
}

/// A field keeps no value-bearing slot when it is protected.
pub open spec fn field_slots_cleared(field: Field) -> bool {
    !is_protected(field) || (is_absent(field.default)
        && field.examples.len() == 0
        && field.possible_values.len() == 0
        && match field.value {
            None => true,
            Some(inner) => value_literals_cleared(inner),
        })
}

pub proof fn clear_members_clears_members(value: ValueType)
    ensures value_literals_cleared(clear_members(value)),
    decreases value,
{
    match value {
        ValueType::Scalar(_tag) => {},
        ValueType::Enum(_members) => {},
        ValueType::List(inner) => clear_members_clears_members(*inner),
        ValueType::Map(inner) => clear_members_clears_members(*inner),
    }
}

pub proof fn clear_value_shape_preserves_cleared(value: Option<ValueType>)
    ensures
        match clear_value_shape(value) {
            None => true,
            Some(inner) => value_literals_cleared(inner),
        },
{
    match value {
        None => {},
        Some(inner) => clear_members_clears_members(inner),
    }
}

pub proof fn redact_field_clears_slots(field: Field)
    ensures field_slots_cleared(redact_field(field)),
{
    if is_protected(field) {
        clear_value_shape_preserves_cleared(field.value);
    }
}

pub proof fn redact_fields_clear_slots(fields: Seq<Field>)
    ensures forall|i: int| 0 <= i < fields.len()
            ==> field_slots_cleared(#[trigger] redact_fields(fields)[i]),
{
    assert forall|i: int| 0 <= i < fields.len()
        implies field_slots_cleared(#[trigger] redact_fields(fields)[i]) by {
        redact_field_clears_slots(fields[i]);
    };
}

}
