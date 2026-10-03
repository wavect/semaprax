//! Deterministic Lean 4 rendering for the admitted profile.
//!
//! # The model, and why it is faithful
//!
//! Every admitted SEMAPRAX scalar is translated to Lean's *unbounded*
//! `Int`, never to a fixed-width type. That would be unfaithful on its own —
//! SEMAPRAX arithmetic is checked (trapping, never wrapping), so `a + b` at
//! `i64` is not mathematical addition. The translation closes that gap the
//! same way [`crate::assurance_manifest::smt_discharge`] does for SMT: every
//! arithmetic node emits its own *separate* range obligation theorem
//! (`min ≤ term ∧ term ≤ max` for the node's declared type), and a
//! postcondition theorem is only ever reported as kernel-checked when every
//! range obligation of the same function was kernel-checked in the same
//! file. Under that conjunction the `Int` term and the runtime value
//! coincide on every input satisfying the hypotheses, so proving the
//! postcondition over `Int` proves it for the trapping semantics.
//!
//! A parameter's declared range is a *hypothesis* (`min ≤ a`, `a ≤ max`);
//! an arithmetic result's range is an *obligation*. Conflating the two is
//! issue #184's worst bug (a `result` range axiom made a real overflow
//! vacuously provable), and the shapes here are deliberately not
//! interchangeable: hypotheses are appended to the binder list, obligations
//! never are.
//!
//! # Determinism
//!
//! The rendered bytes are a pure function of the parsed program: source
//! order everywhere, no map iteration, no timestamps, no host paths (the
//! header carries the module name and the semantic revision, never the file
//! path, so the same module exported from two checkouts renders identically).

use std::fmt::Write as _;

use crate::assurance_manifest::law_vc::{self, Definition, Stage, Term as VcTerm, TermKind};
use crate::assurance_manifest::smt_discharge::{NumericMode, Sort, UnsupportedReason};
use crate::ast::{BinaryOp, Function, Program, TypeDeclarationKind, UnaryOp};

use super::profile::{admit_declaration, value_mode, Excluded, PROFILE_V1};

/// The wire identity of the generated Lean document.
pub const EXPORT_SCHEMA: &str = "semaprax.lean-obligation-export.v1";

/// The single Lean namespace every generated theorem lives in. Fixed, so
/// `#print axioms` lines and certificate `theorem_name` fields are
/// unambiguous.
pub const NAMESPACE: &str = "SemapraxExport";

/// The one tactic this profile emits. Lean 4 core's `omega` decides linear
/// integer arithmetic; it is not a hint the kernel may ignore, it either
/// closes the goal or the file fails to build. A nonlinear goal (any
/// `mul` of two non-constant terms) will simply fail here, which is the
/// intended outcome: no certificate, rather than an unproved claim.
pub const TACTIC: &str = "omega";

/// Every assumption this translation introduces, enumerated in one place
/// and reproduced verbatim in the Lean header, in the coverage report, and
/// in every certificate. Issue #186 names "implicit axioms or admitted
/// lemmas can make success meaningless" as a top failure mode; the defense
/// is that there is exactly one list and it is embedded everywhere the
/// claim travels.
pub const ASSUMPTIONS: [(&str, &str); 8] = [
    (
        "A1-int-model",
        "SEMAPRAX scalars are modeled as Lean `Int` with declared-range hypotheses; this is \
faithful to SEMAPRAX's checked (trapping) arithmetic only in conjunction with A2.",
    ),
    (
        "A2-range-obligations-required",
        "Every arithmetic node emits a separate range obligation theorem; a postcondition is \
claimed only when every range obligation of the same function was kernel-checked in the same \
file.",
    ),
    (
        "A3-requires-assumed",
        "`requires` clauses are assumed, not proved: this export says nothing about whether any \
caller establishes them.",
    ),
    (
        "A4-lean-standard-axioms",
        "Lean's three standard axioms (propext, Classical.choice, Quot.sound) are trusted; any \
other axiom, including sorryAx, invalidates the result.",
    ),
    (
        "A5-lean-kernel-tcb",
        "The pinned Lean 4 toolchain and its kernel are trusted; they are not verified by \
SEMAPRAX.",
    ),
    (
        "A6-translation-tcb",
        "This Rust translation from validated SEMAPRAX source to Lean is itself trusted and \
unverified; a translation bug is not detectable by the Lean kernel.",
    ),
    (
        "A7-purity",
        "The admitted profile is pure, total and effect-free, so SEMAPRAX's left-to-right \
evaluation order is unobservable and is not modeled.",
    ),
    (
        "A8-no-lowering-claim",
        "Nothing here claims the native or Wasm lowering preserves a proved source theorem; a \
certificate binds artifact bytes, it does not verify the backend.",
    ),
];

