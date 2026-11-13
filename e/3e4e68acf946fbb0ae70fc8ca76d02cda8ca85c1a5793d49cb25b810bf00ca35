//! Exercise the unchanged nominal child with an actual source-owned directory.
use super::*;
use crate::workspace_graph::SourceRecordReplayFixture;
const SOURCE: &str = r#"module scalar.records;
@id("app.Row") record Row {@id("app.Row.n") n:i64,@id("app.Row.text") text:string,}
@id("app.weight") fn weight(row:borrow Row)->i64 {row.n}
@id("app.main") fn main()->i64 {let row=Row{n:42,text:"x"};weight(row)}
@id("app.witness") fn witness()->i64 {
 let key=str"x";let mut next=0;
 while next<1 {next=next+1;if str_len_bytes(key)==1 {return 1;} else {0} 0}
 0
}
"#;
fn budget() -> ReplayBudget {
    let mut result = ReplayBudget::new();
    result.admission = AdmissionMode::SelectedRetained;
    result.payer = AllocationPayer::SelectedReservation;
    result.physical = Some(Box::new(PhysicalReservation::new(usize::MAX)));
    result
}
#[test]
fn full_tree_and_policy_facades_keep_source_bound_nominal_signature_and_field_hostiles() {
    let fixture = SourceRecordReplayFixture::new(SOURCE);
    let function = fixture
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "app.weight")
        .unwrap();
    fixture.with_context(|context| {
        assert!(context.borrowed_helper_lease(function).is_none(), "current source-record directory must exercise actual Tree/Arena facts");
        let tree = success(Route::Tree, context, function, usize::MAX, None);
        let legacy = success(Route::LegacyArena, context, function, usize::MAX, None);
        let paid = success(Route::SelectedArena, context, function, usize::MAX, None);
        assert_eq!((legacy.remaining, legacy.reserved), (paid.remaining, paid.reserved));
        assert_eq!(legacy.work, paid.work);
        assert!(tree.work > 0);
        let mut pending = vec![&function.body];
        let mut projection = None;
        while let Some(expression) = pending.pop() {
            if matches!(&expression.kind, ResolvedExprKind::Place(place) if !place.projections.is_empty()) {
                projection = Some(expression);
                break;
            }
            pending.extend(crate::hir::lexical_exit::ChildCursor::new(expression));
        }
        let projection = projection.expect("actual ordinary row.n Field place");
        for mutation in 0..7 {
            let mut forged = projection.clone();
            if mutation != 0 {
                let ResolvedExprKind::Place(place) = &mut forged.kind else { unreachable!() };
                match mutation {
                    1 => place.projections[0] = crate::hir::PlaceProjection::Field(DeclarationId::new("foreign.field")),
                    2 => forged.ownership = OwnershipMode::Own,
                    3 => place.root = crate::hir::ValueId::intrinsic_parameter("foreign.parameter", 0),
                    4 => forged.ty = ResolvedType::Bool,
                    5 => forged.ownership = OwnershipMode::Borrow,
                    _ => place.projections.push(place.projections[0].clone()),
                }
            }
            let ((), metrics) = measure(|| {
                let old = scalar_replay_tree::records::expression_for_test(context, function, &forged, &mut budget()).unwrap();
                let current = scalar_replay::records::expression_for_test(context, function, &forged, &mut budget()).unwrap();
                assert_eq!(old, current);
                assert_eq!(old, Some(mutation == 0), "nominal hostile {mutation}");
            });
            clean(metrics);
        }
        let mut forged = function.clone();
        forged.params[0].ownership = OwnershipMode::Own;
        let ((), metrics) = measure(|| {
            let mut first = None;
            for route in ROUTES {
                let mut selected = session(context, function, usize::MAX);
                let error = run(route, &mut selected, &forged).unwrap_err();
                let error = match (route, error) {
                    (Route::SelectedArena, expression_facts::errors::EngineError::Structural(receipt)) => {
                        assert_eq!(receipt.source(), &forged.body.id);
                        assert_eq!(receipt.stage(), "selected session");
                        receipt.legacy(&forged)
                    }
                    (Route::Tree | Route::LegacyArena, expression_facts::errors::EngineError::Owned(error)) => error,
                    _ => panic!("full nominal signature diagnostic or source-bound selected receipt"),
                };
                assert!(selected.failed);
                assert_eq!(error, replay_error_old(&forged, "selected lexical function is not held by its current context"));
                if let Some(first) = &first { assert_eq!(first, &error); } else { first = Some(error.clone()); }
                drop(run(route, &mut selected, &forged).unwrap_err());
                drop(selected);
                assert_eq!(error, *first.as_ref().unwrap(), "first source-bound primary survives complete Drop");
                drop(error);
            }
            drop(first);
        });
        clean(metrics);
    });
}
