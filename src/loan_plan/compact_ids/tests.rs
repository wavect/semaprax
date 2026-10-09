use super::*;
use crate::cache_codec::{self, codec_struct};
use crate::loan_plan::{Loan, LoanEdge, LoanEndpoint, LoanPlan, LoanPointPhase, LoanProgramPoint};
use std::mem::size_of;

struct LegacyEndpoint {
    point: LoanProgramPoint,
    live_before: Vec<LoanId>,
    starts: Vec<LoanId>,
    kills: Vec<LoanId>,
    live_after: Vec<LoanId>,
}
struct LegacyEdge {
    from: u16,
    to: u16,
    live: Vec<LoanId>,
}
struct LegacyPlan {
    schema: &'static str,
    loans: Vec<Loan>,
    endpoints: Vec<LegacyEndpoint>,
    edges: Vec<LegacyEdge>,
}
codec_struct!(LegacyEndpoint {
    point,
    live_before,
    starts,
    kills,
    live_after
});
codec_struct!(LegacyEdge { from, to, live });
codec_struct!(LegacyPlan {
    schema,
    loans,
    endpoints,
    edges
});

#[test]
fn immutable_loan_lists_remove_capacity_word_without_changing_items_or_wire() {
    assert_eq!(
        size_of::<LegacyEndpoint>() - size_of::<LoanEndpoint>(),
        4 * size_of::<usize>()
    );
    assert_eq!(
        size_of::<LegacyEdge>() - size_of::<LoanEdge>(),
        size_of::<usize>()
    );
    for values in [
        vec![],
        vec![LoanId(7)],
        vec![LoanId(8), LoanId(2), LoanId(8)],
    ] {
        let wire = cache_codec::encode(&values).unwrap();
        let boxed = from_vec(values.clone()).unwrap();
        assert_eq!(boxed.as_ref(), values.as_slice());
        assert_eq!(cache_codec::encode(&boxed).unwrap(), wire);
        assert_eq!(cache_codec::decode::<Box<[LoanId]>>(&wire).unwrap(), boxed);
        assert_eq!(cache_codec::decode::<Vec<LoanId>>(&wire).unwrap(), values);
        if !wire.is_empty() {
            assert_eq!(
                cache_codec::decode::<Box<[LoanId]>>(&wire[..wire.len() - 1]).unwrap_err()[0].code,
                cache_codec::decode::<Vec<LoanId>>(&wire[..wire.len() - 1]).unwrap_err()[0].code
            );
        }
    }
}

#[test]
fn immutable_loan_lists_full_census_keeps_each_exact_payload_and_identity_backing() {
    let expression = crate::hir::ExpressionId::from_owned("physical.loan.point".to_owned());
    let backing = expression.shared_allocation_bytes().unwrap();
    let plan = LoanPlan {
        schema: crate::loan_plan::LOAN_PLAN_SCHEMA_V1,
        loans: Vec::new(),
        endpoints: vec![LoanEndpoint {
            point: LoanProgramPoint {
                expression,
                phase: LoanPointPhase::Before,
            },
            live_before: vec![LoanId(1)].into_boxed_slice(),
            starts: vec![LoanId(2), LoanId(2)].into_boxed_slice(),
            kills: vec![LoanId(3), LoanId(3), LoanId(3)].into_boxed_slice(),
            live_after: vec![LoanId(4); 4].into_boxed_slice(),
        }],
        edges: vec![LoanEdge {
            from: 0,
            to: 0,
            live: vec![LoanId(5); 2].into_boxed_slice(),
        }],
    };
    let expected = plan.endpoints.capacity() * size_of::<LoanEndpoint>()
        + plan.edges.capacity() * size_of::<LoanEdge>()
        + backing
        + 12 * size_of::<LoanId>();
    assert_eq!(
        crate::loan_plan::owned_capacity_bytes(&plan),
        Some(expected)
    );
}

fn excess() -> Vec<LoanId> {
    let mut values = Vec::with_capacity(16);
    values.extend([LoanId(2), LoanId(1), LoanId(2)]);
    assert!(values.capacity() > values.len());
    values
}

#[test]
fn immutable_loan_list_conversion_charges_overlap_and_refuses_before_allocation() {
    let bytes = 3 * size_of::<LoanId>();
    let values = excess();
    let (result, overflow, used) =
        crate::bounded_output::with_limit_usage(bytes, || from_vec(values));
    assert_eq!(result.unwrap().as_ref(), &[LoanId(2), LoanId(1), LoanId(2)]);
    assert!(!overflow);
    assert_eq!(used, bytes);
    let values = excess();
    let (result, overflow, used) =
        crate::bounded_output::with_limit_usage(bytes - 1, || from_vec(values));
    assert_eq!(result.unwrap_err().code, "SPX-H006");
    assert!(overflow);
    assert_eq!(used, 0);
    let values = excess();
    let (result, overflow, used) = crate::bounded_output::with_limit_usage(bytes, || {
        assert!(crate::bounded_output::set_active_floor(1));
        from_vec(values)
    });
    assert!(result.is_err());
    assert!(overflow);
    assert_eq!(used, 0);
    let exact = vec![LoanId(4), LoanId(4)].into_boxed_slice().into_vec();
    assert_eq!(exact.capacity(), exact.len());
    let (result, overflow, used) = crate::bounded_output::with_limit_usage(0, || from_vec(exact));
    assert_eq!(result.unwrap().as_ref(), &[LoanId(4), LoanId(4)]);
    assert!(!overflow);
    assert_eq!(used, 0);
    let empty = Vec::with_capacity(16);
    let (result, overflow, used) = crate::bounded_output::with_limit_usage(0, || from_vec(empty));
    assert!(result.unwrap().is_empty());
    assert!(!overflow);
    assert_eq!(used, 0);
}

#[test]
fn immutable_loan_lists_replay_exact_legacy_plan_wire_and_graph() {
    let source = r#"
module test.compact_loan_lists;
@id("bytes.take") fn take(value: own Bytes) -> i64 { 1 }
@id("loan.run") fn run(input: borrow Slice<u8>, condition: bool) -> i64 {
    let owned = bytes_copy(input);
    let view = bytes_as_slice(owned);
    let seen = if condition { byte_len(view) > 0usize } else { false };
    take(owned)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, "compact-loan-lists.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.run")
        .unwrap();
    let plan = &function.loan_plan;
    assert!(!plan.loans.is_empty());
    assert!(plan.edges.iter().any(|edge| !edge.live.is_empty()));
    let legacy = LegacyPlan {
        schema: plan.schema,
        loans: plan.loans.clone(),
        endpoints: plan
            .endpoints
            .iter()
            .map(|point| LegacyEndpoint {
                point: point.point.clone(),
                live_before: point.live_before.to_vec(),
                starts: point.starts.to_vec(),
                kills: point.kills.to_vec(),
                live_after: point.live_after.to_vec(),
            })
            .collect(),
        edges: plan
            .edges
            .iter()
            .map(|edge| LegacyEdge {
                from: edge.from,
                to: edge.to,
                live: edge.live.to_vec(),
            })
            .collect(),
    };
    let wire = cache_codec::encode(&legacy).unwrap();
    assert_eq!(cache_codec::encode(plan).unwrap(), wire);
    let decoded: LoanPlan = cache_codec::decode(&wire).unwrap();
    assert_eq!(&decoded, plan);
    assert_eq!(
        crate::graph_loan::loan_plan_json(&decoded),
        crate::graph_loan::loan_plan_json(plan)
    );
    assert_eq!(
        crate::loan_plan::build_plan(&program, function).unwrap(),
        *plan
    );
}