/// Escape one arbitrary SEMAPRAX identifier or stable id into an injective
/// Lean-safe suffix: ASCII alphanumerics survive, every other byte becomes
/// `_` followed by exactly two lowercase hex digits. Because `_` itself is
/// escaped (`_5f`), the encoding is prefix-free and therefore injective —
/// two distinct stable ids can never collide on one theorem name, which is
/// issue #186's named "name sanitization can collide and misassociate
/// declarations" failure mode.
#[must_use]
pub fn escape_ident(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() * 2);
    for byte in raw.as_bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push(*byte as char);
        } else {
            let _ = write!(out, "_{byte:02x}");
        }
    }
    out
}

fn int_literal(value: i128) -> String {
    if value < 0 {
        format!("(-{} : Int)", -value)
    } else {
        format!("({value} : Int)")
    }
}

/// One translated expression: its Lean text plus the sort it produced.
struct Term {
    text: String,
    sort: Sort,
}

/// A range obligation discovered during the walk, together with how many
/// binders were in scope when it was discovered. Binders are appended in
/// evaluation order, so the first `binder_count` of them are exactly the
/// hypotheses that are legitimately available at that program point —
/// assuming a later `requires` clause, or the body's own result, while
/// proving an earlier clause's overflow-freedom would be unsound.
struct Pending {
    binder_count: usize,
    guard: String,
    term: String,
    mode: NumericMode,
    origin: String,
}

/// One obligation the export publishes: a theorem the Lean kernel must
/// accept, its stable obligation id, and the goal text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportedObligation {
    pub obligation_id: String,
    pub theorem_name: String,
    /// `"checked_arithmetic_range"` or `"postcondition"`. Closed vocabulary.
    pub kind: &'static str,
    /// `Some(index)` for a postcondition, `None` for a range obligation.
    pub ensures_index: Option<usize>,
    pub origin: String,
    pub goal: String,
    binder_count: usize,
}

/// Everything one admitted function contributes to the generated file.
pub struct FunctionExport {
    pub declaration_id: String,
    pub name: String,
    binders: Vec<String>,
    pub obligations: Vec<ExportedObligation>,
}

#[cfg(test)]
impl FunctionExport {
    /// Test-only: a detached copy of this declaration's obligations, so a
    /// test can build a deliberately tampered certificate without borrowing
    /// the export it is about to mutate.
    pub(super) fn clone_for_test(&self) -> Vec<ExportedObligation> {
        self.obligations.clone()
    }
}

/// The stable id of one checked-arithmetic range obligation.
///
/// Deliberately *not* an Assurance Manifest obligation id: the manifest's
/// `ObligationKind` has no range/overflow kind, and minting one under an
/// existing kind's token would misreport what was proved. Postcondition
/// obligations do reuse the manifest's own id function — see
/// [`super::postcondition_obligation_id`].
#[must_use]
pub fn range_obligation_id(declaration_id: &str, index: usize) -> String {
    format!(
        "semaprax.lean-export.range.v1:{}:{declaration_id}:{index}",
        declaration_id.len()
    )
}

struct Builder {
    binders: Vec<String>,
    pending: Vec<Pending>,
    scope: Vec<(usize, Term)>,
    lets: usize,
}

impl Builder {
    fn record_range(&mut self, term: &str, mode: NumericMode, origin: &str, guard: &str) {
        self.pending.push(Pending {
            binder_count: self.binders.len(),
            guard: guard.to_owned(),
            term: term.to_owned(),
            mode,
            origin: origin.to_owned(),
        });
    }

    fn lookup(&self, binding: usize) -> Option<&Term> {
        self.scope
            .iter()
            .rev()
            .find(|(id, _)| *id == binding)
            .map(|(_, term)| term)
    }

