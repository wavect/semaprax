//! Declared `.spx` session protocols (issue #297): lowering to the kernel's
//! [`ProtocolSpec`], the `SPX-K1xx` source checks, the HIR binding of `via`
//! targets, and the one canonical JSON fact every projection (graph,
//! context, architecture, assurance) shares.
//!
//! # What a declaration proves, and what it never grants
//!
//! A declaration is checked and erased. The verifier proves the declared
//! graph is well formed ([`ProtocolSpec::validate`]) and passes the kernel's
//! bounded reachability check ([`check_bounded`] with `bound = states`).
//! A `via` names the ordinary function that realizes a transition in checked
//! source; the binding is by persistent id, resolved against checked HIR.
//!
//! Legal order is not authority. A transition's `requires capability` is
//! ordering metadata: when a `via` function is named, the capability must
//! already be one of that function's declared `uses { ... }` effects, which
//! the ordinary effect checks (`SPX-E101`/`E102`/`E103`) keep authoritative.
//! The declaration never adds an effect, a capability, or a resource token to
//! anything. A capability on a transition without a `via` is realized outside
//! checked source (for example by a Rust subsystem) and is projected as
//! `"capability_binding":"unattributed"`: it is reported, never granted.
//!
//! # Lowering without leaking
//!
//! The kernel's spec type carries `&'static str` names. Lowering maps every
//! distinct state and label to a bounded, lazily grown static symbol pool
//! (`#00000`, `#00001`, ...) in first-appearance order, runs the kernel's
//! checks, and maps each reported symbol back to its source name. Payload,
//! capability, and cleanup-operation text never influence validation, so they
//! lower to one placeholder. The pool is bounded for the process lifetime, so
//! no compile leaks names of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::ast::{
    Program, SessionProtocolDeclaration, SessionProtocolKind, SessionProtocolName,
    SessionProtocolNext, Span,
};
use crate::diagnostic::{quote_json, Diagnostic};

use super::model_check::{check_bounded, ModelCheckError};
use super::spec::{Kind, Next, OwnershipMove, ProtocolSpec, SpecError, Transition};

/// Upper bound on distinct names one declaration can present to lowering:
/// states, initial, terminals, and per transition its source, label, and
/// either one target or every branch label and target.
const MAX_SYMBOLS: usize = 2 * crate::parser::session_protocol::MAX_SESSION_PROTOCOL_STATES
    + 1
    + crate::parser::session_protocol::MAX_SESSION_PROTOCOL_TRANSITIONS
        * (2 + 2 * crate::parser::session_protocol::MAX_SESSION_PROTOCOL_BRANCHES);
const PLACEHOLDER: &str = "_";

/// The `index`th static symbol, or `None` past [`MAX_SYMBOLS`]. The pool
/// grows monotonically and never past that bound, so its total allocation is
/// bounded for the process lifetime and no compile leaks names of its own.
fn symbol(index: usize) -> Option<&'static str> {
    static POOL: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    if index >= MAX_SYMBOLS {
        return None;
    }
    let mut pool = POOL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    while pool.len() <= index {
        let next = format!("#{:05}", pool.len());
        pool.push(Box::leak(next.into_boxed_str()));
    }
    Some(pool[index])
}

/// Distinct names one declaration presents to lowering.
fn distinct_names(declaration: &SessionProtocolDeclaration) -> usize {
    let mut names = BTreeSet::new();
    names.extend(declaration.states.iter().map(|state| state.name.as_str()));
    names.insert(declaration.initial.name.as_str());
    names.extend(
        declaration
            .terminals
            .iter()
            .map(|terminal| terminal.state.name.as_str()),
    );
    for transition in &declaration.transitions {
        names.insert(transition.from.name.as_str());
        names.insert(transition.label.name.as_str());
        match &transition.next {
            SessionProtocolNext::Then(state) => {
                names.insert(state.name.as_str());
            }
            SessionProtocolNext::Choice(branches) => {
                for (label, state) in branches {
                    names.insert(label.name.as_str());
                    names.insert(state.name.as_str());
                }
            }
        }
    }
    names.len()
}

struct Symbols<'d> {
    by_name: BTreeMap<&'d str, &'static str>,
    names: Vec<&'d str>,
}

impl<'d> Symbols<'d> {
    fn new() -> Self {
        Self {
            by_name: BTreeMap::new(),
            names: Vec::new(),
        }
    }

