//! Actual three-family preparation endpoints, with complete old receivers.
use super::*;
use crate::bounded_output::{
    active_remaining, clear_active_floor, reserve_active_required, set_active_floor,
    with_limit_usage, with_limit_usage_refusal,
};
use crate::{hir::DeclarationIndex, test_allocation_observer::measure};
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"module descriptor.identity;
@id("descriptor.row") record Row {
 @id("descriptor.title") title:string,
 @id("descriptor.rank") rank:i64,
 @id("descriptor.payload") payload:Bytes,
}
@id("descriptor.sort") fn sorted(rows:own Vec<Row>)->Vec<Row> { vec_sort_by<Row>(rows,"rank:asc") }
@id("descriptor.unique") fn unique(rows:borrow Vec<Row>)->bool { vec_unique_by<Row>(rows,"title") }
@id("descriptor.update") fn updated(rows:own Vec<Row>)->Vec<Row> { vec_update_field<Row>(rows,0usize,7,"rank") }
@id("descriptor.main") fn main()->i64 {0}
"#;

fn make_resolver(program: &crate::Program) -> Resolver<'_, '_> {
    Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::from_verified_with_prelude(
            program, crate::prelude::declarations_for_program(program),
        ).unwrap(),
        reuse: None,
        function_work: crate::hir::FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    }
}
fn call<'a>(expression: &'a Expr, wanted: &str) -> &'a Expr {
    if matches!(&expression.kind, ExprKind::Call { name, .. } if name == wanted) {
        return expression;
    }
    fn find<'a>(expression: &'a Expr, wanted: &str) -> Option<&'a Expr> {
        if matches!(&expression.kind, ExprKind::Call { name, .. } if name == wanted) {
            return Some(expression);
        }
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            if let Some(found) = find(child, wanted) { return Some(found); }
            index += 1;
        }
        None
    }
    find(expression, wanted).expect("actual parsed descriptor operation")
}
fn prepare(
    resolver: &Resolver<'_, '_>, owner: &FunctionExecutionId, expression: &Expr, old: bool,
) -> Result<(ResolvedType, DeclarationId), Diagnostic> {
    let ExprKind::Call { type_arguments, args, .. } = &expression.kind else { panic!("call"); };
    if old {
        resolver.prepare_sort_by_original(owner, expression, type_arguments, args)
    } else {
        resolver.prepare_sort_by(owner, expression, type_arguments, args)
    }
}
fn operations() -> [(&'static str, &'static str); 3] {
    [("sorted", crate::vec_ops::sort_keys::NAME),
     ("unique", crate::vec_ops::field_unique::NAME),
     ("updated", crate::vec_ops::field_update::NAME)]
}

#[test]
fn all_three_real_preparations_keep_complete_old_results_diagnostics_and_compatibility() {
    let program = crate::check(SOURCE, "descriptor-identity.spx").unwrap();
    let resolver = make_resolver(&program);
    assert!(active_remaining().is_none(), "no-owner calls are compatibility, not funded");
    for (function, operation) in operations() {
        let function = program.functions.iter().find(|f| f.name == function).unwrap();
        let owner = FunctionExecutionId::Monomorphic(resolver.declarations.held_identity(&function.stable_id));
        let expression = call(&function.body, operation);
        let current = prepare(&resolver, &owner, expression, false).unwrap();
        let old = prepare(&resolver, &owner, expression, true).unwrap();
        assert_eq!(current, old);
        assert_eq!(crate::cache_codec::encode(&current).unwrap(), crate::cache_codec::encode(&old).unwrap());
        for bad in ["missing", "rank:sideways", "", "nul\0λ"] {
            let mut changed = expression.clone();
            let ExprKind::Call { args, .. } = &mut changed.kind else { unreachable!() };
            args.last_mut().unwrap().kind = ExprKind::String(bad.into());
            let old = prepare(&resolver, &owner, &changed, true).unwrap_err();
            let current = prepare(&resolver, &owner, &changed, false).unwrap_err();
            assert_eq!(current, old, "all legacy diagnostic fields and shape precedence");
        }
        let generic = FunctionExecutionId::Generic(crate::hir::FunctionInstanceId("foreign.λ\0".into()));
        assert_eq!(prepare(&resolver, &generic, expression, false).unwrap_err(), prepare(&resolver, &generic, expression, true).unwrap_err());
        // Presence/metadata never acquires authority from a reserved descriptor spelling.
        let mut forged = make_resolver(&program);
        forged.declarations.declarations.get_mut(&DeclarationId::new("descriptor.rank")).unwrap().identity_origin = crate::hir::IdentityOrigin::Automatic;
        assert_eq!(prepare(&forged, &owner, expression, false).unwrap_err(), prepare(&forged, &owner, expression, true).unwrap_err());
    }
}

