//! The shared deterministic conformance driver (issue #181's "shared core
//! conformance corpus") and the one normalizer every case in it reuses.
//!
//! [`drive_to_settlement`] is the single place this SDK concatenates
//! `Delta` events and enforces the event-order, duplicate-completion,
//! contradictory-usage, oversized-response and post-cancellation rules a
//! conforming adapter must never violate — every named hostile case in
//! `super::hostile` is caught here, by exactly one rule, never by a case
//! reimplementing its own copy of the check. [`run_conformance_suite`]
//! binds one drive (plus capability negotiation) into one
//! [`super::report::ConformanceReport`].

use crate::diagnostic::quote_json;
use crate::live_invocation::model_invoke::ModelFailure;

use super::adapter::{
    AdapterEvent, AdapterInvocationCapability, AdapterPoll, AdapterRequest, ProviderAdapter,
};
use super::capability::{negotiate, AdapterCapabilities, RequiredCapabilities};
use super::report::{
    ConformanceReport, ReportCaseResult, NONCLAIM_CANCELLATION_BEST_EFFORT,
    NONCLAIM_NOT_A_SUPPORT_DECISION, NONCLAIM_OFFLINE_ONLY, NONCLAIM_USAGE_UNVERIFIED,
    TEST_CORPUS_ID,
};

/// A second `Completed` event for one request.
pub const DRIVE_DUPLICATE_COMPLETION: &str = "ADAPTER-DUPLICATE-COMPLETION";
/// An event arrived after `cancel()` was called on the adapter under
/// drive.
pub const DRIVE_LATE_AFTER_CANCEL: &str = "ADAPTER-LATE-AFTER-CANCEL";
/// A `Usage` snapshot regressed (a later report claimed fewer tokens or
/// lower cost than an earlier one for the same request).
pub const DRIVE_CONTRADICTORY_USAGE: &str = "ADAPTER-CONTRADICTORY-USAGE";
/// Concatenated `Delta` bytes exceeded the request's declared
/// `max_response_bytes`.
pub const DRIVE_OVERSIZED: &str = "ADAPTER-OVERSIZED-RESPONSE";
/// A `Delta` or `Usage` event arrived after `Completed` but before
/// settlement.
pub const DRIVE_EVENT_AFTER_COMPLETION: &str = "ADAPTER-EVENT-AFTER-COMPLETION";
/// The adapter never reached a terminal outcome within the driver's bounded
/// poll budget. Never an infinite loop: this driver always terminates.
pub const DRIVE_POLL_BUDGET_EXCEEDED: &str = "ADAPTER-POLL-BUDGET-EXCEEDED";

const MAX_DRIVE_POLLS: usize = 10_000;

/// What one settled usage report looked like, for regression comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UsageSnapshot {
    tokens_in: u64,
    tokens_out: u64,
    cost_micros: i64,
}

/// The result of driving one adapter instance through one full
/// request/response cycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DriveOutcome {
    /// The adapter settled cleanly. `response_bytes` is every `Delta`
    /// concatenated in the order it was observed.
    Settled { response_bytes: Vec<u8> },
    /// The driver refused to continue: `code` names the exact rule
    /// violated, matching one of the `DRIVE_*` constants above.
    Refused { code: &'static str, reason: String },
    /// The adapter itself reported a normalized failure.
    Failed {
        failure: ModelFailure,
        attempted_bytes: usize,
    },
}

