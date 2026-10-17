//! Transform bridge documentation IR into compact agent-context metadata.
//!
//! This module owns the `cargo-orthohelp` adapter from the documentation
//! oriented bridge IR to the reusable `ortho_config::agent_context` schema.

use ortho_config::{
    AgentCommand, AgentContext, AgentInput, InteractionMode, Localizer, MutationEffect,
    ProfileSelectionContract, ProfilesDeclaration,
};

use crate::policy::vocabulary::CANONICAL_VERBS;
use crate::schema::{DocMetadata, InteractionKind, MutationKind};

mod input;
use input::build_input;

mod policy;
pub use policy::apply_policy_to_context;

/// Builds an agent-context document from bridge documentation metadata.
///
/// The transform is deterministic: command paths and command inputs are sorted
/// before returning so callers get stable JSON after serialization.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::agent_context::bridge_ir_to_agent_context;
/// use cargo_orthohelp::schema::{
///     DocMetadata, HeadingIds, ORTHO_DOCS_IR_VERSION, SectionsMetadata,
/// };
///
/// let metadata = DocMetadata {
///     ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
///     app_name: "example".to_owned(),
///     bin_name: None,
///     about_id: "example.about".to_owned(),
///     synopsis_id: None,
///     sections: SectionsMetadata {
///         headings_ids: HeadingIds {
///             name: "heading.name".to_owned(),
///             synopsis: "heading.synopsis".to_owned(),
///             description: "heading.description".to_owned(),
///             options: "heading.options".to_owned(),
///             environment: "heading.environment".to_owned(),
///             files: "heading.files".to_owned(),
///             precedence: "heading.precedence".to_owned(),
///             exit_status: "heading.exit-status".to_owned(),
///             examples: "heading.examples".to_owned(),
///             see_also: "heading.see-also".to_owned(),
///             commands: None,
///         },
///         discovery: None,
///         precedence: None,
///         examples: Vec::new(),
///         links: Vec::new(),
///         notes: Vec::new(),
///     },
///     fields: Vec::new(),
///     subcommands: Vec::new(),
///     windows: None,
///     behaviour: None,
///     profiles: None,
/// };
///
/// let context = bridge_ir_to_agent_context(&metadata, "example", None);
/// assert_eq!(context.kind, "example.agent_context");
/// assert_eq!(context.commands[0].path, ["example"]);
/// ```
#[must_use]
pub fn bridge_ir_to_agent_context(
    meta: &DocMetadata,
    package: &str,
    localizer: Option<&dyn Localizer>,
) -> AgentContext {
    log_bridge_start(package, &meta.app_name);
    let mut context = AgentContext::new(package);
    if let Some(profiles) = &meta.profiles {
        context.profiles = ProfilesDeclaration::supported(ProfileSelectionContract {
            flag: profiles.flag.clone(),
            env_var: profiles.env_var.clone(),
        });
    }
    walk(meta, &[], &mut context.commands, localizer);
    expose_profile_selector(&context.profiles, &mut context.commands);
    context
        .commands
        .sort_by(|left, right| left.path.cmp(&right.path));
    for command in &mut context.commands {
        command
            .inputs
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    log_bridge_complete(package, context.commands.len());
    context
}

/// Announce the start of the IR-to-agent-context bridge.
///
/// Extracted so [`bridge_ir_to_agent_context`] holds at most one `tracing`
/// call: when a dependent crate enables `tracing`'s `log` bridge, Cargo's
/// feature unification expands every event into a branch ladder that counts
/// against the workspace's cognitive-complexity ceiling.
fn log_bridge_start(package: &str, root: &str) {
    tracing::debug!(
        package = %package,
        root = %root,
        "starting bridge IR to agent-context transformation",
    );
}

/// Announce the end of the IR-to-agent-context bridge.
fn log_bridge_complete(package: &str, command_count: usize) {
    tracing::debug!(
        package = %package,
        command_count,
        "bridge IR to agent-context transformation complete",
    );
}

/// Exposes the profile selector as an input on every command.
///
/// The selector is emitted with `hide = true`, so it is deliberately absent
/// from the IR's field metadata and therefore never becomes an [`AgentInput`]
/// through [`build_input`]. An agent reading `profiles.selection` would still
/// be told which flag selects a profile while no command admitted accepting
/// it, which is the contradiction this closes: the contract and the invocable
/// surface are made to agree.
///
/// The input is added to every command because the selector is global — the
/// flag is accepted at each level of the command tree, and the profile applies
/// to the whole load rather than to one subcommand.
///
/// Presence is decided by the *long option*, not the name. A selector is only
/// usable if the command surface actually carries `--<flag>`, and an input can
/// share the flag's name while exposing a different long option. Treating that
/// input as the selector would leave the declared selection contract pointing
/// at a flag the command does not accept. An existing input that already
/// exposes the long option is left untouched, so a consumer that models the
/// selector as an ordinary field is not duplicated; the contract assertion
/// below then checks the invariant holds either way.
fn expose_profile_selector(profiles: &ProfilesDeclaration, commands: &mut [AgentCommand]) {
    let Some(selection) = profiles.selection.as_ref() else {
        return;
    };
    let flag = selection.flag.as_str();
    for command in commands.iter_mut() {
        if !command
            .inputs
            .iter()
            .any(|input| exposes_long_option(input, flag))
        {
            command.inputs.push(profile_selector_input(flag));
        }
    }
    debug_assert!(
        commands.iter().all(|command| command
            .inputs
            .iter()
            .any(|input| exposes_long_option(input, flag))),
        "a supported profile declaration must expose its selector on every command",
    );
}

/// Reports whether `input` is available to callers as the long option `long`.
///
/// The name is not consulted: `AgentInput` carries the invocation spelling in
/// `long`, so that field is what decides whether the selector's flag exists on
/// the command.
fn exposes_long_option(input: &AgentInput, long: &str) -> bool {
    input.long.as_deref() == Some(long)
}

/// Builds the agent-context input describing the profile selector.
///
/// The value type is `string` rather than an enum: the profile names live in
/// the operator's configuration files, not in the command surface, so the IR
/// cannot enumerate them. Declaring a closed set here would be a lie an agent
/// might act on.
fn profile_selector_input(flag: &str) -> AgentInput {
    AgentInput {
        name: flag.to_owned(),
        long: Some(flag.to_owned()),
        value_type: Some(String::from("string")),
        required: false,
        default: None,
        enum_values: Vec::new(),
    }
}

/// Recursively transforms a `DocMetadata` node into `AgentCommand` entries.
///
/// `meta` is appended to `out`, `parent_path` supplies the already-resolved
/// command prefix, and `localizer` optionally resolves a concise summary.
/// Every child in `meta.subcommands` is then visited with the current command
/// path as its parent.
fn walk(
    meta: &DocMetadata,
    parent_path: &[String],
    out: &mut Vec<AgentCommand>,
    localizer: Option<&dyn Localizer>,
) {
    let path = command_path(meta, parent_path);
    let last_segment = path.last().map(String::as_str);
    out.push(AgentCommand {
        path: path.clone(),
        summary: resolve_summary(meta, localizer),
        canonical_verb: last_segment.and_then(canonical_verb_for),
        inputs: meta.fields.iter().filter_map(build_input).collect(),
        output_modes: Vec::new(),
        interaction_mode: map_interaction(meta.behaviour.as_ref()),
        mutation_effect: map_mutation(meta.behaviour.as_ref()),
        bypass_flag: meta
            .behaviour
            .as_ref()
            .and_then(|behaviour| behaviour.bypass.clone()),
        dry_run_flag: meta
            .behaviour
            .as_ref()
            .and_then(|behaviour| behaviour.dry_run.clone()),
        async_submission: None,
        delivery_route: None,
        pagination: None,
        examples: Vec::new(),
    });

    for subcommand in &meta.subcommands {
        walk(subcommand, &path, out, localizer);
    }
}

/// Builds the full command path for one metadata node.
///
/// The root command prefers `bin_name` because it is the invocable binary.
/// Child commands append `app_name` to the inherited path; a missing root
/// `bin_name` naturally falls back to `app_name`.
fn command_path(meta: &DocMetadata, parent_path: &[String]) -> Vec<String> {
    if parent_path.is_empty() {
        return vec![meta.bin_name.as_ref().unwrap_or(&meta.app_name).to_owned()];
    }
    let mut path = parent_path.to_vec();
    path.push(meta.app_name.clone());
    path
}

/// Resolves the command summary from the IR's `about_id` Fluent message.
///
/// Returns `None` when no localizer was supplied, when the catalogue has no
/// entry for the message, when the resolved text is blank, or when the lookup
/// returned a `[missing: ...]` placeholder. A summary is cosmetic, so every one
/// of those cases degrades to an absent summary rather than an error.
fn resolve_summary(meta: &DocMetadata, localizer: Option<&dyn Localizer>) -> Option<String> {
    let resolved = localizer?.lookup(&meta.about_id, None)?;
    let trimmed = resolved.trim();
    if trimmed.is_empty() || trimmed.starts_with("[missing:") {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Returns the canonical verb for a command path's final segment.
///
/// Only segments listed in [`CANONICAL_VERBS`] are recognised, so a
/// consumer-specific name yields `None` rather than being coerced into a
/// standard verb.
fn canonical_verb_for(last_segment: &str) -> Option<String> {
    CANONICAL_VERBS
        .contains(&last_segment)
        .then(|| last_segment.to_owned())
}

/// Maps a declared behaviour block onto the agent-context interaction mode.
///
/// Absent or partially-declared behaviour stays `Unknown`: the bridge never
/// infers semantics from command names, verbs, or flags.
fn map_interaction(behaviour: Option<&crate::schema::BehaviourMetadata>) -> InteractionMode {
    match behaviour.and_then(|meta| meta.interaction) {
        Some(InteractionKind::NonInteractive) => InteractionMode::NonInteractive,
        Some(InteractionKind::Interactive) => InteractionMode::Interactive,
        None => InteractionMode::Unknown,
    }
}

/// Maps a declared behaviour block onto the agent-context mutation effect.
///
/// Absent or partially-declared behaviour stays `Unknown`; see
/// [`map_interaction`].
fn map_mutation(behaviour: Option<&crate::schema::BehaviourMetadata>) -> MutationEffect {
    match behaviour.and_then(|meta| meta.mutation) {
        Some(MutationKind::ReadOnly) => MutationEffect::ReadOnly,
        Some(MutationKind::Write) => MutationEffect::Write,
        Some(MutationKind::Delete) => MutationEffect::Delete,
        Some(MutationKind::Submit) => MutationEffect::Submit,
        None => MutationEffect::Unknown,
    }
}

mod default_display;

#[cfg(test)]
mod proptests;

#[cfg(test)]
mod tests;