    fn translate(&mut self, expr: &VcTerm, origin: &str, guard: &str) -> Result<Term, Excluded> {
        match &expr.kind {
            TermKind::Number(value) => Ok(Term {
                text: int_literal(*value),
                sort: expr.sort,
            }),
            TermKind::Bool(value) => Ok(Term {
                text: if *value { "True" } else { "False" }.to_owned(),
                sort: Sort::Bool,
            }),
            TermKind::Binding { id, name } => self
                .lookup(*id)
                .map(|term| Term {
                    text: term.text.clone(),
                    sort: term.sort,
                })
                .ok_or_else(|| {
                    Excluded::Shared(UnsupportedReason::UnknownName { name: name.clone() })
                }),
            TermKind::Unary { op, value } => {
                let inner = self.translate(value, origin, guard)?;
                match op {
                    UnaryOp::Neg => {
                        let Sort::Numeric(mode) = inner.sort else {
                            return Err(Excluded::OperandSort {
                                op: "neg",
                                wanted: "numeric",
                            });
                        };
                        let text = format!("(-{})", inner.text);
                        self.record_range(&text, mode, origin, guard);
                        Ok(Term {
                            text,
                            sort: Sort::Numeric(mode),
                        })
                    }
                    UnaryOp::Not => {
                        if inner.sort != Sort::Bool {
                            return Err(Excluded::OperandSort {
                                op: "not",
                                wanted: "boolean",
                            });
                        }
                        Ok(Term {
                            text: format!("(¬{})", inner.text),
                            sort: Sort::Bool,
                        })
                    }
                }
            }
            TermKind::Binary { op, left, right } => {
                self.translate_binary(*op, left, right, origin, guard)
            }
            TermKind::Block { definitions, tail } => {
                let depth = self.scope.len();
                for definition in definitions {
                    self.translate_definition(definition, origin, guard)?;
                }
                let tail_term = self.translate(tail, origin, guard)?;
                self.scope.truncate(depth);
                Ok(tail_term)
            }
            TermKind::If { .. } => Err(Excluded::Conditional),
        }
    }

    fn translate_binary(
        &mut self,
        op: BinaryOp,
        left: &VcTerm,
        right: &VcTerm,
        origin: &str,
        guard: &str,
    ) -> Result<Term, Excluded> {
        if let BinaryOp::Div | BinaryOp::Rem = op {
            let what = if op == BinaryOp::Div {
                "division (truncating-division encoding deferred)"
            } else {
                "remainder (truncating-division encoding deferred)"
            };
            return Err(Excluded::Shared(UnsupportedReason::Expr { what }));
        }
        let lhs = self.translate(left, origin, guard)?;
        let rhs_guard = match op {
            BinaryOp::And => format!("({guard} ∧ {})", lhs.text),
            BinaryOp::Or => format!("({guard} ∧ ¬{})", lhs.text),
            _ => guard.to_owned(),
        };
        let rhs = self.translate(right, origin, &rhs_guard)?;
        let (symbol, op_name) = match op {
            BinaryOp::Add => ("+", "add"),
            BinaryOp::Sub => ("-", "sub"),
            BinaryOp::Mul => ("*", "mul"),
            BinaryOp::Eq => ("=", "eq"),
            BinaryOp::Ne => ("≠", "ne"),
            BinaryOp::Lt => ("<", "lt"),
            BinaryOp::Le => ("≤", "le"),
            BinaryOp::Gt => (">", "gt"),
            BinaryOp::Ge => ("≥", "ge"),
            BinaryOp::And => ("∧", "and"),
            BinaryOp::Or => ("∨", "or"),
            BinaryOp::Div | BinaryOp::Rem => unreachable!("rejected above"),
        };
        let text = format!("({} {symbol} {})", lhs.text, rhs.text);
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                let (Sort::Numeric(left_mode), Sort::Numeric(right_mode)) = (lhs.sort, rhs.sort)
                else {
                    return Err(Excluded::OperandSort {
                        op: op_name,
                        wanted: "numeric",
                    });
                };
                if left_mode != right_mode {
                    return Err(Excluded::Shared(UnsupportedReason::OperandTypeMismatch {
                        op: op_name,
                    }));
                }
                self.record_range(&text, left_mode, origin, guard);
                Ok(Term {
                    text,
                    sort: Sort::Numeric(left_mode),
                })
            }
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => {
                let (Sort::Numeric(left_mode), Sort::Numeric(right_mode)) = (lhs.sort, rhs.sort)
                else {
                    return Err(Excluded::OperandSort {
                        op: op_name,
                        wanted: "numeric",
                    });
                };
                if left_mode != right_mode {
                    return Err(Excluded::Shared(UnsupportedReason::OperandTypeMismatch {
                        op: op_name,
                    }));
                }
                Ok(Term {
                    text,
                    sort: Sort::Bool,
                })
            }
            BinaryOp::And | BinaryOp::Or => {
                if lhs.sort != Sort::Bool || rhs.sort != Sort::Bool {
                    return Err(Excluded::OperandSort {
                        op: op_name,
                        wanted: "boolean",
                    });
                }
                Ok(Term {
                    text,
                    sort: Sort::Bool,
                })
            }
            BinaryOp::Div | BinaryOp::Rem => unreachable!("rejected above"),
        }
    }

    fn translate_definition(
        &mut self,
        definition: &Definition,
        origin: &str,
        guard: &str,
    ) -> Result<(), Excluded> {
        let bound = self.translate(&definition.value, origin, guard)?;
        let Sort::Numeric(mode) = bound.sort else {
            return Err(Excluded::BoolValued {
                position: format!("`let {}`", definition.name),
            });
        };
        let index = self.lets;
        self.lets += 1;
        let binder = format!("v_{}_{index}", escape_ident(&definition.name));
        self.binders.push(format!("({binder} : Int)"));
        self.binders
            .push(format!("(h_def_{index} : {binder} = {})", bound.text));
        self.scope.push((
            definition.binding,
            Term {
                text: binder,
                sort: Sort::Numeric(mode),
            },
        ));
        Ok(())
    }
}

