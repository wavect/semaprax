//! One shared collector state machine; scripted outcomes carry no OS authority.
use super::wire;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Exit {
    pub(super) pid: i32,
    pub(super) signo: i32,
    pub(super) code: i32,
    pub(super) status: i32,
}
impl Exit {
    fn success(self) -> bool {
        self.pid > 0
            && self.signo == libc::SIGCHLD
            && self.code == libc::CLD_EXITED
            && self.status == 0
    }
}

pub(super) trait Operations {
    fn read(&mut self, stream: usize, bytes: &mut [u8; 8192]) -> Result<Option<usize>, ()>;
    fn observe(&mut self) -> Result<Option<Exit>, ()>;
    fn reap(&mut self) -> Result<Option<Exit>, ()>;
    fn elapsed(&mut self) -> Duration;
    fn pause(&mut self);
}

pub(super) fn collect(operations: &mut impl Operations) -> Result<Vec<u8>, ()> {
    let mut reply = Vec::new();
    reply
        .try_reserve_exact(wire::MAX_REPLY_BYTES)
        .map_err(|_| ())?;
    let mut ended = [false; 2];
    let mut reaped = false;
    let mut total = 0_usize;
    loop {
        if operations.elapsed() >= Duration::from_secs(60) {
            return Err(());
        }
        for (stream, eof) in ended.iter_mut().enumerate() {
            if *eof {
                continue;
            }
            let mut bytes = [0_u8; 8192];
            let Some(count) = operations.read(stream, &mut bytes)? else {
                continue;
            };
            if count > bytes.len() {
                return Err(());
            }
            if count == 0 {
                *eof = true;
                continue;
            }
            total = total.checked_add(count).ok_or(())?;
            if total > wire::MAX_REPLY_BYTES + 65_536 || stream == 1 {
                return Err(());
            }
            let length = reply.len().checked_add(count).ok_or(())?;
            if length > wire::MAX_REPLY_BYTES || length > reply.capacity() {
                return Err(());
            }
            reply.extend_from_slice(&bytes[..count]);
        }
        if !reaped {
            if let Some(observed) = operations.observe()? {
                if !observed.success() {
                    return Err(());
                }
                // WNOWAIT pins the same terminal event. No competing waiter is
                // admitted; an absent/different reap is an ownership failure.
                if operations.reap()? != Some(observed) {
                    return Err(());
                }
                reaped = true;
            }
        }
        if reaped && ended.iter().all(|eof| *eof) {
            return Ok(reply);
        }
        operations.pause();
    }
}

#[cfg(test)]
mod tests;
