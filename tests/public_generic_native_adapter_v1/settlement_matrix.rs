//! Issue #301: one closed, versioned settlement corpus executed against every
//! engine and generated consumer that exists for the same checked
//! `Pair<Bytes>` endpoint, with an asserted matrix of cells.
//!
//! Each engine executes the manifest below through its real boundary and
//! prints one normalized receipt per case (per cycle for repeated
//! lifecycles). The receipt carries the primary and secondary status, the
//! returned or copied-out bytes, the observed endpoint dispatch count, final
//! live resources, peak provider allocations/handles and the physical release
//! order, each only where the engine can actually observe it. A cell is PASS
//! only when every observed value matches the manifest's expectation and the
//! native family agrees on dispatch, peak and release order. Every other
//! cell is either an explicit NOT APPLICABLE with its reason, or a KNOWN
//! DEFECT that must still reproduce with its exact signature; a known defect
//! that stops reproducing fails the gate until its cell is flipped.
//!
//! See `docs/PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#shared-cross-engine-matrix`.

#[path = "settlement_matrix/engines.rs"]
mod engines;

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The corpus identity. Any change to [`CASES`] or to an expectation below is
/// a new corpus version, not an in-place edit.
pub(crate) const CORPUS_VERSION: &str = "semaprax.public-generic.settlement-matrix.v1";

/// Known-defect cells refer to the provider repair tracked here.
const CORE_WASM_PROVIDER_ISSUE: &str = "#288";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Subject {
    /// `auth.identity` with `requires true`.
    Identity,
    /// The same declarations with `requires false`.
    Refusing,
    /// The checked allocating body (`copy(left)`, `remake(right, 0)`).
    Allocating,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Transform = 0,
    Repeated = 1,
    ShortExport = 2,
    /// A leaf path substituted for the trusted canonical inventory: the
    /// frame decodes, but `validate_frame`'s *last* check (leaf sequence
    /// versus the canonical inventory) rejects it.
    WrongPath = 3,
    /// A distinct preparation refusal from [`Self::WrongPath`]: the encoded
    /// frame is bound to the wrong direction (a well-formed *result* frame
    /// submitted as an input), so `validate_frame`'s *first* check
    /// (direction) rejects it instead. Both must still refuse with the same
    /// primary status and zero physical effects; this kind exists to prove
    /// that a genuinely different validate_frame branch is also effect-free,
    /// not merely to repeat [`Self::WrongPath`] under a second name.
    RefusalEffects = 4,
    InjectExportRelease = 5,
    InjectPrepare = 6,
}