/// Translate one function into its obligations, or return the single closed
/// reason it is outside the profile. Wholesale: a function contributes every
/// obligation or none, so a partially translated declaration can never leave
/// an un-exported construct silently unaccounted for.
pub fn export_function(function: &Function) -> Result<FunctionExport, Excluded> {
    admit_declaration(function)?;
    let return_mode = value_mode(&function.return_type, "return type")?;
    let subject = law_vc::build(function).map_err(Excluded::Shared)?;

    let mut builder = Builder {
        binders: Vec::new(),
        pending: Vec::new(),
        scope: Vec::new(),
        lets: 0,
    };

    for param in &subject.parameters {
        let Sort::Numeric(mode) = param.sort else {
            unreachable!("admit_declaration excludes bool-valued parameters");
        };
        let binder = format!("v_{}", escape_ident(&param.name));
        builder.binders.push(format!("({binder} : Int)"));
        builder.scope.push((
            param.binding,
            Term {
                text: binder,
                sort: Sort::Numeric(mode),
            },
        ));
    }
    for (index, param) in subject.parameters.iter().enumerate() {
        let Sort::Numeric(mode) = param.sort else {
            unreachable!("checked above");
        };
        let binder = format!("v_{}", escape_ident(&param.name));
        builder.binders.push(format!(
            "(h_lo_{index} : {} ≤ {binder})",
            int_literal(mode.min())
        ));
        builder.binders.push(format!(
            "(h_hi_{index} : {binder} ≤ {})",
            int_literal(mode.max())
        ));
    }

    for (index, clause) in subject.requires.iter().enumerate() {
        let origin = format!("requires[{index}]");
        let term = builder.translate(clause, &origin, "True")?;
        if term.sort != Sort::Bool {
            return Err(Excluded::OperandSort {
                op: "requires",
                wanted: "boolean",
            });
        }
        builder
            .binders
            .push(format!("(h_req_{index} : {})", term.text));
    }

    let body = builder.translate(
        subject.result.as_ref().expect("ensures requires result"),
        "body",
        "True",
    )?;
    match body.sort {
        Sort::Numeric(mode) if mode == return_mode => {}
        _ => {
            return Err(Excluded::Shared(UnsupportedReason::TypeMismatch {
                detail: "the body's type disagrees with the declared return type".to_owned(),
            }))
        }
    }
    builder.binders.push("(result : Int)".to_owned());
    builder
        .binders
        .push(format!("(h_result : result = {})", body.text));
    builder.scope.push((
        subject.result_binding.expect("result binding"),
        Term {
            text: "result".to_owned(),
            sort: Sort::Numeric(return_mode),
        },
    ));

    let mut ensures_goals = Vec::new();
    for (index, clause) in subject.ensures.iter().enumerate() {
        let origin = format!("ensures[{index}]");
        let term = builder.translate(clause, &origin, "True")?;
        if term.sort != Sort::Bool {
            return Err(Excluded::OperandSort {
                op: "ensures",
                wanted: "boolean",
            });
        }
        ensures_goals.push((index, term.text));
    }
    let all_binders = builder.binders.len();

    let observed = builder
        .pending
        .iter()
        .map(|pending| {
            let stage = if pending.origin == "body" {
                Some(Stage::Body)
            } else if let Some(index) = pending
                .origin
                .strip_prefix("requires[")
                .and_then(|rest| rest.strip_suffix(']'))
                .and_then(|index| index.parse().ok())
            {
                Some(Stage::Requires(index))
            } else {
                pending
                    .origin
                    .strip_prefix("ensures[")
                    .and_then(|rest| rest.strip_suffix(']'))
                    .and_then(|index| index.parse().ok())
                    .map(Stage::Ensures)
            };
            stage.map(|stage| (stage, pending.mode))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            Excluded::Shared(UnsupportedReason::TypeMismatch {
                detail: "Lean operation origin is outside the shared typed VC stages".to_owned(),
            })
        })?;
    let expected = subject
        .operations
        .iter()
        .map(|operation| (operation.stage, operation.mode))
        .collect::<Vec<_>>();
    if observed != expected {
        return Err(Excluded::Shared(UnsupportedReason::TypeMismatch {
            detail: "Lean obligations disagree with the shared typed VC subject".to_owned(),
        }));
    }

    let stem = escape_ident(&function.stable_id);
    let mut obligations = Vec::with_capacity(builder.pending.len() + ensures_goals.len());
    for (index, pending) in builder.pending.iter().enumerate() {
        let range_goal = format!(
            "{} ≤ {} ∧ {} ≤ {}",
            int_literal(pending.mode.min()),
            pending.term,
            pending.term,
            int_literal(pending.mode.max())
        );
        obligations.push(ExportedObligation {
            obligation_id: range_obligation_id(&function.stable_id, index),
            theorem_name: format!("spx_{stem}_range_{index}"),
            kind: "checked_arithmetic_range",
            ensures_index: None,
            origin: pending.origin.clone(),
            goal: if pending.guard == "True" {
                range_goal
            } else {
                format!("{} → ({range_goal})", pending.guard)
            },
            binder_count: pending.binder_count,
        });
    }
    for (index, goal) in ensures_goals {
        obligations.push(ExportedObligation {
            obligation_id: super::postcondition_obligation_id(&function.stable_id, index),
            theorem_name: format!("spx_{stem}_ensures_{index}"),
            kind: "postcondition",
            ensures_index: Some(index),
            origin: format!("ensures[{index}]"),
            goal,
            binder_count: all_binders,
        });
    }

    Ok(FunctionExport {
        declaration_id: function.stable_id.clone(),
        name: function.name.clone(),
        binders: builder.binders,
        obligations,
    })
}

