//! Adapter from a real agent-context tree to an effective command surface.
//!
//! Agent context describes each command's *declared inputs*, not the options
//! the CLI really accepts once global options are resolved. Skill validation
//! needs the effective answer, so a consumer that owns both the tree and the
//! CLI's global options supplies the classification explicitly here.
//!
//! Nothing is inferred. The root command's inputs are never unioned into a
//! child, because a CLI may accept a root-only option on the root alone. A
//! global option reaches a command only because the consumer listed it as
//! global, which is the same statement a bridge makes when it knows the CLI
//! propagates that option everywhere.

use ortho_config::AgentContext;

use super::input::{CommandContract, EffectiveCommandSurface, EffectiveOption};

/// One option a consumer declares as global to every command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalOption {
    /// Long option name without the leading `--`.
    pub long: String,
    /// Declared value type, when the CLI states one.
    pub value_type: Option<String>,
}

impl GlobalOption {
    /// Builds a global option with an undeclared value type.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::skill_validation::GlobalOption;
    ///
    /// let colour = GlobalOption::new("color");
    /// assert_eq!(colour.long, "color");
    /// assert!(colour.value_type.is_none());
    /// ```
    #[must_use]
    pub fn new(long: &str) -> Self {
        Self {
            long: long.to_owned(),
            value_type: None,
        }
    }
}

impl From<&str> for GlobalOption {
    fn from(long: &str) -> Self {
        Self::new(long)
    }
}

impl EffectiveCommandSurface {
    /// Builds an effective surface from a real agent-context tree.
    ///
    /// Each command reports the inputs it declares, plus the declared global
    /// options, each marked as the consumer classified it. A command that
    /// declares a global option itself keeps its own value type and is still
    /// classified global, so one CLI option never becomes two surface entries.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::skill_validation::{EffectiveCommandSurface, GlobalOption};
    /// use ortho_config::{AgentCommand, AgentContext, AgentInput, RendererContract};
    ///
    /// let context = AgentContext {
    ///     commands: vec![AgentCommand {
    ///         path: vec!["demo".to_owned(), "render".to_owned()],
    ///         summary: None,
    ///         canonical_verb: None,
    ///         inputs: vec![AgentInput {
    ///             name: "format".to_owned(),
    ///             long: Some("format".to_owned()),
    ///             value_type: None,
    ///             required: false,
    ///             default: None,
    ///             enum_values: Vec::new(),
    ///         }],
    ///         output_modes: Vec::new(),
    ///         interaction_mode: ortho_config::InteractionMode::default(),
    ///         mutation_effect: ortho_config::MutationEffect::default(),
    ///         bypass_flag: None,
    ///         dry_run_flag: None,
    ///         async_submission: None,
    ///         delivery_route: None,
    ///         pagination: None,
    ///         examples: Vec::new(),
    ///         renderer: RendererContract::default(),
    ///     }],
    ///     ..AgentContext::new("demo")
    /// };
    ///
    /// let surface = EffectiveCommandSurface::from_agent_context(
    ///     &context,
    ///     &[GlobalOption::new("color")],
    /// );
    ///
    /// let render = &surface.commands[0];
    /// assert!(render.options.iter().any(|option| option.long == "format"));
    /// assert!(
    ///     render
    ///         .options
    ///         .iter()
    ///         .any(|option| option.long == "color" && option.is_global)
    /// );
    /// ```
    #[must_use]
    pub fn from_agent_context(context: &AgentContext, globals: &[GlobalOption]) -> Self {
        let commands = context
            .commands
            .iter()
            .map(|command| CommandContract {
                path: command.path.clone(),
                options: effective_options(&command.inputs, globals),
                canonical_verb: command.canonical_verb.clone(),
            })
            .collect();
        Self { commands }
    }
}

/// Returns one command's effective options in a stable order.
///
/// Declared inputs come first in their tree order, so a consumer reading the
/// surface sees the command's own options before the ones every command
/// shares. A global a command already declares is not appended twice.
fn effective_options(
    inputs: &[ortho_config::AgentInput],
    globals: &[GlobalOption],
) -> Vec<EffectiveOption> {
    let mut options: Vec<EffectiveOption> = Vec::new();
    for input in inputs {
        let Some(long) = input.long.as_deref() else {
            continue;
        };
        if options.iter().any(|option| option.long == long) {
            continue;
        }
        options.push(EffectiveOption {
            long: long.to_owned(),
            value_type: input.value_type.clone(),
            is_global: globals.iter().any(|global| global.long == long),
        });
    }
    for global in globals {
        if options.iter().any(|option| option.long == global.long) {
            continue;
        }
        options.push(EffectiveOption {
            long: global.long.clone(),
            value_type: global.value_type.clone(),
            is_global: true,
        });
    }
    options
}
