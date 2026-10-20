//! Unix process-group ownership with non-reaping root observation.

#[cfg(target_os = "linux")]
use cap_std::ambient_authority;
#[cfg(target_os = "linux")]
use cap_std::fs_utf8::Dir;
#[cfg(not(target_os = "linux"))]
use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku"
))]
use nix::sys::wait::{WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use process_wrap::std::{ChildWrapper, CommandWrap, CommandWrapper};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus};

#[derive(Debug)]
struct ProcessGroupOwner;

impl CommandWrapper for ProcessGroupOwner {
    fn pre_spawn(&mut self, command: &mut Command, _core: &CommandWrap) -> io::Result<()> {
        command.process_group(0);
        Ok(())
    }

    fn wrap_child(
        &mut self,
        child: Box<dyn ChildWrapper>,
        _core: &CommandWrap,
    ) -> io::Result<Box<dyn ChildWrapper>> {
        let raw_process_id = child.id();
        let process_id = i32::try_from(raw_process_id)
            .map_err(|_| io::Error::other("child PID exceeds Unix process-group range"))?;
        let group = Pid::from_raw(process_id);
        Ok(Box::new(ProcessGroupChild {
            child: Some(child),
            group,
            root_exit_observed: false,
            termination_requested: false,
            status: None,
            tree_completed: false,
        }))
    }
}

pub(super) fn configure(command: &mut CommandWrap) {
    command.wrap(ProcessGroupOwner);
}

#[derive(Debug)]
struct ProcessGroupChild {
    child: Option<Box<dyn ChildWrapper>>,
    group: Pid,
    root_exit_observed: bool,
    termination_requested: bool,
    status: Option<ExitStatus>,
    tree_completed: bool,
}

impl ProcessGroupChild {
    const fn should_request_group_termination(&self) -> bool {
        self.status.is_none() && !self.termination_requested
    }

    fn request_group_termination(&mut self) -> io::Result<()> {
        if self.termination_requested {
            return Ok(());
        }
        if self.status.is_some() {
            return Err(io::Error::other(
                "refusing to signal a process group after reaping its leader",
            ));
        }
        match killpg(self.group, Some(Signal::SIGKILL)) {
            Ok(()) | Err(nix::errno::Errno::ESRCH) => {
                self.termination_requested = true;
                Ok(())
            }
            Err(error) => Err(io::Error::from(error)),
        }
    }

    fn reap_root_after_termination(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.status.is_none() {
            let child = self
                .child
                .as_mut()
                .ok_or_else(|| io::Error::other("owned root child is unavailable"))?;
            self.status = child.try_wait()?;
        }
        Ok(self.status)
    }
}

impl ChildWrapper for ProcessGroupChild {
    fn inner(&self) -> &dyn ChildWrapper {
        self.child.as_deref().map_or(self, |child| child)
    }

    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        match self.child {
            Some(ref mut child) => child.as_mut(),
            None => self,
        }
    }

    fn into_inner(mut self: Box<Self>) -> Box<dyn ChildWrapper> {
        match self.child.take() {
            Some(child) => {
                drop(self);
                child
            }
            None => self,
        }
    }

    fn start_kill(&mut self) -> io::Result<()> {
        self.request_group_termination()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.tree_completed {
            return Ok(self.status);
        }
        if !self.root_exit_observed {
            self.root_exit_observed = root_exit_is_waitable(self.group)?;
        }
        if !self.root_exit_observed {
            return Ok(None);
        }
        self.request_group_termination()?;
        let Some(status) = self.reap_root_after_termination()? else {
            return Ok(None);
        };
        if !group_has_writers(self.group)? {
            self.tree_completed = true;
            return Ok(Some(status));
        }
        Ok(None)
    }

    fn wait(&mut self) -> io::Result<ExitStatus> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unbounded process-tree wait is unsupported; poll under an operation deadline",
        ))
    }
}