fn render_theorem(export: &FunctionExport, obligation: &ExportedObligation) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "/-- SEMAPRAX declaration `{}` (`{}`) — {} obligation, origin `{}`.",
        export.declaration_id, export.name, obligation.kind, obligation.origin
    );
    let _ = writeln!(out, "    Obligation id: `{}`. -/", obligation.obligation_id);
    let _ = writeln!(out, "theorem {}", obligation.theorem_name);
    for binder in &export.binders[..obligation.binder_count] {
        let _ = writeln!(out, "    {binder}");
    }
    let _ = writeln!(out, "    : {} := by", obligation.goal);
    let _ = writeln!(out, "  {TACTIC}");
    out
}

/// One whole-module export: the Lean document plus the per-declaration
/// accounting a caller needs in order to say, honestly, what was and was
/// not covered.
pub struct ModuleExport {
    pub module: String,
    pub revision: String,
    pub lean_source: String,
    pub exported: Vec<FunctionExport>,
    /// Every declaration this profile refused, in source order, with its
    /// closed reason. Never elided: an empty vector means the module had no
    /// refused declaration, not that refusals were dropped.
    pub unsupported: Vec<(String, String, Excluded)>,
}

impl ModuleExport {
    /// Flatten every exported obligation, in file order. The order is the
    /// order the theorems appear in [`Self::lean_source`], so a kernel
    /// report can be matched positionally as well as by name.
    #[must_use]
    pub fn obligations(&self) -> Vec<&ExportedObligation> {
        self.exported
            .iter()
            .flat_map(|export| export.obligations.iter())
            .collect()
    }

