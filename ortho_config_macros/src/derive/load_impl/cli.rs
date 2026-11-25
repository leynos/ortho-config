//! Generated CLI parsing and layering fragments for derived configuration loads.
//!
//! The CLI layer is built sparsely: only fields that clap attributes to an
//! explicit value source are copied out of the parsed struct. Presence is
//! therefore independent of the parsed value, so an explicit argument that
//! restates a configured default still forms a layer, while a parser-filled
//! default or an omitted argument does not.

use crate::CliFieldInfo;
use proc_macro2::TokenStream;
use quote::quote;
use syn::LitStr;

/// Which parse results the generated compose method reads.
///
/// The parsed struct feeds the generated `config_path` selector and the CLI
/// layer; `ArgMatches` feeds only the CLI layer. Binding a value nothing reads
/// would need an artificial reference to stay warning-free, so the binding set
/// is derived from the fields that actually consume it.
#[derive(Clone, Copy)]
pub(super) struct ParseBindings {
    /// Whether the parsed struct is retained.
    retain_cli: bool,
    /// Whether `ArgMatches` is retained.
    retain_matches: bool,
}

impl ParseBindings {
    /// Derive the binding set from config-path presence and CLI fields.
    ///
    /// A config-path selector reads `cli.config_path` even when the struct has
    /// no configuration fields, while `ArgMatches` is needed only to build the
    /// sparse CLI layer.
    pub(super) const fn new(has_config_path: bool, has_cli_fields: bool) -> Self {
        Self {
            retain_cli: has_config_path || has_cli_fields,
            retain_matches: has_cli_fields,
        }
    }
}

/// Generate CLI parsing that retains the parse results the method reads.
pub(super) fn build_cli_parse_tokens(krate: &TokenStream, bindings: ParseBindings) -> TokenStream {
    let prefix = build_parse_prefix(krate);
    let parse = if bindings.retain_cli {
        build_value_parse(bindings.retain_matches)
    } else {
        build_validation_only_parse()
    };
    quote! { #prefix #parse }
}

/// Emit the argument collection and program label shared by every parse shape.
fn build_parse_prefix(krate: &TokenStream) -> TokenStream {
    quote! {
        let args: Vec<std::ffi::OsString> = iter.into_iter().map(Into::into).collect();
        let program_label = #krate::program_label_from_args(&args);
    }
}

/// Emit the error push shared by every parse shape.
///
/// Each call site is inside an `Err(error) =>` arm, where `error` is bound.
fn push_parse_error() -> TokenStream {
    quote! {
        errors.push(std::sync::Arc::new(error.into()));
    }
}

/// Parse for validation only; no values are bound.
fn build_validation_only_parse() -> TokenStream {
    let push_error = push_parse_error();
    quote! {
        if let Err(error) = Self::command()
            .try_get_matches_from_mut(args)
            .and_then(|matches| Self::from_arg_matches(&matches))
        {
            #push_error
        }
    }
}

/// Parse and bind the parsed struct, retaining `ArgMatches` when requested.
///
/// The tuple is always built because `ArgMatches` is needed to convert the
/// parsed struct; the CLI-only shape discards it with `_matches` rather than
/// omitting the conversion or adding an artificial read.
fn build_value_parse(retain_matches: bool) -> TokenStream {
    let push_error = push_parse_error();
    let parsed = quote! {
        let parsed = Self::command()
            .try_get_matches_from_mut(args)
            .and_then(|matches| {
                Self::from_arg_matches(&matches).map(|cli| (cli, matches))
            });
    };
    if retain_matches {
        quote! {
            #parsed
            let (cli, matches) = match parsed {
                Ok((cli, matches)) => (Some(cli), Some(matches)),
                Err(error) => {
                    #push_error
                    (None, None)
                }
            };
        }
    } else {
        quote! {
            #parsed
            let cli = match parsed {
                Ok((cli, _matches)) => Some(cli),
                Err(error) => {
                    #push_error
                    None
                }
            };
        }
    }
}

/// The clap value sources that count as explicit input for a compose path.
#[derive(Clone, Copy)]
pub(super) enum ExplicitSource {
    /// Only values typed on the command line.
    CommandLine,
    /// Command-line values and values filled from a clap `env` binding.
    CommandLineOrEnvironment,
}

impl ExplicitSource {
    /// Render the `matches!` pattern that recognises this policy.
    fn tokens(self) -> TokenStream {
        match self {
            Self::CommandLine => quote! { Some(clap::parser::ValueSource::CommandLine) },
            Self::CommandLineOrEnvironment => quote! {
                Some(
                    clap::parser::ValueSource::CommandLine
                        | clap::parser::ValueSource::EnvVariable
                )
            },
        }
    }
}

/// Emit a CLI layer containing only fields clap attributes to `explicit_source`.
///
/// The layer is pushed only when at least one field has an explicit source, so
/// a load that supplies no configuration argument adds no CLI layer. A field
/// whose source is a clap default is absent from the layer entirely, which is
/// what makes the `cli_default_as_absent` pruning and the boolean-default
/// pruning unnecessary: the sparse extraction applies the same rule to every
/// field without a per-attribute list.
///
/// Each compose path chooses the sources it admits. The declarative path takes
/// only `CommandLine`; the profile-enabled path also takes `EnvVariable`,
/// because a profile sits below an environment-filled argument and no generated
/// configuration-field argument binds an environment variable today.
pub(super) fn build_cli_layer_tokens(
    krate: &TokenStream,
    cli_field_info: &[CliFieldInfo],
    explicit_source: ExplicitSource,
) -> TokenStream {
    if cli_field_info.is_empty() {
        // No configuration field can enter a CLI layer. The declarative parse
        // is validation-only in this case, so there are no bindings to read.
        return TokenStream::new();
    }

    let source_pattern = explicit_source.tokens();
    let arg_ids: Vec<LitStr> = cli_field_info
        .iter()
        .map(|field| LitStr::new(&field.arg_id, proc_macro2::Span::call_site()))
        .collect::<Vec<_>>();
    let sparse_insertions: Vec<TokenStream> = cli_field_info
        .iter()
        .map(|field| sparse_insertion(field, &source_pattern))
        .collect();

    quote! {
        if let (Some(cli), Some(matches)) = (cli.as_ref(), matches.as_ref()) {
            let has_explicit_cli_value = false
                #( || matches!(matches.value_source(#arg_ids), #source_pattern) )*;
            if has_explicit_cli_value {
                match #krate::sanitize_value(cli) {
                    Ok(#krate::serde_json::Value::Object(mut values)) => {
                        let mut sparse = #krate::serde_json::Map::new();
                        #( #sparse_insertions )*
                        composer.push_cli(#krate::serde_json::Value::Object(sparse));
                    }
                    Ok(other) => errors.push(::std::sync::Arc::new(
                        #krate::OrthoError::Validation {
                            key: ::std::string::String::from("cli"),
                            message: ::std::format!(
                                "expected parsed CLI values to serialize to an object, got {other:?}",
                            ),
                        },
                    )),
                    Err(err) => errors.push(err),
                }
            }
        }
    }
}

