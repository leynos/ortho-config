//! Cancellable Windows pipe reads bounded by the bytes currently available.

use std::io::{self, Read};
use std::os::windows::io::AsRawHandle;
use windows::Win32::Foundation::{ERROR_BROKEN_PIPE, HANDLE};
use windows::Win32::System::Pipes::PeekNamedPipe;
use windows::core::HRESULT;

pub(super) enum ReadProgress {
    Bytes(usize),
    Pending,
    End,
}

pub(crate) trait ReadHandle: AsRawHandle + Read + Send {}

impl<PipeHandle> ReadHandle for PipeHandle where PipeHandle: AsRawHandle + Read + Send {}

pub(crate) trait Input: ReadHandle + 'static {}

impl<PipeHandle> Input for PipeHandle where PipeHandle: ReadHandle + 'static {}

pub(super) struct Pipe {
    pipe: Box<dyn ReadHandle>,
}

impl Pipe {
    pub(super) fn new<PipeHandle: Input>(pipe: PipeHandle) -> Self {
        Self {
            pipe: Box::new(pipe),
        }
    }

    pub(super) fn read(&mut self, buffer: &mut [u8]) -> io::Result<ReadProgress> {
        let mut available = 0_u32;
        let handle = HANDLE(self.pipe.as_raw_handle());
        // SAFETY: the reader owns the open child pipe handle for this call;
        // PeekNamedPipe only observes available bytes and writes to `available`.
        let peek_result =
            unsafe { PeekNamedPipe(handle, None, 0, None, Some(&raw mut available), None) };
        match peek_result {
            Ok(()) if available == 0 => Ok(ReadProgress::Pending),
            Ok(()) => read_available(self.pipe.as_mut(), available, buffer),
            Err(error) if error.code() == HRESULT::from_win32(ERROR_BROKEN_PIPE.0) => {
                Ok(ReadProgress::End)
            }
            Err(error) => Err(io::Error::other(format!("PeekNamedPipe failed: {error}"))),
        }
    }
}

fn read_available(
    pipe: &mut dyn ReadHandle,
    available: u32,
    buffer: &mut [u8],
) -> io::Result<ReadProgress> {
    let available_bytes = usize::try_from(available)
        .map_err(|_| io::Error::other("available pipe byte count exceeds usize"))?;
    let read_limit = available_bytes.min(buffer.len());
    let read_buffer = buffer
        .get_mut(..read_limit)
        .ok_or_else(|| io::Error::other("available pipe bytes exceed the read buffer"))?;
    match pipe.read(read_buffer) {
        Ok(0) => Ok(ReadProgress::End),
        Ok(bytes_read) => Ok(ReadProgress::Bytes(bytes_read)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(ReadProgress::Pending)
        }
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(ReadProgress::End),
        Err(error) => Err(error),
    }
}
