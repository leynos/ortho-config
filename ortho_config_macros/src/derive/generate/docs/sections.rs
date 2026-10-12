//! Section-level documentation IR generation.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::derive::build::{compute_config_env_var, default_app_name};
use crate::derive::parse::{DocStructAttrs, HeadingOverrides, StructAttrs};

use super::types::{AppName, ConfigFileName};
use super::{example_tokens, link_tokens, note_tokens, option_string_tokens};

fn default_headings() -> HeadingOverrides {
    HeadingOverrides {
        name: Some(String::from("ortho.headings.name")),
        synopsis: Some(String::from("ortho.headings.synopsis")),
        description: Some(String::from("ortho.headings.description")),
        options: Some(String::from("ortho.headings.options")),
        environment: Some(String::from("ortho.headings.environment")),
        files: Some(String::from("ortho.headings.files")),
        precedence: Some(String::from("ortho.headings.precedence")),
        exit_status: Some(String::from("ortho.headings.exit_status")),
        examples: Some(String::from("ortho.headings.examples")),
        see_also: Some(String::from("ortho.headings.see_also")),
        commands: Some(String::from("ortho.headings.commands")),
    }
}

/// Resolves the application name the documentation IR records.
///
/// An explicit `app_name` wins; otherwise the name is derived from the
/// struct's own attributes and identifier, so a deriving type that declares
/// nothing still publishes a stable root.
pub(crate) fn resolve_app_name(struct_attrs: &StructAttrs, ident: &Ident) -> String {
    struct_attrs
        .discovery
        .as_ref()
        .and_then(|discovery| discovery.app_name.clone())
        .unwrap_or_else(|| default_app_name(struct_attrs, ident))
}

