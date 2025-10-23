//! Example CLI demonstrating subcommand configuration loading.

use clap::Parser;
use clap_dispatch::clap_dispatch;
use ortho_config::OrthoConfig;
use ortho_config::SubcmdConfigMerge;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

/// Command-line options for the `add-user` subcommand.
///
/// # Examples
///
/// ```ignore
/// use crate::AddUserArgs;
/// let args = AddUserArgs::default();
/// assert!(args.username.is_none());
/// assert_eq!(args.admin, None);
/// ```
#[derive(Parser, Deserialize, Serialize, Default, Debug, Clone, PartialEq, OrthoConfig)]
#[ortho_config(prefix = "REGCTL_")]
pub struct AddUserArgs {
    #[arg(long)]
    username: Option<String>,
    #[arg(long)]
    admin: Option<bool>,
}

/// Command-line options for the `list-items` subcommand.
///
/// # Examples
///
/// ```ignore
/// use crate::ListItemsArgs;
/// let args = ListItemsArgs::default();
/// assert!(args.category.is_none());
/// assert_eq!(args.all, None);
/// ```
#[derive(Parser, Deserialize, Serialize, Default, Debug, Clone, PartialEq, OrthoConfig)]
#[ortho_config(prefix = "REGCTL_")]
pub struct ListItemsArgs {
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    all: Option<bool>,
}

impl Run for AddUserArgs {
    fn run(self, db_url: &str) -> Result<(), String> {
        with_locked_stdout(db_url, |stdout| {
            write_line(
                stdout,
                &format!("Adding user: {:?}, Admin: {:?}", self.username, self.admin),
            )
        })
    }
}

impl Run for ListItemsArgs {
    fn run(self, db_url: &str) -> Result<(), String> {
        with_locked_stdout(db_url, |stdout| {
            write_line(
                stdout,
                &format!(
                    "Listing items in category {:?}, All: {:?}",
                    self.category, self.all
                ),
            )
        })
    }
}

#[derive(Parser)]
#[command(name = "registry-ctl", version = "0.3.0", about = "Manages a registry")]
#[clap_dispatch(fn run(self, db_url: &str) -> Result<(), String>)]
enum Commands {
    AddUser(AddUserArgs),
    ListItems(ListItemsArgs),
}

fn main() -> Result<(), String> {
    let cli = Commands::parse();
    let db_url = "postgres://user:pass@localhost/registry";
    let final_cmd = match cli {
        Commands::AddUser(args) => {
            let merged = args.load_and_merge().map_err(|e| e.to_string())?;
            Commands::AddUser(merged)
        }
        Commands::ListItems(args) => {
            let merged = args.load_and_merge().map_err(|e| e.to_string())?;
            Commands::ListItems(merged)
        }
    };
    final_cmd.run(db_url)
}

fn with_locked_stdout<F>(db_url: &str, emit: F) -> Result<(), String>
where
    F: FnOnce(&mut dyn Write) -> Result<(), String>,
{
    let mut stdout = io::stdout().lock();
    write_line(&mut stdout, &format!("Connecting to database at: {db_url}"))?;
    emit(&mut stdout)
}

fn write_line(writer: &mut dyn Write, message: &str) -> Result<(), String> {
    // Forward stdout failures through the existing `String` error channel
    // so the example trait signature stays stable for consumers.
    writer
        .write_all(message.as_bytes())
        .map_err(|err| err.to_string())?;
    writer.write_all(b"\n").map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result, ensure};
    use serde::de::DeserializeOwned;

    // Serialisation enables persisting configuration. This helper ensures
    // roundtrips do not drop data.
    fn assert_roundtrip<T>(value: &T) -> Result<()>
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json =
            serde_json::to_string(value).context("serialise subcommand arguments to JSON")?;
        let de: T =
            serde_json::from_str(&json).context("deserialise subcommand arguments from JSON")?;
        ensure!(
            de == *value,
            "roundtrip lost data: expected {value:?}, got {de:?}"
        );
        Ok(())
    }

    #[test]
    fn add_user_args_roundtrip() -> Result<()> {
        let args = AddUserArgs {
            username: Some(String::from("alice")),
            admin: Some(true),
        };
        assert_roundtrip(&args)
    }

    #[test]
    fn list_items_args_roundtrip() -> Result<()> {
        let args = ListItemsArgs {
            category: Some(String::from("tools")),
            all: Some(false),
        };
        assert_roundtrip(&args)
    }
}
