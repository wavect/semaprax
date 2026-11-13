use super::interpret;
const SOURCE: &str = r#"module lexical.iterator.resume;
@id("app.main") fn main()->i64 ensures result==43 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(4usize),bytes_zeroed(1usize));
 let b=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let c=vec_push<Bytes>(b,bytes_zeroed(3usize));
 let values=vec_push<Bytes>(c,bytes_zeroed(4usize));
 let mut seen=0; let mut total=0;
 for own item in vec_into_iter<Bytes>(values) {
  let scratch=bytes_zeroed(1usize);
  let width=byte_len(bytes_as_slice(item));
  seen=seen+1;
  if seen==1 {continue;};
  if width==2usize || width==4usize {continue;};
  total=total+3;
  if byte_len(bytes_as_slice(scratch))!=1usize {return 0;};
  0
 }
 return total+seen*10;
}
"#;

#[test]
fn distinct_continue_sites_settle_locals_and_advance_through_last_item_once() {
    use crate::owned_vec_bytes_runtime::{run_native, run_wasm_expected};

    interpret(SOURCE, 43, false);
    run_native(SOURCE, "", 0, 43, "none");
    run_wasm_expected(SOURCE, 0, 8, "none", 43);
    let failed = SOURCE.replace("return total+seen*10;", "return total+seen*10-1;");
    interpret(&failed, 0, true);
    run_native(&failed, "semaprax.contract.v1", 2, 0, "none");
    run_wasm_expected(&failed, 10, 8, "none", 0);
    interpret(SOURCE, 43, false);
}

#[test]
fn forged_resume_transfer_phase_refuses_before_interpreter_or_wasm() {
    use semaprax::{cleanup_plan::*, hir};
    let checked = semaprax::check(SOURCE, "resume-forged.spx").unwrap();
    let mut program = hir::resolve(&checked).unwrap();
    hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let control = function.cleanup_plan.lexical_control.as_ref().unwrap();
    let domain = control
        .exits
        .iter()
        .find(|exit| exit.kind == semaprax::ast::LexicalExitKind::Continue)
        .unwrap()
        .resume
        .unwrap();
    let blocks = control
        .resumes
        .iter()
        .find(|resume| resume.entry == domain)
        .unwrap()
        .blocks
        .clone();
    let wrong_phase = function.body.id.clone();
    // The block result is moved before its scope completion; moving that
    // event to a different expression anchor cannot grant later publication.
    let transfer = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .filter(|block| blocks.contains(&block.id))
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(transition,
            CleanupTransition::TransferVariant { at, destination, .. }
            if destination.storage==StorageId::Temporary(at.clone()))
        })
        .unwrap();
    match transfer {
        CleanupTransition::TransferVariant { at, .. } => {
            assert_ne!(
                at, &wrong_phase,
                "root body is a distinct real producer phase"
            );
            *at = wrong_phase;
        }
        _ => unreachable!(),
    }
    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
    assert!(semaprax::wasm::emit_resolved_module(&program).is_err());
    assert!(
        semaprax::interpreter::evaluate_resolved_owned_data(&program, "app.main", &[], 10_000)
            .is_err()
    );
}