/// Builds the tokens for the command's `about` identifier.
///
/// An explicit `about_id` is emitted as-is; without one the expression asks
/// the runtime to compose the default from `command_path`, which keeps a
/// mounted tree's identifier agreed with the lookup that resolves it.
pub(super) fn about_id_tokens(doc: &DocStructAttrs, krate: &TokenStream) -> TokenStream {
    doc.about_id.as_deref().map_or_else(
        || quote! { #krate::message_id_for(command_path, "about") },
        string_tokens,
    )
}

/// Builds the tokens for the command's `synopsis` identifier.
///
/// Mirrors [`about_id_tokens`], including its fallback: the default is
/// composed from `command_path` rather than hard-coded here, so the emitted
/// constant and the runtime lookup cannot drift.
pub(super) fn synopsis_id_tokens(doc: &DocStructAttrs, krate: &TokenStream) -> TokenStream {
    doc.synopsis_id.as_deref().map_or_else(
        || quote! { Some(#krate::message_id_for(command_path, "usage")) },
        |synopsis_id| {
            let value = string_tokens(synopsis_id);
            quote! { Some(#value) }
        },
    )
}

#[expect(
    clippy::cognitive_complexity,
    reason = "`quote!` expansion inflates the complexity score; keep this wrapper minimal."
)]
pub(super) fn build_sections_metadata(
    app_name: &AppName,
    struct_attrs: &StructAttrs,
    krate: &TokenStream,
) -> syn::Result<TokenStream> {
    let headings = build_headings_ids(&struct_attrs.doc.headings, krate);
    let discovery = build_discovery_metadata(app_name, struct_attrs, krate);
    let precedence = build_precedence_metadata(&struct_attrs.doc, struct_attrs.profiles, krate)?;
    let examples = example_tokens(&struct_attrs.doc.examples, krate);
    let links = link_tokens(&struct_attrs.doc.links, krate);
    let notes = note_tokens(&struct_attrs.doc.notes, krate);

    Ok(quote! {
        #krate::docs::SectionsMetadata {
            headings_ids: #headings,
            discovery: #discovery,
            precedence: #precedence,
            examples: vec![ #( #examples ),* ],
            links: vec![ #( #links ),* ],
            notes: vec![ #( #notes ),* ],
        }
    })
}

pub(super) fn build_windows_metadata(
    struct_attrs: &StructAttrs,
    krate: &TokenStream,
) -> TokenStream {
    let Some(windows) = struct_attrs.doc.windows.as_ref() else {
        return quote! { None };
    };

    let module_name = option_string_tokens(windows.module_name.as_deref());
    let export_aliases = windows
        .export_aliases
        .iter()
        .map(|alias| {
            let lit = syn::LitStr::new(alias, proc_macro2::Span::call_site());
            quote! { String::from(#lit) }
        })
        .collect::<Vec<_>>();
    let include_common_parameters = windows.include_common_parameters.unwrap_or(true);
    let split_subcommands = windows.split_subcommands.unwrap_or(false);
    let help_info_uri = option_string_tokens(windows.help_info_uri.as_deref());

    quote! {
        Some(#krate::docs::WindowsMetadata {
            module_name: #module_name,
            export_aliases: vec![ #( #export_aliases ),* ],
            include_common_parameters: #include_common_parameters,
            split_subcommands_into_functions: #split_subcommands,
            help_info_uri: #help_info_uri,
        })
    }
}

fn build_headings_ids(overrides: &HeadingOverrides, krate: &TokenStream) -> TokenStream {
    let headings = merge_headings(overrides);

    // Helper closure to process each heading field: unwrap with default and tokenize
    let process_heading = |field: Option<String>, default_id: &str| -> TokenStream {
        let value = field.unwrap_or_else(|| String::from(default_id));
        string_tokens(&value)
    };

    let name_tokens = process_heading(headings.name, "ortho.headings.name");
    let synopsis_tokens = process_heading(headings.synopsis, "ortho.headings.synopsis");
    let description_tokens = process_heading(headings.description, "ortho.headings.description");
    let options_tokens = process_heading(headings.options, "ortho.headings.options");
    let environment_tokens = process_heading(headings.environment, "ortho.headings.environment");
    let files_tokens = process_heading(headings.files, "ortho.headings.files");
    let precedence_tokens = process_heading(headings.precedence, "ortho.headings.precedence");
    let exit_status_tokens = process_heading(headings.exit_status, "ortho.headings.exit_status");
    let examples_tokens = process_heading(headings.examples, "ortho.headings.examples");
    let see_also_tokens = process_heading(headings.see_also, "ortho.headings.see_also");
    let commands_tokens = option_string_tokens(headings.commands.as_deref());

    quote! {
        #krate::docs::HeadingIds {
            name: #name_tokens,
            synopsis: #synopsis_tokens,
            description: #description_tokens,
            options: #options_tokens,
            environment: #environment_tokens,
            files: #files_tokens,
            precedence: #precedence_tokens,
            exit_status: #exit_status_tokens,
            examples: #examples_tokens,
            see_also: #see_also_tokens,
            commands: #commands_tokens,
        }
    }
}

/// Emits one string literal as an owned `String` expression.
pub(super) fn string_tokens(value: &str) -> TokenStream {
    let lit = syn::LitStr::new(value, proc_macro2::Span::call_site());
    quote! { String::from(#lit) }
}

fn merge_headings(overrides: &HeadingOverrides) -> HeadingOverrides {
    let defaults = default_headings();
    HeadingOverrides {
        name: overrides.name.clone().or(defaults.name),
        synopsis: overrides.synopsis.clone().or(defaults.synopsis),
        description: overrides.description.clone().or(defaults.description),
        options: overrides.options.clone().or(defaults.options),
        environment: overrides.environment.clone().or(defaults.environment),
        files: overrides.files.clone().or(defaults.files),
        precedence: overrides.precedence.clone().or(defaults.precedence),
        exit_status: overrides.exit_status.clone().or(defaults.exit_status),
        examples: overrides.examples.clone().or(defaults.examples),
        see_also: overrides.see_also.clone().or(defaults.see_also),
        commands: overrides.commands.clone().or(defaults.commands),
    }
}

fn build_precedence_metadata(
    doc: &DocStructAttrs,
    profiles: bool,
    krate: &TokenStream,
) -> syn::Result<TokenStream> {
    let order_values = doc
        .precedence
        .as_ref()
        .map_or(&[][..], |meta| meta.order.as_slice());

    let order = if order_values.is_empty() {
        // The tiers are assembled by push rather than by an inline `vec!`:
        // the profile tier is optional, and an absent tier would otherwise
        // leave an empty token stream between two commas, which the `vec!`
        // macro rejects as a missing expression.
        let mut tiers = vec![
            quote! { #krate::docs::SourceKind::Defaults },
            quote! { #krate::docs::SourceKind::File },
        ];
        if profiles {
            tiers.push(quote! { #krate::docs::SourceKind::Profile });
        }
        tiers.push(quote! { #krate::docs::SourceKind::Env });
        tiers.push(quote! { #krate::docs::SourceKind::Cli });
        tiers
    } else {
        order_values
            .iter()
            .map(|value| source_kind_tokens(value, profiles, krate))
            .collect::<Result<Vec<_>, _>>()?
    };

    let rationale_id = doc
        .precedence
        .as_ref()
        .and_then(|meta| meta.rationale_id.as_deref());
    let rationale_tokens = option_string_tokens(rationale_id);

    Ok(quote! {
        Some(#krate::docs::PrecedenceMeta {
            order: vec![ #( #order ),* ],
            rationale_id: #rationale_tokens,
        })
    })
}

fn is_source_kind(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "default" | "defaults" => Some("Defaults"),
        "file" | "files" => Some("File"),
        "profile" | "profiles" => Some("Profile"),
        "env" | "environment" => Some("Env"),
        "cli" | "commandline" | "command-line" => Some("Cli"),
        _ => None,
    }
}

/// Resolve one explicit `precedence(order = [...])` spelling to a source kind.
///
/// The `profile` tier is meaningful only for a struct that opted in via
/// `#[ortho_config(profiles)]`. Every other spelling is accepted for every
/// struct, so the automatic order — which already omits the tier when
/// `profiles` is false — and the explicit order agree on which tiers the
/// loader actually builds. Accepting it here would let a non-opted-in struct
/// publish a precedence layer that the generated loader never pushes, and the
/// resulting man pages, `PowerShell` help, and agent context would describe a
/// tier that does not exist.
///
/// The rejection is a compile-time error rather than a silent drop: dropping
/// the tier would leave the declared order quietly wrong, and the author
/// cannot fix what is never reported.
fn source_kind_tokens(
    value: &str,
    profiles: bool,
    krate: &TokenStream,
) -> syn::Result<TokenStream> {
    let variant: syn::Ident = is_source_kind(value)
        .filter(|kind| profiles || *kind != "Profile")
        .map(|v| syn::Ident::new(v, proc_macro2::Span::call_site()))
        .ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                source_kind_error(value, profiles),
            )
        })?;
    Ok(quote! { #krate::docs::SourceKind::#variant })
}

/// Build the diagnostic for an unusable explicit precedence spelling.
///
/// A non-opted-in struct gets its own message for the profile tier: the
/// spelling is real, so reporting it as unknown would send the author hunting
/// for a misspelling when the actual remedy is the `profiles` attribute.
fn source_kind_error(value: &str, profiles: bool) -> String {
    if !profiles && is_source_kind(value) == Some("Profile") {
        return format!(
            "precedence source '{value}' requires #[ortho_config(profiles)]; \
             drop the tier or opt the struct into profile support",
        );
    }
    format!("unknown precedence source '{value}'; expected defaults, file, profile, env, or cli")
}

fn build_discovery_metadata(
    app_name: &AppName,
    struct_attrs: &StructAttrs,
    krate: &TokenStream,
) -> TokenStream {
    let Some(discovery) = struct_attrs.discovery.as_ref() else {
        return quote! { None };
    };

    let config_file_name = discovery
        .config_file_name
        .clone()
        .unwrap_or_else(|| String::from("config.toml"));
    let dotfile_name = discovery.dotfile_name.clone().unwrap_or_else(|| {
        default_dotfile_name(app_name, &ConfigFileName::new(config_file_name.clone()))
    });
    let project_file_name = discovery
        .project_file_name
        .clone()
        .unwrap_or_else(|| dotfile_name.clone());

    let formats = collect_formats(
        &[&config_file_name, &dotfile_name, &project_file_name],
        krate,
    );

    let override_flag_long = if discovery.config_cli_visible.unwrap_or(false) {
        let value = discovery
            .config_cli_long
            .clone()
            .unwrap_or_else(|| String::from("config-path"));
        option_string_tokens(Some(value.as_str()))
    } else {
        quote! { None }
    };

    let override_env = discovery
        .env_var
        .clone()
        .unwrap_or_else(|| compute_config_env_var(struct_attrs));
    let override_env_tokens = option_string_tokens(Some(override_env.as_str()));

    quote! {
        Some(#krate::docs::ConfigDiscoveryMeta {
            formats: vec![ #( #formats ),* ],
            search_paths: Vec::new(),
            override_flag_long: #override_flag_long,
            override_env: #override_env_tokens,
            xdg_compliant: ::core::cfg!(any(unix, target_os = "redox")),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FormatKind {
    Toml,
    Yaml,
    Json,
}

fn format_kind_from_extension(name: &str) -> Option<FormatKind> {
    match extension_from_name(name).as_deref() {
        Some("toml") => Some(FormatKind::Toml),
        Some("yaml" | "yml") => Some(FormatKind::Yaml),
        Some("json" | "json5") => Some(FormatKind::Json),
        _ => None,
    }
}

fn collect_formats(names: &[&str], krate: &TokenStream) -> Vec<TokenStream> {
    let mut has_toml = false;
    let mut has_yaml = false;
    let mut has_json = false;

    for name in names {
        if let Some(kind) = format_kind_from_extension(name) {
            has_toml |= kind == FormatKind::Toml;
            has_yaml |= kind == FormatKind::Yaml;
            has_json |= kind == FormatKind::Json;
        }
    }

    let mut formats = Vec::new();
    if has_toml {
        formats.push(quote! { #krate::docs::ConfigFormat::Toml });
    }
    if has_yaml {
        formats.push(quote! { #krate::docs::ConfigFormat::Yaml });
    }
    if has_json {
        formats.push(quote! { #krate::docs::ConfigFormat::Json });
    }
    formats
}

fn extension_from_name(name: &str) -> Option<String> {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.trim())
        .filter(|ext| !ext.is_empty())
        .map(str::to_ascii_lowercase)
}

fn default_dotfile_name(app_name: &AppName, config_file_name: &ConfigFileName) -> String {
    let extension = config_file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty());

    if app_name.trim().is_empty() {
        let mut name = String::from('.');
        name.push_str(extension.unwrap_or("config"));
        return name;
    }

    let mut name = String::from('.');
    name.push_str(app_name.trim());
    if let Some(ext) = extension {
        name.push('.');
        name.push_str(ext);
    }
    name
}
