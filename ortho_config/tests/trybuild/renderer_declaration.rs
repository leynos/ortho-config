//! Compile-time coverage for the `renderer(...)` declaration surface.
//!
//! A downstream crate declares both records with the whole policy vocabulary,
//! so a derive that rejects an accepted leaf or stops emitting one fails to
//! compile here instead of leaving a silent gap in the generated IR.

use ortho_config::OrthoConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "RENDERER_TRYBUILD",
    renderer(
        human(
            supported = true,
            tty = "adapts",
            closed_stdin = "eof",
            colour = "auto",
            emoji = "never",
            progress = "always",
            pager = "auto",
            width = 80,
            accessibility = "text_equivalent",
            plain = "available",
            stdout = "text",
            stderr = "mixed",
            progress_inheritance = "allowed",
            subprocess_inheritance = "forbidden",
            field("message", localization = "localized"),
            field("error_code", localization = "non_localized")
        ),
        machine(
            supported = true,
            tty = "independent",
            closed_stdin = "error",
            colour = "never",
            emoji = "auto",
            progress = "never",
            pager = "always",
            width = "unbounded",
            accessibility = "unspecified",
            plain = "always",
            stdout = "structured",
            stderr = "none",
            progress_inheritance = "forbidden",
            subprocess_inheritance = "allowed",
            field("error_code", localization = "non_localized")
        )
    )
)]
struct RendererTrybuildConfig {
    #[serde(default)]
    recipient: String,
}

/// Manufacturing a value keeps the derive's generated code instantiated.
fn main() {
    let config = RendererTrybuildConfig {
        recipient: String::new(),
    };
    println!("{}", config.recipient.len());
}