fn backing(length: usize) -> usize {
    // Independent two-counter Arc byte layout, including its alignment tail.
    let word = std::mem::size_of::<usize>();
    (2 * word + length).div_ceil(word) * word
}

#[test]
fn descriptor_string_and_arc_overlap_use_same_original_payer_exact_short_floor_and_drop() {
    let program = crate::check(SOURCE, "descriptor-payment.spx").unwrap();
    let resolver = make_resolver(&program);
    for (function, operation) in operations() {
        let function = program.functions.iter().find(|f| f.name == function).unwrap();
        let owner = FunctionExecutionId::Monomorphic(resolver.declarations.held_identity(&function.stable_id));
        let expression = call(&function.body, operation);
        let old = prepare(&resolver, &owner, expression, true).unwrap();
        let bytes = backing(old.1.as_str().len());
        let result_bytes = std::mem::size_of::<Result<DeclarationId, ()>>();
        assert_eq!(DESCRIPTOR_RESULT_BYTES, result_bytes);
        assert_eq!(Some(bytes), DeclarationId::shared_backing_bytes(old.1.as_str().len()));
        let (old_result, refused, old_paid) = with_limit_usage(usize::MAX, || prepare(&resolver, &owner, expression, true));
        assert!(!refused);
        assert_eq!(old_result.unwrap(), old);
        let (current, refused, paid) = with_limit_usage(old_paid + result_bytes + bytes, || prepare(&resolver, &owner, expression, false));
        assert!(!refused);
        assert_eq!(current.unwrap(), old);
        assert_eq!(paid, old_paid + result_bytes + bytes, "all original charges retained, actual result carrier and real backing added");
        let (result, refused, paid, receipt) = with_limit_usage_refusal(old_paid + result_bytes + bytes - 1, || prepare(&resolver, &owner, expression, false));
        assert!(result.is_err() && refused);
        assert_eq!(paid, old_paid + result_bytes);
        assert_eq!(receipt.unwrap().requested, bytes, "new backing is refused before formation");
        let ((result, retry), refused, paid, receipt) = with_limit_usage_refusal(old_paid + result_bytes + bytes, || {
            assert!(set_active_floor(1));
            let result = prepare(&resolver, &owner, expression, false);
            clear_active_floor();
            (result, reserve_active_required(0))
        });
        assert!(result.is_err() && !retry && refused);
        assert_eq!(paid, old_paid + result_bytes);
        assert_eq!(receipt.unwrap().requested, bytes);
        // New-carrier short refusal precedes paid_owned; exact admission then
        // reaches the independent Arc refusal with no remaining byte budget.
        for available in [0, result_bytes - 1] {
            let (result, refused, paid, receipt) = with_limit_usage_refusal(old_paid + available, || prepare(&resolver, &owner, expression, false));
            assert!(result.is_err() && refused);
            assert_eq!(paid, old_paid);
            assert_eq!(receipt.unwrap().requested, result_bytes);
        }
        let (result, refused, paid, receipt) = with_limit_usage_refusal(old_paid + result_bytes, || prepare(&resolver, &owner, expression, false));
        assert!(result.is_err() && refused);
        assert_eq!(paid, old_paid + result_bytes);
        assert_eq!(receipt.unwrap().requested, bytes);
        let ((result, retry), refused, paid, receipt) = with_limit_usage_refusal(old_paid + result_bytes, || {
            assert!(set_active_floor(1));
            let result = prepare(&resolver, &owner, expression, false);
            clear_active_floor();
            (result, reserve_active_required(0))
        });
        assert!(result.is_err() && !retry && refused);
        assert_eq!(paid, old_paid);
        assert_eq!(receipt.unwrap().requested, result_bytes);
        // Same owning String/Arc physical overlap and complete releases; only payment changes.
        let ((), old_metrics) = measure(|| drop(prepare(&resolver, &owner, expression, true).unwrap()));
        let ((), current_metrics) = measure(|| drop(prepare(&resolver, &owner, expression, false).unwrap()));
        assert!(!old_metrics.invalid && !current_metrics.invalid);
        assert_eq!(old_metrics, current_metrics);
        assert_eq!(current_metrics.live, 0);
        assert_eq!(current_metrics.allocated, current_metrics.freed);
        assert_eq!(active_remaining(), None);
    }
}

