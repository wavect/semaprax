use super::*;
const SOURCE: &str = r#"module test.byte_renewal;
@id("app.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let untouched=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<2usize {
  if index==0usize {buffer=bytes_set(buffer,index,65u8);0}else{0}
  index=index+1usize;
  0
 }
 if byte_len(bytes_as_slice(buffer))==2usize && byte_len(bytes_as_slice(untouched))==1usize {7}else{0}
}
"#;
#[test]
fn actual_same_owner_byte_renewal_preserves_original_binding_and_whole_owner() {
    let program = program(SOURCE);
    crate::cleanup_plan::replay::validate_program(&program).unwrap();
    let source = primitive_fixture::Context::new(&program);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let mut setup = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let context = selected_context::Context::new_paid(&source, function, &mut setup, true).unwrap();
    let renewals = context.primitive_renewals(function).unwrap();
    let (at, original) = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .find_map(|t| match t {
            CleanupTransition::ReserveRenewal { at, binding } => Some((at, binding)),
            _ => None,
        })
        .unwrap();
    for case in 0..3 {
        let mut place = original.clone();
        if case == 1 {
            place.storage = StorageId::ProvisionalResult;
        }
        if case == 2 {
            place.projections.push(function.id.clone());
        }
        let old = crate::cleanup_plan::replay::renewal::validate_binding_selected(
            function, at, &place, renewals,
        );
        let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let new = super::super::helpers::validate_binding_selected(
            function,
            at,
            &place,
            renewals,
            &mut budget,
        )
        .map_err(|e| render(e, function));
        assert_eq!(new, old, "renewal child {case}");
        assert_eq!(new.is_ok(), case == 0);
    }
    same_owner(&program, "app.main", true);
}

#[test]
fn held_byte_renewal_binding_drift_keeps_whole_old_first_refusal_and_drop() {
    for case in 0..2 {
        let mut program = program(SOURCE);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let place = function
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|b| &mut b.transitions)
            .find_map(|t| match t {
                CleanupTransition::ReserveRenewal { binding, .. } => Some(binding),
                _ => None,
            })
            .unwrap();
        if case == 0 {
            place.storage = StorageId::ProvisionalResult;
        } else {
            place.projections.push(function.id.clone());
        }
        same_owner(&program, "app.main", false);
    }
}
