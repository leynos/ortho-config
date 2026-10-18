//! Nonblocking Unix pipe reads for bounded subprocess output supervision.

use nix::fcntl::{FcntlArg, OFlag, fcntl};
use std::io::{self, Read};
use std::os::fd::AsFd;

pub(super) trait Input: AsFd + Read + Send + 'static {}

impl<PipeHandle> Input for PipeHandle where PipeHandle: AsFd + Read + Send + 'static {}

pub(super) enum ReadProgress {
    Bytes(usize),
    Pending,
    End,
}

pub(super) struct Pipe {
    pipe: Box<dyn Read + Send>,
}

impl Pipe {
    pub(super) fn new<PipeHandle: Input>(pipe: PipeHandle) -> io::Result<Self> {
        let descriptor_flags = fcntl(pipe.as_fd(), FcntlArg::F_GETFL).map_err(io::Error::from)?;
        let nonblocking_flags = OFlag::from_bits_truncate(descriptor_flags) | OFlag::O_NONBLOCK;
        fcntl(pipe.as_fd(), FcntlArg::F_SETFL(nonblocking_flags)).map_err(io::Error::from)?;
        Ok(Self {
            pipe: Box::new(pipe),
        })
    }

    pub(super) fn read(&mut self, buffer: &mut [u8]) -> io::Result<ReadProgress> {
        match self.pipe.read(buffer) {
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
            Err(error) => Err(error),
        }
    }
}
