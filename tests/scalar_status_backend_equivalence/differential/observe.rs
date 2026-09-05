//! The observation vocabulary every lane reports in, and the frontend and
//! reference-interpreter lanes themselves.
//!
//! Each lane answers the same question about the same generated module: for
//! every observable case, what happened? The vocabulary is closed, so a lane
//! that cannot answer has to say which of `Unavailable`, `Refused`, `Capacity`,
//! or `Aborted` applies. There is no way for a lane to stay silent and be
//! counted as agreement.

use std::collections::BTreeMap;
use std::path::Path;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, graph, hir, parse, verify};

use super::grammar::Type;

/// One observable declaration: its stable identity and the scalar it returns.
/// The lanes take this rather than a generated module, so the same differential
/// checker can be pointed at any hand-written module — including a reduction
/// filed against an open backend divergence.
pub(crate) type Case = (String, Type);

/// Every lane the differential checker knows how to run. `Interpreter` is the
/// reference: it needs no provisioned tool, so it is the one lane that is
/// always available and every other lane is compared against it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Lane {
    Interpreter,
    NativeO0,
    NativeO2,
    CoreWasm,
}

impl Lane {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Interpreter => "interpreter",
            Self::NativeO0 => "native-O0",
            Self::NativeO2 => "native-O2",
            Self::CoreWasm => "core-wasm",
        }
    }
}

/// What one lane observed for one case. Every variant is an explicit outcome;
/// none of them is a silent pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Observation {
    /// A value came back. `value` is the normalized rendering: a decimal `i64`
    /// or `true`/`false`, so a JavaScript BigInt, a C `long long`, and an
    /// interpreter `Value::Int` all compare as the same bytes.
    Returned { scalar: &'static str, value: String },
    /// A checked language failure with the exact selected status.
    Failed { domain: String, code: u32 },
    /// The lane hit a declared capacity ceiling — fuel, call depth, or a
    /// backend's own limit. Capacity rejection is never semantic disagreement.
    Capacity { detail: String },
    /// The lane refused to admit this case even though another lane executed
    /// it. This is the class issue #75 belongs to.
    Refused { code: String, detail: String },
    /// The lane started and died without an answer for this case.
    Aborted { detail: String },
}

impl Observation {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::Returned { scalar, value } => format!("returned {scalar} {value}"),
            Self::Failed { domain, code } => format!("failed {domain}#{code}"),
            Self::Capacity { detail } => format!("capacity {detail}"),
            Self::Refused { code, detail } => format!("refused {code}: {detail}"),
            Self::Aborted { detail } => format!("aborted {detail}"),
        }
    }

    fn is_capacity(&self) -> bool {
        matches!(self, Self::Capacity { .. })
    }
}

/// Whether a lane produced observations at all, and if not, exactly why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LaneStatus {
    /// The lane ran and answered for every case.
    Observed(BTreeMap<String, Observation>),
    /// A provisioned tool is absent, or this target does not admit the profile.
    /// The checker records the reason and never counts it as parity.
    Unavailable { reason: String },
    /// The lane ran and died as a whole.
    Aborted { detail: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LaneReport {
    pub(crate) lane: Lane,
    pub(crate) status: LaneStatus,
    /// Exact commands this lane ran, in order, for the discrepancy report.
    pub(crate) commands: Vec<String>,
}

impl LaneReport {
    pub(crate) fn unavailable(lane: Lane, reason: impl Into<String>) -> Self {
        Self {
            lane,
            status: LaneStatus::Unavailable {
                reason: reason.into(),
            },
            commands: Vec::new(),
        }
    }

    pub(crate) fn observations(&self) -> Option<&BTreeMap<String, Observation>> {
        match &self.status {
            LaneStatus::Observed(map) => Some(map),
            LaneStatus::Unavailable { .. } | LaneStatus::Aborted { .. } => None,
        }
    }
}

/// Per-module frontend agreement: canonical parse-format-parse stability, graph
/// identity, and verifier/HIR agreement. These need no provisioned tool and run
/// on every seed in PR CI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FrontendReport {
    pub(crate) findings: Vec<Finding>,
    /// Deterministic work counter: the resolved HIR function count, used by the
    /// scaling fixtures instead of a wall-clock threshold.
    pub(crate) resolved_functions: usize,
}

/// One recorded disagreement. `class` is stable text a regression test can bind
/// to; the message text is for the human reading the report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Finding {
    pub(crate) class: &'static str,
    pub(crate) case: Option<String>,
    pub(crate) lane: Option<Lane>,
    pub(crate) expected: String,
    pub(crate) observed: String,
}

