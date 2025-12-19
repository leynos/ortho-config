//! Merge layer and finish token generation for declarative merge.
//!
//! Generates the core merge and finish method bodies for the `DeclarativeMerge`
//! trait implementation.

use proc_macro2::TokenStream;
use quote::quote;

use super::guards::generate_non_object_guard;

pub(super) struct FinishCollectionTokens<'a> {
    pub append_destructured: &'a [TokenStream],
    pub map_destructured: &'a [TokenStream],
    pub append_inserts: &'a [TokenStream],
    pub map_inserts: &'a [TokenStream],
}

/// Generate the `merge_layer` method body.
///
/// Produces tokens that extract the JSON object from the layer, apply map and
/// append collection strategies, and merge remaining fields into the state.
pub(super) fn merge_layer_tokens(
    config_ident: &syn::Ident,
    map_merge_logic: &[TokenStream],
    append_merge_logic: &[TokenStream],
) -> TokenStream {
    let non_object_guard = generate_non_object_guard(config_ident);
    quote! {
        let provenance = layer.provenance();
        let path = layer.path().map(|p| p.to_owned());
        let value = layer.into_value();
        let mut map = match value {
            ortho_config::serde_json::Value::Object(map) => map,
            other => { #non_object_guard }
        };
        #( #map_merge_logic )*
        #( #append_merge_logic )*
        if !map.is_empty() {
            ortho_config::declarative::merge_value(
                &mut self.value,
                ortho_config::serde_json::Value::Object(map),
            );
        }

        Ok(())
    }
}

/// Generate the `finish` method body.
///
/// Produces tokens that destructure the state, build an overlay with collected
/// append and map values, merge it into the final value, and deserialize.
pub(super) fn finish_tokens(
    state_ident: &syn::Ident,
    collections: &FinishCollectionTokens<'_>,
) -> TokenStream {
    let append_destructured = collections.append_destructured;
    let map_destructured = collections.map_destructured;
    let append_inserts = collections.append_inserts;
    let map_inserts = collections.map_inserts;
    quote! {
        let #state_ident {
            mut value,
            #( #append_destructured, )*
            #( #map_destructured, )*
        } = self;
        let mut overlay = ortho_config::serde_json::Map::new();
        #( #append_inserts )*
        #( #map_inserts )*
        if !overlay.is_empty() {
            ortho_config::declarative::merge_value(
                &mut value,
                ortho_config::serde_json::Value::Object(overlay),
            );
        }
        ortho_config::declarative::from_value_merge(value)
    }
}
