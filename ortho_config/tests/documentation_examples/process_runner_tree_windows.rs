//! Windows Job Object ownership with active-process completion checks.

use process_wrap::std::{ChildWrapper, CommandWrap, CommandWrapper};
use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Command, ExitStatus};
use std::sync::{Arc, Mutex, MutexGuard};
use windows::Win32::Foundation::{
    ERROR_INVALID_PARAMETER, ERROR_NO_MORE_FILES, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
};
use windows::Win32::System::Threading::{
    CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME, WaitForSingleObject,
};
use windows::core::{HRESULT, PCWSTR};

#[derive(Debug, Clone)]
struct JobObjectOwner {
    job: Arc<Mutex<Option<OwnedHandle>>>,
}

impl JobObjectOwner {
    fn new() -> Self {
        Self {
            job: Arc::new(Mutex::new(None)),
        }
    }
}

impl CommandWrapper for JobObjectOwner {
    fn pre_spawn(&mut self, command: &mut Command, _core: &CommandWrap) -> io::Result<()> {
        *lock_job(&self.job) = Some(create_kill_on_close_job()?);
        command.creation_flags(CREATE_SUSPENDED.0);
        Ok(())
    }

    fn wrap_child(
        &mut self,
        child: Box<dyn ChildWrapper>,
        _core: &CommandWrap,
    ) -> io::Result<Box<dyn ChildWrapper>> {
        let job = lock_job(&self.job)
            .take()
            .ok_or_else(|| io::Error::other("process-tree Job Object was not created"))?;
        let process_id = child.id();
        let process_handle = HANDLE(child.inner_child().as_raw_handle().cast());
        // SAFETY: the retained private job and suspended child belong to this spawn.
        unsafe { AssignProcessToJobObject(as_handle(&job), process_handle) }
            .map_err(io::Error::other)?;
        resume_root_threads(process_id)?;
        Ok(Box::new(JobObjectChild {
            child,
            job,
            termination_requested: false,
            status: None,
            tree_completed: false,
        }))
    }
}

pub(super) fn configure(command: &mut CommandWrap) {
    command.wrap(JobObjectOwner::new());
}

fn lock_job(job: &Mutex<Option<OwnedHandle>>) -> MutexGuard<'_, Option<OwnedHandle>> {
    job.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn create_kill_on_close_job() -> io::Result<OwnedHandle> {
    // SAFETY: a null name requests a private unnamed Job Object.
    let raw_job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(io::Error::other)?;
    // SAFETY: CreateJobObjectW returned a valid owned kernel handle.
    let job = unsafe { OwnedHandle::from_raw_handle(raw_job.0.cast()) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let information_size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(io::Error::other)?;
    // SAFETY: `limits` is initialized and its size matches the selected information class.
    unsafe {
        SetInformationJobObject(
            as_handle(&job),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            information_size,
        )
    }
    .map_err(io::Error::other)?;
    Ok(job)
}

fn resume_root_threads(process_id: u32) -> io::Result<()> {
    // SAFETY: the Toolhelp snapshot flags request only the system thread list.
    let raw_snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.map_err(io::Error::other)?;
    // SAFETY: the successful snapshot call returned an owned kernel handle.
    let snapshot_handle = unsafe { OwnedHandle::from_raw_handle(raw_snapshot.0.cast()) };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).map_err(io::Error::other)?,
        ..THREADENTRY32::default()
    };
    // SAFETY: `entry` is writable and its dwSize is initialized as required.
    match unsafe { Thread32First(as_handle(&snapshot_handle), &mut entry) } {
        Ok(()) => resume_matching_threads(process_id, snapshot_handle, entry),
        Err(error) if is_no_more_threads(&error) => Err(io::Error::other(
            "suspended child has no enumerated thread to resume",
        )),
        Err(error) => Err(io::Error::other(error)),
    }
}

