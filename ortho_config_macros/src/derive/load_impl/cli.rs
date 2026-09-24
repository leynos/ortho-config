//! Generated CLI parsing and layering fragments for derived configuration loads.

use quote::quote;

/// Generate CLI parsing that retains clap's value-source metadata for layering.
pub(super) fn build_cli_parse_tokens() -> proc_macro2::TokenStream {
    quote! {
        let (cli, matches) = match Self::command().try_get_matches_from_mut(iter) {
            Ok(matches) => match Self::from_arg_matches(&matches) {
                Ok(cli) => (Some(cli), Some(matches)),
                Err(error) => {
                    errors.push(std::sync::Arc::new(error.into()));
                    (None, None)
                }
            },
            Err(error) => {
                errors.push(std::sync::Arc::new(error.into()));
                (None, None)
            }
        };
    }
}

/// Generate CLI layering that omits defaults which clap did not receive explicitly.
pub(super) fn build_cli_layer_tokens(
    krate: &proc_macro2::TokenStream,
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    let sanitized_layer = build_sanitized_cli_layer_tokens(krate, cli_default_as_absent_fields);
    quote! {
        if let (Some(cli), Some(matches)) = (cli.as_ref(), matches.as_ref()) {
            #sanitized_layer
        }
    }
}

/// Generate sanitisation handling for the generated CLI layer.
fn build_sanitized_cli_layer_tokens(
    krate: &proc_macro2::TokenStream,
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    let prune_defaults =
        build_default_as_absent_pruning_tokens(cli_default_as_absent_fields, false);
    let push_cli = build_cli_push_tokens();
    quote! {
        match #krate::sanitize_value(cli) {
            Ok(mut value) => {
                #prune_defaults
                #push_cli
            }
            Err(err) => errors.push(err),
        }
    }
}

