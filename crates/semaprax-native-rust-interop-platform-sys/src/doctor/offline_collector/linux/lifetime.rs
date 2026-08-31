//! Shared ownership decisions; only Lifetime's native operations own resources.
use super::capture::{Exit, Operations as CaptureOperations};
use std::time::{Duration, Instant};

mod operations;
use operations::{Descriptor, Native, Operations, Wait};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Unvalidated,
    Owned,
    Reaped,
    Closed,
}

// No descriptors and no Drop: scripted state cannot perform physical cleanup.
// Native Lifetime alone owns the externally transferred fixed descriptor set.
struct State {
    phase: Phase,
}
impl State {
    fn new() -> Self {
        Self {
            phase: Phase::Unvalidated,
        }
    }
    fn authenticate(&mut self, operations: &mut impl Operations) -> Result<(), ()> {
        if self.phase != Phase::Unvalidated {
            return Err(());
        }
        if let Some(exit) = operations.wait(Wait::Observe).map_err(|_| ())? {
            if exit.pid <= 0
                || exit.signo != libc::SIGCHLD
                || !matches!(
                    exit.code,
                    libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
                )
            {
                return Err(());
            }
        }
        self.phase = Phase::Owned;
        Ok(())
    }
    fn observe(&mut self, operations: &mut impl Operations) -> Result<Option<Exit>, ()> {
        if self.phase != Phase::Owned {
            return Err(());
        }
        operations.wait(Wait::Observe).map_err(|_| ())
    }
    fn reap(&mut self, operations: &mut impl Operations) -> Result<Option<Exit>, ()> {
        if self.phase != Phase::Owned {
            return Err(());
        }
        let exit = operations.wait(Wait::Reap).map_err(|_| ())?;
        // Record consumption before the capture caller compares terminal data.
        // Even a mismatching returned event must never authorize another reap.
        if exit.is_some() {
            self.phase = Phase::Reaped;
        }
        Ok(exit)
    }
    fn complete(&mut self, operations: &mut impl Operations) {
        // Establish the close precondition before touching any handle. This
        // includes the pidfd needed by abort while the worker is still owned.
        if self.phase != Phase::Reaped {
            self.abort(operations);
        }
        for descriptor in [
            Descriptor::Request,
            Descriptor::Bundle,
            Descriptor::Reply,
            Descriptor::Stderr,
            Descriptor::Pidfd,
        ] {
            if operations.close(descriptor).is_err() {
                operations.stop();
            }
        }
        self.phase = Phase::Closed;
    }
    fn abort(&mut self, operations: &mut impl Operations) -> ! {
        // Unvalidated descriptors confer no signal authority. Reaped/closed
        // scopes never receive another wait, signal, or close attempt here.
        if self.phase == Phase::Owned {
            let deadline = operations
                .elapsed()
                .checked_add(Duration::from_secs(5))
                .unwrap_or_else(|| operations.stop());
            if let Err(error) = operations.kill() {
                if error != libc::ESRCH {
                    operations.stop();
                }
            }
            loop {
                match self.reap(operations) {
                    Ok(Some(_)) => break,
                    Ok(None) => {}
                    Err(()) => operations.stop(),
                }
                if operations.elapsed() >= deadline {
                    operations.stop();
                }
                operations.pause();
            }
        }
        // Successful forced reap still does not prove tool namespace closure.
        // No ordinary error/result is returned; the provisioner reconciles it.
        operations.stop()
    }
    fn cleanup_on_drop(&mut self, operations: &mut impl Operations) {
        if self.phase != Phase::Closed {
            self.abort(operations);
        }
    }
}

pub(super) struct Lifetime {
    state: State,
    native: Native,
}
impl Lifetime {
    pub(super) fn new(origin: Instant) -> Self {
        Self {
            state: State::new(),
            native: Native::new(origin),
        }
    }
    pub(super) fn authenticate(&mut self) -> Result<(), ()> {
        self.state.authenticate(&mut self.native)
    }
    pub(super) fn require_time(&mut self) -> Result<(), ()> {
        if self.native.elapsed() >= Duration::from_secs(60) {
            Err(())
        } else {
            Ok(())
        }
    }
    pub(super) fn complete(&mut self) {
        self.state.complete(&mut self.native);
    }
    pub(super) fn abort(&mut self) -> ! {
        self.state.abort(&mut self.native)
    }
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.state.cleanup_on_drop(&mut self.native);
    }
}

impl CaptureOperations for Lifetime {
    fn read(&mut self, stream: usize, bytes: &mut [u8; 8192]) -> Result<Option<usize>, ()> {
        self.native.read(stream, bytes)
    }
    fn observe(&mut self) -> Result<Option<Exit>, ()> {
        self.state.observe(&mut self.native)
    }
    fn reap(&mut self) -> Result<Option<Exit>, ()> {
        self.state.reap(&mut self.native)
    }
    fn elapsed(&mut self) -> Duration {
        self.native.elapsed()
    }
    fn pause(&mut self) {
        self.native.pause();
    }
}
