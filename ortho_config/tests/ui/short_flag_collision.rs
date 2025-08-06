use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
struct Collide {
    second: Option<String>,
    sample: Option<String>,
    spare: Option<String>,
}
