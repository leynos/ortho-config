//! Renderer-metadata step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Kept in its own module so `steps_agent_context` stays under the repository's
//! 400-line ceiling. These steps read the generated `renderer` block out of
//! agent-context JSON, which is where an agent consumer meets the declaration.
//! The per-locale IR half lives in [`super::steps_ir_renderer`].
//!
//! See ADR-0016 for the vocabulary and the role-specific legacy support
//! defaults asserted here.

use rstest_bdd_macros::then;
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

use super::steps::{OrthoHelpContext, StepResult};

/// Nested fixture command carrying the dual renderer declaration.
///
/// The scenarios name the command by its leaf, because the agent-context path is
/// rooted at the package (`nested_fixture`) while the IR roots at the binary's
/// display name (`nested-app`). Resolving by leaf keeps one scenario step valid
/// against both artefact families.
pub(super) const GREET_COMMAND: &str = "greet";

/// Nested fixture command that declares no renderer at all.
pub(super) const VERSION_COMMAND: &str = "version";

/// Which renderer record a step addresses.
#[derive(Debug, Clone, Copy)]
enum RendererRole {
    /// The human renderer record.
    Human,
    /// The machine renderer record.
    Machine,
}

impl RendererRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Machine => "machine",
        }
    }
}

impl fmt::Display for RendererRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for RendererRole {
    type Err = String;

    /// Reads the role word used in the feature file.
    ///
    /// An unrecognized role is reported rather than mapped onto `human`, so a
    /// typo cannot quietly assert the wrong record.
    fn from_str(role: &str) -> Result<Self, Self::Err> {
        match role {
            "human" => Ok(Self::Human),
            "machine" => Ok(Self::Machine),
            other => Err(format!(
                "unknown renderer role `{other}`; expected `human` or `machine`"
            )),
        }
    }
}

/// One renderer policy leaf, spelled as it appears in agent-context JSON.
#[derive(Debug, Clone, Copy)]
enum RendererPolicyKey {
    /// Terminal sensitivity.
    Tty,
    /// Colour policy.
    Colour,
    /// Declared width kind.
    Width,
    /// Declared stdout content category.
    Stdout,
}

impl RendererPolicyKey {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Tty => "tty",
            Self::Colour => "colour",
            Self::Width => "width",
            Self::Stdout => "stdout",
        }
    }

    /// Every policy leaf this module can assert.
    const ALL: [Self; 4] = [Self::Tty, Self::Colour, Self::Width, Self::Stdout];
}

impl fmt::Display for RendererPolicyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for RendererPolicyKey {
    type Err = String;

    /// Reads the policy key used in the feature file.
    ///
    /// The vocabulary grows with the schema, so an unknown key is reported
    /// rather than mapped onto a default that would assert the wrong leaf.
    fn from_str(key: &str) -> Result<Self, Self::Err> {
        match key {
            "tty" => Ok(Self::Tty),
            "colour" => Ok(Self::Colour),
            "width" => Ok(Self::Width),
            "stdout" => Ok(Self::Stdout),
            other => Err(format!(
                "unknown renderer policy `{other}`; expected `tty`, `colour`, `width`, or `stdout`"
            )),
        }
    }
}

/// One `policy=token` expectation, bound from a single scenario placeholder.
///
/// The key and the expected value are one claim, so they travel as one step
/// argument. Splitting them across two placeholders would make the sentence
/// longer without making the scenario clearer.
#[derive(Debug, Clone)]
struct PolicyClaim {
    key: RendererPolicyKey,
    expected: String,
}

impl PolicyClaim {
    /// Builds a claim from a policy key and the token it should report.
    fn new(key: RendererPolicyKey, expected: impl Into<String>) -> Self {
        Self {
            key,
            expected: expected.into(),
        }
    }
}

impl FromStr for PolicyClaim {
    type Err = String;

    /// Reads the `key=token` claim used in the feature file.
    ///
    /// The key is parsed through [`RendererPolicyKey`] so a misspelt leaf is
    /// reported rather than compared against the wrong one.
    fn from_str(claim: &str) -> Result<Self, Self::Err> {
        let Some((key, expected)) = claim.split_once('=') else {
            return Err(format!(
                "expected `policy=token` such as `tty=adapts`, got `{claim}`"
            ));
        };
        Ok(Self::new(key.parse()?, expected))
    }
}

/// Reads the single agent-context JSON document the scenario last produced.
fn agent_context(orthohelp_context: &OrthoHelpContext) -> StepResult<Value> {
    let stdout = orthohelp_context
        .last_output
        .with_ref(|output| output.stdout.clone())
        .ok_or("last_output should be set")?;
    Ok(serde_json::from_slice(&stdout)?)
}

