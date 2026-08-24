//! Policy-specific tokens for the derive-generated file discovery path.

use quote::quote;

use super::load_impl::DiscoveryTokens;

struct PolicyLoadingTokens {
    builder_steps: Vec<proc_macro2::TokenStream>,
    env_selectors: Vec<proc_macro2::TokenStream>,
    explicit_mode: proc_macro2::TokenStream,
    automatic_mode: proc_macro2::TokenStream,
    scope_order_call: proc_macro2::TokenStream,
    project_root: Option<proc_macro2::TokenStream>,
}

fn optional_builder_step(
    value: Option<&String>,
    method_name: &str,
) -> Option<proc_macro2::TokenStream> {
    value.map(|contents| {
        let literal = syn::LitStr::new(contents, proc_macro2::Span::call_site());
        let method_ident = syn::Ident::new(method_name, proc_macro2::Span::call_site());
        quote! { builder = builder.#method_ident(#literal); }
    })
}

fn policy_builder_steps(discovery: &DiscoveryTokens) -> Vec<proc_macro2::TokenStream> {
    [
        optional_builder_step(discovery.config_file_name.as_ref(), "config_file_name"),
        optional_builder_step(discovery.dotfile_name.as_ref(), "dotfile_name"),
        optional_builder_step(discovery.project_file_name.as_ref(), "project_file_name"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn selector_tokens(
    env_vars: &[String],
    krate: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    env_vars
        .iter()
        .map(|variable_name| {
            let variable = syn::LitStr::new(variable_name, proc_macro2::Span::call_site());
            quote! { #krate::ConfigPathSelector::env(#variable) }
        })
        .collect()
}

fn mode_tokens(
    mode: Option<&str>,
    krate: &proc_macro2::TokenStream,
    explicit: bool,
) -> syn::Result<proc_macro2::TokenStream> {
    match (
        explicit,
        mode.unwrap_or(if explicit {
            "required_exclusive"
        } else {
            "first_wins"
        }),
    ) {
        (true, "required_exclusive") => Ok(quote! { #krate::ExplicitMode::RequiredExclusive }),
        (true, "optional") => Ok(quote! { #krate::ExplicitMode::Optional }),
        (false, "first_wins") => Ok(quote! { #krate::AutomaticMode::FirstWins }),
        (false, "stack_scopes") => Ok(quote! { #krate::AutomaticMode::StackScopes }),
        (true, _) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "explicit_mode must be required_exclusive or optional",
        )),
        (false, _) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "automatic_mode must be first_wins or stack_scopes",
        )),
    }
}

fn scope_order_tokens(
    scopes: &[String],
    krate: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let variants = scopes
        .iter()
        .map(|scope| match scope.as_str() {
            "system" => Ok(quote! { #krate::DiscoveryScope::System }),
            "user" => Ok(quote! { #krate::DiscoveryScope::User }),
            "project" => Ok(quote! { #krate::DiscoveryScope::Project }),
            _ => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "scope_order values must be system, user, or project",
            )),
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(if variants.is_empty() {
        quote! {}
    } else {
        quote! { .scope_order([ #( #variants ),* ]) }
    })
}

fn policy_tokens(
    discovery: &DiscoveryTokens,
    krate: &proc_macro2::TokenStream,
) -> syn::Result<PolicyLoadingTokens> {
    let project_root = discovery.project_root_from.as_ref().map(|(field_name, is_optional)| {
        let field = syn::Ident::new(field_name, proc_macro2::Span::call_site());
        if *is_optional {
            quote! { if let Some(ref cli) = cli { if let Some(ref root) = cli.#field { policy = policy.project_root(root.clone()); } } }
        } else {
            quote! { if let Some(ref cli) = cli { policy = policy.project_root(cli.#field.clone()); } }
        }
    });
    Ok(PolicyLoadingTokens {
        builder_steps: policy_builder_steps(discovery),
        env_selectors: selector_tokens(&discovery.env_vars, krate),
        explicit_mode: mode_tokens(discovery.explicit_mode.as_deref(), krate, true)?,
        automatic_mode: mode_tokens(discovery.automatic_mode.as_deref(), krate, false)?,
        scope_order_call: scope_order_tokens(&discovery.scope_order, krate)?,
        project_root,
    })
}

/// Emit the opt-in policy branch of the derive-generated file loader.
pub(crate) fn build_policy_based_loading(
    krate: &proc_macro2::TokenStream,
    discovery: &DiscoveryTokens,
    has_config_path: bool,
) -> proc_macro2::TokenStream {
    let tokens = match policy_tokens(discovery, krate) {
        Ok(tokens) => tokens,
        Err(error) => return error.to_compile_error(),
    };
    let app_name = syn::LitStr::new(&discovery.app_name, proc_macro2::Span::call_site());
    let cli_selector = if has_config_path {
        quote! { if let Some(ref cli) = cli { selectors.push(#krate::ConfigPathSelector::cli(cli.config_path.clone())); } }
    } else {
        quote! {}
    };
    let PolicyLoadingTokens {
        builder_steps,
        env_selectors,
        explicit_mode,
        automatic_mode,
        scope_order_call,
        project_root,
    } = tokens;
    quote! {{ let mut builder = #krate::ConfigDiscovery::builder(#app_name); #(#builder_steps)* let mut selectors = Vec::new(); #cli_selector #( selectors.push(#env_selectors); )* let mut policy = #krate::ConfigFilePolicy::from_builder(builder).selectors(selectors).explicit_mode(#explicit_mode).automatic_mode(#automatic_mode) #scope_order_call; #project_root let outcome = policy.resolve_layers(); outcome.into_layers_and_errors(&mut errors) }}
}
