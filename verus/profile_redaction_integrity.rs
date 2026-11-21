//! V2, projection integrity: what the disclosure projection must preserve.
//!
//! Each obligation names one clause of the contract: descriptors and topology
//! survive, protected value slots are emptied, every command-level example is
//! suppressed once any field is protected, and a second projection changes
//! nothing.

use vstd::prelude::*;

verus! {

use crate::profile_redaction_model::*;

// ---------------------------------------------------------------------------
// V2: projection integrity.
// ---------------------------------------------------------------------------

/// Every protected field of one command carries no value slot.
pub open spec fn command_slots_cleared(command: Command) -> bool {
    forall|j: int| 0 <= j < command.fields.len()
        ==> field_slots_cleared(#[trigger] command.fields[j])
}

/// Every protected field carries no value slot.
pub open spec fn protected_slots_cleared(doc: Doc) -> bool {
    forall|i: int| 0 <= i < doc.commands.len()
        ==> command_slots_cleared(#[trigger] doc.commands[i])
}

/// Every command of the document carries no examples.
pub open spec fn all_examples_cleared(doc: Doc) -> bool {
    forall|i: int| 0 <= i < doc.commands.len()
        ==> #[trigger] doc.commands[i].sections_examples.len() == 0
}

/// One field keeps its public descriptor and, when ordinary, every slot.
pub open spec fn field_descriptor_preserved(before: Field, after: Field) -> bool {
    &&& after.name == before.name
    &&& after.help_id == before.help_id
    &&& after.exposure == before.exposure
    &&& after.required == before.required
    &&& after.cli == before.cli
    &&& after.env == before.env
    &&& after.file == before.file
    &&& clear_value_shape(after.value) == clear_value_shape(before.value)
    &&& (!is_protected(before)
        ==> after.default == before.default && after.examples == before.examples
            && after.possible_values == before.possible_values && after.value == before.value)
}

/// Public descriptors, ordinary slots, and topology survive projection.
///
/// One clause per preserved production slot; the enumeration is the point of
/// the obligation, because a projection that dropped a field would otherwise
/// still satisfy a merely existential statement.
pub open spec fn descriptors_preserved(before: Doc, after: Doc) -> bool {
    &&& after.commands.len() == before.commands.len()
    &&& (forall|i: int| 0 <= i < before.commands.len() ==> {
        &&& #[trigger] after.commands[i].parent == before.commands[i].parent
        &&& after.commands[i].fields.len() == before.commands[i].fields.len()
        &&& (forall|j: int| 0 <= j < before.commands[i].fields.len()
            ==> field_descriptor_preserved(
                #[trigger] before.commands[i].fields[j],
                after.commands[i].fields[j],
            ))
    })
}

proof fn redact_field_preserves_descriptor(field: Field)
    ensures field_descriptor_preserved(field, redact_field(field)),
{
    redact_field_keeps_exposure(field);
    if is_protected(field) {
        clear_value_shape_idempotent(field.value);
    } else {
        assert(redact_field(field) == field);
    }
}

proof fn redact_fields_preserve_descriptors(before: Seq<Field>, after: Seq<Field>)
    requires after == redact_fields(before),
    ensures
        after.len() == before.len(),
        forall|j: int| 0 <= j < before.len()
            ==> field_descriptor_preserved(
                #[trigger] before[j],
                after[j],
            ),
{
    assert forall|j: int| 0 <= j < before.len() implies
        field_descriptor_preserved(#[trigger] before[j], after[j]) by {
        redact_field_preserves_descriptor(before[j]);
    };
}

proof fn project_preserves_descriptors(doc: Doc)
    ensures descriptors_preserved(doc, project(doc)),
{
    if contains_protected(doc) {
        assert forall|i: int| 0 <= i < doc.commands.len() implies {
            &&& #[trigger] hard_project(doc).commands[i].parent == doc.commands[i].parent
            &&& hard_project(doc).commands[i].fields.len()
                == doc.commands[i].fields.len()
            &&& (forall|j: int| 0 <= j < doc.commands[i].fields.len()
                ==> field_descriptor_preserved(
                    #[trigger] doc.commands[i].fields[j],
                    hard_project(doc).commands[i].fields[j],
                ))
        } by {
            redact_fields_preserve_descriptors(
                doc.commands[i].fields,
                hard_project(doc).commands[i].fields,
            );
        };
    }
}

proof fn project_clears_protected_slots(doc: Doc)
    ensures protected_slots_cleared(project(doc)),
{
    if contains_protected(doc) {
        assert forall|i: int| 0 <= i < doc.commands.len() implies
            command_slots_cleared(#[trigger] hard_project(doc).commands[i]) by {
            redact_fields_clear_slots(doc.commands[i].fields);
        };
    }
}

proof fn project_suppresses_command_examples(doc: Doc)
    requires contains_protected(doc),
    ensures all_examples_cleared(project(doc)),
{
}

/// Redaction clears slots, not labels, so a second pass over an already
/// redacted document changes nothing.
proof fn hard_project_is_idempotent(doc: Doc)
    ensures hard_project(hard_project(doc)) == hard_project(doc),
{
    assert forall|i: int| 0 <= i < doc.commands.len() implies
        #[trigger] hard_project(hard_project(doc)).commands[i]
            == hard_project(doc).commands[i] by {
        redact_fields_idempotent(doc.commands[i].fields);
        assert(hard_project(hard_project(doc)).commands[i].fields
            =~= hard_project(doc).commands[i].fields);
    };
    assert(hard_project(hard_project(doc)).commands =~= hard_project(doc).commands);
}

proof fn project_is_idempotent(doc: Doc)
    ensures project(project(doc)) == project(doc),
{
    if contains_protected(doc) {
        assert(project(doc) == hard_project(doc));
        hard_project_is_idempotent(doc);
        if contains_protected(hard_project(doc)) {
            assert(project(hard_project(doc)) == hard_project(hard_project(doc)));
        } else {
            assert(project(hard_project(doc)) == hard_project(doc));
        }
    } else {
        assert(project(doc) == doc);
    }
}

proof fn project_preserves_tree(doc: Doc)
    requires is_tree_like(doc),
    ensures is_tree_like(project(doc)),
{
    if contains_protected(doc) {
        assert forall|i: int| 1 <= i < doc.commands.len() implies
            0 <= #[trigger] hard_project(doc).commands[i].parent < i by {};
    }
}

}
