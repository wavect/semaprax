//! Agent Stage Semantic Work v1 admission and shared transport decoding.
//!
//! A metered stage dispatch first admits the stage's reachable call closure
//! into the metered profile, before any compiler or Node process exists. The
//! profile is closed: direct calls to monomorphic source functions and
//! `while` loops are the only metered constructs, so every backend charges
//! exactly the same semantic points (see
//! `docs/AGENT-ITERATIVE-LIFECYCLE-V2.md`, "Stage semantic work v1"). A
//! construct whose work the compiled backends cannot meter identically —
//! function values, closures, generic instances, host or native imports and
//! yields — is refused rather than run unmetered. There is no fallback to an
//! unmetered or interpreter execution.

use std::collections::{BTreeMap, BTreeSet};

use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ResolvedExprKind};
use crate::interpreter::retained_call::SemanticCleanupEvent;

use crate::agent_lifecycle::stages::invariant;

/// The largest semantic fuel limit one stage dispatch may request. It equals
/// the retained-call instruction-step ceiling, but the two are never compared.
pub(in crate::agent_lifecycle) const MAX_STAGE_SEMANTIC_FUEL: u64 = 1_000_000;

/// One admitted metered stage profile: the semantic fuel limit plus the
/// ordinal of every metered source function. Ordinals identify performed
/// cleanup events on the target transports; they carry no authority.
#[derive(Debug)]
pub struct StageSemanticProfile {
    fuel_limit: u64,
    functions: Vec<DeclarationId>,
}

impl StageSemanticProfile {
    /// Admit `entry`'s reachable call closure under `fuel_limit`.
    pub(in crate::agent_lifecycle) fn admit(
        program: &hir::ResolvedProgram,
        entry: &str,
        fuel_limit: u64,
    ) -> Result<Self, Diagnostic> {
        if !(1..=MAX_STAGE_SEMANTIC_FUEL).contains(&fuel_limit) {
            return Err(invariant("semantic_work.fuel_limit"));
        }
        let by_id = program
            .functions
            .iter()
            .map(|function| (function.id.as_str(), function))
            .collect::<BTreeMap<_, _>>();
        let mut pending = vec![*by_id
            .get(entry)
            .ok_or_else(|| invariant("semantic_work.entry"))?];
        let mut seen = BTreeSet::new();
        while let Some(function) = pending.pop() {
            if !seen.insert(function.id.as_str()) {
                continue;
            }
            let mut refusal = None;
            hir::function_value::walk(function, |expression| {
                let refused = match &expression.kind {
                    ResolvedExprKind::Closure { .. } => Some("closure"),
                    ResolvedExprKind::FunctionReference { .. }
                    | ResolvedExprKind::Invoke { .. } => Some("function_value"),
                    ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::HostCommandCall(_) => Some("import"),
                    ResolvedExprKind::Yield { .. } => Some("yield"),
                    ResolvedExprKind::Call {
                        instance: Some(_), ..
                    } => Some("generic_call"),
                    ResolvedExprKind::Call { callee, .. } => {
                        if let Some(callee) = by_id.get(callee.as_str()) {
                            pending.push(*callee);
                        }
                        None
                    }
                    _ => None,
                };
                if refusal.is_none() {
                    refusal = refused;
                }
            });
            if let Some(kind) = refusal {
                return Err(invariant(&format!("semantic_work.profile.{kind}")));
            }
        }
        Ok(Self {
            fuel_limit,
            functions: program
                .functions
                .iter()
                .map(|function| function.id.clone())
                .collect(),
        })
    }

    pub(in crate::agent_lifecycle) const fn fuel_limit(&self) -> u64 {
        self.fuel_limit
    }

    /// Metered function ordinals, by position in the admitted program.
    pub(in crate::agent_lifecycle) fn ordinals(&self) -> BTreeMap<DeclarationId, u32> {
        self.functions
            .iter()
            .enumerate()
            .filter_map(|(ordinal, id)| Some((id.clone(), u32::try_from(ordinal).ok()?)))
            .collect()
    }

    /// Decode one transported `function << 32 | liveness_flag` event.
    pub(in crate::agent_lifecycle) fn decode_event(
        &self,
        raw: u64,
    ) -> Result<SemanticCleanupEvent, Diagnostic> {
        let function = usize::try_from(raw >> 32)
            .ok()
            .and_then(|ordinal| self.functions.get(ordinal))
            .ok_or_else(|| invariant("semantic_work.event.function"))?;
        Ok(SemanticCleanupEvent {
            function: function.clone(),
            liveness_flag: u32::try_from(raw & u64::from(u32::MAX))
                .map_err(|_| invariant("semantic_work.event.flag"))?,
        })
    }
}

/// Map one compiler-owned `(domain, code)` status observed on a compiled
/// target back to its normalized status. The semantic fuel domain is never a
/// language failure; the caller settles it as fuel exhaustion instead.
pub(in crate::agent_lifecycle) fn compiler_status(
    domain: &str,
    code: u32,
) -> Result<NormalizedStatus, Diagnostic> {
    use crate::cleanup_plan::{ContractPhase, StatusCase};
    let arithmetic = match code {
        1 => Some(StatusCase::AddOverflow),
        2 => Some(StatusCase::SubOverflow),
        3 => Some(StatusCase::MulOverflow),
        4 => Some(StatusCase::DivisionByZero),
        5 => Some(StatusCase::DivisionOverflow),
        6 => Some(StatusCase::RemainderByZero),
        7 => Some(StatusCase::RemainderOverflow),
        8 => Some(StatusCase::NegationOverflow),
        _ => None,
    };
    match (domain, code) {
        ("semaprax.arithmetic.v1", _) => arithmetic
            .map(crate::runtime_status::normalize_arithmetic)
            .ok_or_else(|| invariant("semantic_work.status.arithmetic")),
        ("semaprax.contract.v1", 1) => Ok(crate::runtime_status::normalize_contract(
            ContractPhase::Requires,
        )),
        ("semaprax.contract.v1", 2) => Ok(crate::runtime_status::normalize_contract(
            ContractPhase::Ensures,
        )),
        _ => Err(invariant("semantic_work.status.domain")),
    }
}