fn resume_matching_threads(
    process_id: u32,
    snapshot: OwnedHandle,
    mut entry: THREADENTRY32,
) -> io::Result<()> {
    let mut resumed_root_thread = false;
    loop {
        if entry.th32OwnerProcessID == process_id {
            resume_thread(entry.th32ThreadID)?;
            resumed_root_thread = true;
        }
        // SAFETY: `entry` remains writable and initialized for each enumeration step.
        match unsafe { Thread32Next(as_handle(&snapshot), &mut entry) } {
            Ok(()) => {}
            Err(error) if is_no_more_threads(&error) => break,
            Err(error) => return Err(io::Error::other(error)),
        }
    }
    if resumed_root_thread {
        Ok(())
    } else {
        Err(io::Error::other(
            "suspended child thread was not present in the snapshot",
        ))
    }
}

fn resume_thread(thread_id: u32) -> io::Result<()> {
    // SAFETY: OpenThread requests only the right needed to resume this enumerated thread.
    let thread =
        unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, thread_id) }.map_err(io::Error::other)?;
    // SAFETY: OpenThread returned an owned handle to the enumerated thread.
    let thread = unsafe { OwnedHandle::from_raw_handle(thread.0.cast()) };
    // SAFETY: the handle remains valid for the duration of ResumeThread.
    let previous_suspend_count = unsafe { ResumeThread(as_handle(&thread)) };
    if previous_suspend_count == u32::MAX {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn is_no_more_threads(error: &windows::core::Error) -> bool {
    error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0)
}

fn as_handle(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle().cast())
}

#[derive(Debug)]
struct JobObjectChild {
    child: Box<dyn ChildWrapper>,
    job: OwnedHandle,
    termination_requested: bool,
    status: Option<ExitStatus>,
    tree_completed: bool,
}

impl JobObjectChild {
    fn active_processes(&self) -> io::Result<u32> {
        let mut information = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let information_size = u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>())
            .map_err(io::Error::other)?;
        // SAFETY: `information` is initialized writable storage of the expected class size.
        unsafe {
            QueryInformationJobObject(
                Some(as_handle(&self.job)),
                JobObjectBasicAccountingInformation,
                (&mut information as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                information_size,
                None,
            )
        }
        .map_err(io::Error::other)?;
        Ok(information.ActiveProcesses)
    }

    fn terminate_remaining(&mut self) -> io::Result<()> {
        if self.termination_requested {
            return Ok(());
        }
        if self.active_processes()? > 0 {
            // SAFETY: the retained Job Object handle names only this command's owned tree.
            unsafe { TerminateJobObject(as_handle(&self.job), 1) }.map_err(io::Error::other)?;
        }
        self.termination_requested = true;
        Ok(())
    }
}

impl ChildWrapper for JobObjectChild {
    fn inner(&self) -> &dyn ChildWrapper {
        self.child.as_ref()
    }

    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        self.child.as_mut()
    }

    fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
        self.child
    }

    fn start_kill(&mut self) -> io::Result<()> {
        self.terminate_remaining()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.tree_completed {
            return Ok(self.status);
        }
        if self.status.is_none() {
            self.status = self.child.try_wait()?;
        }
        let Some(status) = self.status else {
            return Ok(None);
        };
        self.terminate_remaining()?;
        if self.active_processes()? == 0 {
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

pub(super) fn process_is_active(process_id: u32) -> io::Result<bool> {
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };

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
        Err(error) => return Err(io::Error::other(error)),
    };
    // SAFETY: OpenProcess returned an owned process handle.
    let process_handle = unsafe { OwnedHandle::from_raw_handle(raw_process.0.cast()) };
    // SAFETY: the handle remains valid and a zero timeout never blocks.
    let state = unsafe { WaitForSingleObject(as_handle(&process_handle), 0) };
    if state == WAIT_TIMEOUT {
        Ok(true)
    } else if state == WAIT_OBJECT_0 {
        Ok(false)
    } else if state == WAIT_FAILED {
        Err(io::Error::last_os_error())
    } else {
        Err(io::Error::other(format!(
            "unexpected process wait result: {}",
            state.0
        )))
    }
}