    #[must_use]
    pub fn theorem_names(&self) -> Vec<String> {
        self.obligations()
            .iter()
            .map(|obligation| format!("{NAMESPACE}.{}", obligation.theorem_name))
            .collect()
    }
}

/// Render the whole module. Declarations outside the profile are recorded
/// in [`ModuleExport::unsupported`] and in the generated file's own header;
/// they are never silently omitted.
#[must_use]
pub fn export_module(program: &Program, revision: &str) -> ModuleExport {
    let mut exported = Vec::new();
    let mut unsupported = Vec::new();
    // Non-function declarations first, in a fixed kind order, so a coverage
    // reader is told about every declaration in the module rather than only
    // the functions this profile could in principle have exported.
    for declaration in &program.types {
        unsupported.push((
            declaration.stable_id.clone(),
            declaration.name.clone(),
            Excluded::NonFunctionDeclaration {
                kind: match declaration.kind {
                    TypeDeclarationKind::Resource { .. } => "resource",
                    TypeDeclarationKind::Record { .. } => "record",
                    TypeDeclarationKind::Variant { .. } => "variant",
                    TypeDeclarationKind::Class { .. } => "class",
                },
            },
        ));
    }
    for declaration in &program.interfaces {
        unsupported.push((
            declaration.stable_id.clone(),
            declaration.name.clone(),
            Excluded::NonFunctionDeclaration { kind: "interface" },
        ));
    }
    for declaration in &program.protocols {
        unsupported.push((
            declaration.stable_id.clone(),
            declaration.name.clone(),
            Excluded::NonFunctionDeclaration { kind: "protocol" },
        ));
    }
    for declaration in &program.implementations {
        unsupported.push((
            declaration.stable_id.clone(),
            declaration.protocol_id.clone(),
            Excluded::NonFunctionDeclaration {
                kind: "implementation",
            },
        ));
    }
    for declaration in &program.agents {
        unsupported.push((
            declaration.stable_id.clone(),
            declaration.name.clone(),
            Excluded::NonFunctionDeclaration { kind: "agent" },
        ));
    }
    for function in &program.functions {
        match export_function(function) {
            Ok(export) => exported.push(export),
            Err(reason) => {
                unsupported.push((function.stable_id.clone(), function.name.clone(), reason))
            }
        }
    }

    let mut out = String::new();
    let _ = writeln!(out, "/-");
    let _ = writeln!(out, "SEMAPRAX Lean obligation export");
    let _ = writeln!(out, "schema: {EXPORT_SCHEMA}");
    let _ = writeln!(out, "module: {}", program.module);
    let _ = writeln!(out, "revision: {revision}");
    let _ = writeln!(out, "profile: {PROFILE_V1}");
    let _ = writeln!(out);
    let _ = writeln!(out, "Trusted base and assumptions:");
    for (id, text) in ASSUMPTIONS {
        let _ = writeln!(out, "  {id}: {text}");
    }
    let _ = writeln!(out);
    if unsupported.is_empty() {
        let _ = writeln!(
            out,
            "Declarations outside this profile: none in this module."
        );
    } else {
        let _ = writeln!(
            out,
            "Declarations outside this profile (NOT exported, NOT claimed):"
        );
        for (id, name, reason) in &unsupported {
            let _ = writeln!(
                out,
                "  {id} (`{name}`): {} — {}",
                reason.code(),
                reason.detail()
            );
        }
    }
    let _ = writeln!(out, "-/");
    let _ = writeln!(out);
    let _ = writeln!(out, "namespace {NAMESPACE}");
    let _ = writeln!(out);
    for export in &exported {
        for obligation in &export.obligations {
            out.push_str(&render_theorem(export, obligation));
            let _ = writeln!(out);
        }
    }
    let _ = writeln!(out, "end {NAMESPACE}");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "/-! No-admitted-holes gate: every exported theorem must report only Lean's three"
    );
    let _ = writeln!(
        out,
        "standard axioms. Any other axiom, and `sorryAx` in particular, invalidates it. -/"
    );
    for export in &exported {
        for obligation in &export.obligations {
            let _ = writeln!(out, "#print axioms {NAMESPACE}.{}", obligation.theorem_name);
        }
    }

    ModuleExport {
        module: program.module.clone(),
        revision: revision.to_owned(),
        lean_source: out,
        exported,
        unsupported,
    }
}
