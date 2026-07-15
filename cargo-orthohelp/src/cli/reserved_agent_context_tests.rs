//! Tests that reserve downstream context command names for applications.

use clap::CommandFactory;

use super::Cli;

const RESERVED_AGENT_CONTEXT_COMMANDS: [&str; 2] = ["context", "agent-context"];

#[test]
fn no_context_or_agent_context_subcommand_alias() {
    let command = Cli::command();
    let mut violations = Vec::new();

    collect_reserved_agent_context_commands(&command, &mut Vec::new(), &mut violations);

    assert!(
        violations.is_empty(),
        "reserved downstream context command names leaked into cargo-orthohelp: {violations:?}"
    );
}

fn collect_reserved_agent_context_commands(
    command: &clap::Command,
    path: &mut Vec<String>,
    violations: &mut Vec<String>,
) {
    for subcommand in command.get_subcommands() {
        path.push(subcommand.get_name().to_owned());
        let display_path = path.join(" ");

        record_reserved_subcommand_name(subcommand, &display_path, violations);
        record_reserved_aliases(subcommand, &display_path, violations);
        collect_reserved_agent_context_commands(subcommand, path, violations);
        path.pop();
    }
}

fn record_reserved_subcommand_name(
    subcommand: &clap::Command,
    display_path: &str,
    violations: &mut Vec<String>,
) {
    if is_reserved_agent_context_command(subcommand.get_name()) {
        violations.push(format!("subcommand `{display_path}`"));
    }
}

fn record_reserved_aliases(
    subcommand: &clap::Command,
    display_path: &str,
    violations: &mut Vec<String>,
) {
    for alias in subcommand
        .get_all_aliases()
        .filter(|alias| is_reserved_agent_context_command(alias))
    {
        violations.push(format!("alias `{alias}` on `{display_path}`"));
    }
}

fn is_reserved_agent_context_command(candidate: &str) -> bool {
    RESERVED_AGENT_CONTEXT_COMMANDS.contains(&candidate)
}
