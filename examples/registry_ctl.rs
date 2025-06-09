use clap::Parser;
use clap_dispatch::clap_dispatch;
use serde::Deserialize;
use ortho_config::{load_subcommand_config, merge_cli_over_defaults};

#[derive(Parser, Deserialize, Default, Debug, Clone)]
pub struct AddUserArgs {
    #[arg(long)]
    username: Option<String>,
    #[arg(long)]
    admin: Option<bool>,
}

#[derive(Parser, Deserialize, Default, Debug, Clone)]
pub struct ListItemsArgs {
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    all: Option<bool>,
}

trait Run {
    fn run(&self, db_url: &str) -> Result<(), String>;
}

impl Run for AddUserArgs {
    fn run(&self, db_url: &str) -> Result<(), String> {
        println!("Connecting to database at: {db_url}");
        println!("Adding user: {:?}, Admin: {:?}", self.username, self.admin);
        Ok(())
    }
}

impl Run for ListItemsArgs {
    fn run(&self, db_url: &str) -> Result<(), String> {
        println!("Connecting to database at: {db_url}");
        println!(
            "Listing items in category {:?}, All: {:?}",
            self.category,
            self.all
        );
        Ok(())
    }
}

#[derive(Parser)]
#[command(name = "registry-ctl", version = "0.1.0", about = "Manages a registry")]
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
            let defaults: AddUserArgs =
                load_subcommand_config("REGCTL_", "add-user").unwrap_or_default();
            let merged = merge_cli_over_defaults(&defaults, &args);
            Commands::AddUser(merged)
        }
        Commands::ListItems(args) => {
            // `ListItems` becomes `list-items` when parsed by clap
            let defaults: ListItemsArgs =
                load_subcommand_config("REGCTL_", "list-items").unwrap_or_default();
            let merged = merge_cli_over_defaults(&defaults, &args);
            Commands::ListItems(merged)
        }
    };
    final_cmd.run(db_url)
}