pub(crate) const CLASS_PARSE_ERROR: &str = "parse_error";
pub(crate) const CLASS_VERIFIER_ERROR: &str = "verifier_error";
pub(crate) const CLASS_CANONICAL_INSTABILITY: &str = "canonical_instability";
pub(crate) const CLASS_GRAPH_INSTABILITY: &str = "graph_instability";
pub(crate) const CLASS_HIR_DISAGREEMENT: &str = "hir_disagreement";
pub(crate) const CLASS_VALUE_DISAGREEMENT: &str = "value_disagreement";
pub(crate) const CLASS_FAILURE_SELECTION: &str = "failure_selection_disagreement";
pub(crate) const CLASS_ADMISSION_DISAGREEMENT: &str = "admission_disagreement";
pub(crate) const CLASS_CAPACITY_REJECTION: &str = "capacity_rejection";
pub(crate) const CLASS_LANE_ABORT: &str = "lane_abort";
pub(crate) const CLASS_MISSING_CASE: &str = "missing_case";

/// Run every lane that needs no provisioned tool: parse, verify, canonical
/// round-trip, graph identity, and HIR resolution/validation.
pub(crate) fn observe_frontend(source: &str, path: &Path) -> FrontendReport {
    let mut findings = Vec::new();
    let program = match parse(source, path) {
        Ok(program) => program,
        Err(error) => {
            return FrontendReport {
                findings: vec![Finding {
                    class: CLASS_PARSE_ERROR,
                    case: None,
                    lane: None,
                    expected: "a generated module parses".to_owned(),
                    observed: format!("{}: {}", error.code, error.message),
                }],
                resolved_functions: 0,
            };
        }
    };
    let diagnostics = verify::verify(&program);
    for diagnostic in diagnostics.iter().filter(|item| item.severity.is_error()) {
        findings.push(Finding {
            class: CLASS_VERIFIER_ERROR,
            case: None,
            lane: None,
            expected: "a generated module verifies without errors".to_owned(),
            observed: format!("{}: {}", diagnostic.code, diagnostic.message),
        });
    }
    if !findings.is_empty() {
        return FrontendReport {
            findings,
            resolved_functions: 0,
        };
    }

    // Canonical parse-format-parse stability: formatting is idempotent and the
    // reparse of canonical text formats to the same bytes.
    let canonical = format::canonical(&program);
    match parse(&canonical, path) {
        Ok(reparsed) => {
            let again = format::canonical(&reparsed);
            if again != canonical {
                findings.push(Finding {
                    class: CLASS_CANONICAL_INSTABILITY,
                    case: None,
                    lane: None,
                    expected: "canonical(parse(canonical(p))) == canonical(p)".to_owned(),
                    observed: first_difference(&canonical, &again),
                });
            }
            // Graph identity across the round trip: same revision, same JSON.
            let before = graph::to_json(&program);
            let after = graph::to_json(&reparsed);
            match (before, after) {
                (Ok(before), Ok(after)) if before != after => findings.push(Finding {
                    class: CLASS_GRAPH_INSTABILITY,
                    case: None,
                    lane: None,
                    expected: "graph JSON survives a canonical round trip".to_owned(),
                    observed: first_difference(&before, &after),
                }),
                (Err(error), _) | (_, Err(error)) => findings.push(Finding {
                    class: CLASS_GRAPH_INSTABILITY,
                    case: None,
                    lane: None,
                    expected: "a verified module emits graph JSON".to_owned(),
                    observed: error
                        .first()
                        .map(|item| format!("{}: {}", item.code, item.message))
                        .unwrap_or_else(|| "graph emission failed".to_owned()),
                }),
                _ => {}
            }
            if graph::revision(&program) != graph::revision(&reparsed) {
                findings.push(Finding {
                    class: CLASS_GRAPH_INSTABILITY,
                    case: None,
                    lane: None,
                    expected: "graph revision survives a canonical round trip".to_owned(),
                    observed: format!(
                        "{} then {}",
                        graph::revision(&program),
                        graph::revision(&reparsed)
                    ),
                });
            }
        }
        Err(error) => findings.push(Finding {
            class: CLASS_CANONICAL_INSTABILITY,
            case: None,
            lane: None,
            expected: "canonical output reparses".to_owned(),
            observed: format!("{}: {}", error.code, error.message),
        }),
    }

    // Verifier/HIR agreement: a module the verifier accepts must resolve and
    // validate, because every backend consumes that HIR and not the AST.
    let resolved_functions = match hir::resolve(&program) {
        Ok(resolved) => {
            if let Err(error) = hir::validate(&resolved) {
                findings.push(Finding {
                    class: CLASS_HIR_DISAGREEMENT,
                    case: None,
                    lane: None,
                    expected: "verified source produces valid HIR".to_owned(),
                    observed: format!("{}: {}", error.code, error.message),
                });
            }
            resolved.functions.len()
        }
        Err(diagnostics) => {
            findings.push(Finding {
                class: CLASS_HIR_DISAGREEMENT,
                case: None,
                lane: None,
                expected: "verified source resolves to HIR".to_owned(),
                observed: diagnostics
                    .first()
                    .map(|item| format!("{}: {}", item.code, item.message))
                    .unwrap_or_else(|| "HIR resolution failed".to_owned()),
            });
            0
        }
    };

    FrontendReport {
        findings,
        resolved_functions,
    }
}

