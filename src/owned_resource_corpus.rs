//! Canonical executable definitions for the direct-trivial-resource corpus.
//!
//! This doc-hidden module prevents native, Wasm, and physical-host evidence
//! from silently assigning different inputs or reference outcomes to the
//! authoritative scenario identifiers.

use std::collections::BTreeMap;
use std::path::Path;

use crate::cleanup_plan::{
    execute_for_conformance, CleanupScenario, ContractPhase, StatusCase, StatusProducer,
    StatusSourceId,
};
use crate::conformance::{ConformanceTrace, NormalizedStatus, OperationOutcome, TraceResult};
use crate::hir::{self, DeclarationId, ExpressionId, ResolvedFunction, ResolvedProgram};
use crate::semantic_trace::OWNED_RESOURCE_CORPUS_V1_SCENARIOS;

pub const OWNED_RESOURCE_CORPUS_SOURCE_V1: &str = r#"module test.owned_resource_corpus;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{
    0
}

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{
    number + 1
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token
    requires count >= 0
{
    second
}

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token
    ensures false
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedResourceCorpusArgument {
    Owned(u64),
    Bool(bool),
    I64(i64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedResourceCorpusCase {
    pub scenario_id: &'static str,
    pub function_id: &'static str,
    pub arguments: Vec<OwnedResourceCorpusArgument>,
    pub expected_owned_result_ordinal: Option<usize>,
    pub reference: ConformanceTrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedResourceCorpus {
    pub program: ResolvedProgram,
    pub cases: Vec<OwnedResourceCorpusCase>,
}

struct CasePlan {
    scenario_id: &'static str,
    function_id: &'static str,
    arguments: Vec<OwnedResourceCorpusArgument>,
    booleans: BTreeMap<ExpressionId, bool>,
    operations: BTreeMap<StatusSourceId, OperationOutcome>,
    result: Option<TraceResult>,
    expected_owned_result_ordinal: Option<usize>,
}

/// Build the exact compiler-owned corpus and execute its independent cleanup
/// oracle once per scenario.
pub fn build_owned_resource_corpus_v1() -> Result<OwnedResourceCorpus, String> {
    let parsed = crate::parse(
        OWNED_RESOURCE_CORPUS_SOURCE_V1,
        Path::new("owned-resource-corpus-v1.spx"),
    )
    .map_err(|error| error.to_string())?;
    let program = hir::resolve(&parsed).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let plans = plans(&program)?;
    if plans
        .iter()
        .map(|case| case.scenario_id)
        .collect::<Vec<_>>()
        != OWNED_RESOURCE_CORPUS_V1_SCENARIOS
    {
        return Err("owned-resource corpus identifiers diverged from v1".to_owned());
    }
    let mut cases = Vec::new();
    cases
        .try_reserve_exact(plans.len())
        .map_err(|_| "owned-resource corpus allocation failed".to_owned())?;
    for (index, plan) in plans.into_iter().enumerate() {
        let mut scenario = CleanupScenario::new(plan.scenario_id, plan.result);
        scenario.booleans = plan.booleans;
        scenario.operations = plan.operations;
        scenario.context_nonce = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| "owned-resource corpus nonce overflow".to_owned())?;
        let reference =
            execute_for_conformance(&program, &DeclarationId::new(plan.function_id), scenario)
                .map_err(|error| error.to_string())?;
        cases.push(OwnedResourceCorpusCase {
            scenario_id: plan.scenario_id,
            function_id: plan.function_id,
            arguments: plan.arguments,
            expected_owned_result_ordinal: plan.expected_owned_result_ordinal,
            reference,
        });
    }
    Ok(OwnedResourceCorpus { program, cases })
}

fn plans(program: &ResolvedProgram) -> Result<Vec<CasePlan>, String> {
    let requires = function(program, "token.requires")?;
    let checked = function(program, "token.checked")?;
    let choose = function(program, "token.choose-second")?;
    let ensures = function(program, "token.ensures-false")?;
    let requires_source = contract_source(requires, ContractPhase::Requires)?;
    let checked_requires = contract_source(checked, ContractPhase::Requires)?;
    let checked_add = arithmetic_source(checked)?;
    let choose_requires = contract_source(choose, ContractPhase::Requires)?;
    let ensures_source = contract_source(ensures, ContractPhase::Ensures)?;
    let owned_result = || TraceResult::Owned {
        type_id: DeclarationId::new("token.type"),
    };

    Ok(vec![
        case(
            "discard-zero",
            "token.discard",
            vec![owned(0)],
            Some(TraceResult::I64(0)),
        ),
        case(
            "discard-max",
            "token.discard",
            vec![owned(u64::MAX)],
            Some(TraceResult::I64(0)),
        ),
        case(
            "discard-two-reverse",
            "token.discard-two",
            vec![owned(0), owned(u64::MAX)],
            Some(TraceResult::I64(0)),
        ),
        CasePlan {
            scenario_id: "requires-false",
            function_id: "token.requires",
            arguments: vec![owned(u64::MAX), OwnedResourceCorpusArgument::Bool(false)],
            booleans: BTreeMap::from([(requires_source.expression.clone(), false)]),
            operations: BTreeMap::new(),
            result: None,
            expected_owned_result_ordinal: None,
        },
        CasePlan {
            scenario_id: "requires-true",
            function_id: "token.requires",
            arguments: vec![owned(0), OwnedResourceCorpusArgument::Bool(true)],
            booleans: BTreeMap::from([(requires_source.expression, true)]),
            operations: BTreeMap::new(),
            result: Some(TraceResult::I64(0)),
            expected_owned_result_ordinal: None,
        },
        CasePlan {
            scenario_id: "checked-success",
            function_id: "token.checked",
            arguments: vec![owned(0), OwnedResourceCorpusArgument::I64(41)],
            booleans: BTreeMap::from([(checked_requires.expression.clone(), true)]),
            operations: BTreeMap::from([(checked_add.clone(), OperationOutcome::Success)]),
            result: Some(TraceResult::I64(42)),
            expected_owned_result_ordinal: None,
        },
        CasePlan {
            scenario_id: "checked-add-overflow",
            function_id: "token.checked",
            arguments: vec![owned(u64::MAX), OwnedResourceCorpusArgument::I64(i64::MAX)],
            booleans: BTreeMap::from([(checked_requires.expression.clone(), true)]),
            operations: BTreeMap::from([(
                checked_add,
                OperationOutcome::Failure(NormalizedStatus::arithmetic(StatusCase::AddOverflow)),
            )]),
            result: None,
            expected_owned_result_ordinal: None,
        },
        CasePlan {
            scenario_id: "checked-precondition-false",
            function_id: "token.checked",
            arguments: vec![owned(0), OwnedResourceCorpusArgument::I64(-1)],
            booleans: BTreeMap::from([(checked_requires.expression, false)]),
            operations: BTreeMap::new(),
            result: None,
            expected_owned_result_ordinal: None,
        },
        owned_case(
            "identity-zero",
            "token.identity",
            vec![owned(0)],
            owned_result(),
            0,
        ),
        owned_case(
            "identity-max",
            "token.identity",
            vec![owned(u64::MAX)],
            owned_result(),
            0,
        ),
        CasePlan {
            scenario_id: "choose-second-zero-max",
            function_id: "token.choose-second",
            arguments: vec![
                owned(0),
                OwnedResourceCorpusArgument::I64(17),
                owned(u64::MAX),
            ],
            booleans: BTreeMap::from([(choose_requires.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(owned_result()),
            expected_owned_result_ordinal: Some(1),
        },
        CasePlan {
            scenario_id: "choose-second-zero-zero",
            function_id: "token.choose-second",
            arguments: vec![owned(0), OwnedResourceCorpusArgument::I64(17), owned(0)],
            booleans: BTreeMap::from([(choose_requires.expression.clone(), true)]),
            operations: BTreeMap::new(),
            result: Some(owned_result()),
            expected_owned_result_ordinal: Some(1),
        },
        CasePlan {
            scenario_id: "choose-second-requires-false",
            function_id: "token.choose-second",
            arguments: vec![
                owned(0),
                OwnedResourceCorpusArgument::I64(-1),
                owned(u64::MAX),
            ],
            booleans: BTreeMap::from([(choose_requires.expression, false)]),
            operations: BTreeMap::new(),
            result: None,
            expected_owned_result_ordinal: None,
        },
        CasePlan {
            scenario_id: "ensures-false",
            function_id: "token.ensures-false",
            arguments: vec![owned(u64::MAX)],
            booleans: BTreeMap::from([(ensures_source.expression, false)]),
            operations: BTreeMap::new(),
            result: None,
            expected_owned_result_ordinal: None,
        },
    ])
}

fn case(
    scenario_id: &'static str,
    function_id: &'static str,
    arguments: Vec<OwnedResourceCorpusArgument>,
    result: Option<TraceResult>,
) -> CasePlan {
    CasePlan {
        scenario_id,
        function_id,
        arguments,
        booleans: BTreeMap::new(),
        operations: BTreeMap::new(),
        result,
        expected_owned_result_ordinal: None,
    }
}

fn owned_case(
    scenario_id: &'static str,
    function_id: &'static str,
    arguments: Vec<OwnedResourceCorpusArgument>,
    result: TraceResult,
    ordinal: usize,
) -> CasePlan {
    let mut plan = case(scenario_id, function_id, arguments, Some(result));
    plan.expected_owned_result_ordinal = Some(ordinal);
    plan
}

const fn owned(payload: u64) -> OwnedResourceCorpusArgument {
    OwnedResourceCorpusArgument::Owned(payload)
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> Result<&'a ResolvedFunction, String> {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .ok_or_else(|| format!("owned-resource corpus function `{id}` is missing"))
}

fn contract_source(
    function: &ResolvedFunction,
    phase: ContractPhase,
) -> Result<StatusSourceId, String> {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| match source.producer {
            StatusProducer::ContractFalse { phase: actual, .. } if actual == phase => {
                Some(source.id.clone())
            }
            _ => None,
        })
        .ok_or_else(|| format!("corpus function `{}` has no {phase:?} source", function.id))
}

fn arithmetic_source(function: &ResolvedFunction) -> Result<StatusSourceId, String> {
    function
        .cleanup_plan
        .status_sources
        .iter()
        .find_map(|source| {
            matches!(source.producer, StatusProducer::CheckedArithmetic { .. })
                .then(|| source.id.clone())
        })
        .ok_or_else(|| format!("corpus function `{}` has no arithmetic source", function.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_corpus_builds_in_authoritative_order() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        assert_eq!(
            corpus
                .cases
                .iter()
                .map(|case| case.scenario_id)
                .collect::<Vec<_>>(),
            OWNED_RESOURCE_CORPUS_V1_SCENARIOS
        );
    }
}