#[test]
fn actual_serializer_capacity_precharge_and_owned_backing_have_no_unfunded_format_step() {
    // Final identities are created only from these already-paid exact serializers.
    let field = DeclarationId::new("field.λ\0é");
    fn inspect(mut serialize: impl FnMut() -> Option<String>) {
        let expected = serialize().unwrap();
        let len = expected.len();
        assert_eq!(expected.capacity(), len, "exact original requested String capacity");
        let arc = backing(len);
        let result_bytes = std::mem::size_of::<Result<DeclarationId, ()>>();
        let (((), observed), refused, paid) = with_limit_usage(len + result_bytes + arc, || measure(|| {
            let text = serialize().unwrap();
            assert_eq!(text, expected);
            assert_eq!(text.capacity(), len);
            assert!(reserve_active_required(result_bytes));
            let id = DeclarationId::paid_owned(text, reserve_descriptor_backing).unwrap();
            assert_eq!(id.as_str(), expected);
            drop(id);
        }));
        assert!(!refused && !observed.invalid);
        assert_eq!(paid, len + result_bytes + arc);
        assert_eq!(observed.allocated, len + arc);
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
        let (((), observed), refused, paid, receipt) = with_limit_usage_refusal(len + result_bytes + arc - 1, || measure(|| {
            let text = serialize().unwrap();
            assert!(reserve_active_required(result_bytes));
            assert!(DeclarationId::paid_owned(text, reserve_descriptor_backing).is_err());
        }));
        assert!(refused && !observed.invalid);
        assert_eq!(paid, len + result_bytes);
        assert_eq!(receipt.unwrap().requested, arc);
        assert_eq!((observed.allocated, observed.freed, observed.live), (len, len, 0));
    }
    let sort = crate::vec_ops::sort_keys::Descriptor { element: "element.λ\0".into(), layout: "layout.é".into(), keys: vec![crate::vec_ops::sort_keys::Key { field: field.clone(), descending: false }] };
    let unique = crate::vec_ops::field_unique::Descriptor { element: sort.element.clone(), layout: sort.layout.clone(), field: field.clone() };
    let update = crate::vec_ops::field_update::Descriptor { element: sort.element.clone(), layout: sort.layout.clone(), field };
    inspect(|| sort.identity());
    inspect(|| unique.identity());
    inspect(|| update.identity());
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
#[test]
fn complete_old_preparation_and_all_three_serializer_sources_remain_authenticated() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("identity_inverse.json")).unwrap();
    let current = include_str!("../sort_keys.rs");
    assert_eq!(sha(current.as_bytes()), proof["current_sha256"].as_str().unwrap());
    let mut old = current.to_owned();
    for edit in proof["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let new = edit["new"].as_str().unwrap();
        assert_eq!(old.get(start..start + new.len()), Some(new));
        old.replace_range(start..start + new.len(), edit["old"].as_str().unwrap());
    }
    assert_eq!(sha(old.as_bytes()), proof["before_sha256"].as_str().unwrap());
    let start = old.find("    pub(in crate::hir) fn prepare_sort_by(").unwrap();
    let end = old.find("    pub(in crate::hir) fn finish_sort_by").unwrap();
    let body = old[start..end].replacen("fn prepare_sort_by(", "fn prepare_sort_by_original(", 1);
    assert_eq!(include_str!("identity_before.rs"), format!("//! Complete immediate preparation receiver; only its test-private name differs.\nuse super::*;\n\nimpl Resolver<'_, '_> {{\n{body}}}\n"));
    for (path, current) in [
        ("src/vec_ops/sort_keys.rs", include_str!("../../../vec_ops/sort_keys.rs")),
        ("src/vec_ops/field_unique.rs", include_str!("../../../vec_ops/field_unique.rs")),
        ("src/vec_ops/field_update.rs", include_str!("../../../vec_ops/field_update.rs")),
    ] {
        let row = proof["unchanged_serializers"].as_array().unwrap().iter().find(|row| row["source"] == path).unwrap();
        assert_eq!(sha(current.as_bytes()), row["sha256"].as_str().unwrap());
    }
}