    fn intern(&mut self, name: &'d str) -> &'static str {
        if let Some(symbol) = self.by_name.get(name) {
            return symbol;
        }
        // `kernel_defects` refuses a declaration with more distinct names than
        // the pool holds before lowering, so the placeholder is unreachable.
        let symbol = symbol(self.names.len()).unwrap_or(PLACEHOLDER);
        self.names.push(name);
        self.by_name.insert(name, symbol);
        symbol
    }

    fn name(&self, symbol: &str) -> &'d str {
        symbol
            .strip_prefix('#')
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| self.names.get(index).copied())
            .unwrap_or("?")
    }
}

pub(crate) fn kernel_kind(kind: SessionProtocolKind) -> Kind {
    match kind {
        SessionProtocolKind::Send => Kind::Send,
        SessionProtocolKind::Receive => Kind::Receive,
        SessionProtocolKind::Call => Kind::Call,
        SessionProtocolKind::Return => Kind::Return,
        SessionProtocolKind::Cancel => Kind::Cancel,
        SessionProtocolKind::Timeout => Kind::Timeout,
        SessionProtocolKind::Fail => Kind::Fail,
    }
}

/// Lower one declaration onto the kernel and run both of its static checks,
/// returning source-named defect messages (sorted, deduplicated) for each.
/// `Err(count)` when the declaration presents more distinct names than the
/// lowering pool admits (`SPX-K106`); nothing is lowered then.
fn kernel_defects(
    declaration: &SessionProtocolDeclaration,
) -> Result<(Vec<String>, Vec<String>), usize> {
    let count = distinct_names(declaration);
    if count > MAX_SYMBOLS {
        return Err(count);
    }
    let mut symbols = Symbols::new();
    let states = declaration
        .states
        .iter()
        .map(|state| symbols.intern(&state.name))
        .collect::<BTreeSet<_>>();
    let initial = symbols.intern(&declaration.initial.name);
    let terminal = declaration
        .terminals
        .iter()
        .map(|terminal| symbols.intern(&terminal.state.name))
        .collect::<BTreeSet<_>>();
    let cleanup = declaration
        .terminals
        .iter()
        .map(|terminal| (symbols.intern(&terminal.state.name), vec![PLACEHOLDER]))
        .collect::<Vec<_>>();
    let transitions = declaration
        .transitions
        .iter()
        .map(|transition| Transition {
            from: symbols.intern(&transition.from.name),
            label: symbols.intern(&transition.label.name),
            kind: kernel_kind(transition.kind),
            payload_type: PLACEHOLDER,
            required_capability: None,
            ownership: OwnershipMove::None,
            next: match &transition.next {
                SessionProtocolNext::Then(state) => Next::Then(symbols.intern(&state.name)),
                SessionProtocolNext::Choice(branches) => Next::Choice(
                    branches
                        .iter()
                        .map(|(label, state)| {
                            (symbols.intern(&label.name), symbols.intern(&state.name))
                        })
                        .collect(),
                ),
            },
        })
        .collect::<Vec<_>>();
    let spec = ProtocolSpec {
        name: PLACEHOLDER,
        states,
        initial,
        terminal,
        transitions,
        cleanup,
    };
    let mut validation = spec
        .validate()
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|error| spec_error_text(&error, &symbols))
        .collect::<Vec<_>>();
    validation.sort();
    validation.dedup();
    let mut model = if validation.is_empty() {
        check_bounded(&spec, spec.states.len())
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|error| match error {
                ModelCheckError::UnreachableState { state } => format!(
                    "state `{}` is unreachable from `{}`",
                    symbols.name(state),
                    declaration.initial.name
                ),
                ModelCheckError::NoBoundedPathToTerminal { state, bound } => format!(
                    "state `{}` has no path of at most {bound} transitions to a terminal state",
                    symbols.name(state)
                ),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    model.sort();
    model.dedup();
    Ok((validation, model))
}

fn spec_error_text(error: &SpecError, symbols: &Symbols<'_>) -> String {
    let n = |symbol: &str| symbols.name(symbol).to_owned();
    match error {
        SpecError::UnknownInitialState => "initial state is not a declared state".to_owned(),
        SpecError::UnknownTerminalState { state } => {
            format!("terminal `{}` is not a declared state", n(state))
        }
        SpecError::UnknownState { label, state } => format!(
            "transition `{}` names undeclared state `{}`",
            n(label),
            n(state)
        ),
        SpecError::DuplicateLabelFromState { state, label } => format!(
            "state `{}` declares transition `{}` more than once",
            n(state),
            n(label)
        ),
        SpecError::ChoiceNeedsAtLeastTwo { state, label } => format!(
            "choice on `{}.{}` needs at least two branches",
            n(state),
            n(label)
        ),
        SpecError::DuplicateChoiceLabel {
            state,
            label,
            choice,
        } => format!(
            "choice on `{}.{}` repeats branch `{}`",
            n(state),
            n(label),
            n(choice)
        ),
        SpecError::TerminalStateHasOutgoingTransition { state, label } => format!(
            "terminal state `{}` has outgoing transition `{}`",
            n(state),
            n(label)
        ),
        SpecError::DeadEnd { state } => {
            format!(
                "non-terminal state `{}` has no outgoing transition",
                n(state)
            )
        }
        SpecError::MissingEscape { state } => format!(
            "non-terminal state `{}` has no `cancel`, `timeout`, or `fail` escape",
            n(state)
        ),
        SpecError::MissingCleanupEntry { state } => {
            format!("terminal `{}` has no cleanup inventory", n(state))
        }
    }
}

fn k_error(program: &Program, code: &'static str, message: String, span: Span) -> Diagnostic {
    Diagnostic::error(code, message, span).at_path(&program.path)
}

/// `SPX-K1xx` source checks for every declared session protocol:
///
/// - `SPX-K101` duplicate protocol name or identity, identity colliding with
///   another declaration, or a state/terminal/cleanup name repeated in one set;
/// - `SPX-K102` the kernel's static validation refused the declared graph;
/// - `SPX-K103` the kernel's bounded reachability check refused it;
/// - `SPX-K104` a `via` names no ordinary monomorphic function in this module;
/// - `SPX-K105` a `requires capability` names an effect its `via` function
///   does not declare (ordering metadata cannot mint authority).
///
/// `SPX-K106` (capacity) is raised by the parser.
pub(crate) fn check(program: &Program) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if program.session_protocols.is_empty() {
        return diagnostics;
    }
    let mut taken = program
        .functions
        .iter()
        .map(|function| function.stable_id.as_str())
        .chain(program.types.iter().map(|item| item.stable_id.as_str()))
        .chain(
            program
                .interfaces
                .iter()
                .map(|item| item.stable_id.as_str()),
        )
        .chain(program.protocols.iter().map(|item| item.stable_id.as_str()))
        .collect::<BTreeSet<_>>();
    let mut names = BTreeSet::new();
    let functions = program
        .functions
        .iter()
        .map(|function| (function.stable_id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    for declaration in &program.session_protocols {
        if !names.insert(declaration.name.as_str()) {
            diagnostics.push(k_error(
                program,
                "SPX-K101",
                format!("duplicate session protocol `{}`", declaration.name),
                declaration.name_span,
            ));
        }
        if !taken.insert(declaration.stable_id.as_str()) {
            diagnostics.push(k_error(
                program,
                "SPX-K101",
                format!(
                    "session protocol identity `{}` is already declared",
                    declaration.stable_id
                ),
                declaration.name_span,
            ));
        }
        let terminal_states = declaration
            .terminals
            .iter()
            .map(|terminal| terminal.state.clone())
            .collect::<Vec<_>>();
        for (set, entries) in [
            ("state", &declaration.states),
            ("terminal", &terminal_states),
        ] {
            if let Some(repeated) = first_repeat(entries) {
                diagnostics.push(k_error(
                    program,
                    "SPX-K101",
                    format!(
                        "session protocol `{}` repeats {set} `{}`",
                        declaration.name, repeated.name
                    ),
                    repeated.span,
                ));
            }
        }
        for terminal in &declaration.terminals {
            if let Some(repeated) = first_repeat(&terminal.cleanup) {
                diagnostics.push(k_error(
                    program,
                    "SPX-K101",
                    format!(
                        "terminal `{}` repeats cleanup operation `{}`",
                        terminal.state.name, repeated.name
                    ),
                    repeated.span,
                ));
            }
        }
        let (validation, model) = match kernel_defects(declaration) {
            Ok(defects) => defects,
            Err(count) => {
                diagnostics.push(k_error(
                    program,
                    "SPX-K106",
                    format!(
                        "session protocol `{}` presents {count} distinct names; lowering admits at most {MAX_SYMBOLS}",
                        declaration.name
                    ),
                    declaration.name_span,
                ));
                continue;
            }
        };
        for message in validation {
            diagnostics.push(k_error(
                program,
                "SPX-K102",
                format!("session protocol `{}`: {message}", declaration.name),
                declaration.name_span,
            ));
        }
        for message in model {
            diagnostics.push(k_error(
                program,
                "SPX-K103",
                format!("session protocol `{}`: {message}", declaration.name),
                declaration.name_span,
            ));
        }
        for transition in &declaration.transitions {
            let Some(via) = &transition.via else {
                continue;
            };
            let Some(function) = functions
                .get(via.name.as_str())
                .filter(|function| function.type_parameters.is_empty())
            else {
                diagnostics.push(
                    k_error(
                        program,
                        "SPX-K104",
                        format!(
                            "transition `{}.{}` is realized `via` `{}`, which is not an ordinary monomorphic function in this module",
                            transition.from.name, transition.label.name, via.name
                        ),
                        via.span,
                    )
                    .with_help("name the realizing function by its `@id`"),
                );
                continue;
            };
            if let Some(capability) = &transition.capability {
                if !function
                    .effects
                    .iter()
                    .any(|effect| effect == &capability.name)
                {
                    diagnostics.push(
                        k_error(
                            program,
                            "SPX-K105",
                            format!(
                                "transition `{}.{}` requires capability `{}`, but its `via` function `{}` does not declare that effect",
                                transition.from.name,
                                transition.label.name,
                                capability.name,
                                function.name
                            ),
                            capability.span,
                        )
                        .with_help(
                            "a session protocol never grants authority; declare the effect in the function's `uses { ... }` and permit it in the module",
                        ),
                    );
                }
            }
        }
    }
    diagnostics
}

fn first_repeat(entries: &[SessionProtocolName]) -> Option<&SessionProtocolName> {
    let mut seen = BTreeSet::new();
    entries
        .iter()
        .find(|entry| !seen.insert(entry.name.as_str()))
}

/// HIR binding: every `via` of every declaration in `program` names a
/// function retained in the checked HIR `resolved` built from that same
/// program. Source verification already refused anything else (`SPX-K104`),
/// so a miss here is an inconsistent source/HIR pair and fails closed. Every
/// projection calls this before it emits a declaration fact.
pub(crate) fn bind_to_hir(
    program: &Program,
    resolved: &crate::hir::ResolvedProgram,
) -> Result<(), Diagnostic> {
    if program.session_protocols.is_empty() {
        return Ok(());
    }
    let retained = resolved
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<BTreeSet<_>>();
    for declaration in &program.session_protocols {
        for transition in &declaration.transitions {
            if let Some(via) = &transition.via {
                if !retained.contains(via.name.as_str()) {
                    return Err(Diagnostic::io(
                        "SPX-K104",
                        format!(
                            "session protocol `{}` names `via` `{}`, which checked HIR does not retain",
                            declaration.name, via.name
                        ),
                    )
                    .at_path(&program.path));
                }
            }
        }
    }
    Ok(())
}

fn string_array<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let mut rendered = String::from("[");
    for (index, value) in values.enumerate() {
        if index != 0 {
            rendered.push(',');
        }
        rendered.push_str(&quote_json(value));
    }
    rendered.push(']');
    rendered
}

fn span_json(span: Span) -> String {
    format!(
        "{{\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}",
        span.start, span.end, span.line, span.column
    )
}

/// The canonical, deterministic fact for one verified declaration, shared by
/// every projection. Fixed key order; `null` for absent optional values.
/// `static_validation` is `"passed"` only for a declaration that survived
/// `SPX-K102`/`SPX-K103`, which every caller guarantees by projecting only
/// verified programs. `authority` is always `"none"`.
pub(crate) fn declaration_json(declaration: &SessionProtocolDeclaration) -> String {
    let (validation, model) =
        kernel_defects(declaration).unwrap_or_else(|_| (vec!["capacity".to_owned()], Vec::new()));
    let terminals = declaration
        .terminals
        .iter()
        .map(|terminal| {
            format!(
                "{{\"state\":{},\"cleanup\":{}}}",
                quote_json(&terminal.state.name),
                string_array(terminal.cleanup.iter().map(|op| op.name.as_str()))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let transitions = declaration
        .transitions
        .iter()
        .map(|transition| {
            let next = match &transition.next {
                SessionProtocolNext::Then(state) => {
                    format!("{{\"kind\":\"then\",\"state\":{}}}", quote_json(&state.name))
                }
                SessionProtocolNext::Choice(branches) => format!(
                    "{{\"kind\":\"choice\",\"branches\":[{}]}}",
                    branches
                        .iter()
                        .map(|(label, state)| format!(
                            "{{\"label\":{},\"state\":{}}}",
                            quote_json(&label.name),
                            quote_json(&state.name)
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            };
            let capability_binding = match (&transition.capability, &transition.via) {
                (None, _) => "null".to_owned(),
                (Some(_), Some(_)) => "\"via_declared_effect\"".to_owned(),
                (Some(_), None) => "\"unattributed\"".to_owned(),
            };
            format!(
                "{{\"from\":{},\"label\":{},\"kind\":{},\"payload_type\":{},\"required_capability\":{},\"capability_binding\":{},\"ownership\":{},\"via\":{},\"next\":{},\"span\":{}}}",
                quote_json(&transition.from.name),
                quote_json(&transition.label.name),
                quote_json(transition.kind.keyword()),
                quote_json(&transition.payload.name),
                transition
                    .capability
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |c| quote_json(&c.name)),
                capability_binding,
                quote_json(if transition.consumes_resource {
                    "consumes_resource"
                } else {
                    "none"
                }),
                transition
                    .via
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |v| quote_json(&v.name)),
                next,
                span_json(transition.span)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"stable_id\":{},\"name\":{},\"span\":{},\"states\":{},\"initial\":{},\"terminals\":[{}],\"transitions\":[{}],\"static_validation\":{},\"bounded_reachability\":{},\"authority\":\"none\"}}",
        quote_json(&declaration.stable_id),
        quote_json(&declaration.name),
        span_json(declaration.span),
        string_array(declaration.states.iter().map(|state| state.name.as_str())),
        quote_json(&declaration.initial.name),
        terminals,
        transitions,
        quote_json(if validation.is_empty() { "passed" } else { "refused" }),
        quote_json(if validation.is_empty() && model.is_empty() {
            "passed"
        } else {
            "refused"
        }),
    )
}

/// JSON array of [`declaration_json`] for every declaration, in source order.
pub(crate) fn declarations_json(declarations: &[SessionProtocolDeclaration]) -> String {
    format!(
        "[{}]",
        declarations
            .iter()
            .map(declaration_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Field-for-field comparison of a declaration against a built-in kernel
/// spec: name, state set, initial, terminal set, per-terminal cleanup
/// inventory (in canonical order), and every transition (in order, including
/// payload, capability, ownership, and continuation). Returns every
/// divergence, empty when the declaration is an exact transcription.
pub fn drift_from_spec(
    declaration: &SessionProtocolDeclaration,
    spec: &ProtocolSpec,
) -> Vec<String> {
    let mut drift = Vec::new();
    if declaration.name != spec.name {
        drift.push(format!("name `{}` != `{}`", declaration.name, spec.name));
    }
    let states = declaration
        .states
        .iter()
        .map(|state| state.name.as_str())
        .collect::<BTreeSet<_>>();
    if states != spec.states {
        drift.push(format!("states {states:?} != {:?}", spec.states));
    }
    if declaration.initial.name != spec.initial {
        drift.push(format!(
            "initial `{}` != `{}`",
            declaration.initial.name, spec.initial
        ));
    }
    let terminal = declaration
        .terminals
        .iter()
        .map(|terminal| terminal.state.name.as_str())
        .collect::<BTreeSet<_>>();
    if terminal != spec.terminal {
        drift.push(format!("terminal {terminal:?} != {:?}", spec.terminal));
    }
    let cleanup = declaration
        .terminals
        .iter()
        .map(|terminal| {
            (
                terminal.state.name.as_str(),
                terminal
                    .cleanup
                    .iter()
                    .map(|op| op.name.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    if cleanup != spec.cleanup {
        drift.push(format!("cleanup {cleanup:?} != {:?}", spec.cleanup));
    }
    if declaration.transitions.len() != spec.transitions.len() {
        drift.push(format!(
            "transition count {} != {}",
            declaration.transitions.len(),
            spec.transitions.len()
        ));
    }
    for (index, (declared, kernel)) in declaration
        .transitions
        .iter()
        .zip(&spec.transitions)
        .enumerate()
    {
        let next_matches = match (&declared.next, &kernel.next) {
            (SessionProtocolNext::Then(state), Next::Then(target)) => state.name == *target,
            (SessionProtocolNext::Choice(branches), Next::Choice(targets)) => {
                branches.len() == targets.len()
                    && branches
                        .iter()
                        .zip(targets)
                        .all(|((label, state), (l, s))| label.name == *l && state.name == *s)
            }
            _ => false,
        };
        let ownership = if declared.consumes_resource {
            OwnershipMove::ConsumesResource
        } else {
            OwnershipMove::None
        };
        if declared.from.name != kernel.from
            || declared.label.name != kernel.label
            || kernel_kind(declared.kind) != kernel.kind
            || declared.payload.name != kernel.payload_type
            || declared.capability.as_ref().map(|c| c.name.as_str()) != kernel.required_capability
            || ownership != kernel.ownership
            || !next_matches
        {
            drift.push(format!(
                "transition {index} `{}.{}` differs from kernel `{}.{}`",
                declared.from.name, declared.label.name, kernel.from, kernel.label
            ));
        }
    }
    drift
}
