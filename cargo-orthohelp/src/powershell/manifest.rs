//! `PowerShell` module manifest rendering.

const CRLF: &str = "\r\n";

/// Input data required to build a module manifest.
pub struct ManifestConfig<'a> {
    /// Module name for the manifest.
    pub module_name: &'a str,
    /// Module version string.
    pub module_version: &'a str,
    /// Functions exported by the module.
    pub functions_to_export: &'a [String],
    /// Aliases exported by the module.
    pub aliases_to_export: &'a [String],
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
            format_array(config.functions_to_export)
        ),
    );
    push_line(
        &mut output,
        &format!(
            "  AliasesToExport = {}",
            format_array(config.aliases_to_export)
        ),
    );
    push_line(
        &mut output,
        &format!(
            "  ExternalHelp = {}",
            quote_single(&format!("{}-help.xml", config.module_name))
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

fn format_array(values: &[String]) -> String {
    if values.is_empty() {
        return "@()".to_owned();
    }

    let joined = values
        .iter()
        .map(|value| quote_single(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("@({joined})")
}

fn push_line(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push_str(CRLF);
}

fn quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_includes_external_help_key() {
        let functions = vec!["fixture".to_owned()];
        let aliases = vec!["fixture-help".to_owned()];
        let manifest = render_manifest(&ManifestConfig {
            module_name: "FixtureHelp",
            module_version: "0.1.0",
            functions_to_export: &functions,
            aliases_to_export: &aliases,
            help_info_uri: None,
        });

        assert!(manifest.contains("RootModule = 'FixtureHelp.psm1'"));
        assert!(manifest.contains("FunctionsToExport = @('fixture')"));
        assert!(manifest.contains("ExternalHelp = 'FixtureHelp-help.xml'"));
    }

    #[test]
    fn manifest_includes_help_info_uri_when_provided() {
        let functions = vec!["fixture".to_owned()];
        let aliases = vec!["fixture-help".to_owned()];
        let manifest = render_manifest(&ManifestConfig {
            module_name: "FixtureHelp",
            module_version: "0.1.0",
            functions_to_export: &functions,
            aliases_to_export: &aliases,
            help_info_uri: Some("https://example.invalid/help"),
        });

        assert!(manifest.contains("HelpInfoUri = 'https://example.invalid/help'"));
    }
}
