//! Regression coverage for the greet command's generic configuration identity.

use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use clap::CommandFactory;
use ortho_config::{MapEnv, SubcommandFileContext, load_and_merge_subcommand_for_with_sources_at};

use crate::cli::GreetCommand;

#[test]
fn greet_uses_its_public_name_for_file_and_environment_defaults() -> Result<()> {
    ensure!(
        GreetCommand::command().get_name() == "greet",
        "standalone command name must match the public CLI verb"
    );

    let root = tempfile::tempdir().context("create greet configuration fixture")?;
    let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
        .context("open greet configuration fixture")?;
    directory
        .write(
            ".hello_world.toml",
            concat!(
                "[cmds.greet]\n",
                "preamble = \"from greet file\"\n",
                "[cmds.hello_world]\n",
                "preamble = \"wrong command\"\n",
            )
            .as_bytes(),
        )
        .context("write greet configuration fixture")?;

    let discovery = MapEnv::new().with_var("XDG_CONFIG_DIRS", root.path());
    let files = SubcommandFileContext::new(root.path(), &discovery);
    let from_file = load_and_merge_subcommand_for_with_sources_at(
        &GreetCommand::default(),
        files,
        Arc::new(MapEnv::new()),
    )
    .context("load greet file section")?;
    ensure!(
        from_file.preamble.as_deref() == Some("from greet file"),
        "generic loader must select [cmds.greet]"
    );

    let from_env = load_and_merge_subcommand_for_with_sources_at(
        &GreetCommand::default(),
        files,
        Arc::new(
            MapEnv::new()
                .with_var("HELLO_WORLD_CMDS_GREET_PREAMBLE", "from greet env")
                .with_var("HELLO_WORLD_CMDS_HELLO_WORLD_PREAMBLE", "wrong command"),
        ),
    )
    .context("load greet environment section")?;
    ensure!(
        from_env.preamble.as_deref() == Some("from greet env"),
        "generic loader must select the greet environment prefix"
    );
    Ok(())
}
