//! Bound child-process ownership in cross-process bridge-build tests.

use anyhow::{Context, Result, ensure};
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

const PROBE_WAIT: Duration = Duration::from_secs(8);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(in crate::bridge::bridge_build::tests) struct ProbeProcess(Option<Child>);

impl ProbeProcess {
    pub(in crate::bridge::bridge_build::tests) fn spawn(command: &mut Command) -> Result<Self> {
        Ok(Self(Some(command.spawn()?)))
    }

    pub(in crate::bridge::bridge_build::tests) fn wait(mut self) -> Result<()> {
        let mut child = self
            .0
            .take()
            .context("controlled probe process was already reaped")?;
        wait_for_success(&mut child)
    }
}

impl Drop for ProbeProcess {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            kill_probe(child);
            reap_after_kill(child);
        }
    }
}

fn wait_for_success(child: &mut Child) -> Result<()> {
    let deadline = Instant::now() + PROBE_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                ensure!(status.success(), "probe process exited with {status}");
                return Ok(());
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                kill_probe(child);
                reap_after_kill(child);
                anyhow::bail!("controlled owner process exceeded its bounded wait")
            }
            Err(error) => {
                kill_probe(child);
                reap_after_kill(child);
                return Err(error).context("waiting for controlled owner process");
            }
        }
    }
}

fn reap_after_kill(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                tracing::warn!(error = %error, "failed to reap controlled test probe");
                break;
            }
        }
    }
}

fn kill_probe(child: &mut Child) {
    if let Err(error) = child.kill() {
        tracing::warn!(error = %error, "failed to terminate controlled test probe");
    }
}
