//! Token emission for the optional `discovery` IR block.
//!
//! Kept in its own module so `sections.rs` stays under the repository's
//! 400-line ceiling. The block owns the whole config-discovery vocabulary:
//! the resolved file names, the formats inferred from their extensions, and
//! the override flag and environment variable the runtime will accept.

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::build::compute_config_env_var;
use crate::derive::parse::StructAttrs;

use super::option_string_tokens;
use super::types::{AppName, ConfigFileName};

/// Builds the token stream for the optional `discovery` IR block.
///
/// Returns `None` when the struct declares no `discovery(...)` attributes, so
/// a consumer sees "undeclared" rather than a defaulted chain. The three file
/// names resolve from the declaration with the same fallbacks the runtime
/// generator applies, so the published IR and the loader cannot disagree about
/// which files are searched.
pub(super) fn build_discovery_metadata(
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

/// Emits one `ConfigFormat` per distinct extension named by the file chain.
///
/// The declaration order is the IR's order rather than the order the names
/// appear in, so a struct that declares both a TOML config file and a JSON
/// project file publishes a stable `[Toml, Json]` list however it spelled the
/// attributes.
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

/// Derives the dotfile name from the app name and the config file extension.
///
/// An empty app name still yields a usable dotfile (`.config`), because the IR
/// must name a file the loader will actually search; a nameless application
/// would otherwise publish the bare `.` as its dotfile.
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
