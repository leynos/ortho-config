//! V1, noninterference: low-equivalent documents project equally.
//!
//! Low-equivalence is the observation the output is allowed to make. It fixes
//! every public descriptor and permits protected value slots to differ. When no
//! field anywhere is protected, command examples are published, so it requires
//! them to agree as well; when any field is protected, every example is
//! suppressed, so it leaves them free.

use vstd::prelude::*;

verus! {

use crate::profile_redaction_model::*;

// ---------------------------------------------------------------------------
// V1: noninterference.
// ---------------------------------------------------------------------------

/// Two fields agree on everything the policy permits to reveal.
pub open spec fn field_low_equiv(left: Field, right: Field) -> bool {
    &&& left.name == right.name
    &&& left.help_id == right.help_id
    &&& left.exposure == right.exposure
    &&& left.required == right.required
    &&& left.cli == right.cli
    &&& left.env == right.env
    &&& left.file == right.file
    &&& clear_value_shape(left.value) == clear_value_shape(right.value)
    &&& (!is_protected(left)
        ==> left.default == right.default && left.examples == right.examples
            && left.possible_values == right.possible_values && left.value == right.value)
}

/// Two commands agree on their topology and on every field's public part.
pub open spec fn command_low_equiv(left: Command, right: Command) -> bool {
    &&& left.parent == right.parent
    &&& left.fields.len() == right.fields.len()
    &&& (forall|j: int| 0 <= j < left.fields.len()
        ==> field_low_equiv(#[trigger] left.fields[j], right.fields[j]))
}

/// Low-equivalence: the observation the output is allowed to make.
///
/// When the whole document has no protected field, every example is published,
/// so command examples must agree. A protected field anywhere suppresses every
/// example, which is why the consequent is an implication rather than a
/// biconditional: the suppressed case constrains nothing.
pub open spec fn low_equiv(left: Doc, right: Doc) -> bool {
    &&& left.commands.len() == right.commands.len()
    &&& (forall|i: int| 0 <= i < left.commands.len()
        ==> command_low_equiv(#[trigger] left.commands[i], right.commands[i]))
    &&& (!contains_protected(left)
        ==> (forall|i: int| 0 <= i < left.commands.len()
            ==> #[trigger] left.commands[i].sections_examples
                == right.commands[i].sections_examples))
}

proof fn low_equiv_shares_decision(left: Doc, right: Doc)
    requires low_equiv(left, right),
    ensures contains_protected(left) == contains_protected(right),
{
    if contains_protected(left) != contains_protected(right) {
        if contains_protected(left) {
            let i = choose|i: int| 0 <= i < left.commands.len()
                && command_has_protected(#[trigger] left.commands[i]);
            let j = choose|j: int| 0 <= j < left.commands[i].fields.len()
                && is_protected(#[trigger] left.commands[i].fields[j]);
            assert(is_protected(right.commands[i].fields[j]));
            assert(contains_protected(right));
        } else {
            let i = choose|i: int| 0 <= i < right.commands.len()
                && command_has_protected(#[trigger] right.commands[i]);
            let j = choose|j: int| 0 <= j < right.commands[i].fields.len()
                && is_protected(#[trigger] right.commands[i].fields[j]);
            assert(is_protected(left.commands[i].fields[j]));
            assert(contains_protected(left));
        }
    }
}

proof fn redact_field_congruence(left: Field, right: Field)
    requires field_low_equiv(left, right),
    ensures redact_field(left) == redact_field(right),
{
    if is_protected(left) {
        assert(redact_field(left).exposure == redact_field(right).exposure);
    } else {
        assert(!is_protected(right));
        assert(redact_field(left) == left);
        assert(redact_field(right) == right);
        assert(left == right);
    }
}

proof fn redact_fields_congruence(left: Seq<Field>, right: Seq<Field>)
    requires
        left.len() == right.len(),
        forall|j: int| 0 <= j < left.len() ==> field_low_equiv(#[trigger] left[j], right[j]),
    ensures redact_fields(left) == redact_fields(right),
{
    assert forall|j: int| 0 <= j < left.len() implies
        #[trigger] redact_fields(left)[j] == redact_fields(right)[j] by {
        redact_field_congruence(left[j], right[j]);
    };
}

proof fn hard_project_congruence(left: Doc, right: Doc)
    requires low_equiv(left, right),
    ensures hard_project(left) == hard_project(right),
{
    assert forall|i: int| 0 <= i < left.commands.len() implies
        hard_project(left).commands[i] == hard_project(right).commands[i] by {
        assert(#[trigger] left.commands[i].fields.len() == right.commands[i].fields.len());
        redact_fields_congruence(left.commands[i].fields, right.commands[i].fields);
        assert(hard_project(left).commands[i].fields
            =~= hard_project(right).commands[i].fields);
    };
    assert(hard_project(left).commands =~= hard_project(right).commands);
}

/// With nothing protected, low-equivalence is full equality.
proof fn low_equiv_ordinary_equality(left: Doc, right: Doc)
    requires low_equiv(left, right), !contains_protected(left),
    ensures left == right,
{
    assert forall|i: int| 0 <= i < left.commands.len() implies
        #[trigger] left.commands[i] == right.commands[i] by {
        assert(left.commands[i].parent == right.commands[i].parent);
        assert(left.commands[i].sections_examples
            == right.commands[i].sections_examples);
        assert forall|j: int| 0 <= j < left.commands[i].fields.len()
            implies #[trigger] left.commands[i].fields[j]
                == right.commands[i].fields[j] by {
            assert(!is_protected(left.commands[i].fields[j]));
        };
        assert(left.commands[i].fields =~= right.commands[i].fields);
    };
    assert(left.commands =~= right.commands);
}

proof fn project_congruence(left: Doc, right: Doc)
    requires low_equiv(left, right),
    ensures project(left) == project(right),
{
    low_equiv_shares_decision(left, right);
    if contains_protected(left) {
        assert(contains_protected(right));
        hard_project_congruence(left, right);
    } else {
        assert(!contains_protected(right));
        low_equiv_ordinary_equality(left, right);
    }
}

}
