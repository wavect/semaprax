//! The sole causal phase validator for source-mode entries.

use super::*;

#[derive(Clone)]
enum Phase {
    Start,
    BeforeTurn {
        turn: u32,
        scope: Option<u32>,
    },
    NeedAttempt {
        turn: u32,
        attempt: u32,
        prior_attempt: Option<u32>,
    },
    AwaitAttempt {
        turn: u32,
        attempt: u32,
        response_limit: usize,
    },
    Decode {
        turn: u32,
        attempt: u32,
    },
    NeedAuthorization {
        turn: u32,
        attempt: u32,
    },
    NeedEffect {
        turn: u32,
        attempt: u32,
    },
    AwaitEffect {
        turn: u32,
        attempt: u32,
        operation: String,
    },
    NeedTransition {
        turn: u32,
        attempt: u32,
    },
    NeedStop {
        turn: Option<u32>,
        attempt: Option<u32>,
        status: SourceStopStatus,
    },
    Candidate {
        turn: u32,
        attempt: u32,
        case: SourceTransitionCase,
        carrier: String,
    },
    Terminal {
        turn: Option<u32>,
        status: SourceTerminalStatus,
        carrier: Option<String>,
    },
    Done,
}

fn is_policy_stop(reason: SourceStopReason) -> bool {
    matches!(
        reason,
        SourceStopReason::Cancelled
            | SourceStopReason::DeadlineExceeded
            | SourceStopReason::BudgetExhausted
            | SourceStopReason::StageRefused
    )
}

fn stop_matches(
    actual_scope: (Option<u32>, Option<u32>),
    status: SourceStopStatus,
    reason: SourceStopReason,
    expected_scope: (Option<u32>, Option<u32>),
    expected_status: Option<SourceStopStatus>,
    policy_only: bool,
) -> bool {
    actual_scope == expected_scope
        && status == reason.status()
        && expected_status.is_none_or(|expected| status == expected)
        && (!policy_only || is_policy_stop(reason))
}

fn valid_operation(operation: &str) -> bool {
    !operation.is_empty() && operation.len() <= 1_024 && !operation.chars().any(char::is_control)
}

fn transition_status(case: SourceTransitionCase) -> Option<SourceTerminalStatus> {
    match case {
        SourceTransitionCase::Continue => None,
        SourceTransitionCase::Complete => Some(SourceTerminalStatus::Complete),
        SourceTransitionCase::Suspend => Some(SourceTerminalStatus::Suspend),
        SourceTransitionCase::Fail => Some(SourceTerminalStatus::Fail),
    }
}

