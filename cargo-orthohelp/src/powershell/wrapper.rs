//! `PowerShell` wrapper module rendering.

use crate::ir::LocalizedDocMetadata;

const CRLF: &str = "\r\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BinName(String);

impl BinName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for BinName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionName(String);

impl FunctionName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for FunctionName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Alias(String);

impl Alias {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Alias {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Renders the `PowerShell` wrapper module content.
#[must_use]
pub fn render_wrapper(
    metadata: &LocalizedDocMetadata,
    bin_name: &BinName,
    export_aliases: &[Alias],
    split_subcommands: bool,
) -> String {
    let mut output = String::new();

    push_line(&mut output, "[CmdletBinding(PositionalBinding = $false)]");
    push_line(&mut output, "param()");
    push_line(&mut output, "");

    let function_name = FunctionName::new(bin_name.as_str().to_owned());
    output.push_str(&render_function(&function_name, bin_name, &[]));
    output.push_str(&render_subcommand_functions(
        metadata,
        bin_name,
        split_subcommands,
    ));
    output.push_str(&render_aliases(bin_name, export_aliases));

    output.push_str(CRLF);
    output.push_str(&render_completion_block(bin_name));

    output
}

fn render_subcommand_functions(
    metadata: &LocalizedDocMetadata,
    bin_name: &BinName,
    split_subcommands: bool,
) -> String {
    if !split_subcommands {
        return String::new();
    }

    let mut output = String::new();
    for subcommand in &metadata.subcommands {
        let sub_name = subcommand
            .bin_name
            .as_deref()
            .unwrap_or(&subcommand.app_name);
        let function_name = FunctionName::new(format!("{}_{}", bin_name.as_str(), sub_name));
        output.push_str(CRLF);
        output.push_str(&render_function(
            &function_name,
            bin_name,
            &[sub_name.to_owned()],
        ));
    }

    output
}

fn render_aliases(bin_name: &BinName, export_aliases: &[Alias]) -> String {
    if export_aliases.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    output.push_str(CRLF);
    for alias in export_aliases {
        push_line(
            &mut output,
            &format!(
                "Set-Alias -Name {} -Value {}",
                quote_single(alias.as_str()),
                quote_single(bin_name.as_str())
            ),
        );
    }

    output
}

fn render_function(
    function_name: &FunctionName,
    exe_name: &BinName,
    extra_args: &[String],
) -> String {
    let mut output = String::new();

    push_line(
        &mut output,
        &format!("function {} {{", function_name.as_str()),
    );
    push_line(&mut output, "  [CmdletBinding(PositionalBinding = $false)]");
    push_line(
        &mut output,
        "  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)",
    );
    push_line(
        &mut output,
        &format!(
            "  $exe = Join-Path $PSScriptRoot '..' 'bin' {}",
            quote_single(&format!("{}.exe", exe_name.as_str()))
        ),
    );
    push_line(&mut output, "  $exe = (Resolve-Path $exe).ProviderPath");

    if extra_args.is_empty() {
        push_line(&mut output, "  & $exe @Args");
    } else {
        let joined = extra_args
            .iter()
            .map(|arg| quote_single(arg))
            .collect::<Vec<_>>()
            .join(" ");
        push_line(&mut output, &format!("  & $exe {joined} @Args"));
    }

    push_line(&mut output, "  $global:LASTEXITCODE = $LASTEXITCODE");
    push_line(&mut output, "}");
    output
}

fn render_completion_block(command_name: &BinName) -> String {
    let mut output = String::new();
    push_line(&mut output, "$sb = {");
    push_line(
        &mut output,
        "  param($wordToComplete, $commandAst, $cursorPosition)",
    );
    push_line(&mut output, "  # TODO: generated completion logic");
    push_line(&mut output, "}");
    push_line(
        &mut output,
        "$hasNative = (Get-Command Register-ArgumentCompleter).Parameters.ContainsKey('Native')",
    );
    push_line(&mut output, "if ($hasNative) {");
    push_line(
        &mut output,
        &format!(
            "  Register-ArgumentCompleter -Native -CommandName {} -ScriptBlock $sb",
            quote_single(command_name.as_str())
        ),
    );
    push_line(&mut output, "} else {");
    push_line(
        &mut output,
        &format!(
            "  Register-ArgumentCompleter -CommandName {} -ScriptBlock $sb",
            quote_single(command_name.as_str())
        ),
    );
    push_line(&mut output, "}");
    output
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
    use crate::ir::{LocalizedHeadings, LocalizedSectionsMetadata};
    use rstest::rstest;

    fn minimal_metadata() -> LocalizedDocMetadata {
        LocalizedDocMetadata {
            ir_version: "1.1".to_owned(),
            locale: "en-US".to_owned(),
            app_name: "fixture".to_owned(),
            bin_name: None,
            about: "Fixture".to_owned(),
            synopsis: None,
            sections: LocalizedSectionsMetadata {
                headings: LocalizedHeadings {
                    name: "NAME".to_owned(),
                    synopsis: "SYNOPSIS".to_owned(),
                    description: "DESCRIPTION".to_owned(),
                    options: "OPTIONS".to_owned(),
                    environment: "ENVIRONMENT".to_owned(),
                    files: "FILES".to_owned(),
                    precedence: "PRECEDENCE".to_owned(),
                    exit_status: "EXIT STATUS".to_owned(),
                    examples: "EXAMPLES".to_owned(),
                    see_also: "SEE ALSO".to_owned(),
                    commands: "COMMANDS".to_owned(),
                },
                discovery: None,
                precedence: None,
                examples: vec![],
                links: vec![],
                notes: vec![],
            },
            fields: vec![],
            subcommands: vec![LocalizedDocMetadata {
                ir_version: "1.1".to_owned(),
                locale: "en-US".to_owned(),
                app_name: "greet".to_owned(),
                bin_name: None,
                about: "Greet".to_owned(),
                synopsis: None,
                sections: LocalizedSectionsMetadata {
                    headings: LocalizedHeadings {
                        name: "NAME".to_owned(),
                        synopsis: "SYNOPSIS".to_owned(),
                        description: "DESCRIPTION".to_owned(),
                        options: "OPTIONS".to_owned(),
                        environment: "ENVIRONMENT".to_owned(),
                        files: "FILES".to_owned(),
                        precedence: "PRECEDENCE".to_owned(),
                        exit_status: "EXIT STATUS".to_owned(),
                        examples: "EXAMPLES".to_owned(),
                        see_also: "SEE ALSO".to_owned(),
                        commands: "COMMANDS".to_owned(),
                    },
                    discovery: None,
                    precedence: None,
                    examples: vec![],
                    links: vec![],
                    notes: vec![],
                },
                fields: vec![],
                subcommands: vec![],
                windows: None,
            }],
            windows: None,
        }
    }

    #[rstest]
    fn wrapper_includes_completion_registration() {
        let metadata = minimal_metadata();
        let output = render_wrapper(&metadata, &BinName::new("fixture"), &[], false);
        assert!(output.contains("Register-ArgumentCompleter"));
        assert!(output.contains("[CmdletBinding"));
    }

    #[rstest]
    fn wrapper_renders_subcommand_functions() {
        let metadata = minimal_metadata();
        let output = render_wrapper(&metadata, &BinName::new("fixture"), &[], true);
        assert!(output.contains("function fixture_greet"));
    }
}