/// Emit the guarded move of one field from the sanitized CLI object.
///
/// Three spellings meet here. The argument ID answers `value_source`; the Rust
/// member name is the key the synthetic CLI struct serializes; and the
/// deserialized key is the configuration member the merge feeds to serde. They
/// differ under `#[arg(id = "...")]`, `#[serde(rename = "...")]`, a
/// serialize-only rename, or a container-level rename rule, so reading or
/// writing with the wrong one drops an explicit value or places it where the
/// deserializer cannot see it.
fn sparse_insertion(field: &CliFieldInfo, explicit_source: &TokenStream) -> TokenStream {
    let arg_id = LitStr::new(&field.arg_id, proc_macro2::Span::call_site());
    let field_name = LitStr::new(&field.field_name, proc_macro2::Span::call_site());
    let deserialized_key = LitStr::new(&field.deserialized_key, proc_macro2::Span::call_site());
    quote! {
        if matches!(matches.value_source(#arg_id), #explicit_source) {
            if let Some(field_value) = values.remove(#field_name) {
                sparse.insert(#deserialized_key.to_owned(), field_value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the CLI-layer token builders.
    //!
    //! These pin the source policy each builder passes to the shared sparse
    //! extractor. The merged result is covered end to end by
    //! `ortho_config/tests/cli_explicit_equals_default.rs`; these tests make
    //! the generated contract visible, namely which clap value sources may
    //! enter the declarative and profile-enabled layers, and that membership is
    //! decided per field rather than by comparing whole objects.

    use super::*;

    /// The `ValueSource` spelling for an environment-filled value.
    const ENV: &str = "EnvVariable";
    /// The `ValueSource` spelling for an explicitly typed value.
    const CLI: &str = "CommandLine";

    /// One field with distinct member, clap, serialize, and deserialize spellings.
    fn field(
        field_name: &str,
        arg_id: &str,
        serialized_key: &str,
        deserialized_key: &str,
    ) -> CliFieldInfo {
        CliFieldInfo {
            field_name: field_name.to_owned(),
            serialized_key: serialized_key.to_owned(),
            deserialized_key: deserialized_key.to_owned(),
            arg_id: arg_id.to_owned(),
            is_default_as_absent: false,
        }
    }

    /// Counts how often a builder rendered a spelling.
    fn occurrences(rendered: &str, needle: &str) -> usize {
        rendered.matches(needle).count()
    }

    /// Each compose path admits exactly the sources it documents.
    ///
    /// The declarative path takes only command-line values: its environment
    /// value arrives through the environment layer, which is pushed above the
    /// CLI layer. The profile path also takes `EnvVariable`, because an
    /// environment-filled argument must stay above the profile layer. No
    /// generated configuration-field argument binds an environment variable
    /// today, so that arm is latent; it is pinned so the paths cannot drift.
    #[test]
    fn layer_source_policy_matches_each_compose_path() {
        for (source, allows_environment, context) in [
            (ExplicitSource::CommandLine, false, "declarative"),
            (
                ExplicitSource::CommandLineOrEnvironment,
                true,
                "profile-enabled",
            ),
        ] {
            let rendered =
                build_cli_layer_tokens(&quote! { ::ortho_config }, &pruned_field(), source)
                    .to_string();
            assert_eq!(
                occurrences(&rendered, ENV) > 0,
                allows_environment,
                "the {context} path must treat an environment value as explicit only when \
                 documented: {rendered}"
            );
            assert!(
                occurrences(&rendered, CLI) >= 1,
                "the {context} path must still count an explicitly typed value: {rendered}"
            );
        }
    }

    /// Each field is read by its CLI member and stored under its config key.
    #[test]
    fn sparse_layer_maps_member_names_to_serialized_keys() {
        let fields = [
            field("port", "listen_port", "wire_port", "service_port"),
            field("nickname", "nickname", "nickname", "display_name"),
        ];
        let rendered = build_cli_layer_tokens(
            &quote! { ::ortho_config },
            &fields,
            ExplicitSource::CommandLine,
        )
        .to_string();
        for needle in [
            "\"listen_port\"",
            "\"port\"",
            "\"service_port\"",
            "\"nickname\"",
            "\"display_name\"",
        ] {
            assert!(
                rendered.contains(needle),
                "the sparse layer must name {needle}: {rendered}"
            );
        }
        assert!(
            !rendered.contains("\"wire_port\""),
            "the sparse layer must use the deserialization key, not the serialized one: \
             {rendered}"
        );
    }

    /// The layer no longer compares the whole CLI object with the defaults.
    #[test]
    fn sparse_layer_has_no_whole_object_comparison() {
        let rendered = build_cli_layer_tokens(
            &quote! { ::ortho_config },
            &pruned_field(),
            ExplicitSource::CommandLine,
        )
        .to_string();
        assert!(
            !rendered.contains("defaults_value") && !rendered.contains("differs_from_defaults"),
            "the layer must decide membership per field, not by object equality: {rendered}"
        );
        assert!(
            rendered.contains("has_explicit_cli_value") && rendered.contains("value_source"),
            "the layer must gate on clap value sources: {rendered}"
        );
        assert!(
            rendered.contains("sanitize_value") && rendered.contains("push_cli"),
            "the layer must still sanitise and push the sparse object: {rendered}"
        );
    }

    /// A struct with no CLI field retains only the config-path selector.
    ///
    /// No layer is emitted because no configuration value can enter one, but
    /// the discovery chain reads `cli.config_path`, so the parsed struct must
    /// remain bound. `ArgMatches` has no consumer and must not be bound.
    #[test]
    fn empty_field_list_retains_only_the_config_path_selector() {
        let layer =
            build_cli_layer_tokens(&quote! { ::ortho_config }, &[], ExplicitSource::CommandLine)
                .to_string();
        assert!(
            layer.is_empty(),
            "the empty field list must emit no CLI layer: {layer}"
        );

        let parse =
            build_cli_parse_tokens(&quote! { ::ortho_config }, ParseBindings::new(true, false))
                .to_string();
        assert!(
            parse.contains("let cli") && !parse.contains("let (cli"),
            "the config-path selector must retain only the parsed struct: {parse}"
        );
        assert!(
            !parse.contains("matches . as_ref") && !parse.contains("_parsed"),
            "nothing may read ArgMatches when no CLI field exists: {parse}"
        );

        let validation =
            build_cli_parse_tokens(&quote! { ::ortho_config }, ParseBindings::new(false, false))
                .to_string();
        assert!(
            validation.contains("try_get_matches_from_mut") && validation.contains("errors . push"),
            "the validation-only parse must still report parse errors: {validation}"
        );
        assert!(
            !validation.contains("let cli") && !validation.contains("_parsed"),
            "the validation-only parse must not bind values nothing reads: {validation}"
        );
    }

    /// A single field for the builders to extract.
    fn pruned_field() -> Vec<CliFieldInfo> {
        vec![field("retries", "retries", "retries", "retries")]
    }
}
