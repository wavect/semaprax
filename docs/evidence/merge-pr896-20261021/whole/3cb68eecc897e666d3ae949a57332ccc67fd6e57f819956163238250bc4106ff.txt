use super::*;
const CONCAT: &str = r#"module oracle.concat;
@id("o.main") fn main()->i64 {
 let mut text=string_from_i64(1);
 let mut count=0;
 while count<2 {
  text=string_concat(text,"x");
  count=count+1;
  0
 }
 string_len(text)
}"#;
#[test]
fn genuine_string_append_success_preserves_complete_old_and_ordinary_routes() {
    let program = program(CONCAT);
    crate::cleanup_plan::replay::validate_program(&program).unwrap();
    same_owner(&program, "o.main", true);
}
#[test]
fn held_string_append_duplicate_reservation_preserves_first_full_old_diagnostic() {
    let mut program = program(CONCAT);
    let at = {
        let source = primitive_fixture::Context::new(&program);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "o.main")
            .unwrap();
        let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let context =
            selected_context::Context::new_paid(&source, function, &mut budget, true).unwrap();
        let sites = context.primitive_renewals(function).unwrap();
        sites
            .appends()
            .keys()
            .next()
            .expect("genuine current String append site")
            .clone()
    };
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "o.main")
        .unwrap();
    let block = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block.transitions.iter().any(|transition|
        matches!(transition,CleanupTransition::Transfer {at:site,..} if site==&at))
        })
        .unwrap();
    let index = block
        .transitions
        .iter()
        .position(
            |transition| matches!(transition,CleanupTransition::Transfer {at:site,..} if site==&at),
        )
        .unwrap();
    block
        .transitions
        .insert(index + 1, block.transitions[index].clone());
    same_owner(&program, "o.main", false);
    let source = primitive_fixture::Context::new(&program);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "o.main")
        .unwrap();
    assert_eq!(
        owner(&source, function, false),
        owner(&source, function, true)
    );
}
