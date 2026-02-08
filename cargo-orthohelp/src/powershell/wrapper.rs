//! `PowerShell` wrapper module rendering.

use crate::ir::LocalizedDocMetadata;
use crate::powershell::text::{CRLF, push_line, quote_single};
use std::fmt;

macro_rules! string_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_newtype!(BinName);
string_newtype!(FunctionName);
string_newtype!(Alias);

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

    let function_name = FunctionName::new(bin_name.to_string());
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
        let function_name = FunctionName::new(format!("{bin_name}_{sub_name}"));
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
                quote_single(alias.as_ref()),
                quote_single(bin_name.as_ref())
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

    push_line(&mut output, &format!("function {function_name} {{"));
    push_line(&mut output, "  [CmdletBinding(PositionalBinding = $false)]");
    push_line(
        &mut output,
        "  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$RemainingArgs)",
    );
    push_line(
        &mut output,
        &format!(
            "  $exe = Join-Path $PSScriptRoot '..' 'bin' {}",
            quote_single(&format!("{exe_name}.exe"))
        ),
    );
    push_line(&mut output, "  $exe = (Resolve-Path $exe).ProviderPath");

    if extra_args.is_empty() {
        push_line(&mut output, "  & $exe @RemainingArgs");
    } else {
        let joined = extra_args
            .iter()
            .map(|arg| quote_single(arg))
            .collect::<Vec<_>>()
            .join(" ");
        push_line(&mut output, &format!("  & $exe {joined} @RemainingArgs"));
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
    push_line(&mut output, "  if ($wordToComplete) {");
    push_line(
        &mut output,
        "    [System.Management.Automation.CompletionResult]::new(",
    );
    push_line(&mut output, "      $wordToComplete,");
    push_line(&mut output, "      $wordToComplete,");
    push_line(&mut output, "      'ParameterValue',");
    push_line(&mut output, "      $wordToComplete");
    push_line(&mut output, "    )");
    push_line(&mut output, "  }");
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
            quote_single(command_name.as_ref())
        ),
    );
    push_line(&mut output, "} else {");
    push_line(
        &mut output,
        &format!(
            "  Register-ArgumentCompleter -CommandName {} -ScriptBlock $sb",
            quote_single(command_name.as_ref())
        ),
    );
    push_line(&mut output, "}");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::powershell::test_fixtures::minimal_doc_with_subcommand;
    use rstest::rstest;

    #[rstest]
    fn wrapper_includes_completion_registration() {
        let metadata = minimal_doc_with_subcommand();
        let output = render_wrapper(&metadata, &BinName::new("fixture"), &[], false);
        assert!(output.contains("Register-ArgumentCompleter"));
        assert!(output.contains("[CmdletBinding"));
        assert!(output.contains("@RemainingArgs"));
    }

    #[rstest]
    fn wrapper_renders_subcommand_functions() {
        let metadata = minimal_doc_with_subcommand();
        let output = render_wrapper(&metadata, &BinName::new("fixture"), &[], true);
        assert!(output.contains("function fixture_greet"));
    }
}
