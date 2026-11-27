//! Uniqueness checks for names the subcommand mount projects.
//!
//! A mounted subcommand reads environment variables from
//! `<PREFIX>_CMDS_<NAME>_` and file keys from `cmds.<name>`. When the payload
//! declares `#[command(name = "...")]`, the mount name is fixed, so a generated
//! name and a verbatim `env(name = ...)` or `file(key_path = ...)` override can
//! be compared at expansion time. Without a declared name the mount is a
//! runtime argument, and only the standalone namespace can be checked.

use std::collections::HashMap;

use proc_macro2::Span;

use super::defaults::default_env_name;
use super::validation::ensure_unique;

/// One field's environment claim before projection.
#[derive(Clone, Copy)]
pub(super) struct EnvClaim<'a> {
    /// Field identifier, used as the diagnostic span.
    pub(super) field: &'a syn::Ident,
    /// Field name used to compose the default environment suffix.
    pub(super) field_name: &'a str,
    /// Whether the claim is an explicit documentation override.
    pub(super) is_explicit: bool,
    /// Resolved standalone environment name.
    pub(super) env_name: &'a str,
}

/// One field's file-key claim before projection.
#[derive(Clone, Copy)]
pub(super) struct FileClaim<'a> {
    /// Field identifier, used as the diagnostic span.
    pub(super) field: &'a syn::Ident,
    /// Whether the claim is an explicit documentation override.
    pub(super) is_explicit: bool,
    /// Resolved standalone file key.
    pub(super) key_path: &'a str,
}

/// Tracks projected names already claimed under the statically known mount.
pub(super) struct ProjectedNames {
    /// Prefix with at most one trailing underscore removed, as at runtime.
    prefix_base: String,
    /// Mount name, when the payload declares one.
    mount_name: Option<String>,
    /// Projected environment variables already claimed.
    env_seen: HashMap<String, Span>,
    /// Projected file keys already claimed.
    file_seen: HashMap<String, Span>,
}

impl ProjectedNames {
    /// Captures the prefix and mount name used by the runtime projection.
    pub(super) fn new(prefix: Option<&str>, mount_name: Option<&str>) -> Self {
        Self {
            prefix_base: prefix
                .map(|value| value.strip_suffix('_').unwrap_or(value).to_owned())
                .unwrap_or_default(),
            mount_name: mount_name.map(str::to_owned),
            env_seen: HashMap::new(),
            file_seen: HashMap::new(),
        }
    }

    /// Rejects a projected environment name already claimed under the mount.
    pub(super) fn check_env(&mut self, claim: EnvClaim<'_>) -> syn::Result<()> {
        let Some(mount) = self.mount_name.as_deref() else {
            return Ok(());
        };
        let projected = if claim.is_explicit {
            claim.env_name.to_owned()
        } else {
            let env_key = mount.replace('-', "_").to_ascii_uppercase();
            let suffix = default_env_name(None, claim.field_name);
            format!("{}_CMDS_{env_key}_{suffix}", self.prefix_base)
        };
        ensure_unique("env", claim.field, &projected, &mut self.env_seen)
    }

    /// Rejects a projected file key already claimed under the mount.
    pub(super) fn check_file(&mut self, claim: FileClaim<'_>) -> syn::Result<()> {
        let Some(mount) = self.mount_name.as_deref() else {
            return Ok(());
        };
        let projected = if claim.is_explicit {
            claim.key_path.to_owned()
        } else {
            format!("cmds.{mount}.{}", claim.key_path)
        };
        ensure_unique("file", claim.field, &projected, &mut self.file_seen)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for projected-name collision detection.

    use super::{EnvClaim, FileClaim, ProjectedNames};

    /// Parses a field identifier for a claim.
    fn ident(name: &str) -> syn::Ident {
        syn::parse_str(name).expect("test identifier should parse")
    }

    #[test]
    fn a_projected_environment_collision_is_rejected() {
        let mut names = ProjectedNames::new(Some("APP"), Some("run"));
        let token = ident("token");
        names
            .check_env(EnvClaim {
                field: &token,
                field_name: "token",
                is_explicit: false,
                env_name: "APP_TOKEN",
            })
            .expect("the first claim should be accepted");

        let override_token = ident("override_token");
        let error = names
            .check_env(EnvClaim {
                field: &override_token,
                field_name: "override_token",
                is_explicit: true,
                env_name: "APP_CMDS_RUN_TOKEN",
            })
            .expect_err("the explicit override must collide once mounted");
        assert!(
            error
                .to_string()
                .contains("duplicate env identifier 'APP_CMDS_RUN_TOKEN'"),
            "unexpected collision diagnostic: {error}"
        );
    }

    #[test]
    fn a_projected_file_collision_is_rejected() {
        let mut names = ProjectedNames::new(Some("APP"), Some("run"));
        let token = ident("token");
        names
            .check_file(FileClaim {
                field: &token,
                is_explicit: false,
                key_path: "token",
            })
            .expect("the first claim should be accepted");

        let override_token = ident("override_token");
        let error = names
            .check_file(FileClaim {
                field: &override_token,
                is_explicit: true,
                key_path: "cmds.run.token",
            })
            .expect_err("the explicit override must collide once mounted");
        assert!(
            error
                .to_string()
                .contains("duplicate file identifier 'cmds.run.token'"),
            "unexpected collision diagnostic: {error}"
        );
    }

    #[test]
    fn an_unknown_mount_skips_the_projected_check() {
        let mut names = ProjectedNames::new(Some("APP"), None);
        let token = ident("token");
        names
            .check_env(EnvClaim {
                field: &token,
                field_name: "token",
                is_explicit: true,
                env_name: "APP_CMDS_RUN_TOKEN",
            })
            .expect("without a declared mount the projected namespace is unknown");
    }
}
