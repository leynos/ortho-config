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
    quote! {
        if let (Some(cli), Some(matches)) = (cli.as_ref(), matches.as_ref()) {
            match #krate::sanitize_value(cli) {
                Ok(mut value) => {
                    let has_explicit_default_as_absent_value = false
                        #( || matches.value_source(#cli_default_as_absent_fields)
                            == Some(clap::parser::ValueSource::CommandLine) )*;
                    if let Some(values) = value.as_object_mut() {
                        #(
                            if matches.value_source(#cli_default_as_absent_fields)
                                != Some(clap::parser::ValueSource::CommandLine)
                            {
                                values.remove(#cli_default_as_absent_fields);
                            }
                        )*
                    }
                    let differs_from_defaults = defaults_value
                        .as_ref()
                        .map_or(true, |defaults| defaults != &value);
                    if differs_from_defaults || has_explicit_default_as_absent_value {
                        composer.push_cli(value);
                    }
                }
                Err(err) => errors.push(err),
            }
        }
    }
}
