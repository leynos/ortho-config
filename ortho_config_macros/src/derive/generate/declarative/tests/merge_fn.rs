//! Tests that validate generated merge constructor and post-merge hook tokens.

use anyhow::{Result, ensure};
use quote::quote;
use rstest::rstest;

use crate::derive::generate::declarative::generate_declarative_merge_from_layers_fn;

use super::helpers::{default_krate, parse_ident};

#[rstest]
fn generate_declarative_merge_from_layers_fn_emits_post_merge_hook() -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let tokens =
        generate_declarative_merge_from_layers_fn(&state_ident, &config_ident, true, &krate);
    let norm = tokens.to_string().replace(" :: ", "::").replace(' ', "");

    // Check for PostMergeContext::new(Self::prefix()).
    ensure!(
        norm.contains("PostMergeContext::new(Self::prefix())"),
        "expected PostMergeContext::new(Self::prefix()): {norm}"
    );

    // Check for ctx.with_file(...) when layer.path() is Some.
    ensure!(
        norm.contains("ctx.with_file(path.to_owned())"),
        "expected ctx.with_file(path.to_owned()): {norm}"
    );

    // Check for MergeProvenance::Cli check that calls ctx.with_cli_input().
    ensure!(
        norm.contains("MergeProvenance::Cli"),
        "expected MergeProvenance::Cli check: {norm}"
    );
    ensure!(
        norm.contains("ctx.with_cli_input()"),
        "expected ctx.with_cli_input(): {norm}"
    );

    // Check for PostMergeHook::post_merge(&mut result, &ctx)?.
    ensure!(
        norm.contains("PostMergeHook::post_merge(&mutresult,&ctx)?"),
        "expected PostMergeHook::post_merge(&mut result, &ctx)?: {norm}"
    );

    Ok(())
}

#[rstest]
fn generate_declarative_merge_from_layers_fn_emits_constructor() -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let tokens =
        generate_declarative_merge_from_layers_fn(&state_ident, &config_ident, false, &krate);
    let expected = quote! {
        impl Sample {
            /// Merge the configuration struct from declarative layers.
            ///
            /// See the
            /// [declarative merging design](https://github.com/leynos/ortho-config/blob/main/docs/design.md#43-declarative-configuration-merging)
            /// for background and trade-offs.
            ///
            /// # Feature Requirements
            ///
            /// This method requires the `serde_json` feature (enabled by default).
            ///
            /// # Examples
            ///
            /// ```rust,ignore
            /// use ortho_config::{MergeComposer, OrthoConfig};
            /// use serde::{Deserialize, Serialize};
            /// use serde_json::json;
            ///
            /// #[derive(Debug, Deserialize, Serialize, OrthoConfig)]
            /// #[ortho_config(prefix = "APP")]
            /// struct AppConfig {
            ///     #[ortho_config(default = 8080)]
            ///     port: u16,
            /// }
            ///
            /// let mut composer = MergeComposer::new();
            /// composer.push_defaults(json!({"port": 8080}));
            /// composer.push_environment(json!({"port": 9090}));
            ///
            /// let config = AppConfig::merge_from_layers(composer.layers())
            ///     .expect("layers merge successfully");
            /// assert_eq!(config.port, 9090);
            /// ```
            pub fn merge_from_layers<'a, I>(layers: I) -> ortho_config::OrthoResult<Self>
            where
                I: IntoIterator<Item = ortho_config::MergeLayer<'a>>,
            {
                let mut state = __SampleDeclarativeMergeState::default();
                for layer in layers {
                    ortho_config::DeclarativeMerge::merge_layer(&mut state, layer)?;
                }
                ortho_config::DeclarativeMerge::finish(state)
            }
        }
    };
    let actual = tokens.to_string();
    let expected_rendered = expected.to_string();
    ensure!(
        actual == expected_rendered,
        "merge_from_layers constructor mismatch\nactual:\n{actual}\nexpected:\n{expected_rendered}"
    );
    Ok(())
}
