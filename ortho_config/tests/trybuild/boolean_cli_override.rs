//! Compile-time contract for the generated optional-value boolean CLI.
//!
//! The derive changes procedural-macro *output*: a `bool` or `Option<bool>`
//! field now emits an argument that accepts an optional `=<BOOL>` value. A
//! unit test that exercises the generated parser proves today's expansion
//! runs, but only a downstream-style compilation proves the emitted code is
//! usable from outside the crate — that the hidden parser struct, the
//! layering helpers, and the `clap` bounds they rely on all still resolve for
//! a consumer who writes nothing but `#[derive(OrthoConfig)]`.
//!
//! The plain `bool` and the `Option<bool>` are declared together because that
//! pair is where the shapes diverge: the plain field defaults to `false` and
//! can be cleared from the command line, while the optional field must
//! distinguish "supplied nowhere" from `Some(false)`.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "BOOLEAN_CLI")]
struct BooleanCliConfig {
    /// A plain boolean that still carries a struct default.
    #[serde(default)]
    #[ortho_config(default = false)]
    verbose: bool,
    /// An optional boolean, which distinguishes absent from `false`.
    #[serde(default)]
    quiet: Option<bool>,
}

/// The hidden parser struct the derive generates for [`BooleanCliConfig`].
type GeneratedCli = __BooleanCliConfigCli;

fn main() {
    // Parse both spellings through the generated argument definitions: the
    // bare flag, and an explicit value that must reach the field as `false`.
    let bare = GeneratedCli::try_parse_from(["prog", "--verbose"]).expect("bare flag parses");
    let cleared =
        GeneratedCli::try_parse_from(["prog", "--verbose=false"]).expect("explicit false parses");
    let absent = GeneratedCli::try_parse_from(["prog"]).expect("an omitted flag parses");
    assert_eq!(bare.verbose, Some(true));
    assert_eq!(cleared.verbose, Some(false));
    assert!(absent.verbose.is_none());

    let quiet = GeneratedCli::try_parse_from(["prog", "--quiet=false"]).expect("optional false");
    assert_eq!(quiet.quiet, Some(false));

    // The layering helper is part of the same expansion and must stay callable
    // from outside the crate. It is named rather than called because this
    // fixture runs inside `trybuild`'s harness, and building a composition
    // reads the real process environment and discovers configuration files on
    // the machine running the test. Naming it proves the emitted signature
    // resolves from a downstream crate; the runtime behaviour belongs to the
    // unit suites, which inject their own sources.
    let _compose = BooleanCliConfig::compose_layers_from_iter::<[&str; 1], &str>;
}
