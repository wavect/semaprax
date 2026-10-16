use super::{Diagnostic, PrivateTypes, Program};
use crate::workspace_graph::{
    expected_projection::synthetic_program,
    index_authored, reconstruct_workspace_declaration_facts,
    validation::{validate_stub_signatures, WorkspaceValidationIndex},
};
const PROVIDER: &str = r#"module private.provider;
@id("payload") record Payload { @id("payload.text") text:string, }
@id("root") variant Root {
 @id("root.ok") Ok { @id("root.payload") payload:Payload, },
 @id("root.no") No { @id("root.code") code:i64, @id("root.offset") offset:usize, @id("root.field") field:i64, },
}
@id("read") fn read(input:borrow Slice<u8>)->Root {Root::No {code:7,offset:0usize,field:0}}
"#;
const CALLER: &str = r#"module private.caller;
use type @id("root") from private.provider as Root;
use function @id("read") from private.provider as read;
@id("main") fn main()->i64 {let bytes=[0u8];let value=read(array_as_slice(bytes));match own value {Root::No {code,offset,field}=>code,Root::Ok {payload}=>0,}}
"#;
fn programs() -> [Program; 2] {
    [
        crate::parse(CALLER, "caller.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
    ]
}
fn resolved(
    programs: &[Program],
) -> Vec<(
    String,
    crate::project::incremental::checked_hir::ResolvedModuleInput,
)> {
    let authored = index_authored(programs).unwrap();
    programs
        .iter()
        .map(|p| {
            let synthetic = synthetic_program(p, &authored, programs).unwrap();
            (
                p.module.clone(),
                crate::hir::resolve(&synthetic).unwrap().into(),
            )
        })
        .collect()
}
#[test]
fn current_private_type_authority_has_exact_and_one_short_preallocation_bounds() {
    let programs = programs();
    let authored = index_authored(&programs).unwrap();
    let (result, overflow, used) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let private = PrivateTypes::derive(&programs[0], &programs, &authored)?;
        assert!(private.contains("payload", Some("private.provider"))?);
        assert!(!private.contains("payload", Some("foreign.provider"))?);
        assert!(!private.contains("read", Some("private.provider"))?);
        assert!(!private.contains("root", Some("private.provider"))?);
        Ok::<_, Vec<Diagnostic>>(())
    });
    result.unwrap();
    assert!(!overflow);
    assert!(used > 0);
    let (exact, overflow, consumed) = crate::bounded_output::with_limit_usage(used, || {
        PrivateTypes::derive(&programs[0], &programs, &authored)
    });
    assert!(exact.is_ok());
    assert!(!overflow);
    assert_eq!(consumed, used);
    let (short, overflow, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        PrivateTypes::derive(&programs[0], &programs, &authored)
    });
    assert!(short.is_err());
    assert!(overflow);
    let mut foreign = programs[0].clone();
    foreign.module_uses[0].target_module = "foreign.provider".into();
    assert!(PrivateTypes::derive(&foreign, &programs, &authored).is_err());
}
#[test]
fn fresh_and_retained_private_signatures_reject_missing_stale_and_forged_layouts() {
    let programs = programs();
    let original = resolved(&programs);
    validate_stub_signatures(&programs, &original).unwrap();
    let authored = index_authored(&programs).unwrap();
    let expected =
        reconstruct_workspace_declaration_facts(&original, &programs, &authored).unwrap();
    let mut compact = WorkspaceValidationIndex::new(&programs).unwrap();
    for (module, program) in &original {
        compact.record_module(module, program, &programs).unwrap();
    }
    compact.validate_stub_signatures(&programs).unwrap();
    assert_eq!(compact.finish(&programs).unwrap(), expected);
    for mutation in 0..4 {
        let mut modules = original.clone();
        let caller = &mut modules[0].1;
        if mutation == 0 {
            caller.types.retain(|t| t.id.as_str() != "payload");
        } else {
            let declaration = caller
                .types
                .iter_mut()
                .find(|t| t.id.as_str() == "payload")
                .unwrap();
            if mutation == 1 {
                declaration.id = crate::hir::DeclarationId::new("stale.payload");
            } else {
                let crate::hir::ResolvedTypeDeclarationKind::Record { fields } =
                    &mut declaration.kind
                else {
                    unreachable!()
                };
                if mutation == 2 {
                    fields[0].ty = crate::hir::ResolvedType::Bytes;
                } else {
                    fields[0].id = crate::hir::DeclarationId::new("foreign.field");
                }
            }
        }
        assert_eq!(
            validate_stub_signatures(&programs, &modules).unwrap_err()[0].code,
            "SPX-G173",
            "mutation {mutation}"
        );
        let mut compact = WorkspaceValidationIndex::new(&programs).unwrap();
        let recorded = modules
            .iter()
            .try_for_each(|(module, program)| compact.record_module(module, program, &programs));
        if let Err(errors) = recorded {
            assert!(
                mutation == 1 || mutation == 3,
                "layout-only mutation must reach signature parity"
            );
            assert_eq!(errors[0].code, "SPX-G173");
            continue;
        }
        assert_eq!(
            compact.validate_stub_signatures(&programs).unwrap_err()[0].code,
            "SPX-G173",
            "compact mutation {mutation}"
        );
    }
}
