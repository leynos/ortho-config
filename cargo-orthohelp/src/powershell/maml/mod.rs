//! Microsoft Assistance Markup Language (MAML) help rendering.

mod render;
#[cfg(test)]
mod tests;
mod types;
mod xml_writer;

pub use types::{CommandSpec, MamlOptions};

/// Renders the MAML help XML for the provided command entries.
#[must_use]
pub fn render_help(commands: &[CommandSpec<'_>], options: MamlOptions) -> String {
    render::render_help(commands, options)
}
