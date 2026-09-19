//! Coordinates agent-context generation and policy checking for one run.
//!
//! This module owns the shared en-US localizer and transformed agent context
//! used by both the optional agent-context artefact and the agent-native policy
//! check. Keeping these resources together prevents duplicate bridge work and
//! ensures a localizer failure produces one warning with summary-free output.

use crate::agent_context;
use crate::cli::OutputFormat;
use crate::error::OrthohelpError;
use crate::metadata::PackageSelection;
use crate::output;
use crate::schema::DocMetadata;
use camino::Utf8PathBuf;
use cargo_orthohelp::policy::{PolicyMode, rules::behaviour::check_behaviour};
use ortho_config::{AgentContext, FluentLocalizer, LanguageIdentifier, Localizer};
use std::io::Write;
use std::str::FromStr;

/// Decides which artefact families a run should generate.
///
/// The five booleans each gate one distinct artefact family (IR, man page,
/// `PowerShell`, agent context, localized docs), so collapsing them into
/// two-variant enums would obscure the per-family skip decisions made in
/// [`Self::for_run`]. The lint is suppressed with that rationale.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each boolean gates one distinct artefact family; collapsing them into enums would obscure the per-family skip decisions"
)]
pub(crate) struct GenerationPlan {
    pub(crate) should_generate_ir: bool,
    pub(crate) should_generate_man: bool,
    pub(crate) should_generate_ps: bool,
    pub(crate) should_generate_agent_context: bool,
    pub(crate) should_generate_localized_docs: bool,
}

impl GenerationPlan {
    /// Builds the plan for a run.
    ///
    /// When an enforcing lint flag is present and the default `--format ir` was
    /// not explicitly requested, artefact generation is skipped entirely: the
    /// answer to the check is on stdout and no files were asked for.
    #[must_use]
    pub(crate) const fn for_run(
        format: OutputFormat,
        check_is_enforcing: bool,
        format_was_explicit: bool,
    ) -> Self {
        let should_skip_artefacts = check_is_enforcing && !format_was_explicit;
        let should_generate_ir =
            !should_skip_artefacts && matches!(format, OutputFormat::Ir | OutputFormat::All);
        let should_generate_man =
            !should_skip_artefacts && matches!(format, OutputFormat::Man | OutputFormat::All);
        let should_generate_ps =
            !should_skip_artefacts && matches!(format, OutputFormat::Ps | OutputFormat::All);
        let should_generate_agent_context = !should_skip_artefacts
            && matches!(format, OutputFormat::AgentContext | OutputFormat::All);
        let should_generate_localized_docs =
            should_generate_ir || should_generate_man || should_generate_ps;
        Self {
            should_generate_ir,
            should_generate_man,
            should_generate_ps,
            should_generate_agent_context,
            should_generate_localized_docs,
        }
    }
}

/// Shared agent-context resources for one command invocation.
pub(crate) struct AgentContextResources {
    context: AgentContext,
    en_us_localizer: Option<(LanguageIdentifier, FluentLocalizer)>,
}

impl AgentContextResources {
    /// Returns the shared transformed agent context.
    pub(crate) const fn context(&self) -> &AgentContext {
        &self.context
    }

    /// Returns the cached en-US localizer, when resource loading succeeded.
    pub(crate) const fn en_us_localizer(&self) -> Option<&(LanguageIdentifier, FluentLocalizer)> {
        self.en_us_localizer.as_ref()
    }
}

/// Builds the shared agent context and its optional en-US summary localizer.
pub(crate) fn build_resources(
    doc_metadata: &DocMetadata,
    selection: &PackageSelection,
) -> AgentContextResources {
    let en_us_localizer = match build_en_us_localizer(&selection.package_root) {
        Ok(localizer) => Some(localizer),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "no en-US localizer available; agent-context summaries will be omitted",
            );
            None
        }
    };
    let summary_localizer = en_us_localizer
        .as_ref()
        .map(|(_, localizer)| localizer as &dyn Localizer);
    let context = agent_context::bridge_ir_to_agent_context(
        doc_metadata,
        &selection.package_name,
        summary_localizer,
    );
    AgentContextResources {
        context,
        en_us_localizer,
    }
}

/// Writes the shared agent context to its requested artefact path.
pub(crate) fn write_agent_context(
    out_dir: &Utf8PathBuf,
    resources: &AgentContextResources,
) -> Result<(), OrthohelpError> {
    tracing::debug!(
        package = %resources.context.package,
        command_count = resources.context.commands.len(),
        "writing agent-context artefact",
    );
    output::write_agent_context(out_dir.as_path(), resources.context())?;
    Ok(())
}

/// Runs the agent-native behaviour lint and reports whether deny findings exist.
///
/// The policy report is written to stdout as exactly one JSON document, a
/// human-readable summary goes to stderr, and the returned boolean is `true`
/// if and only if the report contains at least one `deny` finding. The caller
/// delays the exit-code-3 decision until explicitly requested artefact
/// generation completes. Runtime errors keep exit code 1; clap usage errors
/// keep exit code 2.
pub(crate) fn run_check(context: &AgentContext, mode: PolicyMode) -> Result<bool, OrthohelpError> {
    let report = check_behaviour(context, mode);
    let report_json = serde_json::to_string(&report)?;
    {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{report_json}").map_err(|source| OrthohelpError::Io {
            path: Utf8PathBuf::from("<stdout>"),
            source,
        })?;
    }
    {
        let mut stderr = std::io::stderr().lock();
        writeln!(
            stderr,
            "agent-native behaviour check: {} finding(s) ({} deny)",
            report.summary.total, report.summary.deny
        )
        .map_err(|source| OrthohelpError::Io {
            path: Utf8PathBuf::from("<stderr>"),
            source,
        })?;
    }
    Ok(report.summary.deny > 0)
}

fn build_en_us_localizer(
    package_root: &Utf8PathBuf,
) -> Result<(LanguageIdentifier, FluentLocalizer), OrthohelpError> {
    let locale =
        LanguageIdentifier::from_str("en-US").map_err(|err| OrthohelpError::InvalidLocale {
            value: "en-US".to_owned(),
            message: err.to_string(),
        })?;
    let resources = crate::locale::load_consumer_resources(package_root, &locale)?;
    let localizer = crate::locale::build_localizer(&locale, resources)?;
    Ok((locale, localizer))
}

#[cfg(test)]
mod tests {
    //! Tests for agent-native generation planning.

    use super::GenerationPlan;
    use crate::cli::OutputFormat;

    #[test]
    fn enforcing_check_without_explicit_format_skips_artefacts() {
        let plan = GenerationPlan::for_run(OutputFormat::Ir, true, false);

        assert!(!plan.should_generate_ir);
        assert!(!plan.should_generate_agent_context);
    }

    #[test]
    fn off_mode_keeps_default_ir_generation() {
        let plan = GenerationPlan::for_run(OutputFormat::Ir, false, false);

        assert!(plan.should_generate_ir);
        assert!(plan.should_generate_localized_docs);
    }

    #[test]
    fn explicit_format_composes_with_an_enforcing_check() {
        let plan = GenerationPlan::for_run(OutputFormat::AgentContext, true, true);

        assert!(plan.should_generate_agent_context);
        assert!(!plan.should_generate_ir);
    }
}
