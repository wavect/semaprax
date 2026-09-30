//! Durable physical release; observers cannot replace the real release loop.
use super::*;

pub(crate) struct UnpublishedOwnedFrameResult {
    pub(super) plan: CheckedOwnedFramePlan,
    pub(super) root: Value,
}
// Intentionally no semantic Drop. Until claim ACK this is invocation backing.
pub(crate) struct DurableRelease {
    pub(crate) unpublished: Option<UnpublishedOwnedFrameResult>,
    pub(crate) failure: Option<OwnedFrameFailure>,
    pub(crate) observations: Vec<(FinalizeAction, bool)>,
}

pub(crate) fn settle(
    terminal: OwnedFrameStagedTerminal,
    observer: &mut dyn FnMut(&FinalizeAction) -> bool,
) -> Result<DurableRelease, OwnedFrameSettlementRejection> {
    settle_guarded(terminal, observer, &mut || true)
}
pub(crate) fn settle_guarded(
    terminal: OwnedFrameStagedTerminal,
    observer: &mut dyn FnMut(&FinalizeAction) -> bool,
    current_authority: &mut dyn FnMut() -> bool,
) -> Result<DurableRelease, OwnedFrameSettlementRejection> {
    if !current_authority() {
        return Err(OwnedFrameSettlementRejection {
            terminal,
            diagnostic: rejected("durable lease authority changed"),
        });
    }
    if !exclusive(&terminal.root) {
        return Err(OwnedFrameSettlementRejection {
            terminal,
            diagnostic: rejected("unaccounted root/leaf alias"),
        });
    }
    if terminal.failure.is_none() {
        return Ok(DurableRelease {
            unpublished: Some(UnpublishedOwnedFrameResult {
                plan: terminal.plan,
                root: terminal.root,
            }),
            failure: None,
            observations: Vec::new(),
        });
    }
    let actions = if terminal.provisional {
        terminal.plan.liveness().result_disposal.clone()
    } else {
        terminal.plan.liveness().failure_cleanup.clone()
    };
    let OwnedFrameStagedTerminal {
        plan,
        root,
        failure,
        provisional,
    } = terminal;
    let mut root = Some(root);
    let mut observations = Vec::new();
    let released = release_guarded(&mut root, &actions, current_authority, |action| {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observer(action)))
            .unwrap_or(false);
        observations.push((action.clone(), outcome));
    });
    match released {
        Ok(_) => Ok(DurableRelease {
            unpublished: None,
            failure,
            observations,
        }),
        Err(diagnostic) => Err(OwnedFrameSettlementRejection {
            terminal: OwnedFrameStagedTerminal {
                plan,
                root: root.expect("pre-release refusal retains root"),
                failure,
                provisional,
            },
            diagnostic,
        }),
    }
}
