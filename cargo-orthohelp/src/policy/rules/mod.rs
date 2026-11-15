//! Policy rules evaluated by `cargo-orthohelp` agent-native checks.

pub mod behaviour;
pub mod output;

#[cfg(test)]
mod output_proptests;

#[cfg(test)]
mod output_tests;

#[cfg(test)]
mod output_tie_tests;

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod tests;
