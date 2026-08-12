//! Token emission for the optional `behaviour` IR block.
//!
//! Kept in its own module so `sections.rs` stays under the repository's
//! 400-line ceiling.

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::parse::{BehaviourAttrs, DocStructAttrs};

/// Builds the token stream for the optional `behaviour` IR block.
///
/// Returns `None` when no `behaviour(...)` declaration exists, so consumers
/// keep seeing the undeclared state rather than an inferred value.
pub(super) fn build_behaviour_metadata(doc: &DocStructAttrs, krate: &TokenStream) -> TokenStream {
    let Some(behaviour) = doc.behaviour.as_ref() else {
        return quote! { None };
    };
    let interaction = interaction_tokens(behaviour, krate);
    let mutation = mutation_tokens(behaviour, krate);
    let bypass = string_option_tokens(behaviour.bypass.as_deref());
    let dry_run = string_option_tokens(behaviour.dry_run.as_deref());

    quote! {
        Some(#krate::docs::BehaviourMetadata {
            interaction: #interaction,
            mutation: #mutation,
            bypass: #bypass,
            dry_run: #dry_run,
        })
    }
}

fn interaction_tokens(behaviour: &BehaviourAttrs, krate: &TokenStream) -> TokenStream {
    let Some(value) = behaviour.interaction.as_deref() else {
        return quote! { None };
    };
    match value {
        "non_interactive" => quote! { Some(#krate::docs::InteractionKind::NonInteractive) },
        "interactive" => quote! { Some(#krate::docs::InteractionKind::Interactive) },
        _ => quote! { None },
    }
}

fn mutation_tokens(behaviour: &BehaviourAttrs, krate: &TokenStream) -> TokenStream {
    let Some(value) = behaviour.mutation.as_deref() else {
        return quote! { None };
    };
    let variant = match value {
        "read_only" => quote! { ReadOnly },
        "write" => quote! { Write },
        "delete" => quote! { Delete },
        "submit" => quote! { Submit },
        _ => return quote! { None },
    };
    quote! { Some(#krate::docs::MutationKind::#variant) }
}

fn string_option_tokens(value: Option<&str>) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |text| {
            let lit = syn::LitStr::new(text, proc_macro2::Span::call_site());
            quote! { Some(String::from(#lit)) }
        },
    )
}
