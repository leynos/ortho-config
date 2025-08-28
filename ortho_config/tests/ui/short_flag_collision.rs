use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
struct Collide {
    vfield: Option<String>,
    V: Option<String>,
}