fn first_difference(left: &str, right: &str) -> String {
    let position = left
        .bytes()
        .zip(right.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| left.len().min(right.len()));
    let window = |text: &str| {
        let start = position.saturating_sub(40);
        let end = (position + 40).min(text.len());
        text.get(start..end).unwrap_or(text).to_owned()
    };
    format!(
        "byte {position}: `{}` versus `{}`",
        window(left),
        window(right)
    )
}

/// Deterministic per-case work counters the reference lane reports alongside
/// its observations, so complexity regressions are caught with a counter rather
/// than a global time threshold.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkCounters {
    pub(crate) steps_by_case: BTreeMap<String, u64>,
}

impl WorkCounters {
    pub(crate) fn total(&self) -> u64 {
        self.steps_by_case.values().copied().sum()
    }
}

/// The reference lane. It writes the module to the caller's temporary path and
/// reads it back through the ordinary public interpreter entry, which is the
/// same route `semaprax run` takes; the generated module declares no effects,
/// so evaluation grants no filesystem, network, process, or signing authority.
pub(crate) fn observe_interpreter(
    cases: &[Case],
    source_path: &Path,
    max_steps: usize,
) -> (LaneReport, WorkCounters) {
    let options = match InterpreterOptions::new(1 << 20, max_steps) {
        Ok(options) => options,
        Err(error) => {
            return (
                LaneReport::unavailable(
                    Lane::Interpreter,
                    format!("interpreter options rejected: {}", error.message),
                ),
                WorkCounters::default(),
            )
        }
    };
    let mut observations = BTreeMap::new();
    let mut counters = WorkCounters::default();
    let mut commands = Vec::new();
    for (stable_id, _) in cases {
        let stable_id = stable_id.clone();
        commands.push(format!(
            "semaprax run {} {stable_id}",
            source_path.display()
        ));
        match interpreter::interpret(source_path, &stable_id, &[], &options) {
            Ok(interpretation) => {
                let (observation, steps) = decode_envelope(&interpretation.envelope);
                counters.steps_by_case.insert(stable_id.clone(), steps);
                observations.insert(stable_id, observation);
            }
            Err(diagnostics) => {
                let diagnostic = diagnostics
                    .iter()
                    .find(|item| item.severity.is_error())
                    .or_else(|| diagnostics.first());
                let (code, detail) = diagnostic
                    .map(|item| (item.code.to_owned(), item.message.clone()))
                    .unwrap_or_else(|| {
                        (
                            "SPX-UNKNOWN".to_owned(),
                            "no diagnostic reported".to_owned(),
                        )
                    });
                observations.insert(stable_id, Observation::Refused { code, detail });
            }
        }
    }
    (
        LaneReport {
            lane: Lane::Interpreter,
            status: LaneStatus::Observed(observations),
            commands,
        },
        counters,
    )
}