#[derive(Clone, Copy)]
pub(crate) enum Payload {
    Literal(&'static [u8]),
    Pattern { len: usize, mul: usize, add: usize },
}

impl Payload {
    pub(crate) fn bytes(self) -> Vec<u8> {
        match self {
            Self::Literal(bytes) => bytes.to_vec(),
            Self::Pattern { len, mul, add } => (0..len)
                .map(|index| u8::try_from((index * mul + add) & 0xff).unwrap())
                .collect(),
        }
    }
}

pub(crate) struct Case {
    pub(crate) id: &'static str,
    pub(crate) subject: Subject,
    pub(crate) kind: Kind,
    pub(crate) cycles: u32,
    pub(crate) left: Payload,
    pub(crate) right: Payload,
}

const SMALL_LEFT: Payload = Payload::Literal(&[1, 7, 13]);
const SMALL_RIGHT: Payload = Payload::Literal(&[2, 11, 17, 23]);
const EMPTY: Payload = Payload::Literal(&[]);

const fn case(
    id: &'static str,
    subject: Subject,
    kind: Kind,
    left: Payload,
    right: Payload,
) -> Case {
    Case {
        id,
        subject,
        kind,
        cycles: 1,
        left,
        right,
    }
}

/// The closed corpus. Rows are ordered; ids are stable receipt labels.
pub(crate) const CASES: &[Case] = &[
    case(
        "success-small",
        Subject::Identity,
        Kind::Transform,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    case(
        "success-empty-leaves",
        Subject::Identity,
        Kind::Transform,
        EMPTY,
        EMPTY,
    ),
    case(
        "success-2k-boundary",
        Subject::Identity,
        Kind::Transform,
        Payload::Pattern {
            len: 1024,
            mul: 13,
            add: 1,
        },
        Payload::Pattern {
            len: 1024,
            mul: 7,
            add: 2,
        },
    ),
    case(
        "success-over-2k",
        Subject::Identity,
        Kind::Transform,
        Payload::Pattern {
            len: 1025,
            mul: 13,
            add: 1,
        },
        Payload::Pattern {
            len: 1024,
            mul: 7,
            add: 2,
        },
    ),
    case(
        "success-max-leaf",
        Subject::Identity,
        Kind::Transform,
        Payload::Pattern {
            len: 65_536,
            mul: 17,
            add: 3,
        },
        Payload::Pattern {
            len: 65_536,
            mul: 31,
            add: 5,
        },
    ),
    Case {
        id: "repeated-lifecycle",
        subject: Subject::Identity,
        kind: Kind::Repeated,
        cycles: 3,
        left: SMALL_LEFT,
        right: SMALL_RIGHT,
    },
    case(
        "export-short-capacity",
        Subject::Identity,
        Kind::ShortExport,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    case(
        "prepare-wrong-leaf-path",
        Subject::Identity,
        Kind::WrongPath,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    case(
        "prepare-refusal-effect-free",
        Subject::Identity,
        Kind::RefusalEffects,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    case(
        "injected-export-and-release-failure",
        Subject::Identity,
        Kind::InjectExportRelease,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    case(
        "injected-prepare-allocation-failure",
        Subject::Identity,
        Kind::InjectPrepare,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    case(
        "contract-failure",
        Subject::Refusing,
        Kind::Transform,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
    Case {
        id: "contract-failure-repeated",
        subject: Subject::Refusing,
        kind: Kind::Repeated,
        cycles: 2,
        left: SMALL_LEFT,
        right: SMALL_RIGHT,
    },
    case(
        "allocating-success",
        Subject::Allocating,
        Kind::Transform,
        SMALL_LEFT,
        SMALL_RIGHT,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Engine {
    Interpreter,
    NativeO0,
    NativeO2,
    NativeAsan,
    CoreWasm,
    GeneratedC11,
    GeneratedRust,
    GeneratedCxx17,
    GeneratedTypeScript,
}

pub(crate) const ENGINES: [Engine; 9] = [
    Engine::Interpreter,
    Engine::NativeO0,
    Engine::NativeO2,
    Engine::NativeAsan,
    Engine::CoreWasm,
    Engine::GeneratedC11,
    Engine::GeneratedRust,
    Engine::GeneratedCxx17,
    Engine::GeneratedTypeScript,
];

impl Engine {
    fn label(self) -> &'static str {
        match self {
            Self::Interpreter => "interpreter",
            Self::NativeO0 => "native-c11-O0",
            Self::NativeO2 => "native-c11-O2",
            Self::NativeAsan => "native-c11-asan(local)",
            Self::CoreWasm => "core-wasm",
            Self::GeneratedC11 => "generated-c11",
            Self::GeneratedRust => "generated-rust",
            Self::GeneratedCxx17 => "generated-c++17",
            Self::GeneratedTypeScript => "generated-typescript",
        }
    }

    fn native(self) -> bool {
        matches!(
            self,
            Self::NativeO0
                | Self::NativeO2
                | Self::NativeAsan
                | Self::GeneratedC11
                | Self::GeneratedRust
                | Self::GeneratedCxx17
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Expect {
    Pass,
    NotApplicable(&'static str),
    KnownDefect {
        issue: &'static str,
        defect: &'static str,
        signature: &'static str,
    },
}

const NA_INTERPRETER_FRAME: &str =
    "the retained-call seam takes typed values, not carrier frames or export buffers";
const NA_INTERPRETER_INJECTION: &str =
    "the interpreter exposes no authorized physical allocation or release injection seam";
const NA_WASM_INJECTION: &str = "the compiled Core provider has no physical injection hooks; adding them would change its closed export inventory";
const NA_GENERATED_EXPORT: &str =
    "the generated caller sizes and retries the export buffer internally; the raw ABI cells own this path";
const NA_GENERATED_FRAME: &str = "the generated caller derives canonical leaf paths from the descriptor and cannot express a wrong path without mutating generated source";

/// Known defects on this base, one row per cell: (case, engine, defect,
/// required failure signature). Every row is tracked by
/// [`CORE_WASM_PROVIDER_ISSUE`]. Flipping a cell after a provider fix is a
/// one-line change: delete its row; the cell then must PASS. A row whose
/// defect no longer reproduces fails the gate, so a fixed cell cannot stay
/// here. The skip keeps one cell per line so the flip stays one line.
///
/// Empty on this base: R07 fixed the Core Wasm provider's owned-byte
/// runtime, admission-before-`memory.grow`, 128 KiB payload window, and ABI
/// v2 status 14 (SPX-PG803), so all eight cells that used to sit here now
/// PASS. The mechanism stays for the next reproducible defect.
#[rustfmt::skip]
const KNOWN_DEFECTS: &[(&str, Engine, &str, &str)] = &[];

/// The asserted matrix. Every (case, engine) pair is decided here.
pub(crate) fn expected_cell(case: &Case, engine: Engine) -> Expect {
    let wasm = matches!(engine, Engine::CoreWasm | Engine::GeneratedTypeScript);
    match case.kind {
        Kind::ShortExport | Kind::WrongPath | Kind::RefusalEffects => match engine {
            Engine::Interpreter => return Expect::NotApplicable(NA_INTERPRETER_FRAME),
            Engine::GeneratedC11 | Engine::GeneratedRust | Engine::GeneratedCxx17 => {
                return Expect::NotApplicable(if case.kind == Kind::ShortExport {
                    NA_GENERATED_EXPORT
                } else {
                    NA_GENERATED_FRAME
                })
            }
            Engine::GeneratedTypeScript => {
                return Expect::NotApplicable(if case.kind == Kind::ShortExport {
                    NA_GENERATED_EXPORT
                } else {
                    NA_GENERATED_FRAME
                })
            }
            _ => {}
        },
        Kind::InjectExportRelease | Kind::InjectPrepare => {
            if engine == Engine::Interpreter {
                return Expect::NotApplicable(NA_INTERPRETER_INJECTION);
            }
            if wasm {
                return Expect::NotApplicable(NA_WASM_INJECTION);
            }
        }
        Kind::Transform | Kind::Repeated => {}
    }
    if let Some((_, _, defect, signature)) = KNOWN_DEFECTS
        .iter()
        .find(|(id, column, _, _)| *id == case.id && *column == engine)
    {
        return Expect::KnownDefect {
            issue: CORE_WASM_PROVIDER_ISSUE,
            defect,
            signature,
        };
    }
    Expect::Pass
}

/// One normalized receipt line.
#[derive(Clone, Debug)]
pub(crate) struct Observation {
    pub(crate) label: String,
    pub(crate) primary: String,
    pub(crate) secondary: Option<String>,
    pub(crate) dispatch: Option<u64>,
    pub(crate) live: Option<u64>,
    pub(crate) peak: Option<String>,
    pub(crate) order: Option<String>,
    pub(crate) leaves: Option<(Vec<u8>, Vec<u8>)>,
    pub(crate) note: BTreeMap<String, String>,
}

fn optional(token: &str) -> Option<String> {
    (token != "-").then(|| token.to_owned())
}

fn hex_leaf(token: &str) -> Vec<u8> {
    if token == "e" {
        return Vec::new();
    }
    assert_eq!(token.len() % 2, 0, "odd receipt hex");
    (0..token.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&token[at..at + 2], 16).unwrap())
        .collect()
}

/// Parse receipts. `frame` decodes a `frame:<hex>` result carrier.
pub(crate) fn parse_receipts(
    stdout: &[u8],
    frame: &dyn Fn(&[u8]) -> (Vec<u8>, Vec<u8>),
) -> Vec<Observation> {
    std::str::from_utf8(stdout)
        .expect("receipts are ASCII")
        .lines()
        .map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            assert_eq!(fields.len(), 10, "malformed receipt: {line}");
            let leaves = match (fields[7], fields[8]) {
                ("-", "-") => None,
                (encoded, "frame") => {
                    Some(frame(&hex_leaf(encoded.strip_prefix("frame:").unwrap())))
                }
                (left, right) => Some((hex_leaf(left), hex_leaf(right))),
            };
            let note = if fields[9] == "-" {
                BTreeMap::new()
            } else {
                fields[9]
                    .split(',')
                    .map(|pair| {
                        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                        (key.to_owned(), value.to_owned())
                    })
                    .collect()
            };
            Observation {
                label: fields[0].to_owned(),
                primary: fields[1].to_owned(),
                secondary: optional(fields[2]),
                dispatch: optional(fields[3]).map(|value| value.parse().unwrap()),
                live: optional(fields[4]).map(|value| value.parse().unwrap()),
                peak: optional(fields[5]),
                order: optional(fields[6]),
                leaves,
                note,
            }
        })
        .collect()
}

fn expected_leaves(case: &Case) -> Option<(Vec<u8>, Vec<u8>)> {
    match (case.subject, case.kind) {
        (Subject::Refusing, _)
        | (_, Kind::WrongPath | Kind::RefusalEffects | Kind::InjectPrepare) => None,
        (_, Kind::InjectExportRelease) => None,
        (Subject::Allocating, _) => Some((case.left.bytes(), vec![9, 0, 0])),
        (Subject::Identity, _) => Some((case.left.bytes(), case.right.bytes())),
    }
}

fn expected_primary(case: &Case) -> &'static str {
    match (case.subject, case.kind) {
        (Subject::Refusing, _) | (_, Kind::InjectExportRelease) => "11",
        (_, Kind::WrongPath | Kind::RefusalEffects) => "14",
        (_, Kind::InjectPrepare) => "10",
        _ => "0",
    }
}

fn expected_secondary(case: &Case) -> &'static str {
    if case.kind == Kind::InjectExportRelease {
        "11"
    } else {
        "0"
    }
}

/// Reverse-obligation physical release order from the settlement plan:
/// input leaves (direction 0) in reverse, then result leaves (direction 1).
fn expected_order(case: &Case) -> &'static str {
    match (case.subject, case.kind) {
        (_, Kind::WrongPath | Kind::RefusalEffects | Kind::InjectPrepare) => "none",
        (Subject::Refusing, _) => "0.1,0.0",
        _ => "0.1,0.0,1.1,1.0",
    }
}

fn cycle_of(label: &str) -> u64 {
    label
        .split_once('#')
        .map_or(0, |(_, cycle)| cycle.parse().unwrap())
}

fn require(problems: &mut Vec<String>, what: &str, expected: &str, observed: &str) {
    if expected != observed {
        problems.push(format!("{what}: expected {expected}, observed {observed}"));
    }
}

fn require_note(problems: &mut Vec<String>, observation: &Observation, key: &str, expected: &str) {
    match observation.note.get(key) {
        Some(value) => require(problems, &format!("note {key}"), expected, value),
        None => problems.push(format!("note {key}: missing")),
    }
}

/// Check one observation against the manifest; returns every mismatch.
fn check_observation(
    case: &Case,
    engine: Engine,
    observation: &Observation,
    last: bool,
) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        "primary",
        expected_primary(case),
        &observation.primary,
    );
    if observation.primary == "trap" {
        return problems;
    }
    if let Some(secondary) = &observation.secondary {
        if expected_leaves(case).is_some() || case.kind == Kind::InjectExportRelease {
            require(
                &mut problems,
                "secondary",
                expected_secondary(case),
                secondary,
            );
        }
    }
    if observation.leaves != expected_leaves(case) {
        problems.push(format!(
            "leaves differ (expected {:?} bytes, observed {:?} bytes)",
            expected_leaves(case).map(|(l, r)| (l.len(), r.len())),
            observation.leaves.as_ref().map(|(l, r)| (l.len(), r.len()))
        ));
    }
    let cycle = cycle_of(&observation.label);
    if let Some(dispatch) = observation.dispatch {
        let per_cycle = u64::from(!matches!(
            case.kind,
            Kind::WrongPath | Kind::RefusalEffects | Kind::InjectPrepare
        ));
        // Observers count cumulatively since the case reset.
        let expected = per_cycle * (cycle + 1);
        require(
            &mut problems,
            "dispatch",
            &expected.to_string(),
            &dispatch.to_string(),
        );
    }
    if let (Some(live), true) = (observation.live, last) {
        require(&mut problems, "live", "0", &live.to_string());
    }
    if engine.native() {
        if let Some(order) = &observation.order {
            let expected = expected_order(case);
            // Cumulative across repeated cycles on one provider.
            let repeated = std::iter::repeat_n(expected, usize::try_from(cycle + 1).unwrap())
                .collect::<Vec<_>>()
                .join(",");
            require(&mut problems, "release order", &repeated, order);
        }
    }
    check_notes(case, engine, observation, last, &mut problems);
    problems
}

fn check_notes(
    case: &Case,
    engine: Engine,
    observation: &Observation,
    last: bool,
    problems: &mut Vec<String>,
) {
    let succeeded = expected_leaves(case).is_some();
    match engine {
        Engine::NativeO0 | Engine::NativeO2 | Engine::NativeAsan => {
            match case.kind {
                Kind::WrongPath | Kind::RefusalEffects | Kind::InjectPrepare => {
                    require_note(problems, observation, "effects", "0");
                    require_note(problems, observation, "input", "0");
                }
                _ => {
                    require_note(problems, observation, "dup", "8");
                    require_note(problems, observation, "redispatch", "0");
                    if succeeded || case.kind == Kind::InjectExportRelease {
                        require_note(problems, observation, "stale", "8");
                    } else {
                        require_note(problems, observation, "release", "8");
                    }
                }
            }
            if case.kind == Kind::ShortExport {
                for (key, value) in [
                    ("short", "12"),
                    ("untouched", "1"),
                    ("retry", "0"),
                    ("same", "1"),
                ] {
                    require_note(problems, observation, key, value);
                }
            }
            if last {
                for (key, value) in [("handles", "0"), ("close", "0"), ("armed", "0")] {
                    require_note(problems, observation, key, value);
                }
            }
        }
        Engine::CoreWasm => {
            match case.kind {
                Kind::WrongPath | Kind::RefusalEffects => {
                    require_note(problems, observation, "effects", "0");
                    require_note(problems, observation, "input", "0");
                }
                _ if succeeded => {
                    require_note(problems, observation, "dup", "8");
                    require_note(problems, observation, "stale", "8");
                }
                _ => {
                    // A failed call retains its input without manufacturing
                    // an output handle, so one explicit recall fails again
                    // with the same sticky status; the explicit release then
                    // succeeds. Native consumes instead (see above): the two
                    // routes agree on bytes, status and final zero, not on
                    // the retained handle's recall admission.
                    require_note(problems, observation, "dup", "11");
                    require_note(problems, observation, "release", "0");
                }
            }
            if case.kind == Kind::ShortExport {
                for (key, value) in [
                    ("short", "12"),
                    ("untouched", "1"),
                    ("retry", "0"),
                    ("same", "1"),
                ] {
                    require_note(problems, observation, key, value);
                }
            }
            if last {
                require_note(problems, observation, "close", "0");
            }
        }
        Engine::GeneratedC11 => {
            require_note(problems, observation, "consumed", "1");
            if last {
                for (key, value) in [("handles", "0"), ("close", "0/0"), ("armed", "0")] {
                    require_note(problems, observation, key, value);
                }
            }
        }
        Engine::GeneratedRust => {
            if last {
                for (key, value) in [("handles", "0"), ("close", "0"), ("armed", "0")] {
                    require_note(problems, observation, key, value);
                }
            }
        }
        Engine::GeneratedCxx17 => {
            if last {
                require_note(problems, observation, "close", "0");
                require_note(problems, observation, "armed", "0");
            }
        }
        Engine::GeneratedTypeScript => {
            let cycles = cycle_of(&observation.label) + 1;
            let (value_releases, result_releases) =
                if succeeded { (0, cycles) } else { (cycles, 0) };
            require_note(problems, observation, "vr", &value_releases.to_string());
            require_note(problems, observation, "rr", &result_releases.to_string());
            if last {
                require_note(problems, observation, "closes", "1");
            }
        }
        Engine::Interpreter => {
            require_note(
                problems,
                observation,
                "copyout",
                if succeeded { "2" } else { "0" },
            );
        }
    }
}

/// The observed outcome of one cell.
#[derive(Clone, Debug)]
pub(crate) enum Outcome {
    Pass,
    Fail(Vec<String>),
    NotRun,
}

/// Evaluate one engine's receipts for one case. `receipts` holds exactly the
/// receipt lines labelled with this case.
pub(crate) fn evaluate(case: &Case, engine: Engine, receipts: &[&Observation]) -> Outcome {
    let expected_lines = if case.kind == Kind::Repeated {
        usize::try_from(case.cycles).unwrap()
    } else {
        1
    };
    let trapped = receipts.iter().any(|receipt| receipt.primary == "trap");
    if receipts.len() != expected_lines && !trapped {
        return Outcome::Fail(vec![format!(
            "expected {expected_lines} receipt line(s), observed {}",
            receipts.len()
        )]);
    }
    let mut problems = Vec::new();
    // Between cycles only the open provider may stay live, and that
    // footprint must not grow from one cycle to the next.
    let intermediate = receipts[..receipts.len().saturating_sub(1)]
        .iter()
        .filter_map(|receipt| receipt.live)
        .collect::<Vec<_>>();
    if intermediate.windows(2).any(|pair| pair[0] != pair[1]) {
        problems.push(format!("live grew between cycles: {intermediate:?}"));
    }
    for (index, receipt) in receipts.iter().enumerate() {
        let last = index + 1 == receipts.len();
        problems.extend(check_observation(case, engine, receipt, last));
    }
    if problems.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(problems)
    }
}

/// Native engines observe the same provider: their dispatch counts, peaks
/// and release orders must be identical for each case, not merely each
/// individually plausible.
fn native_agreement(
    case: &Case,
    observed: &BTreeMap<Engine, Vec<Observation>>,
) -> Result<(), String> {
    let mut reference: Option<(Engine, Vec<(Option<u64>, Option<String>, Option<String>)>)> = None;
    for (engine, receipts) in observed {
        if !engine.native() {
            continue;
        }
        let facts = receipts
            .iter()
            .map(|receipt| {
                (
                    receipt.dispatch,
                    receipt.peak.clone(),
                    receipt.order.clone(),
                )
            })
            .collect::<Vec<_>>();
        match &reference {
            None => reference = Some((*engine, facts)),
            Some((first, expected)) if *expected != facts => {
                return Err(format!(
                    "{}: {} {:?} disagrees with {} {:?}",
                    case.id,
                    engine.label(),
                    facts,
                    first.label(),
                    expected
                ))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

pub(crate) struct Matrix {
    pub(crate) cells: Vec<(usize, Engine, Expect, Outcome)>,
}

impl Matrix {
    fn render(&self) -> String {
        let mut text = format!("{CORPUS_VERSION}\n");
        for (index, case) in CASES.iter().enumerate() {
            let _ = write!(text, "{:<38}", case.id);
            for engine in ENGINES {
                let cell = self
                    .cells
                    .iter()
                    .find(|(row, column, _, _)| *row == index && *column == engine)
                    .unwrap();
                let mark = match (&cell.2, &cell.3) {
                    (Expect::Pass, Outcome::Pass) => "PASS".to_owned(),
                    (Expect::NotApplicable(_), _) => "N/A".to_owned(),
                    (Expect::KnownDefect { issue, .. }, Outcome::Fail(_)) => format!("KD{issue}"),
                    _ => "MISMATCH".to_owned(),
                };
                let _ = write!(text, " {}={mark}", engine.label());
            }
            text.push('\n');
        }
        text
    }
}

/// Decide the asserted matrix from observed receipts. Panics with the full
/// rendered matrix on any cell whose outcome differs from its expectation.
pub(crate) fn assert_matrix(observed: &BTreeMap<(usize, Engine), Vec<Observation>>) -> Matrix {
    let mut cells = Vec::new();
    let mut failures = Vec::new();
    for (index, case) in CASES.iter().enumerate() {
        let per_engine: BTreeMap<Engine, Vec<Observation>> = ENGINES
            .iter()
            .filter_map(|engine| {
                observed
                    .get(&(index, *engine))
                    .map(|r| (*engine, r.clone()))
            })
            .collect();
        let agreement = native_agreement(case, &per_engine);
        for engine in ENGINES {
            let expect = expected_cell(case, engine);
            let outcome = match (&expect, per_engine.get(&engine)) {
                (Expect::NotApplicable(_), None) => Outcome::NotRun,
                (Expect::NotApplicable(_), Some(_)) => {
                    Outcome::Fail(vec!["not-applicable cell was executed".to_owned()])
                }
                (_, None) => Outcome::Fail(vec!["required cell produced no receipt".to_owned()]),
                (_, Some(receipts)) => {
                    let refs = receipts.iter().collect::<Vec<_>>();
                    match (evaluate(case, engine, &refs), &agreement) {
                        (Outcome::Pass, Err(problem)) if engine.native() => {
                            Outcome::Fail(vec![problem.clone()])
                        }
                        (outcome, _) => outcome,
                    }
                }
            };
            match (&expect, &outcome) {
                (Expect::Pass, Outcome::Pass) | (Expect::NotApplicable(_), Outcome::NotRun) => {}
                (Expect::KnownDefect { signature, .. }, Outcome::Fail(problems))
                    if problems.iter().any(|problem| problem.contains(signature)) => {}
                _ => failures.push(format!(
                    "{} / {}: expected {expect:?}, observed {outcome:?}",
                    case.id,
                    engine.label()
                )),
            }
            cells.push((index, engine, expect, outcome));
        }
    }
    let matrix = Matrix { cells };
    eprintln!("{}", matrix.render());
    assert!(
        failures.is_empty(),
        "settlement matrix mismatches:\n{}",
        failures.join("\n")
    );
    matrix
}

#[test]
fn shared_settlement_corpus_matrix_is_complete_and_asserted() {
    for (id, engine, _, _) in KNOWN_DEFECTS {
        let case = CASES
            .iter()
            .find(|case| case.id == *id)
            .expect("known defect names a case");
        assert!(
            matches!(expected_cell(case, *engine), Expect::KnownDefect { .. }),
            "{id}"
        );
    }
    let observed = engines::execute_all(&ENGINES, false);
    let matrix = assert_matrix(&observed);
    let passes = matrix
        .cells
        .iter()
        .filter(|cell| matches!((&cell.2, &cell.3), (Expect::Pass, Outcome::Pass)))
        .count();
    let known = matrix
        .cells
        .iter()
        .filter(|cell| matches!(cell.2, Expect::KnownDefect { .. }))
        .count();
    let not_applicable = matrix
        .cells
        .iter()
        .filter(|cell| matches!(cell.2, Expect::NotApplicable(_)))
        .count();
    assert_eq!(passes + known + not_applicable, CASES.len() * ENGINES.len());
    // Exact split, not a non-vacuous floor: this corpus is closed (14 cases x
    // 9 engines = 126 cells), so a changed count means a cell moved classes
    // and the new split must be reviewed and re-pinned, not silently widened.
    assert_eq!(
        (passes, known, not_applicable),
        (105, 0, 21),
        "settlement matrix split changed: {passes} pass, {known} known-defect, \
         {not_applicable} not-applicable cells"
    );
    eprintln!(
        "{CORPUS_VERSION}: {passes} pass, {known} known-defect, {not_applicable} not-applicable cells"
    );
}

/// Negative control: a raw native caller that skips its result release must
/// fail the `success-small` cell (live resources and a refused close), even
/// though its returned bytes are correct.
#[test]
fn skipped_release_negative_control_fails_its_cell() {
    let observed = engines::execute_all(&[Engine::NativeO2], true);
    let index = CASES
        .iter()
        .position(|case| case.id == "success-small")
        .unwrap();
    let receipts = observed
        .get(&(index, Engine::NativeO2))
        .expect("control receipt");
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].leaves, expected_leaves(&CASES[index]));
    let refs = receipts.iter().collect::<Vec<_>>();
    let Outcome::Fail(problems) = evaluate(&CASES[index], Engine::NativeO2, &refs) else {
        panic!("a caller that skips its result release passed its cell");
    };
    assert!(
        problems.iter().any(|problem| problem.starts_with("live:")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|problem| problem.starts_with("note close:")),
        "{problems:?}"
    );
}
