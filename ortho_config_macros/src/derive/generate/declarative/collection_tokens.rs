//! Collection token generation for declarative merge.
//!
//! Generates tokens for handling append-strategy vectors and replace-strategy
//! maps during the declarative merge process.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;

use crate::derive::build::CollectionStrategies;

/// Parallel token fragments for collection-state declaration, merging, and
/// extraction into the final overlay.
pub(super) struct CollectionTokens {
    /// Per-layer statements applying the collection's merge policy.
    pub merge_logic: Vec<TokenStream>,
    /// Pattern entries that bind the generated collection state during finish.
    pub destructured: Vec<TokenStream>,
    /// Statements that transfer collected values into the output overlay.
    pub inserts: Vec<TokenStream>,
}

/// Builds corresponding merge, destructuring, and insertion fragments from one
/// ordered collection field iterator.
fn build_collection_tokens<'a, I, F, G>(
    fields: I,
    prefix: &str,
    merge_fn: F,
    insert_fn: G,
) -> CollectionTokens
where
    I: IntoIterator<Item = (&'a syn::Ident, &'a syn::Type)>,
    F: Fn(&syn::Ident, &proc_macro2::Ident, &str) -> TokenStream,
    G: Fn(&syn::Ident, &proc_macro2::Ident, &str) -> TokenStream,
{
    let mut merge_logic = Vec::new();
    let mut destructured = Vec::new();
    let mut inserts = Vec::new();

    for (field_ident, _ty) in fields {
        let state_field_ident = format_ident!("{}{}", prefix, field_ident);
        let field_name = field_ident.to_string();
        merge_logic.push(merge_fn(field_ident, &state_field_ident, &field_name));
        destructured.push(quote! { #state_field_ident });
        inserts.push(insert_fn(field_ident, &state_field_ident, &field_name));
    }

    CollectionTokens {
        merge_logic,
        destructured,
        inserts,
    }
}

/// Emits layer accumulation and final overlay insertion for append vectors.
///
/// A null layer value resets the accumulated vector; a scalar is normalized to
/// one element so file and environment sources can provide either shape.
pub(super) fn append_collection_tokens(
    strategies: &CollectionStrategies,
    krate: &TokenStream,
) -> CollectionTokens {
    build_collection_tokens(
        unique_append_fields(&strategies.append),
        "append_",
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(value) = map.remove(#field_name) {
                    if value.is_null() {
                        self.#state_field_ident = Some(Vec::new());
                    } else {
                        let normalised = match value {
                            #krate::serde_json::Value::Array(_) => value,
                            other => #krate::serde_json::Value::Array(vec![other]),
                        };
                        let incoming: Vec<_> =
                            #krate::declarative::from_value_merge(normalised)?;
                        let acc = self
                            .#state_field_ident
                            .get_or_insert_with(Default::default);
                        acc.extend(incoming);
                    }
                }
            }
        },
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(values) = #state_field_ident {
                    overlay.insert(
                        #field_name.to_owned(),
                        #krate::serde_json::Value::Array(values),
                    );
                }
            }
        },
    )
}

/// Emits whole-map replacement fragments so the highest-precedence supplied
/// map is the value written to the final overlay.
pub(super) fn map_collection_tokens(strategies: &CollectionStrategies) -> CollectionTokens {
    build_collection_tokens(
        strategies.map_replace.iter().map(|(ident, ty)| (ident, ty)),
        "replace_",
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(value) = map.remove(#field_name) {
                    self.#state_field_ident = Some(value);
                }
            }
        },
        |_, state_field_ident, field_name| {
            quote! {
                if let Some(value) = #state_field_ident {
                    overlay.insert(#field_name.to_owned(), value);
                }
            }
        },
    )
}

/// Deduplicate append fields by identifier.
///
/// Returns a vector of unique `(ident, type)` pairs, preserving the first
/// occurrence for each identifier in the input slice.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::parse_str;
///
/// let fields = vec![
///     (parse_str("items").unwrap(), parse_str("String").unwrap()),
///     (parse_str("items").unwrap(), parse_str("String").unwrap()),
///     (parse_str("tags").unwrap(), parse_str("String").unwrap()),
/// ];
///
/// let unique = unique_append_fields(&fields);
/// assert_eq!(unique.len(), 2);
/// assert_eq!(unique[0].0.to_string(), "items");
/// assert_eq!(unique[1].0.to_string(), "tags");
/// ```
pub(crate) fn unique_append_fields(
    append_fields: &[(syn::Ident, syn::Type)],
) -> Vec<(&syn::Ident, &syn::Type)> {
    let mut seen = HashSet::new();
    append_fields
        .iter()
        .filter_map(|(ident, ty)| {
            let key = ident.to_string();
            seen.insert(key).then_some((ident, ty))
        })
        .collect()
}