/// Decode one interpreter envelope into the shared vocabulary plus the fuel
/// counter. A malformed envelope becomes an explicit `Aborted`, never a pass.
fn decode_envelope(envelope: &str) -> (Observation, u64) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(envelope) else {
        return (
            Observation::Aborted {
                detail: "interpreter envelope is not JSON".to_owned(),
            },
            0,
        );
    };
    let payload = &value["payload"];
    let steps = payload["fuel"]["steps_used"].as_u64().unwrap_or(0);
    let outcome = &payload["outcome"];
    let observation = match outcome["kind"].as_str() {
        Some("returned") => {
            let scalar = match outcome["type"].as_str() {
                Some("bool") => "bool",
                Some("i64") => "i64",
                Some(other) => {
                    return (
                        Observation::Aborted {
                            detail: format!("unexpected interpreter result type `{other}`"),
                        },
                        steps,
                    )
                }
                None => {
                    return (
                        Observation::Aborted {
                            detail: "interpreter outcome carries no type".to_owned(),
                        },
                        steps,
                    )
                }
            };
            Observation::Returned {
                scalar,
                value: outcome["value"].as_str().unwrap_or_default().to_owned(),
            }
        }
        Some("failed") => Observation::Failed {
            domain: outcome["status"]["domain_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            code: outcome["status"]["code"].as_u64().unwrap_or(u64::MAX) as u32,
        },
        Some(capacity @ ("fuel_exhausted" | "call_depth_exceeded")) => Observation::Capacity {
            detail: capacity.to_owned(),
        },
        other => Observation::Aborted {
            detail: format!("unknown interpreter outcome kind {other:?}"),
        },
    };
    (observation, steps)
}

/// Compare every lane against the reference and classify each disagreement.
///
/// An unavailable lane is returned separately and never contributes an
/// agreement. A lane that ran but answered `Capacity` where the reference
/// returned a value is a capacity rejection, which is a different class from
/// semantic disagreement and is reported as such.
pub(crate) fn compare(reference: &LaneReport, lanes: &[LaneReport]) -> Comparison {
    let mut comparison = Comparison::default();
    let Some(expected) = reference.observations() else {
        comparison.findings.push(Finding {
            class: CLASS_LANE_ABORT,
            case: None,
            lane: Some(reference.lane),
            expected: "the reference lane observes every case".to_owned(),
            observed: match &reference.status {
                LaneStatus::Unavailable { reason } => format!("unavailable: {reason}"),
                LaneStatus::Aborted { detail } => format!("aborted: {detail}"),
                LaneStatus::Observed(_) => unreachable!("observed lanes take the other branch"),
            },
        });
        return comparison;
    };
    for lane in lanes {
        match &lane.status {
            LaneStatus::Unavailable { reason } => {
                comparison.unavailable.push((lane.lane, reason.clone()));
                continue;
            }
            LaneStatus::Aborted { detail } => {
                comparison.findings.push(Finding {
                    class: CLASS_LANE_ABORT,
                    case: None,
                    lane: Some(lane.lane),
                    expected: "the lane observes every case".to_owned(),
                    observed: format!("aborted: {detail}"),
                });
                continue;
            }
            LaneStatus::Observed(actual) => {
                comparison.compared.push(lane.lane);
                for (case, expectation) in expected {
                    let Some(observation) = actual.get(case) else {
                        comparison.findings.push(Finding {
                            class: CLASS_MISSING_CASE,
                            case: Some(case.clone()),
                            lane: Some(lane.lane),
                            expected: expectation.render(),
                            observed: "the lane reported no outcome for this case".to_owned(),
                        });
                        continue;
                    };
                    if observation == expectation {
                        continue;
                    }
                    comparison.findings.push(Finding {
                        class: classify(expectation, observation),
                        case: Some(case.clone()),
                        lane: Some(lane.lane),
                        expected: expectation.render(),
                        observed: observation.render(),
                    });
                }
            }
        }
    }
    comparison
}

fn classify(expected: &Observation, observed: &Observation) -> &'static str {
    match (expected, observed) {
        (Observation::Aborted { .. }, _) | (_, Observation::Aborted { .. }) => CLASS_LANE_ABORT,
        (Observation::Refused { .. }, _) | (_, Observation::Refused { .. }) => {
            CLASS_ADMISSION_DISAGREEMENT
        }
        (left, right) if left.is_capacity() || right.is_capacity() => CLASS_CAPACITY_REJECTION,
        (Observation::Failed { .. }, _) | (_, Observation::Failed { .. }) => {
            CLASS_FAILURE_SELECTION
        }
        _ => CLASS_VALUE_DISAGREEMENT,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Comparison {
    pub(crate) findings: Vec<Finding>,
    /// Lanes that produced observations and were actually compared.
    pub(crate) compared: Vec<Lane>,
    /// Lanes that could not run, with the exact reason. Never parity.
    pub(crate) unavailable: Vec<(Lane, String)>,
}

impl Comparison {
    pub(crate) fn agrees(&self) -> bool {
        self.findings.is_empty()
    }
}
