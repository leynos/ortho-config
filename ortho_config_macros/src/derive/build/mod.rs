//! Code generation helpers for the `OrthoConfig` derive macro.
//!
//! This module provides focused submodules that construct the code emitted by
//! the derive implementation. Each builder is responsible for a distinct
//! concern such as CLI argument handling, environment discovery, or override
//! wiring. Keeping the logic in dedicated modules mirrors the structure used by
//! `load_impl` and `parse`, making it easier to reason about the macro surface.

mod cli;
mod config_flag;
mod defaults;
mod env;
mod r#override;
#[cfg(test)]
mod override_tests;

pub(crate) use cli::build_cli_struct_fields;
pub(crate) use config_flag::build_config_flag_field;
pub(crate) use defaults::{build_default_struct_fields, build_default_struct_init};
pub(crate) use env::{
    build_config_env_var, build_env_provider, compute_config_env_var, compute_dotfile_name,
    default_app_name,
};
pub(crate) use r#override::{CollectionStrategies, collect_collection_strategies};

#[cfg(test)]
#[expect(
    unused_imports,
    reason = "Collection override helpers are exercised only in macro tests"
)]
pub(crate) use r#override::{build_collection_logic, build_override_struct};
