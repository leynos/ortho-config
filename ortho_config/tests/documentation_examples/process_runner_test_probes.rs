//! Controlled child processes for subprocess-supervision regressions.

use super::probe_command;
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

#[cfg(unix)]
pub(super) fn process_is_active(process_id: u32) -> std::io::Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let proc_directory = Dir::open_ambient_dir("/proc", ambient_authority())?;
        let stat_path = format!("{process_id}/stat");
        let stat = match proc_directory.read_to_string(stat_path.as_str()) {
            Ok(stat) => stat,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(nix::libc::ESRCH) =>
            {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        let (_, fields) = stat
            .rsplit_once(')')
            .ok_or_else(|| std::io::Error::other("malformed process stat"))?;
        let state = fields
            .split_whitespace()
            .next()
            .and_then(|value| value.chars().next())
            .ok_or_else(|| std::io::Error::other("missing process state"))?;
        Ok(!matches!(state, 'Z' | 'X'))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let raw_process_id =
            i32::try_from(process_id).map_err(|_| std::io::Error::other("invalid process ID"))?;
        match nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw_process_id), None) {
            Ok(()) => Ok(true),
            Err(nix::errno::Errno::ESRCH) => Ok(false),
            Err(error) => Err(std::io::Error::from(error)),
        }
    }
}

#[cfg(windows)]
pub(super) fn process_is_active(process_id: u32) -> std::io::Result<bool> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows::Win32::Foundation::{
        ERROR_INVALID_PARAMETER, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };
    use windows::core::HRESULT;

    // SAFETY: the process query is limited to the test-owned probe PID.
    let raw_process = match unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            process_id,
        )
    } {
        Ok(raw_process) => raw_process,
        Err(error) if error.code() == HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => {
            return Ok(false);
        }
        Err(error) => return Err(std::io::Error::other(error.to_string())),
    };
    // SAFETY: OpenProcess returned an owned process handle.
    let process_handle = unsafe { OwnedHandle::from_raw_handle(raw_process.0.cast()) };
    // SAFETY: the handle remains valid and a zero timeout never blocks.
    let state = unsafe {
        WaitForSingleObject(
            windows::Win32::Foundation::HANDLE(process_handle.as_raw_handle().cast()),
            0,
        )
    };
    if state == WAIT_TIMEOUT {
        Ok(true)
    } else if state == WAIT_OBJECT_0 {
        Ok(false)
    } else if state == WAIT_FAILED {
        Err(std::io::Error::last_os_error())
    } else {
        Err(std::io::Error::other(format!(
            "unexpected process wait result: {}",
            state.0
        )))
    }
}

#[test]
#[ignore = "executed as a bounded output timeout subprocess"]
fn output_then_sleep_probe() {
    let output = [b'x'; 4096];
    std::io::stdout()
        .lock()
        .write_all(b"STDOUT_TIMEOUT_SENTINEL")
        .expect("write stdout timeout marker");
    std::io::stdout()
        .lock()
        .write_all(&output)
        .expect("write stdout probe data");
    std::io::stdout()
        .lock()
        .flush()
        .expect("flush stdout timeout data");
    std::io::stderr()
        .lock()
        .write_all(b"STDERR_TIMEOUT_SENTINEL")
        .expect("write stderr timeout marker");
    std::io::stderr()
        .lock()
        .write_all(&output)
        .expect("write stderr probe data");
    std::io::stderr()
        .lock()
        .flush()
        .expect("flush stderr timeout data");
    std::thread::sleep(Duration::from_secs(2));
}

#[test]
#[ignore = "executed as a sleeping child for cleanup regressions"]
fn sleep_probe() {
    std::thread::sleep(Duration::from_secs(2));
}

#[test]
#[ignore = "executed as a ready pipe holder for reader-cancellation coverage"]
fn reader_cancellation_ready_probe() {
    {
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        output
            .write_all(b"!")
            .expect("write the reader-cancellation readiness byte");
        output
            .flush()
            .expect("flush the reader-cancellation readiness byte");
    }
    std::thread::sleep(Duration::from_secs(5));
}

#[test]
#[ignore = "executed as a successful high-output subprocess"]
fn bounded_success_output_probe() {
    std::io::stdout()
        .lock()
        .write_all(&[b'x'; 4096])
        .expect("write successful stdout probe data");
}

#[test]
#[ignore = "executed with a grandchild retaining inherited output pipes"]
fn descendants_hold_pipe_probe() {
    let child = probe_command("probes::hold_pipe_open_probe")
        .expect("the pipe-holding probe command should be available")
        .spawn()
        .expect("the pipe-holding descendant should start");
    // The probe process exits immediately; its parent process group owns cleanup.
    std::mem::forget(child);
}

#[test]
#[ignore = "executed as a root with a closed-pipe writing descendant"]
fn closed_pipe_outer_probe() {
    let directory = coordination_directory().expect("the coordination directory should open");
    let child = probe_command("probes::closed_pipe_writer_probe")
        .expect("the closed-pipe writer command should be available")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the closed-pipe writer should start");
    directory
        .write("writer-pid", child.id().to_string())
        .expect("the writer PID should be recorded");
    wait_for_marker(&directory, "writer-started")
        .expect("the writer should signal readiness before the root exits");
    std::mem::forget(child);
}

#[test]
#[ignore = "executed as a writer that closes both output streams"]
fn closed_pipe_writer_probe() {
    let directory = coordination_directory().expect("the coordination directory should open");
    directory
        .write("writer-started", b"ready")
        .expect("writer readiness should be recorded");
    std::thread::sleep(Duration::from_secs(3));
    directory
        .write("writer-finished", b"finished")
        .expect("writer completion should be recorded");
}

#[test]
#[ignore = "executed as a test-owned process outside the supervised group"]
fn external_sentinel_probe() {
    std::thread::sleep(Duration::from_secs(3));
}

#[test]
#[ignore = "executed as a descendant that retains inherited output pipes"]
fn hold_pipe_open_probe() {
    std::thread::sleep(Duration::from_secs(3));
}

fn coordination_directory() -> std::io::Result<Dir> {
    let path = std::env::var("ORTHO452_TREE_PROBE_DIRECTORY")
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Dir::open_ambient_dir(Utf8Path::new(&path), ambient_authority())
}

fn wait_for_marker(directory: &Dir, marker: &str) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while std::time::Instant::now() < deadline {
        match directory.read_to_string(marker) {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "writer readiness marker did not appear",
    ))
}
