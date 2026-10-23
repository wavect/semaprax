//! Fund the complete legacy owner search before its first owned construction.
use crate::cleanup_plan::replay::ReplayBudget;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedFunction;
use std::mem::size_of;

// The independently retained owner helper calls validate_owner for every
// candidate, dropping its owned fixed diagnostic on a rejected candidate.
// Fund the whole possible result for each visit, including an unused bank for
// the matching candidate. Generic execution IDs also clone real String bytes.
const CANDIDATE_ERROR: usize =
    "lexical function owner differs from canonical result/body identities".len();
const FINAL_ERROR: usize =
    "unregistered lexical function requires an explicit authenticated execution context".len();
type Call<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
);
type Payment<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
type Instances<'a> = std::slice::Iter<'a, crate::hir::ResolvedFunctionInstance>;
const PAYMENT: usize = 2 * size_of::<Payment<'static>>()
    + 2 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<Option<usize>>();
// One next attempt is prepaid before iteration. Each successful visit buys
// the next attempt, including terminal None, before inspecting the ID bytes.
const INSTANCE_STEP: usize = 2 * PAYMENT
    + 2 * size_of::<&mut Instances<'static>>()
    + 2 * size_of::<Option<&crate::hir::ResolvedFunctionInstance>>()
    + 2 * size_of::<&crate::hir::ResolvedFunctionInstance>()
    + 2 * size_of::<&crate::hir::FunctionInstanceId>()
    + 2 * size_of::<&str>()
    + 4 * size_of::<usize>();
const CONTROLS: usize = 2 * PAYMENT
    + 2 * size_of::<Call<'static>>()
    + 2 * size_of::<Result<crate::hir::FunctionExecutionId, Diagnostic>>()
    + 2 * size_of::<&[ResolvedFunction]>()
    + 2 * size_of::<&[crate::hir::ResolvedFunctionInstance]>()
    + 2 * size_of::<Instances<'static>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<bool>()
    + INSTANCE_STEP;

pub(super) fn owner(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<crate::hir::FunctionExecutionId, Diagnostic> {
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            CONTROLS + FINAL_ERROR,
            "ordinary execution owner receiving controls",
        )?;
        let functions = program.functions();
        budget.reserve_array::<[u8; CANDIDATE_ERROR]>(
            function,
            functions.len(),
            "ordinary execution owner candidate diagnostics",
        )?;
        let instances = program.function_instances();
        for instance in instances {
            budget.reserve_bytes(
                function,
                INSTANCE_STEP + CANDIDATE_ERROR,
                "ordinary generic owner visit and diagnostic",
            )?;
            budget.reserve_bytes(
                function,
                instance.id.as_str().len(),
                "ordinary generic execution owner clone",
            )?;
        }
    }
    // Preserve the complete old search, clone/drop order, candidate validation,
    // missing/ambiguous selection, six public fields and returned owner.
    crate::hir::lexical_exit::owner(program, function)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn custody(path: &str, current: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    assert_eq!(
        digest(include_bytes!("ordinary/inverse.json")),
        "6426e24c75df550056d08b9097ee2416a1f0cf8f17322ead07ba960af630d060"
    );
    let binding: serde_json::Value =
        serde_json::from_str(include_str!("ordinary/inverse.json")).unwrap();
    let row = &binding[path];
    assert_eq!(current.len() as u64, row["length"].as_u64().unwrap());
    assert_eq!(digest(current), row["sha256"].as_str().unwrap());
    let mut bytes = current.to_vec();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let end = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert_eq!(
            bytes.get(start..end).unwrap(),
            edit["current"].as_str().unwrap().as_bytes()
        );
        bytes.splice(
            start..end,
            edit["before"].as_str().unwrap().as_bytes().iter().copied(),
        );
    }
    assert_eq!(bytes.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(digest(&bytes), row["before_sha256"].as_str().unwrap());
    bytes
}
