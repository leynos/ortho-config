//! Test case exercising an invalid `scope_order` value on a policy-enabled
//! derive.
//!
//! The attribute parser collects `scope_order` entries as plain strings and
//! does not validate them, so an unrecognised scope reaches the policy token
//! generator in `policy_impl.rs` and is rejected there. That rejection is the
//! path this fixture pins: it must produce a `to_compile_error` naming the
//! three legal values rather than a confusing failure inside the generated
//! code.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "BAD_SCOPE_",
    discovery(
        app_name = "bad_scope_app",
        automatic_mode = "stack_scopes",
        scope_order = ["user", "universe"]
    )
)]
struct BadScopeOrder {
    #[ortho_config(default = 1)]
    value: u32,
}

fn main() {}
