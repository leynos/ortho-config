//! `PowerShell` module manifest rendering.

use crate::powershell::ExportAlias;
use crate::powershell::text::{push_line, quote_single};

/// Input data required to build a module manifest.
pub struct ManifestConfig<'a> {
    /// Module name for the manifest.
    pub module_name: &'a str,
    /// Module version string.
    pub module_version: &'a str,
    /// Functions exported by the module.
    pub functions_to_export: &'a [String],
    /// Aliases exported by the module.
    pub aliases_to_export: &'a [ExportAlias],
    /// Optional Update-Help URI.
    pub help_info_uri: Option<&'a str>,
}

/// Renders the module manifest (.psd1).
#[must_use]
pub fn render_manifest(config: &ManifestConfig<'_>) -> String {
    let mut output = String::new();

    push_line(&mut output, "@{");
    push_line(
        &mut output,
        &format!(
            "  RootModule = {}",
            quote_single(&format!("{}.psm1", config.module_name))
        ),
    );
    push_line(
        &mut output,
        &format!("  ModuleVersion = {}", quote_single(config.module_version)),
    );
    push_line(&mut output, "  CompatiblePSEditions = @('Desktop', 'Core')");
    push_line(
        &mut output,
        &format!(
            "  FunctionsToExport = {}",
            format_string_array(config.functions_to_export)
        ),
    );
    push_line(
        &mut output,
        &format!(
            "  AliasesToExport = {}",
            format_alias_array(config.aliases_to_export)
        ),
    );
    if let Some(uri) = config.help_info_uri {
        push_line(
            &mut output,
            &format!("  HelpInfoUri = {}", quote_single(uri)),
        );
    }
    push_line(&mut output, "}");

    output
}

fn format_string_array(values: &[String]) -> String {
    format_array(values.iter().map(String::as_str))
}

fn format_alias_array(values: &[ExportAlias]) -> String {
    format_array(values.iter().map(AsRef::as_ref))
}

fn format_array<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let quoted = values.map(quote_single).collect::<Vec<_>>();
    if quoted.is_empty() {
        return "@()".to_owned();
    }

    format!("@({})", quoted.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::without_help_info(None, false)]
    #[case::with_help_info(Some("https://example.invalid/help"), true)]
    fn manifest_renders_expected_keys(
        #[case] help_info_uri: Option<&str>,
        #[case] expects_help_info: bool,
    ) {
        let functions = vec!["fixture".to_owned()];
        let aliases = vec![ExportAlias::from("fixture-help")];
        let manifest = render_manifest(&ManifestConfig {
            module_name: "FixtureHelp",
            module_version: "0.1.0",
            functions_to_export: &functions,
            aliases_to_export: &aliases,
            help_info_uri,
        });

        assert!(manifest.contains("RootModule = 'FixtureHelp.psm1'"));
        assert!(manifest.contains("FunctionsToExport = @('fixture')"));
        assert!(!manifest.contains("ExternalHelp"));
        assert_eq!(
            manifest.contains("HelpInfoUri = 'https://example.invalid/help'"),
            expects_help_info
        );
    }
}
