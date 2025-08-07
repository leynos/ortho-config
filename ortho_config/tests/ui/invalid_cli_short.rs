use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
struct Bad {
    #[ortho_config(cli_short = '?')]
    field: String,
}