/// Drives `adapter` through `request`, calling `adapter.cancel(..)` after
/// exactly `cancel_after_events` observed events (`None` never cancels).
/// Deterministic: given the same adapter script and the same
/// `cancel_after_events`, this always returns byte-identical results — it
/// reads no clock, no environment, and makes no host call of its own.
pub fn drive_to_settlement(
    adapter: &mut dyn ProviderAdapter,
    capability: &AdapterInvocationCapability,
    request: &AdapterRequest,
    cancel_after_events: Option<usize>,
) -> DriveOutcome {
    if let Err(refusal) = adapter.start(capability, request) {
        return DriveOutcome::Refused {
            code: "ADAPTER-START-REFUSED",
            reason: refusal.0,
        };
    }

    let mut response_bytes = Vec::new();
    let mut last_usage: Option<UsageSnapshot> = None;
    let mut completed = false;
    let mut cancelled = false;
    let mut events_seen = 0usize;

    for _ in 0..MAX_DRIVE_POLLS {
        if !cancelled {
            if let Some(threshold) = cancel_after_events {
                if events_seen >= threshold {
                    adapter.cancel("conformance-suite-requested-cancel");
                    cancelled = true;
                }
            }
        }
        match adapter.poll() {
            AdapterPoll::Pending => continue,
            AdapterPoll::Event(event) => {
                if cancelled {
                    return DriveOutcome::Refused {
                        code: DRIVE_LATE_AFTER_CANCEL,
                        reason: format!("a {} event arrived after cancel() was called", event_name(&event)),
                    };
                }
                match event {
                    AdapterEvent::Delta(chunk) => {
                        if completed {
                            return DriveOutcome::Refused {
                                code: DRIVE_EVENT_AFTER_COMPLETION,
                                reason: "a Delta event arrived after Completed".to_owned(),
                            };
                        }
                        if response_bytes.len().saturating_add(chunk.len())
                            > request.max_response_bytes
                        {
                            return DriveOutcome::Refused {
                                code: DRIVE_OVERSIZED,
                                reason: format!(
                                    "response exceeded the {}-byte bound",
                                    request.max_response_bytes
                                ),
                            };
                        }
                        response_bytes.extend_from_slice(&chunk);
                    }
                    AdapterEvent::Usage {
                        tokens_in,
                        tokens_out,
                        cost_micros,
                    } => {
                        if completed {
                            return DriveOutcome::Refused {
                                code: DRIVE_EVENT_AFTER_COMPLETION,
                                reason: "a Usage event arrived after Completed".to_owned(),
                            };
                        }
                        let next = UsageSnapshot {
                            tokens_in,
                            tokens_out,
                            cost_micros,
                        };
                        if let Some(previous) = last_usage {
                            if next.tokens_in < previous.tokens_in
                                || next.tokens_out < previous.tokens_out
                                || next.cost_micros < previous.cost_micros
                            {
                                return DriveOutcome::Refused {
                                    code: DRIVE_CONTRADICTORY_USAGE,
                                    reason: format!(
                                        "usage regressed from {{tokens_in:{},tokens_out:{},cost_micros:{}}} to {{tokens_in:{},tokens_out:{},cost_micros:{}}}",
                                        previous.tokens_in, previous.tokens_out, previous.cost_micros,
                                        next.tokens_in, next.tokens_out, next.cost_micros,
                                    ),
                                };
                            }
                        }
                        last_usage = Some(next);
                    }
                    AdapterEvent::Completed => {
                        if completed {
                            return DriveOutcome::Refused {
                                code: DRIVE_DUPLICATE_COMPLETION,
                                reason: "a second Completed event arrived for one request".to_owned(),
                            };
                        }
                        completed = true;
                    }
                }
                events_seen += 1;
            }
            AdapterPoll::Settled(settlement) => {
                if cancelled {
                    return DriveOutcome::Refused {
                        code: DRIVE_LATE_AFTER_CANCEL,
                        reason: "settlement arrived after cancel() was called".to_owned(),
                    };
                }
                // Trust the driver's own concatenation over whatever the
                // adapter separately claims as `response_bytes`, when the
                // adapter emitted at least one Delta: an adapter's own
                // `response_bytes` field is informational only, matching
                // `AdapterPoll::Settled`'s own doc comment.
                let bytes = if response_bytes.is_empty() {
                    settlement.response_bytes
                } else {
                    response_bytes
                };
                return DriveOutcome::Settled {
                    response_bytes: bytes,
                };
            }
            AdapterPoll::Failed {
                failure,
                attempted_bytes,
            } => {
                return DriveOutcome::Failed {
                    failure,
                    attempted_bytes,
                };
            }
        }
    }
    DriveOutcome::Refused {
        code: DRIVE_POLL_BUDGET_EXCEEDED,
        reason: format!("adapter did not reach a terminal outcome within {MAX_DRIVE_POLLS} polls"),
    }
}

fn event_name(event: &AdapterEvent) -> &'static str {
    match event {
        AdapterEvent::Delta(_) => "Delta",
        AdapterEvent::Usage { .. } => "Usage",
        AdapterEvent::Completed => "Completed",
    }
}

/// What a caller expects [`drive_to_settlement`] to produce for one
/// scripted scenario. [`run_conformance_suite`]'s "drive_to_settlement"
/// case passes exactly when the actual [`DriveOutcome`] matches this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedOutcome {
    Settled { response_bytes: Vec<u8> },
    RefusedWithCode(&'static str),
    FailedWithClass(ModelFailure),
}

fn drive_outcome_matches(actual: &DriveOutcome, expected: &ExpectedOutcome) -> bool {
    match (actual, expected) {
        (
            DriveOutcome::Settled { response_bytes },
            ExpectedOutcome::Settled {
                response_bytes: expected_bytes,
            },
        ) => response_bytes == expected_bytes,
        (DriveOutcome::Refused { code, .. }, ExpectedOutcome::RefusedWithCode(expected_code)) => {
            code == expected_code
        }
        (DriveOutcome::Failed { failure, .. }, ExpectedOutcome::FailedWithClass(expected)) => {
            failure == expected
        }
        _ => false,
    }
}

fn describe_outcome(outcome: &DriveOutcome) -> String {
    match outcome {
        DriveOutcome::Settled { response_bytes } => {
            format!("settled:{}", quote_json(&String::from_utf8_lossy(response_bytes)))
        }
        DriveOutcome::Refused { code, reason } => format!("refused:{code}:{reason}"),
        DriveOutcome::Failed {
            failure,
            attempted_bytes,
        } => format!("failed:{}:{attempted_bytes}", failure.as_str()),
    }
}

