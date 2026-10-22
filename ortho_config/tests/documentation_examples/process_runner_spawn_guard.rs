//! Kill a spawned root child if process-tree wrapper setup fails.

use process_wrap::std::{ChildWrapper, CommandWrap, CommandWrapper};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct State {
    armed: AtomicBool,
    root_child_seen: AtomicBool,
    kill_attempted: AtomicBool,
    kill_error: Mutex<Option<String>>,
}

/// Shares an armed kill attempt with the child wrapper installed before tree ownership.
#[derive(Debug, Clone)]
pub(crate) struct SpawnGuard {
    state: Arc<State>,
}

impl SpawnGuard {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(State {
                armed: AtomicBool::new(true),
                ..State::default()
            }),
        }
    }

    pub(crate) fn install(&self, command: &mut CommandWrap) {
        command.wrap(GuardWrapper {
            state: Arc::clone(&self.state),
        });
    }

    pub(crate) fn disarm(&self) {
        self.state.armed.store(false, Ordering::Release);
    }

    pub(crate) fn root_child_seen(&self) -> bool {
        self.state.root_child_seen.load(Ordering::Acquire)
    }

    pub(crate) fn kill_attempted(&self) -> bool {
        self.state.kill_attempted.load(Ordering::Acquire)
    }

    pub(crate) fn kill_error(&self) -> Option<String> {
        if !self.kill_attempted() {
            return None;
        }
        self.state
            .kill_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[derive(Debug)]
struct GuardWrapper {
    state: Arc<State>,
}

impl CommandWrapper for GuardWrapper {
    fn wrap_child(
        &mut self,
        child: Box<dyn ChildWrapper>,
        _command: &CommandWrap,
    ) -> io::Result<Box<dyn ChildWrapper>> {
        self.state.root_child_seen.store(true, Ordering::Release);
        Ok(Box::new(GuardedChild {
            child: Some(child),
            state: Arc::clone(&self.state),
        }))
    }
}

#[derive(Debug)]
struct GuardedChild {
    child: Option<Box<dyn ChildWrapper>>,
    state: Arc<State>,
}

impl GuardedChild {
    fn request_kill(&mut self) {
        if self.state.armed.load(Ordering::Acquire) {
            self.state.kill_attempted.store(true, Ordering::Release);
            if let Some(child) = self.child.as_mut()
                && let Err(error) = child.start_kill()
            {
                *self
                    .state
                    .kill_error
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error.to_string());
            }
        }
    }
}

impl Drop for GuardedChild {
    fn drop(&mut self) {
        self.request_kill();
    }
}

impl ChildWrapper for GuardedChild {
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
        self.request_kill();
        match self.child.take() {
            Some(child) => {
                drop(self);
                child
            }
            None => self,
        }
    }
}
