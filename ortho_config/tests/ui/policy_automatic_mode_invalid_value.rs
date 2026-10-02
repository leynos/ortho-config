//! Test case exercising an invalid `automatic_mode` value on a policy-enabled
//! derive.
//!
//! `automatic_mode` selects whether discovery stops at its first success or
//! stacks every applicable scope, and only two words name a mode. The attribute
//! parser takes the word as written, so an unrecognised one reaches the policy
//! token generator in `policy_impl.rs` and is rejected there. The near-miss
//! spelling here is deliberate: `first_wins` is a real mode, so a diagnostic
//! that only echoed the input back would not tell a reader which words are
//! legal. This fixture pins that both are named.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "BAD_AUTOMATIC_",
    discovery(app_name = "bad_automatic_app", automatic_mode = "first_win")
)]
struct BadAutomaticMode {
    #[ortho_config(default = 1)]
    value: u32,
}

fn main() {}
