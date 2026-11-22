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
    if !bindings.retain_cli {
        return quote! {
            let args: Vec<std::ffi::OsString> = iter.into_iter().map(Into::into).collect();
            let program_label = #krate::program_label_from_args(&args);
            if let Err(error) = Self::command()
                .try_get_matches_from_mut(args)
                .and_then(|matches| Self::from_arg_matches(&matches))
            {
                errors.push(std::sync::Arc::new(error.into()));
            }
        };
    }

    if !bindings.retain_matches {
        return quote! {
            let args: Vec<std::ffi::OsString> = iter.into_iter().map(Into::into).collect();
            let program_label = #krate::program_label_from_args(&args);
            let parsed = Self::command()
                .try_get_matches_from_mut(args)
                .and_then(|matches| Self::from_arg_matches(&matches));
            let cli = match parsed {
                Ok(cli) => Some(cli),
                Err(error) => {
                    errors.push(std::sync::Arc::new(error.into()));
                    None
                }
            };
        };
    }

    quote! {
        let args: Vec<std::ffi::OsString> = iter.into_iter().map(Into::into).collect();
        let program_label = #krate::program_label_from_args(&args);
        let parsed = Self::command()
            .try_get_matches_from_mut(args)
            .and_then(|matches| {
                Self::from_arg_matches(&matches).map(|cli| (cli, matches))
            });
        let (cli, matches) = match parsed {
            Ok((cli, matches)) => (Some(cli), Some(matches)),
            Err(error) => {
                errors.push(std::sync::Arc::new(error.into()));
                (None, None)
            }
        };
    }
}

/// The clap value sources that count as explicit input for a compose path.
#[derive(Clone, Copy)]
enum ExplicitSource {
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

/// Generate the declarative CLI layer.
///
/// Only fields whose clap value source is the command line enter the layer. An
/// explicit value equal to the configured default is therefore retained, while
/// a parser default or an absent argument is omitted. Comparing the whole
/// serialized CLI object with the defaults object cannot make that distinction:
/// it either drops an explicit default-valued argument or admits parser-filled
/// defaults that the user never typed.
pub(super) fn build_cli_layer_tokens(
    krate: &TokenStream,
    cli_field_info: &[CliFieldInfo],
) -> TokenStream {
    build_sparse_cli_layer_tokens(krate, cli_field_info, ExplicitSource::CommandLine)
}

/// Generate the profile-enabled CLI layer.
///
/// `--profile` sits below explicit CLI input, and an argument filled from a
/// clap `env` binding must keep outranking the profile layer, so this path also
/// accepts `ValueSource::EnvVariable`. No generated configuration-field
/// argument binds an environment variable today; the arm preserves the
/// existing environment-aware contract rather than silently narrowing it.
pub(super) fn build_profile_cli_layer_tokens(
    krate: &TokenStream,
    cli_field_info: &[CliFieldInfo],
) -> TokenStream {
    build_sparse_cli_layer_tokens(
        krate,
        cli_field_info,
        ExplicitSource::CommandLineOrEnvironment,
    )
}

/// Emit a CLI layer containing only fields clap attributes to `explicit_source`.
///
/// The layer is pushed only when at least one field has an explicit source, so
/// a load that supplies no configuration argument adds no CLI layer. A field
/// whose source is a clap default is absent from the layer entirely, which is
/// what makes the `cli_default_as_absent` pruning and the boolean-default
/// pruning unnecessary: the sparse extraction applies the same rule to every
/// field without a per-attribute list.
fn build_sparse_cli_layer_tokens(
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
/// The argument ID answers `value_source`, while the serialized key names the
/// JSON member. They differ when the field carries `#[serde(rename)]` or a
/// container-level rename rule, so using one for both would either test the
/// wrong argument or remove the wrong member.
fn sparse_insertion(field: &CliFieldInfo, explicit_source: &TokenStream) -> TokenStream {
    let arg_id = LitStr::new(&field.arg_id, proc_macro2::Span::call_site());
    let serialized_key = LitStr::new(&field.serialized_key, proc_macro2::Span::call_site());
    quote! {
        if matches!(matches.value_source(#arg_id), #explicit_source) {
            if let Some(field_value) = values.remove(#serialized_key) {
                sparse.insert(#serialized_key.to_owned(), field_value);
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

    /// One field with distinct clap and serde spellings.
    fn field(arg_id: &str, serialized_key: &str) -> CliFieldInfo {
        CliFieldInfo {
            serialized_key: serialized_key.to_owned(),
            arg_id: arg_id.to_owned(),
            is_default_as_absent: false,
        }
    }

    /// Counts how often a builder rendered a spelling.
    fn occurrences(rendered: &str, needle: &str) -> usize {
        rendered.matches(needle).count()
    }

    /// The declarative path keeps pruning to command-line values.
    ///
    /// Its environment value arrives through the environment layer, which is
    /// pushed above the CLI layer, so accepting `EnvVariable` here would add
    /// nothing and would change the generated tokens for every existing user.
    #[test]
    fn declarative_layer_accepts_only_command_line_values() {
        let rendered =
            build_cli_layer_tokens(&quote! { ::ortho_config }, &pruned_field()).to_string();
        assert_eq!(
            occurrences(&rendered, ENV),
            0,
            "the declarative path must not treat an environment value as explicit: {rendered}"
        );
        assert!(
            occurrences(&rendered, CLI) >= 1,
            "the declarative path must still count an explicitly typed value: {rendered}"
        );
    }

    /// The profile path must also retain an environment-filled value.
    ///
    /// The profile layer is inserted beneath the environment, so an argument
    /// clap fills from an `env` binding has to survive this extraction to reach
    /// the composer at its documented rank. No generated configuration-field
    /// argument binds an environment variable today, so the case is latent; the
    /// wiring is still pinned so the two paths cannot drift apart.
    #[test]
    fn profile_layer_accepts_command_line_and_environment_values() {
        let rendered =
            build_profile_cli_layer_tokens(&quote! { ::ortho_config }, &pruned_field()).to_string();
        assert!(
            occurrences(&rendered, ENV) >= 1,
            "the profile path must count an environment-filled value as explicit: {rendered}"
        );
        assert!(
            occurrences(&rendered, CLI) >= 1,
            "the profile path must still count an explicitly typed value: {rendered}"
        );
    }

    /// Each field is moved by its serialized key and tested by its argument ID.
    #[test]
    fn sparse_layer_moves_each_field_by_its_serialized_key() {
        let fields = [
            field("port", "listen_port"),
            field("nickname", "display_name"),
        ];
        let rendered = build_cli_layer_tokens(&quote! { ::ortho_config }, &fields).to_string();
        for needle in [
            "\"port\"",
            "\"listen_port\"",
            "\"nickname\"",
            "\"display_name\"",
        ] {
            assert!(
                rendered.contains(needle),
                "the sparse layer must name {needle}: {rendered}"
            );
        }
    }

    /// The layer no longer compares the whole CLI object with the defaults.
    #[test]
    fn sparse_layer_has_no_whole_object_comparison() {
        let rendered =
            build_cli_layer_tokens(&quote! { ::ortho_config }, &pruned_field()).to_string();
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
        let layer = build_cli_layer_tokens(&quote! { ::ortho_config }, &[]).to_string();
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
        vec![field("retries", "retries")]
    }
}