/// Generate removal of default-derived fields from the generated CLI JSON object.
///
/// The emitted statements expect `matches: &clap::ArgMatches` in scope.
/// Both the declarative and the profile-enabled CLI layers share this pruning
/// so an inferred clap default cannot outrank a profile or file layer.
///
/// `retain_environment_values` decides whether an environment-sourced value
/// counts as explicit input. The profile path passes `true` so the pruning
/// agrees with the `EnvVariable` arm of `has_explicitly_provided_value` in
/// the same generated block: the two value-source checks must not disagree
/// about what counts as explicit input, or an environment-supplied value
/// could be pruned out from under a profile. No generated configuration-field
/// argument carries a clap `env` binding today, so the arm is latent rather
/// than live. The declarative path passes `false`, keeping its long-standing
/// command-line-only pruning.
fn build_default_as_absent_pruning_tokens(
    cli_default_as_absent_fields: &[syn::LitStr],
    retain_environment_values: bool,
) -> proc_macro2::TokenStream {
    let explicit_source = if retain_environment_values {
        quote! {
            Some(
                clap::parser::ValueSource::CommandLine
                    | clap::parser::ValueSource::EnvVariable
            )
        }
    } else {
        quote! { Some(clap::parser::ValueSource::CommandLine) }
    };
    quote! {
        let has_explicit_default_as_absent_value = false
            #( || matches!(matches.value_source(#cli_default_as_absent_fields), #explicit_source) )*;
        if let Some(values) = value.as_object_mut() {
            #(
                if !matches!(
                    matches.value_source(#cli_default_as_absent_fields),
                    #explicit_source
                ) {
                    values.remove(#cli_default_as_absent_fields);
                }
            )*
        }
    }
}

/// Generate the guard that decides whether the CLI layer may be pushed.
///
/// The CLI layer must be pushed whenever the user supplied *any* argument,
/// regardless of whether the resulting sanitised object happens to equal the
/// defaults object. Comparing whole objects instead silently discards the CLI
/// layer for a single-field configuration whose explicit value matches the
/// struct default, letting a lower-precedence file or environment value win
/// over an explicit `--flag=false`. Ask clap's per-argument `value_source`
/// instead, which reports `CommandLine` independently of the parsed value.
///
/// `has_explicit_default_as_absent_value` is retained in the condition even
/// though a supplied `cli_default_as_absent` field already implies
/// `CommandLine`: the pruning helper emits that binding in this same block, and
/// naming it here is what keeps it read in generated user code compiled under
/// `-D warnings`.
fn build_cli_push_tokens() -> proc_macro2::TokenStream {
    quote! {
        let has_command_line_value = matches.ids().any(|id| {
            matches.value_source(id.as_ref()) == Some(clap::parser::ValueSource::CommandLine)
        });
        if has_command_line_value || has_explicit_default_as_absent_value {
            composer.push_cli(value);
        }
    }
}

/// Remove bool fields whose value came from clap's `SetTrue` default.
///
/// A `bool` field is emitted with `ArgAction::SetTrue`, so clap fills an absent
/// flag with a real `false` rather than leaving it unset. `sanitize_value`
/// strips only `null`, so that `false` survives into the CLI layer, and the CLI
/// layer outranks the profile layer — an absent `--enabled` would therefore
/// silently reset a profile's `enabled = true`.
///
/// Unlike `cli_default_as_absent`, this must not depend on the author having
/// opted in: `SetTrue` is what the macro emits for *every* bool field, so the
/// default-origin value is spurious whether or not the attribute was written.
/// The environment layer still supplies `<PREFIX>ENABLED=true` to the
/// composer; only an untouched flag is pruned.
fn build_bool_default_pruning_tokens(
    cli_bool_fields: &[(String, String)],
) -> proc_macro2::TokenStream {
    if cli_bool_fields.is_empty() {
        return proc_macro2::TokenStream::new();
    }
    let bool_pairs = cli_bool_fields.iter().map(|(arg_id, serialized_key)| {
        let id_lit = syn::LitStr::new(arg_id, proc_macro2::Span::call_site());
        let key_lit = syn::LitStr::new(serialized_key, proc_macro2::Span::call_site());
        quote! {
            if matches.value_source(#id_lit)
                == Some(clap::parser::ValueSource::DefaultValue)
            {
                values.remove(#key_lit);
            }
        }
    });
    quote! {
        if let Some(values) = value.as_object_mut() {
            #( #bool_pairs )*
        }
    }
}

/// Generate the CLI layer for structs that opt into profile support.
///
/// The push is gated on clap value sources rather than on the declarative
/// default-as-absent list alone: `--profile` sits below explicit CLI input, so
/// any config field clap reports as command-line *or* environment-filled must
/// still reach the composer. The environment origin matters because the
/// selector may itself be supplied through `<PREFIX>PROFILE`, and in that case
/// the remaining environment-derived fields must keep outranking the profile.
///
/// Fields filled from an inferred clap default are pruned first, so a built-in
/// default cannot mask a profile value. `cli_bool_fields` extends that pruning
/// to `SetTrue` bools, which carry a default-origin `false` that
/// `cli_default_as_absent` cannot describe.
pub(super) fn build_profile_cli_layer_tokens(
    krate: &proc_macro2::TokenStream,
    cli_arg_ids: &[String],
    cli_bool_fields: &[(String, String)],
    cli_default_as_absent_fields: &[syn::LitStr],
) -> proc_macro2::TokenStream {
    let arg_id_lits: Vec<syn::LitStr> = cli_arg_ids
        .iter()
        .map(|id| syn::LitStr::new(id, proc_macro2::Span::call_site()))
        .collect::<Vec<_>>();
    let prune_defaults = build_default_as_absent_pruning_tokens(cli_default_as_absent_fields, true);
    let prune_bool_defaults = build_bool_default_pruning_tokens(cli_bool_fields);
    // `matches` is already `&ArgMatches` here, courtesy of the `if let` above.
    let has_explicitly_provided = quote! {
        let has_explicitly_provided_value = {
            let provided = [ #( #arg_id_lits ),* ];
            provided.iter().any(|id| {
                matches!(
                    matches.value_source(id),
                    Some(
                        clap::parser::ValueSource::CommandLine
                            | clap::parser::ValueSource::EnvVariable
                    )
                )
            })
        };
    };
    quote! {
        if let (Some(cli), Some(matches)) = (cli.as_ref(), matches.as_ref()) {
            match #krate::sanitize_value(cli) {
                Ok(mut value) => {
                    #prune_defaults
                    #prune_bool_defaults
                    #has_explicitly_provided
                    let differs_from_defaults = defaults_value
                        .as_ref()
                        .map_or(true, |defaults| defaults != &value);
                    let has_user_input = has_explicitly_provided_value
                        || has_explicit_default_as_absent_value;
                    if differs_from_defaults || has_user_input {
                        composer.push_cli(value);
                    }
                }
                Err(err) => errors.push(err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the CLI-layer token builders.
    //!
    //! These drive the two builders rather than the shared pruning helper, so a
    //! call site that passed the wrong `retain_environment_values` value fails
    //! here. Asserting on the helper alone cannot see its call sites, and the
    //! two paths must not disagree: the profile path counts an
    //! environment-sourced value as explicit input, while the declarative path
    //! keeps its long-standing command-line-only pruning.
    //!
    //! The environment source is rendered three times by the profile path: the
    //! pruning names it twice (the explicit-value check and the per-field
    //! removal condition) and `has_explicitly_provided_value` names it once.
    //! Counting occurrences therefore separates "the profile path is env-aware
    //! in both checks" from "the flag never reached this call site" — a bare
    //! `contains` cannot, because `has_explicitly_provided_value` renders the
    //! same spelling whether or not the flag arrived.

    use super::*;

    /// The `ValueSource` spelling for an environment-filled value.
    const ENV: &str = "EnvVariable";
    /// The `ValueSource` spelling for an explicitly typed value.
    const CLI: &str = "CommandLine";

    /// A single `cli_default_as_absent` field for the builders to prune.
    fn pruned_field() -> Vec<syn::LitStr> {
        vec![syn::LitStr::new("retries", proc_macro2::Span::call_site())]
    }

    /// Counts how often a builder rendered a value source.
    fn occurrences(rendered: &str, needle: &str) -> usize {
        rendered.matches(needle).count()
    }

    /// The profile path must treat an environment-sourced value as explicit
    /// input, so the pruning leaves it in the CLI layer.
    ///
    /// The profile layer is inserted beneath the environment, so an
    /// environment-filled value has to survive this pruning to reach the
    /// composer at its documented rank. Pruning it here would let the profile
    /// value outrank the environment and invert the tier order. No generated
    /// configuration-field argument binds an environment variable today, so
    /// the case is latent; the wiring is still pinned so the two generated
    /// checks cannot drift apart.
    ///
    /// Three occurrences is the env-aware count: the pruning's explicit-value
    /// check, the pruning's per-field removal condition, and
    /// `has_explicitly_provided_value`. Passing `false` at this call site
    /// leaves only the last, which is what makes this assertion detect a
    /// swapped flag rather than merely restate the builder's input.
    #[test]
    fn profile_path_retains_environment_sourced_values() {
        let rendered = build_profile_cli_layer_tokens(
            &quote! { ::ortho_config },
            &["retries".to_owned()],
            &[],
            &pruned_field(),
        )
        .to_string();
        assert_eq!(
            occurrences(&rendered, ENV),
            3,
            "the profile path must count an environment-filled value as explicit in both pruning \
             checks and in `has_explicitly_provided_value`: {rendered}"
        );
        assert!(
            occurrences(&rendered, CLI) >= 1,
            "the profile path must still count an explicitly typed value: {rendered}"
        );
    }

    /// The declarative path keeps pruning only command-line values.
    ///
    /// Its environment value arrives through the environment layer, which is
    /// pushed above the CLI layer, so retaining it here would add nothing and
    /// would change the generated tokens for every existing user.
    #[test]
    fn declarative_path_prunes_only_command_line_values() {
        let rendered =
            build_cli_layer_tokens(&quote! { ::ortho_config }, &pruned_field()).to_string();
        assert_eq!(
            occurrences(&rendered, ENV),
            0,
            "the declarative path must not treat an environment value as explicit: {rendered}"
        );
        assert!(
            occurrences(&rendered, CLI) >= 1,
            "the declarative path must still count an explicit value: {rendered}"
        );
    }
}
