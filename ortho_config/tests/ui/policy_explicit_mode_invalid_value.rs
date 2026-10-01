//! Test case exercising an invalid `explicit_mode` value on a policy-enabled
//! derive.
//!
//! `explicit_mode` selects how a required explicit path behaves, and only two
//! words name a mode. The attribute parser takes the word as written, so an
//! unrecognised one reaches the policy token generator in `policy_impl.rs` and
//! is rejected there. That rejection is the path this fixture pins: the
//! diagnostic must name both legal values rather than letting the generated
//! code fail somewhere further down.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "BAD_EXPLICIT_",
    discovery(
        app_name = "bad_explicit_app",
        explicit_mode = "always_required"
    )
)]
struct BadExplicitMode {
    #[ortho_config(default = 1)]
    value: u32,
}

fn main() {}
