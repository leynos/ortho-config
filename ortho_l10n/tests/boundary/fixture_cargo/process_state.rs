//! Platform-specific process checks for boundary command runner tests.

use anyhow::Result;

/// Verify a timed-out command has been reaped or removed from the process table.
#[cfg(unix)]
pub(super) fn assert_child_was_cleaned_up(process_id: u32) -> Result<()> {
    use anyhow::{Context, bail};
    use nix::errno::Errno;
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::Pid;

    let pid = Pid::from_raw(i32::try_from(process_id)?);
    match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
        Err(Errno::ECHILD) => Ok(()),
        Ok(WaitStatus::StillAlive) => bail!("timed-out child {process_id} is still running"),
        Ok(status) => bail!("runner returned before reaping child {process_id}: {status:?}"),
        Err(error) => Err(error).context("check whether timed-out child was reaped"),
    }
}

/// Verify a timed-out Windows command no longer has a process-table entry.
#[cfg(windows)]
pub(super) fn assert_child_was_cleaned_up(process_id: u32) -> Result<()> {
    use anyhow::{Context, bail};
    use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::core::HRESULT;

    // SAFETY: the query-only handle is closed before this function returns.
    match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) } {
        Err(error) if error.code() == HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => Ok(()),
        Err(error) => Err(error).context("inspect timed-out Windows child"),
        Ok(handle) => {
            // SAFETY: `OpenProcess` returned this valid handle to the inspection.
            unsafe { CloseHandle(handle) }.context("close Windows process inspection handle")?;
            bail!("timed-out Windows child {process_id} still exists")
        }
    }
}
