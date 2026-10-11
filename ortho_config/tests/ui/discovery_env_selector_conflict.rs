//! Test case declaring both `env_var` and `env_vars` on one derive.
//!
//! The two selectors answer the same question -- which environment variable
//! names an explicit configuration path -- in a single-valued and a
//! multi-valued form. Declaring both leaves discovery no way to choose, so the
//! derive refuses rather than silently preferring one and letting the other be
//! ignored at run time. This fixture pins that the refusal happens while the
//! macro runs, naming the pair, instead of producing a configuration whose
//! behaviour depends on an undocumented precedence.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "CONFLICTING_ENV_",
    discovery(
        app_name = "conflicting_env_app",
        env_var = "DEMO_CONFIG",
        env_vars = ["DEMO_CONFIG_A", "DEMO_CONFIG_B"]
    )
)]
struct ConflictingEnvSelectors {
    #[ortho_config(default = 1)]
    value: u32,
}

fn main() {}
