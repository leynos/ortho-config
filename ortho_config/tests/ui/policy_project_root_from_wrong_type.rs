//! Test case naming a non-`PathBuf` field in `project_root_from`.
//!
//! The project root is a path, so the field it is read from must be a
//! `PathBuf` or an `Option<PathBuf>`. A `String` field would need a
//! conversion the derive does not perform, so it is refused here rather
//! than at the point the generated CLI is built.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(prefix = "WRONG_TYPE_ROOT_", discovery(project_root_from = "root_name"))]
struct WrongTypeProjectRoot {
    root_name: String,
    #[ortho_config(default = 1)]
    value: u32,
}

fn main() {}