/// Renders `caps` as one canonical, single-line snapshot for
/// [`ConformanceReport::observed_capabilities`].
#[must_use]
pub fn render_capabilities(caps: &AdapterCapabilities) -> String {
    let modes: Vec<&'static str> = caps
        .structured_output_modes
        .iter()
        .map(|mode| mode.as_str())
        .collect();
    let retryable: Vec<&'static str> = caps
        .retryable_failure_classes
        .iter()
        .map(|class| class.as_str())
        .collect();
    let endpoint_policy = match &caps.endpoint_policy {
        super::capability::EndpointPolicy::HostInjected => "host_injected".to_owned(),
        super::capability::EndpointPolicy::AdapterDeclaredAmbient(value) => {
            format!("adapter_declared_ambient:{value}")
        }
    };
    format!(
        "{{\"adapter_identity\":{},\"adapter_version\":{},\"provider_profile\":{},\"structured_output_modes\":{:?},\"supports_streaming\":{},\"token_accounting_source\":{},\"cancellation_semantics\":{},\"retryable_failure_classes\":{:?},\"endpoint_policy\":{},\"max_request_bytes\":{},\"max_response_bytes\":{},\"max_context_tokens\":{},\"max_output_tokens\":{}}}",
        quote_json(&caps.adapter_identity),
        quote_json(&caps.adapter_version),
        quote_json(&caps.provider_profile),
        modes,
        caps.supports_streaming,
        quote_json(caps.token_accounting_source.as_str()),
        quote_json(match caps.cancellation_semantics {
            super::capability::CancellationSemantics::BestEffortRequestStop => "best_effort_request_stop",
            super::capability::CancellationSemantics::Unsupported => "unsupported",
        }),
        retryable,
        quote_json(&endpoint_policy),
        caps.max_request_bytes,
        caps.max_response_bytes,
        caps.max_context_tokens,
        caps.max_output_tokens,
    )
}

pub const CASE_CAPABILITY_NEGOTIATION: &str = "capability_negotiation";
pub const CASE_DRIVE_TO_SETTLEMENT: &str = "drive_to_settlement";

/// Runs the shared corpus against one adapter instance: capability
/// negotiation, then (only if negotiation admits the adapter) one driven
/// request/response cycle, judged against `expected`. Returns one
/// deterministic [`ConformanceReport`] — given the same adapter script,
/// the same `required`/`request`/`expected`/`cancel_after_events`, this
/// always produces a byte-identical report (`ConformanceReport::render`
/// output and digest included).
#[must_use]
pub fn run_conformance_suite(
    adapter: &mut dyn ProviderAdapter,
    capability: &AdapterInvocationCapability,
    required: &RequiredCapabilities,
    request: &AdapterRequest,
    cancel_after_events: Option<usize>,
    expected: &ExpectedOutcome,
) -> ConformanceReport {
    let caps = adapter.capabilities().clone();
    let observed_capabilities = render_capabilities(&caps);
    let mut cases = Vec::new();

    match negotiate(&caps, required) {
        Ok(()) => cases.push(ReportCaseResult {
            case_name: CASE_CAPABILITY_NEGOTIATION,
            passed: true,
            detail: String::new(),
        }),
        Err(refusal) => {
            cases.push(ReportCaseResult {
                case_name: CASE_CAPABILITY_NEGOTIATION,
                passed: false,
                detail: format!("{refusal:?}"),
            });
            return ConformanceReport {
                adapter_identity: caps.adapter_identity,
                adapter_version: caps.adapter_version,
                provider_profile: caps.provider_profile,
                test_corpus_id: TEST_CORPUS_ID.to_owned(),
                observed_capabilities,
                cases,
                nonclaims: standard_nonclaims(),
            };
        }
    }

    let outcome = drive_to_settlement(adapter, capability, request, cancel_after_events);
    let passed = drive_outcome_matches(&outcome, expected);
    cases.push(ReportCaseResult {
        case_name: CASE_DRIVE_TO_SETTLEMENT,
        passed,
        detail: describe_outcome(&outcome),
    });

    ConformanceReport {
        adapter_identity: caps.adapter_identity,
        adapter_version: caps.adapter_version,
        provider_profile: caps.provider_profile,
        test_corpus_id: TEST_CORPUS_ID.to_owned(),
        observed_capabilities,
        cases,
        nonclaims: standard_nonclaims(),
    }
}

fn standard_nonclaims() -> Vec<&'static str> {
    vec![
        NONCLAIM_OFFLINE_ONLY,
        NONCLAIM_CANCELLATION_BEST_EFFORT,
        NONCLAIM_USAGE_UNVERIFIED,
        NONCLAIM_NOT_A_SUPPORT_DECISION,
    ]
}
