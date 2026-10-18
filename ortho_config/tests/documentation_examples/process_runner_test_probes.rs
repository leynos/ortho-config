//! Controlled child processes for subprocess-supervision regressions.

use super::probe_command;
use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use std::io::Write;
use std::process::Stdio;
use std::time::Duration;

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
