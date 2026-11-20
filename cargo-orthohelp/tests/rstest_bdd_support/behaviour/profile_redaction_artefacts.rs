//! Artefact scanning and inventory assertions for profile-redaction scenarios.
//!
//! The scenarios prove the disclosure boundary by reading every file a run
//! produced, so the scan, the inventory check, and the sentinel search live
//! here rather than in the step module. Keeping them separate leaves the step
//! module to read as the Gherkin text does and holds each module under the
//! repository's line budget.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};

use super::profile_redaction_support::require;
use super::steps::{OrthoHelpContext, StepResult, get_out_dir};
use crate::fixtures::profile_redaction_sentinels::ARTEFACT_LOCALES;

/// One generated file, or one captured process stream.
#[derive(Debug)]
pub(super) struct Artefact {
    pub path: String,
    pub content: String,
}

impl Artefact {
    pub(super) fn is_roff(&self) -> bool {
        self.path.ends_with(".1")
    }

    pub(super) fn is_maml(&self) -> bool {
        self.path.ends_with("-help.xml")
    }

    fn locale_contains(&self, locale: &str) -> bool {
        self.path.contains(locale)
    }
}

/// Reads every file beneath the scenario output directory plus both streams.
pub(super) fn read_artefacts(orthohelp_context: &OrthoHelpContext) -> StepResult<Vec<Artefact>> {
    let out_dir = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_dir, ambient_authority())?;
    let mut artefacts = Vec::new();
    collect_entries(&dir, Utf8Path::new(""), &mut artefacts)?;
    let (stdout, stderr) = orthohelp_context
        .last_output
        .with_ref(|output| (output.stdout.clone(), output.stderr.clone()))
        .ok_or("last_output should be set")?;
    artefacts.push(stream_artefact("<stdout>", &stdout));
    artefacts.push(stream_artefact("<stderr>", &stderr));
    Ok(artefacts)
}

fn stream_artefact(label: &str, bytes: &[u8]) -> Artefact {
    Artefact {
        path: label.to_owned(),
        content: String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn collect_entries(dir: &Dir, prefix: &Utf8Path, artefacts: &mut Vec<Artefact>) -> StepResult<()> {
    for (name, is_dir) in sorted_entries(dir)? {
        let path = prefix.join(&name);
        if is_dir {
            collect_entries(&dir.open_dir(name.as_str())?, &path, artefacts)?;
        } else {
            artefacts.push(read_file_artefact(dir, &name, path)?);
        }
    }
    Ok(())
}

/// Lists one directory's entries by name, flagging each as file or directory.
///
/// Sorting here keeps the recursive scan deterministic, so a failure always
/// names the same first offender regardless of the filesystem's enumeration
/// order.
fn sorted_entries(dir: &Dir) -> StepResult<Vec<(String, bool)>> {
    let mut entries = dir
        .read_dir(".")?
        .map(|entry| {
            let resolved = entry?;
            Ok((resolved.file_name()?, resolved.file_type()?.is_dir()))
        })
        .collect::<StepResult<Vec<_>>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

/// Reads one file entry into an [`Artefact`] under its relative path.
fn read_file_artefact(dir: &Dir, name: &str, path: Utf8PathBuf) -> StepResult<Artefact> {
    let bytes = dir.read(name)?;
    Ok(Artefact {
        path: path.into_string(),
        content: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// Fails unless the run produced every artefact family the scan claims to cover.
pub(super) fn assert_artefact_inventory(artefacts: &[Artefact]) -> StepResult<()> {
    require(
        has_suffix(artefacts, "agent-context.json"),
        "agent-context.json missing",
    )?;
    for locale in ARTEFACT_LOCALES {
        require(
            has_suffix(artefacts, &format!("ir/{locale}.json")),
            format!("IR for {locale} missing"),
        )?;
        require(
            artefacts
                .iter()
                .any(|artefact| artefact.is_roff() && artefact.locale_contains(locale)),
            format!("roff page for {locale} missing"),
        )?;
        require(
            artefacts
                .iter()
                .any(|artefact| artefact.is_maml() && artefact.locale_contains(locale)),
            format!("MAML help for {locale} missing"),
        )?;
    }
    Ok(())
}

fn has_suffix(artefacts: &[Artefact], suffix: &str) -> bool {
    artefacts
        .iter()
        .any(|artefact| artefact.path.ends_with(suffix))
}

/// Fails naming the first artefact that repeats a protected literal.
pub(super) fn assert_sentinels_absent(
    artefacts: &[Artefact],
    sentinels: &[&str],
) -> StepResult<()> {
    for sentinel in sentinels {
        for artefact in artefacts {
            require(
                !artefact.content.contains(sentinel),
                format!(
                    "protected sentinel {sentinel} appeared in {}",
                    artefact.path
                ),
            )?;
        }
    }
    Ok(())
}
