//! Assertion helpers for validating command output and declarative globals.
use super::{Expect, Harness};
use anyhow::{Result, anyhow, ensure};
use hello_world::cli::GlobalArgs;

impl Harness {
    pub(crate) fn set_declarative_globals(&mut self, globals: GlobalArgs) {
        self.declarative_globals = Some(globals);
    }

    fn declarative_globals(&self) -> Result<&GlobalArgs> {
        self.declarative_globals
            .as_ref()
            .ok_or_else(|| anyhow!("declarative globals composed before assertion"))
    }

    pub(crate) fn assert_declarative_recipient<S>(&self, expected: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let globals = self.declarative_globals()?;
        let recipient = globals.recipient.as_deref().unwrap_or("");
        ensure!(
            recipient == expected.as_ref(),
            "unexpected recipient {recipient:?}"
        );
        Ok(())
    }

    pub(crate) fn assert_declarative_salutations(&self, expected: &[String]) -> Result<()> {
        let globals = self.declarative_globals()?;
        ensure!(
            globals.salutations == expected,
            "unexpected salutations: {:?}",
            globals.salutations
        );
        Ok(())
    }

    pub(crate) fn assert_outcome(&mut self, expect: Expect<'_>) -> Result<()> {
        let result = self.result()?;
        let context = result.command_context();
        match expect {
            Expect::Success => ensure!(
                result.success,
                "expected success; {context}; stderr: {}",
                result.stderr
            ),
            Expect::Failure => ensure!(
                !result.success,
                "expected failure; {context}; stdout: {}",
                result.stdout
            ),
            Expect::StdoutContains(expected) => ensure!(
                result.stdout.contains(expected),
                "stdout did not contain {expected:?}; {context}; stdout was: {:?}",
                result.stdout
            ),
            Expect::StderrContains(expected) => ensure!(
                result.stderr.contains(expected),
                "stderr did not contain {expected:?}; {context}; stderr was: {:?}",
                result.stderr
            ),
        }
        Ok(())
    }

    pub(crate) fn assert_success(&mut self) -> Result<()> {
        self.assert_outcome(Expect::Success)
    }

    pub(crate) fn assert_failure(&mut self) -> Result<()> {
        self.assert_outcome(Expect::Failure)
    }

    pub(crate) fn assert_stdout_contains<S>(&mut self, expected: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let expected_ref = expected.as_ref();
        self.assert_outcome(Expect::StdoutContains(expected_ref))
    }

    pub(crate) fn assert_stderr_contains<S>(&mut self, expected: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        let expected_ref = expected.as_ref();
        self.assert_outcome(Expect::StderrContains(expected_ref))
    }
}
