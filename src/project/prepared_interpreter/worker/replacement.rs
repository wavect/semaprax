//! Same-worker revision transaction. No prepared state is reused across subjects.
use super::{
    prepare_closures, request_error, worker_error, Diagnostic, PreparedClosures, ProjectRevision,
};
use std::sync::{mpsc, Arc};

pub(super) struct WorkerState {
    pub(super) revision: Arc<ProjectRevision>,
    pub(super) closures: PreparedClosures,
}

pub(super) struct ReplacementRequest {
    pub(super) expected: String,
    pub(super) revision: Arc<ProjectRevision>,
    pub(super) reply: mpsc::SyncSender<Result<(), Vec<Diagnostic>>>,
    #[cfg(test)]
    pub(super) hook: Option<TestHook>,
}

pub(super) fn validate_expected_revision(expected: &str) -> Result<(), Vec<Diagnostic>> {
    // Check length before copying any caller-controlled bytes into a request.
    if expected.len() != 71
        || !expected.strip_prefix("sha256:").is_some_and(|hex| {
            hex.bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
    {
        return Err(vec![request_error(
            "expected Project revision must be sha256: followed by 64 lowercase hex digits"
                .to_owned(),
        )]);
    }
    Ok(())
}

/// False is terminal: neither a panic nor a lost acknowledgement is rollback.
pub(super) fn process(state: &mut WorkerState, request: ReplacementRequest) -> bool {
    let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if request.expected != state.revision.project_revision() {
            return Err(vec![request_error(
                "prepared replacement expected Project revision is stale".to_owned(),
            )]);
        }
        #[cfg(test)]
        if let Some(hook) = &request.hook {
            hook.before_prepare();
        }
        let candidate = WorkerState {
            closures: prepare_closures(&request.revision)?,
            revision: Arc::clone(&request.revision),
        };
        // Whole-state pivot only after both closures and all source origins
        // pass. Dropping the old state is part of the caught handoff too.
        let previous = std::mem::replace(state, candidate);
        drop(previous);
        #[cfg(test)]
        if let Some(hook) = &request.hook {
            hook.after_commit();
        }
        Ok(())
    }));
    match attempted {
        Ok(result) => request.reply.send(result).is_ok(),
        Err(_) => {
            let _ = request.reply.send(Err(vec![worker_error(
                "prepared interpreter replacement panicked and is now terminal",
            )]));
            false
        }
    }
}

#[cfg(test)]
pub(super) enum TestHook {
    Pause {
        entered: mpsc::SyncSender<std::thread::ThreadId>,
        resume: mpsc::Receiver<()>,
    },
    PanicBeforePrepare,
    PanicAfterCommit,
}

#[cfg(test)]
impl TestHook {
    fn before_prepare(&self) {
        match self {
            Self::Pause { entered, resume } => {
                entered.send(std::thread::current().id()).unwrap();
                resume.recv().unwrap();
            }
            Self::PanicBeforePrepare => panic!("injected replacement preparation panic"),
            _ => {}
        }
    }

    fn after_commit(&self) {
        if matches!(self, Self::PanicAfterCommit) {
            panic!("injected replacement handoff panic");
        }
    }
}
