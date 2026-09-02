//! Recursive dictionary merging for independently nested environment entries.
//!
//! Figment's provider implementation uses an internal coalescing trait that
//! is not public. This helper preserves its merge behaviour: incoming scalar
//! values replace earlier ones, while incoming dictionaries retain sibling
//! keys already nested under the same parent.

use figment::value::{Dict, Value};

/// Merge an incoming nested entry into the dictionary collected so far.
pub(super) fn merge_dicts(mut existing: Dict, incoming: Dict) -> Dict {
    for (key, incoming_value) in incoming {
        let value = match (existing.remove(&key), incoming_value) {
            (Some(Value::Dict(tag, existing_child)), Value::Dict(_, incoming_child)) => {
                Value::Dict(tag, merge_dicts(existing_child, incoming_child))
            }
            (_, replacement) => replacement,
        };
        existing.insert(key, value);
    }
    existing
}