impl Drop for ProcessGroupChild {
    fn drop(&mut self) {
        if self.should_request_group_termination()
            && let Err(error) = self.request_group_termination()
        {
            tracing::warn!(
                process_group = self.group.as_raw(),
                error = %error,
                "failed to terminate owned process group during wrapper drop"
            );
        }
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku"
))]
fn root_exit_is_waitable(group: Pid) -> io::Result<bool> {
    match nix::sys::wait::waitid(
        nix::sys::wait::Id::Pid(group),
        WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
    )
    .map_err(io::Error::from)?
    {
        WaitStatus::StillAlive => Ok(false),
        WaitStatus::Exited(_, _) | WaitStatus::Signaled(_, _, _) => Ok(true),
        _ => Err(io::Error::other("unexpected waitid status for owned root")),
    }
}

#[cfg(target_os = "macos")]
fn root_exit_is_waitable(group: Pid) -> io::Result<bool> {
    let mut information_slot = std::mem::MaybeUninit::<nix::libc::siginfo_t>::zeroed();
    // SAFETY: waitid writes a siginfo_t into the valid output pointer. On
    // success, macOS initializes si_pid and si_code, the only fields read.
    let result = unsafe {
        nix::libc::waitid(
            nix::libc::P_PID,
            group.as_raw().try_into().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid owned process ID")
            })?,
            information_slot.as_mut_ptr(),
            nix::libc::WEXITED | nix::libc::WNOHANG | nix::libc::WNOWAIT,
        )
    };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful waitid call initialized the siginfo_t output.
    let process_information = unsafe { information_slot.assume_init() };
    if process_information.si_pid == 0 {
        Ok(false)
    } else if process_information.si_pid == group.as_raw() {
        Ok(matches!(
            process_information.si_code,
            nix::libc::CLD_EXITED | nix::libc::CLD_KILLED | nix::libc::CLD_DUMPED
        ))
    } else {
        Err(io::Error::other("waitid reported a different process"))
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "haiku",
    target_os = "macos"
)))]
fn root_exit_is_waitable(_group: Pid) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "non-reaping root observation is unavailable on this Unix target",
    ))
}

#[cfg(target_os = "linux")]
fn group_has_writers(group: Pid) -> io::Result<bool> {
    let proc_directory = Dir::open_ambient_dir("/proc", ambient_authority())?;
    for entry_result in proc_directory.read_dir(".")? {
        let directory_entry = entry_result?;
        let name = directory_entry.file_name()?;
        let Ok(process_id) = name.parse::<u32>() else {
            continue;
        };
        let relative_stat = format!("{process_id}/stat");
        let Some(stat) = read_proc_stat(|| proc_directory.read_to_string(relative_stat.as_str()))?
        else {
            continue;
        };
        if proc_stat_has_writer(&stat, group.as_raw())? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(target_os = "linux")]
pub(crate) fn read_proc_stat(
    read: impl FnOnce() -> io::Result<String>,
) -> io::Result<Option<String>> {
    match read() {
        Ok(stat) => Ok(Some(stat)),
        Err(error) if process_entry_disappeared(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "linux")]
fn process_entry_disappeared(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(nix::libc::ESRCH)
}

#[cfg(not(target_os = "linux"))]
fn group_has_writers(group: Pid) -> io::Result<bool> {
    match killpg(group, None) {
        Ok(()) => Ok(true),
        Err(Errno::ESRCH) => Ok(false),
        Err(error) => Err(io::Error::from(error)),
    }
}

#[cfg(target_os = "linux")]
fn proc_stat_has_writer(stat: &str, group: i32) -> io::Result<bool> {
    let (_, remaining_stat_fields) = stat
        .rsplit_once(')')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed proc stat"))?;
    let mut fields = remaining_stat_fields.split_whitespace();
    let state = fields
        .next()
        .and_then(|value| value.chars().next())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing proc state"))?;
    let _parent = fields
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing proc parent ID"))?;
    let process_group = fields
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing proc group ID"))?;
    Ok(process_group == group && !matches!(state, 'Z' | 'X'))
}
