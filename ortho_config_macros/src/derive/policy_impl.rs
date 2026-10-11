//! Policy-specific tokens for the derive-generated file discovery path.

use quote::quote;

use super::load_impl::{DiscoveryTokens, LoadSourceTokens};

struct PolicyLoadingTokens {
    builder_steps: Vec<proc_macro2::TokenStream>,
    env_selectors: Vec<proc_macro2::TokenStream>,
    explicit_mode: proc_macro2::TokenStream,
    automatic_mode: proc_macro2::TokenStream,
    scope_order_call: proc_macro2::TokenStream,
    project_root: Option<proc_macro2::TokenStream>,
}

/// One `builder = builder.<method>(<literal>);` step, when the key was given.
///
/// The generated builder calls are all the same shape and all optional, so the
/// three of them `policy_builder_steps` emits share this. `None` means the
/// attribute omitted the key, which leaves the builder's own default in force
/// rather than overwriting it with an empty string.
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

/// The `builder = builder.…;` steps the policy path needs before its own.
///
/// Only the keys that were actually written become steps, and the injected
/// source is appended last because it has to be installed on the builder rather
/// than consulted by it.
fn policy_builder_steps(
    discovery: &DiscoveryTokens,
    source_tokens: Option<&LoadSourceTokens<'_>>,
) -> Vec<proc_macro2::TokenStream> {
    let mut steps: Vec<proc_macro2::TokenStream> = [
        optional_builder_step(discovery.config_file_name.as_ref(), "config_file_name"),
        optional_builder_step(discovery.dotfile_name.as_ref(), "dotfile_name"),
        optional_builder_step(discovery.project_file_name.as_ref(), "project_file_name"),
    ]
    .into_iter()
    .flatten()
    .collect();
    // The policy path consults the environment twice over — the selector's
    // `env` rungs and automatic discovery's own lookups — so an injected
    // source has to reach the builder here exactly as it does on the legacy
    // path. Without this step a `load_from_iter_with_sources` caller would
    // have its `MapEnv` ignored and discovery would read the real process
    // environment instead.
    if let Some(injected_sources) = source_tokens {
        let discovery_source = injected_sources.discovery;
        steps.push(quote! { builder = builder.env_source(#discovery_source); });
    }
    steps
}

/// One `ConfigPathSelector::env(…)` rung per declared variable, in order.
///
/// The order is the declared order and is load-bearing: the rungs are tried in
/// sequence, so the first variable named wins. `env_vars` names the compatibility
/// chain, not a set.
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

/// The env selector rungs a policy-enabled struct declares.
///
/// `env_vars` is the ordered alias chain, but `env_var` carries a materialised
/// default (`<PREFIX>_CONFIG_PATH`, or `CONFIG_PATH` unprefixed) that the
/// legacy emitter always installs. Opting into the policy path must not
/// silently drop that override: a struct setting only `automatic_mode` would
/// otherwise ignore the very variable its non-policy form honours. The two
/// attributes are mutually exclusive, so preferring `env_vars` when it is
/// non-empty cannot lose an explicitly written `env_var`.
fn env_selector_tokens(
    discovery: &DiscoveryTokens,
    krate: &proc_macro2::TokenStream,
) -> Vec<proc_macro2::TokenStream> {
    if discovery.env_vars.is_empty() {
        selector_tokens(std::slice::from_ref(&discovery.env_var), krate)
    } else {
        selector_tokens(&discovery.env_vars, krate)
    }
}

/// The mode constant the policy is built with, refusing an unknown spelling.
///
/// `explicit` selects which of the two attributes this is: the caller passes
/// `true` for `explicit_mode` and `false` for `automatic_mode`. An absent
/// attribute takes that mode's default, so the two defaults are named here
/// rather than at the call site. Because the pair is matched exhaustively, an
/// unrecognised value is a compile error naming both legal spellings, and a
/// value legal for the *other* attribute is rejected rather than accepted.
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

/// The `.scope_order([…])` call, or nothing when no order was declared.
///
/// Scope order is precedence order, so the declared sequence is preserved
/// exactly. An empty list emits no call at all rather than an empty one, which
/// leaves the builder's default order in force; an unrecognised scope is a
/// compile error naming the three legal values.
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

/// Every emitted fragment the policy branch is assembled from.
///
/// Gathering them here rather than in the emitter keeps the validation in one
/// place: `mode_tokens` and `scope_order_tokens` both reject invalid spellings,
/// and the first rejection returned names the attribute at fault rather than
/// surfacing as a confusing error in the generated code.
///
/// `project_root_from` is the exception that is not a builder step. It names a
/// CLI field rather than a literal, so it becomes a guard emitted at the use
/// site, and it applies only when that field carries a value.
///
/// # Errors
///
/// Returns the first rejection from `mode_tokens` or `scope_order_tokens`.
fn policy_tokens(
    discovery: &DiscoveryTokens,
    krate: &proc_macro2::TokenStream,
    source_tokens: Option<&LoadSourceTokens<'_>>,
) -> syn::Result<PolicyLoadingTokens> {
    let project_root = discovery.project_root_from.as_ref().map(|field_name| {
        let field = syn::Ident::new(field_name, proc_macro2::Span::call_site());
        quote! { if let Some(ref cli) = cli { if let Some(root) = cli.#field.clone() { policy = policy.project_root(root); } } }
    });
    Ok(PolicyLoadingTokens {
        builder_steps: policy_builder_steps(discovery, source_tokens),
        env_selectors: env_selector_tokens(discovery, krate),
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
    source_tokens: Option<&LoadSourceTokens<'_>>,
) -> proc_macro2::TokenStream {
    let tokens = match policy_tokens(discovery, krate, source_tokens) {
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

#[cfg(test)]
mod tests {
    //! Unit tests for the policy mode and scope-order token builders.

    use super::*;
    use anyhow::{Result, anyhow, ensure};
    use rstest::rstest;

    #[rstest]
    #[case::explicit_default(None, true, "ExplicitMode :: RequiredExclusive")]
    #[case::explicit_required(
        Some("required_exclusive"),
        true,
        "ExplicitMode :: RequiredExclusive"
    )]
    #[case::explicit_optional(Some("optional"), true, "ExplicitMode :: Optional")]
    #[case::automatic_default(None, false, "AutomaticMode :: FirstWins")]
    #[case::automatic_first_wins(Some("first_wins"), false, "AutomaticMode :: FirstWins")]
    #[case::automatic_stack_scopes(Some("stack_scopes"), false, "AutomaticMode :: StackScopes")]
    /// Every legal spelling, and each absent attribute's default, maps to
    /// the variant that spelling names.
    fn mode_tokens_accepts_supported_modes(
        #[case] mode: Option<&str>,
        #[case] explicit: bool,
        #[case] expected_variant: &str,
    ) -> Result<()> {
        let krate = quote! { ::ortho_config };
        let tokens = mode_tokens(mode, &krate, explicit).map_err(|err| anyhow!(err))?;
        ensure!(
            tokens.to_string().contains(expected_variant),
            "expected {expected_variant}, got {tokens}"
        );
        Ok(())
    }

    #[test]
    /// A spelling legal only for `automatic_mode` is refused here, and the
    /// error names this attribute rather than the other one.
    fn mode_tokens_rejects_unknown_explicit_mode() -> Result<()> {
        let krate = quote! { ::ortho_config };
        let err = mode_tokens(Some("bogus"), &krate, true)
            .err()
            .ok_or_else(|| anyhow!("expected an explicit_mode error"))?;
        ensure!(
            err.to_string() == "explicit_mode must be required_exclusive or optional",
            "unexpected error message: {err}",
        );
        Ok(())
    }

    #[test]
    /// The mirror of the explicit case: the two attributes do not share a
    /// vocabulary, so a swapped value is caught rather than accepted.
    fn mode_tokens_rejects_unknown_automatic_mode() -> Result<()> {
        let krate = quote! { ::ortho_config };
        let err = mode_tokens(Some("bogus"), &krate, false)
            .err()
            .ok_or_else(|| anyhow!("expected an automatic_mode error"))?;
        ensure!(
            err.to_string() == "automatic_mode must be first_wins or stack_scopes",
            "unexpected error message: {err}",
        );
        Ok(())
    }

    #[test]
    /// Scope order is precedence order, so the emitted variants must appear
    /// in the declared sequence.
    fn scope_order_tokens_preserves_declared_order() -> Result<()> {
        let krate = quote! { ::ortho_config };
        let scopes = vec!["system".to_owned(), "user".to_owned(), "project".to_owned()];
        let rendered = scope_order_tokens(&scopes, &krate)
            .map_err(|err| anyhow!(err))?
            .to_string();
        let system = rendered
            .find("DiscoveryScope :: System")
            .ok_or_else(|| anyhow!("missing System variant: {rendered}"))?;
        let user = rendered
            .find("DiscoveryScope :: User")
            .ok_or_else(|| anyhow!("missing User variant: {rendered}"))?;
        let project = rendered
            .find("DiscoveryScope :: Project")
            .ok_or_else(|| anyhow!("missing Project variant: {rendered}"))?;
        ensure!(
            system < user && user < project,
            "scope order not preserved: {rendered}"
        );
        Ok(())
    }

    #[test]
    /// An undeclared order emits no call, leaving the builder's default
    /// rather than pinning an empty list.
    fn scope_order_tokens_emits_nothing_when_empty() -> Result<()> {
        let krate = quote! { ::ortho_config };
        let tokens = scope_order_tokens(&[], &krate).map_err(|err| anyhow!(err))?;
        ensure!(
            tokens.is_empty(),
            "expected an empty token stream, got {tokens}"
        );
        Ok(())
    }

    #[test]
    /// The rejection is total: one bad entry fails the whole list, so a
    /// partly valid declaration cannot emit a truncated order.
    fn scope_order_tokens_rejects_unknown_scope() -> Result<()> {
        let krate = quote! { ::ortho_config };
        let scopes = vec!["user".to_owned(), "userland".to_owned()];
        let err = scope_order_tokens(&scopes, &krate)
            .err()
            .ok_or_else(|| anyhow!("expected a scope_order error"))?;
        ensure!(
            err.to_string() == "scope_order values must be system, user, or project",
            "unexpected error message: {err}",
        );
        Ok(())
    }
}
