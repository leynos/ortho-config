//! Example CLI demonstrating subcommand configuration loading.

use clap::Parser;
use clap_dispatch::clap_dispatch;
use ortho_config::OrthoConfig;
use ortho_config::SubcmdConfigMerge;
use serde::{Deserialize, Serialize};

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
        println!("Connecting to database at: {db_url}");
        println!("Adding user: {:?}, Admin: {:?}", self.username, self.admin);
        Ok(())
    }
}

impl Run for ListItemsArgs {
    fn run(self, db_url: &str) -> Result<(), String> {
        println!("Connecting to database at: {db_url}");
        println!(
            "Listing items in category {:?}, All: {:?}",
            self.category, self.all
        );
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    // Serialisation enables persisting configuration. This helper ensures
    // roundtrips do not drop data.
    fn assert_roundtrip<T>(value: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).expect("serialise");
        let de: T = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(de, *value);
    }

    #[test]
    fn add_user_args_roundtrip() {
        let args = AddUserArgs {
            username: Some(String::from("alice")),
            admin: Some(true),
        };
        assert_roundtrip(&args);
    }

    #[test]
    fn list_items_args_roundtrip() {
        let args = ListItemsArgs {
            category: Some(String::from("tools")),
            all: Some(false),
        };
        assert_roundtrip(&args);
    }
}
