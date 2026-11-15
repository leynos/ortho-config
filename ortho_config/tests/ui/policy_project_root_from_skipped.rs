//! Test case naming a `skip_cli` field in `project_root_from`.
//!
//! A field the CLI does not carry has no argument for the project root to
//! be read from, so the derive refuses rather than emitting a policy whose
//! root silently falls back to the default. The field does exist on the
//! struct, which is what makes this case distinct from a name matching no
//! field at all: the lookup succeeds and the refusal comes from the field's
//! own declaration.

use ortho_config::OrthoConfig;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(prefix = "SKIPPED_ROOT_", discovery(project_root_from = "config_path"))]
struct SkippedProjectRoot {
    #[ortho_config(skip_cli)]
    config_path: Option<PathBuf>,
    #[ortho_config(default = 1)]
    value: u32,
}

fn main() {}