/// Finds the single command whose `path` ends in `leaf`.
///
/// A leaf is unique in this fixture, and requiring uniqueness means a renamed or
/// duplicated command fails here rather than silently asserting the first match.
fn command_by_leaf<'a>(json: &'a Value, leaf: &str) -> StepResult<&'a Value> {
    let matches = json
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("commands field missing")?
        .iter()
        .filter(|command| {
            command
                .get("path")
                .and_then(Value::as_array)
                .and_then(|path| path.last())
                .and_then(Value::as_str)
                == Some(leaf)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [command] => Ok(command),
        other => Err(format!(
            "expected exactly one command named {leaf:?}, found {}",
            other.len()
        )
        .into()),
    }
}

/// Returns one command's renderer record for the requested role.
///
/// The record is the object a consumer reads, so its absence is reported
/// against the command path and role rather than as a bare JSON type error.
fn renderer_record<'a>(json: &'a Value, leaf: &str, role: RendererRole) -> StepResult<&'a Value> {
    let command = command_by_leaf(json, leaf)?;
    command
        .get("renderer")
        .and_then(|renderer| renderer.get(role.as_str()))
        .ok_or_else(|| format!("command {leaf:?} has no {role} renderer record").into())
}

/// Asserts one renderer's `supported` leaf.
fn assert_supported(
    orthohelp_context: &OrthoHelpContext,
    leaf: &str,
    role: RendererRole,
    expected: bool,
) -> StepResult<()> {
    let json = agent_context(orthohelp_context)?;
    let record = renderer_record(&json, leaf, role)?;
    let actual = record
        .get("supported")
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("command {leaf:?} {role} renderer supported is not a boolean"))?;
    if actual == expected {
        return Ok(());
    }
    Err(
        format!("command {leaf:?} {role} renderer supported should be {expected}, got {actual}")
            .into(),
    )
}

/// Renders a declared policy leaf as the token or width kind it serializes to.
///
/// Width is the one tagged object in this vocabulary, so a reader that expected
/// a bare string there would silently compare an absent value against a token.
/// The `kind` fallback keeps the comparison honest for both shapes.
fn policy_token<'a>(record: &'a Value, key: RendererPolicyKey) -> Option<&'a str> {
    let leaf = record.get(key.as_str())?;
    leaf.as_str().or_else(|| {
        leaf.get("kind")
            .and_then(Value::as_str)
            .filter(|_| matches!(key, RendererPolicyKey::Width))
    })
}

/// Asserts a declared policy leaf, or its documented absence.
fn assert_policy(
    orthohelp_context: &OrthoHelpContext,
    leaf: &str,
    role: RendererRole,
    claim: &PolicyClaim,
) -> StepResult<()> {
    let json = agent_context(orthohelp_context)?;
    let record = renderer_record(&json, leaf, role)?;
    let actual = policy_token(record, claim.key);
    let matches = match claim.expected.as_str() {
        "undeclared" => actual.is_none(),
        declared => actual == Some(declared),
    };
    if matches {
        return Ok(());
    }
    Err(format!(
        "command {leaf:?} {role} renderer {} should be {}, got {actual:?}",
        claim.key, claim.expected
    )
    .into())
}

/// Declared support survives into agent context for the declaring command.
#[then("the command {command} reports {role} renderer supported {expected}")]
fn command_reports_renderer_supported(
    orthohelp_context: &mut OrthoHelpContext,
    command: String,
    role: RendererRole,
    expected: bool,
) -> StepResult<()> {
    assert_supported(orthohelp_context, &command, role, expected)
}

/// A declared presentation policy is reported verbatim for the named command.
#[then("the command {command} reports {role} renderer policy {claim}")]
fn command_reports_renderer_policy(
    orthohelp_context: &mut OrthoHelpContext,
    command: String,
    role: RendererRole,
    claim: PolicyClaim,
) -> StepResult<()> {
    assert_policy(orthohelp_context, &command, role, &claim)
}

/// A child that declares nothing keeps the legacy support defaults.
///
/// The two assertions are deliberately adjacent and opposite: a bridge that
/// copied a parent or sibling declaration into the child would satisfy one and
/// fail the other.
#[then("the undeclared nested command keeps legacy support defaults")]
fn undeclared_command_keeps_defaults(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    assert_supported(
        orthohelp_context,
        VERSION_COMMAND,
        RendererRole::Human,
        true,
    )?;
    assert_supported(
        orthohelp_context,
        VERSION_COMMAND,
        RendererRole::Machine,
        false,
    )
}

/// No presentation claim is invented for a command that declared none.
#[then("the undeclared nested command makes no presentation claim")]
fn undeclared_command_makes_no_claim(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    for role in [RendererRole::Human, RendererRole::Machine] {
        for key in RendererPolicyKey::ALL {
            let claim = PolicyClaim::new(key, "undeclared");
            assert_policy(orthohelp_context, VERSION_COMMAND, role, &claim)?;
        }
    }
    Ok(())
}
