//! Profile selection and layering (roadmap 9.1.1).
//!
//! This module hosts the generic profile mechanics behind the opt-in
//! `#[ortho_config(profiles)]` derive attribute: extraction of
//! `[profile.<name>]` tables from the resolved file chain, stateless
//! selection resolution, name-grammar validation, and the post-load
//! selection surface. Milestone 2 (merge-layer plumbing) and milestone 3
//! (extraction, selection, validation, errors) of
//! [execplan 9-1-1](https://github.com/leynos/ortho-config/blob/main/docs/execplans/9-1-1-profile-metadata.md)
//! are implemented here.
//!
//! The test modules are split from the start to stay beneath the 400-line
//! file cap: names and grammar, selection resolution, table extraction, and
//! error paths each live in their own file.

#[cfg(test)]
mod tests_errors;
#[cfg(test)]
mod tests_extraction;
#[cfg(test)]
mod tests_names;
#[cfg(test)]
mod tests_selection;