/// Accepts a valid prefix, including a bare model/effect intent. Returns the
/// sum of every committed reservation; an uncertain intent stays charged.
pub(super) fn validate(
    binding: &SourceInvocationBinding,
    entries: &[SourceJournalEntry],
) -> Result<i64, SourceJournalError> {
    if entries.len() > MAX_SOURCE_ENTRIES {
        return Err(SourceJournalError::Capacity);
    }
    let mut phase = Phase::Start;
    let mut committed = 0i64;
    let mut turns_observed = 0u32;
    let mut stages = 0u32;

    for entry in entries {
        phase = match (phase, entry) {
            (Phase::Start, SourceJournalEntry::RunOpened) => Phase::BeforeTurn {
                turn: 0,
                scope: None,
            },

            (
                Phase::BeforeTurn { turn, .. },
                SourceJournalEntry::TurnObserved {
                    turn: actual,
                    state,
                    observation,
                    feedback,
                },
            ) if *actual == turn
                && turn < binding.max_iterations
                && [state, observation, feedback]
                    .iter()
                    .all(|value| looks_like_digest(value)) =>
            {
                turns_observed = turns_observed
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?;
                if turns_observed > binding.max_iterations {
                    return Err(SourceJournalError::Capacity);
                }
                // initialize (once), then one observe per turn.
                stages = stages
                    .checked_add(if turn == 0 { 2 } else { 1 })
                    .ok_or(SourceJournalError::Capacity)?;
                Phase::NeedAttempt {
                    turn,
                    attempt: 0,
                    prior_attempt: None,
                }
            }

            (
                Phase::NeedAttempt { turn, attempt, .. },
                SourceJournalEntry::AttemptIntent {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    attempt_digest,
                    request_digest,
                    prompt_digest,
                    request_bytes,
                    reserved_units,
                    response_limit,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && attempt < binding.max_attempts
                && *reserved_units == binding.reservation_units
                && (1..=MAX_SOURCE_REQUEST_BYTES).contains(request_bytes)
                && *response_limit == binding.response_limit
                && looks_like_digest(request_digest)
                && looks_like_digest(prompt_digest)
                && *attempt_digest
                    == binding.attempt_digest(
                        turn,
                        attempt,
                        request_digest,
                        prompt_digest,
                        *request_bytes,
                    ) =>
            {
                committed = committed
                    .checked_add(*reserved_units)
                    .ok_or(SourceJournalError::Capacity)?;
                if committed > binding.ceiling {
                    return Err(SourceJournalError::Capacity);
                }
                Phase::AwaitAttempt {
                    turn,
                    attempt,
                    response_limit: *response_limit,
                }
            }

            (
                Phase::AwaitAttempt {
                    turn,
                    attempt,
                    response_limit,
                },
                SourceJournalEntry::AttemptSettled {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    response,
                    response_digest,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && response.len() <= response_limit
                && *response_digest == source_response_digest(response) =>
            {
                Phase::Decode { turn, attempt }
            }

            (
                Phase::AwaitAttempt {
                    turn,
                    attempt,
                    response_limit,
                },
                SourceJournalEntry::AttemptFailed {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    reason,
                    attempted_bytes,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && *attempted_bytes <= response_limit.saturating_add(1) =>
            {
                let status = match reason {
                    SourceAttemptFailure::Cancelled => SourceStopStatus::Cancelled,
                    SourceAttemptFailure::DeadlineExceeded => SourceStopStatus::DeadlineExceeded,
                    _ => SourceStopStatus::ModelFailed,
                };
                Phase::NeedStop {
                    turn: Some(turn),
                    attempt: Some(attempt),
                    status,
                }
            }

            (
                Phase::Decode { turn, attempt },
                SourceJournalEntry::ProposalRefused {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    reason,
                },
            ) if *actual_turn == turn && *actual_attempt == attempt => match reason {
                SourceProposalRefusal::MalformedDecode if attempt + 1 < binding.max_attempts => {
                    Phase::NeedAttempt {
                        turn,
                        attempt: attempt + 1,
                        prior_attempt: Some(attempt),
                    }
                }
                SourceProposalRefusal::DeadlineExceeded => Phase::NeedStop {
                    turn: Some(turn),
                    attempt: Some(attempt),
                    status: SourceStopStatus::DeadlineExceeded,
                },
                _ => Phase::NeedStop {
                    turn: Some(turn),
                    attempt: Some(attempt),
                    status: SourceStopStatus::ModelFailed,
                },
            },

            (
                Phase::Decode { turn, attempt },
                SourceJournalEntry::ProposalAdmitted {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    proposal_digest,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && looks_like_digest(proposal_digest) =>
            {
                Phase::NeedAuthorization { turn, attempt }
            }

            (
                Phase::NeedAuthorization { turn, attempt },
                SourceJournalEntry::AuthorizationConsumed {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    grant_digest,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && looks_like_digest(grant_digest) =>
            {
                stages = stages.checked_add(1).ok_or(SourceJournalError::Capacity)?;
                Phase::NeedEffect { turn, attempt }
            }

            (
                Phase::NeedAuthorization { turn, attempt },
                SourceJournalEntry::AuthorizationRefused {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    reason,
                },
            ) if *actual_turn == turn && *actual_attempt == attempt => {
                let status = match reason {
                    SourceAuthorizationRefusal::GateDenied => SourceStopStatus::Rejected,
                    SourceAuthorizationRefusal::Undecided => SourceStopStatus::BudgetExhausted,
                    SourceAuthorizationRefusal::Cancelled => SourceStopStatus::Cancelled,
                    SourceAuthorizationRefusal::DeadlineExceeded => {
                        SourceStopStatus::DeadlineExceeded
                    }
                };
                Phase::NeedStop {
                    turn: Some(turn),
                    attempt: Some(attempt),
                    status,
                }
            }

            (
                Phase::NeedEffect { turn, attempt },
                SourceJournalEntry::EffectIntent {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    operation,
                    request_digest,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && valid_operation(operation)
                && looks_like_digest(request_digest)
                && stages < binding.max_stages =>
            {
                Phase::AwaitEffect {
                    turn,
                    attempt,
                    operation: operation.clone(),
                }
            }

            (
                Phase::AwaitEffect {
                    turn,
                    attempt,
                    operation,
                },
                SourceJournalEntry::EffectObserved {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    operation: actual_operation,
                    observation,
                    observation_digest,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && *actual_operation == operation
                && observation.len() <= MAX_SOURCE_EFFECT_BYTES
                && *observation_digest == source_effect_digest(observation) =>
            {
                Phase::NeedTransition { turn, attempt }
            }

            (
                Phase::AwaitEffect {
                    turn,
                    attempt,
                    operation,
                },
                SourceJournalEntry::EffectFailed {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    operation: actual_operation,
                    reason,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && *actual_operation == operation =>
            {
                let status = match reason {
                    SourceEffectFailure::HandlerFailed | SourceEffectFailure::ResultLimit => {
                        SourceStopStatus::EffectFailed
                    }
                    SourceEffectFailure::Cancelled => SourceStopStatus::Cancelled,
                    SourceEffectFailure::DeadlineExceeded => SourceStopStatus::DeadlineExceeded,
                };
                Phase::NeedStop {
                    turn: Some(turn),
                    attempt: Some(attempt),
                    status,
                }
            }

            (
                Phase::NeedTransition { turn, attempt },
                SourceJournalEntry::Transition {
                    turn: actual_turn,
                    attempt: actual_attempt,
                    case,
                    carrier_digest,
                },
            ) if *actual_turn == turn
                && *actual_attempt == attempt
                && looks_like_digest(carrier_digest) =>
            {
                stages = stages.checked_add(1).ok_or(SourceJournalError::Capacity)?;
                if *case == SourceTransitionCase::Continue {
                    let next_turn = turn.checked_add(1).ok_or(SourceJournalError::Capacity)?;
                    Phase::BeforeTurn {
                        turn: next_turn,
                        scope: Some(next_turn),
                    }
                } else {
                    Phase::Candidate {
                        turn,
                        attempt,
                        case: *case,
                        carrier: carrier_digest.clone(),
                    }
                }
            }

            (
                Phase::Candidate {
                    turn,
                    case,
                    carrier,
                    ..
                },
                SourceJournalEntry::TerminalOutcome {
                    turn: actual_turn,
                    status,
                    carrier_digest,
                },
            ) if *actual_turn == Some(turn)
                && Some(*status) == transition_status(case)
                && carrier_digest.as_deref() == Some(carrier.as_str()) =>
            {
                Phase::Done
            }

            (
                Phase::Terminal {
                    turn,
                    status,
                    carrier,
                },
                SourceJournalEntry::TerminalOutcome {
                    turn: actual_turn,
                    status: actual_status,
                    carrier_digest,
                },
            ) if *actual_turn == turn
                && *actual_status == status
                && carrier_digest.as_deref() == carrier.as_deref() =>
            {
                Phase::Done
            }

            (
                Phase::Decode {
                    turn: expected_turn,
                    attempt: expected_attempt,
                },
                SourceJournalEntry::Stop {
                    turn,
                    attempt,
                    status,
                    reason,
                },
            ) if binding.is_execution_profile()
                && *turn == Some(expected_turn)
                && *attempt == Some(expected_attempt)
                && matches!(
                    (*status, *reason),
                    (
                        SourceStopStatus::DeadlineExceeded,
                        SourceStopReason::DeadlineExceeded
                    ) | (SourceStopStatus::Cancelled, SourceStopReason::Cancelled)
                        | (SourceStopStatus::ModelFailed, SourceStopReason::ModelFailed)
                ) =>
            {
                Phase::Terminal {
                    turn: *turn,
                    status: (*status).into(),
                    carrier: None,
                }
            }

            (
                phase,
                SourceJournalEntry::Stop {
                    turn,
                    attempt,
                    status,
                    reason,
                },
            ) => {
                let (expected_turn, expected_attempt, expected_status, policy_only) = match &phase {
                    Phase::BeforeTurn { scope, .. } => (*scope, None, None, true),
                    Phase::NeedAttempt {
                        turn,
                        prior_attempt,
                        ..
                    } => (Some(*turn), *prior_attempt, None, true),
                    Phase::NeedEffect { turn, attempt }
                    | Phase::NeedTransition { turn, attempt } => {
                        (Some(*turn), Some(*attempt), None, true)
                    }
                    Phase::NeedAuthorization { turn, attempt }
                        if binding.is_execution_profile() =>
                    {
                        (Some(*turn), Some(*attempt), None, true)
                    }
                    Phase::NeedStop {
                        turn,
                        attempt,
                        status,
                    } => (*turn, *attempt, Some(*status), false),
                    Phase::Candidate {
                        turn,
                        attempt,
                        case,
                        ..
                    } if *case != SourceTransitionCase::Fail => {
                        (Some(*turn), Some(*attempt), None, true)
                    }
                    _ => return Err(SourceJournalError::Order),
                };
                if !stop_matches(
                    (*turn, *attempt),
                    *status,
                    *reason,
                    (expected_turn, expected_attempt),
                    expected_status,
                    policy_only,
                ) {
                    return Err(SourceJournalError::Order);
                }
                Phase::Terminal {
                    turn: *turn,
                    status: (*status).into(),
                    carrier: None,
                }
            }

            _ => return Err(SourceJournalError::Order),
        };
        if stages > binding.max_stages {
            return Err(SourceJournalError::Capacity);
        }
    }
    Ok(committed)
}
