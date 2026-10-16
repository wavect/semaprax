//! Certificate phase uses complete old/current algorithms, no semantic repair.
use super::program;
use crate::cleanup_plan::replay::{
    expression_facts::errors::EngineError, lexical_cert, lexical_hir, ReplayBudget,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{FunctionExecutionId, ResolvedFunction};
use crate::test_allocation_observer::measure;
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn render(error: EngineError, f: &ResolvedFunction) -> Diagnostic {
    match error {
        EngineError::Owned(error) => error,
        EngineError::Structural(receipt) => receipt.legacy(f),
        other => panic!("unexpected certificate error {other:?}"),
    }
}
#[test]
fn complete_type_drop_certificate_keeps_all_fields_order_and_hostile_failures() {
    let p = program();
    for f in p
        .functions
        .iter()
        .filter(|f| f.cleanup_plan.lexical_control.is_some())
    {
        let owner = FunctionExecutionId::Monomorphic(f.id.clone());
        crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
        for mask in 0..12 {
            let mut changed = f.clone();
            let control = changed.cleanup_plan.lexical_control.as_deref_mut().unwrap();
            match mask {
                0 => {}
                1 => control.schema = "unknown.lexical.certificate",
                2 => {
                    control.loops.pop();
                }
                3 => {
                    control.exits.pop();
                }
                4 => {
                    if let Some(resume) = control.resumes.first_mut() {
                        resume.entry = crate::cleanup_plan::BlockId(u32::MAX);
                    }
                }
                5 => control
                    .abrupt_only_regions
                    .push(crate::cleanup_plan::CleanupRegionId(u32::MAX)),
                6 => {
                    control.function_success_exits.pop();
                }
                7 => {
                    if let Some(edge) = changed
                        .cleanup_plan
                        .edges
                        .iter()
                        .find(|e| {
                            matches!(
                                &e.condition,
                                crate::cleanup_plan::EdgeCondition::BooleanResult(..)
                            )
                        })
                        .cloned()
                    {
                        changed.cleanup_plan.edges.push(edge);
                    }
                }
                _ => {}
            }
            if let Some(item) = changed
                .cleanup_plan
                .lexical_control
                .as_deref_mut()
                .unwrap()
                .execution_boundaries
                .first_mut()
            {
                match mask {
                    8 => item.site = crate::hir::ExpressionId::new(&owner, "foreign.boundary"),
                    9 => item.block = crate::cleanup_plan::BlockId(u32::MAX),
                    10 => item.leaves_regions.clear(),
                    _ => {}
                }
            }
            if mask == 11 {
                changed
                    .cleanup_plan
                    .lexical_control
                    .as_deref_mut()
                    .unwrap()
                    .execution_boundaries
                    .pop();
            }
            let facts = lexical_hir::collect(&changed, &mut ReplayBudget::new()).unwrap();
            let old = lexical_cert::before_type_drop::authenticate(
                &p,
                &changed,
                &owner,
                &facts,
                &mut ReplayBudget::with_physical(&changed, usize::MAX).unwrap(),
            );
            let mut b = ReplayBudget::with_physical(&changed, usize::MAX).unwrap();
            let new = lexical_cert::paid::authenticate(&p, &changed, &owner, &facts, &mut b);
            match (old, new) {
                (Ok(a), Ok(b)) => assert!(
                    a == b,
                    "all five Auth maps, including private boundary fields, must match"
                ),
                (Err(a), Err(b)) => same(&render(a, &changed), &render(b, &changed)),
                _ => panic!("complete certificate verdict/order differs"),
            }
        }
    }
}
#[test]
fn type_drop_certificate_actual_receiving_exact_short_work_and_whole_drop() {
    let p = program();
    for f in p
        .functions
        .iter()
        .filter(|f| f.cleanup_plan.lexical_control.is_some())
    {
        let owner = FunctionExecutionId::Monomorphic(f.id.clone());
        crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
        // The caller owns independently collected immutable HIR input. Both
        // constructor phase observers borrow the same input; no cache warming.
        let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
        let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let before = old.remaining;
        drop(
            lexical_cert::before_type_drop::authenticate(&p, f, &owner, &facts, &mut old).unwrap(),
        );
        let old_paid = old.physical.as_deref().unwrap().charged();
        let old_work = before - old.remaining;
        drop(old);
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let setup = b.physical.as_deref().unwrap().charged();
        let before = b.remaining;
        drop(lexical_cert::paid::authenticate(&p, f, &owner, &facts, &mut b).unwrap());
        let exact = b.physical.as_deref().unwrap().charged();
        let work = before - b.remaining;
        drop(b);
        eprintln!("certificate-receiving {} old_paid={old_paid} new_paid={exact} old_work={old_work} new_work={work}",f.id);
        for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
            let ((), seen) = measure(|| {
                let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                let ledger = b.physical.as_deref().unwrap() as *const _;
                let text = b.refusal_text.as_ref().unwrap().as_ptr();
                let result = lexical_cert::paid::authenticate(&p, f, &owner, &facts, &mut b);
                assert_eq!(result.is_ok(), accepted);
                match &result {
                    Ok(_) => assert!(accepted),
                    Err(EngineError::Owned(error)) => {
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(error.message.as_ptr(), text);
                        let physical = b.physical.as_deref_mut().unwrap();
                        let first = physical.reserve(0).unwrap_err();
                        assert_eq!(physical.reserve(1).unwrap_err(), first);
                    }
                    _ => panic!("physical refusal must preserve prepaid message"),
                }
                assert_eq!(b.physical.as_deref().unwrap() as *const _, ledger);
                if accepted {
                    assert_eq!(b.physical.as_deref().unwrap().charged(), exact);
                }
                drop(result);
                drop(b);
            });
            assert!(!seen.invalid);
            assert_eq!(seen.live, 0);
            assert_eq!(seen.allocated, seen.freed);
        }
        for remaining in [0, work - 1, work] {
            let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
            b.remaining = remaining;
            assert_eq!(
                lexical_cert::paid::authenticate(&p, f, &owner, &facts, &mut b).is_ok(),
                remaining == work
            );
        }
    }
}

fn typed_source() -> String {
    super::SOURCE.to_owned()+"\n@id(\"flat.vec_type\") fn vec_type(values:borrow Vec<i64>)->usize {vec_len<i64>(values)}\n"
}
fn typed_program() -> crate::hir::ResolvedProgram {
    let text = typed_source();
    let ast = crate::check(&text, "type-drop.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let checked = crate::check(&canonical, "type-drop.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn types() -> Vec<crate::hir::ResolvedType> {
    use crate::hir::{DeclarationId, ResolvedType as T};
    let nominal = |id: &str, arguments| T::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    };
    vec![
        T::Never,
        T::Unit,
        T::I64,
        T::I32,
        T::Char,
        T::U8,
        T::Usize,
        T::ArrayU8(0),
        T::ArrayU8(u32::MAX),
        T::F32,
        T::F64,
        T::Bool,
        T::Str,
        T::SliceU8,
        T::MutFunctionI64,
        T::Bytes,
        T::String,
        T::StringMap,
        T::OnceFunction,
        T::OnceFunctionI64,
        T::OnceFunctionI64Pair,
        T::Function {
            parameters: vec![T::I64],
            result: Box::new(T::I64),
        },
        T::TypeParameter {
            owner: DeclarationId::new("flat.Packet"),
            index: 0,
        },
        nominal("flat.Packet", vec![]),
        nominal("missing.type", vec![]),
        nominal("flat.Packet", vec![T::I64]),
        nominal(crate::prelude::OPTION_ID, vec![T::U8]),
        nominal(crate::prelude::OPTION_ID, vec![T::I64]),
        nominal(crate::prelude::VEC_ID, vec![]),
        nominal(crate::prelude::VEC_ID, vec![T::I64]),
        nominal(crate::prelude::VEC_ID, vec![T::String]),
        nominal(crate::prelude::VEC_ID, vec![nominal("flat.Packet", vec![])]),
        nominal(
            crate::prelude::VEC_ID,
            vec![nominal("missing.type", vec![])],
        ),
        nominal(crate::prelude::VEC_ID, vec![T::I64, T::U8]),
    ]
}
#[test]
fn selected_type_drop_complete_original_shapes_and_six_fields() {
    use crate::cleanup_plan::replay::{selected_context, selected_context::child::Permit};
    let text = typed_source();
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(&text);
    forged_directories_keep_original_verdicts();
    fixture.with_context(|source| {
        let f = source
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "flat.make")
            .unwrap();
        for ty in types() {
            let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
            let ctx = selected_context::Context::new_paid(source, f, &mut b, true).unwrap();
            let old = crate::cleanup_plan::replay::type_needs_drop(&ctx, f, &ty);
            let new = (|| -> Result<bool, EngineError> {
                let permit = Permit::acquire::<bool>(f, &mut b)?;
                Ok(
                    crate::cleanup_plan::replay::selected_type_drop::type_needs_drop!(
                        permit, &ctx, f, &ty
                    ),
                )
            })();
            match (old, new) {
                (Ok(a), Ok(b)) => assert_eq!(a, b, "{ty:?}"),
                (Err(a), Err(b)) => same(&a, &render(b, f)),
                _ => panic!("complete original type/drop verdict differs: {ty:?}"),
            }
        }
    });
}

#[test]
fn selected_type_drop_whole_context_limits_work_and_drop_are_actual() {
    use crate::cleanup_plan::replay::{selected_context, selected_context::child::Permit};
    use crate::hir::ResolvedType;
    let text = typed_source();
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(&text);
    fixture.with_context(|source|{
        let f=source.functions().iter().find(|f|f.id.as_str()=="flat.make").unwrap();
        for ty in [ResolvedType::I64,ResolvedType::String,f.return_type.clone()] {
            let observe=|limit:usize,work:usize,current:bool|{
                let ((accepted,paid,used),seen)=measure(||{
                    let mut b=ReplayBudget::with_physical(f,limit).unwrap();
                    let ctx=selected_context::Context::new_paid(source,f,&mut b,true);
                    let result=match ctx {
                        Err(error)=>Err(error),
                        Ok(ctx)=>{
                            b.remaining=work;
                            let result=if current {
                                (||->Result<bool,EngineError>{let permit=Permit::acquire::<bool>(f,&mut b)?;Ok(crate::cleanup_plan::replay::selected_type_drop::type_needs_drop!(permit,&ctx,f,&ty))})()
                            } else {crate::cleanup_plan::replay::type_needs_drop(&ctx,f,&ty).map_err(EngineError::Owned)};
                            drop(ctx);
                            result
                        }
                    };
                    let accepted=result.is_ok();let paid=b.physical.as_deref().unwrap().charged();let used=work.saturating_sub(b.remaining);
                    drop(result);drop(b);(accepted,paid,used)
                });
                assert!(!seen.invalid);assert_eq!(seen.live,0);assert_eq!(seen.allocated,seen.freed);
                (accepted,paid,used,seen)
            };
            let old=observe(usize::MAX,usize::MAX,false);let new=observe(usize::MAX,usize::MAX,true);
            eprintln!("type-drop {ty:?} old={old:?} new={new:?}");assert!(old.0&&new.0);
            assert!(!observe(new.1-1,usize::MAX,true).0);assert!(observe(new.1,usize::MAX,true).0);
            assert!(!observe(usize::MAX,0,true).0);assert!(!observe(usize::MAX,new.2-1,true).0);assert!(observe(usize::MAX,new.2,true).0);
        }
    });
}

// Routing alone lends no fact or payer authority. This direct-query fixture
// deliberately changes current declarations while retaining the original index.
struct ForgedTypes<'a>(&'a crate::hir::ResolvedProgram);
impl crate::cleanup::ReplayContext for ForgedTypes<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        &self.0.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
        crate::hir::items::View::slice(&self.0.types)
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        &self.0.interfaces
    }
    fn functions(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedFunction> {
        crate::hir::items::View::slice(&self.0.functions)
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        &self.0.function_templates
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        &self.0.function_instances
    }
    fn selected_flat_records(&self) -> bool {
        true
    }
}
fn forged_directories_keep_original_verdicts() {
    use crate::cleanup_plan::replay::selected_context::child::Permit;
    use crate::hir::{DeclarationId, ResolvedType, ResolvedTypeDeclarationKind};
    for mask in 0..9 {
        let mut p = typed_program();
        match mask {
            1 => p.types.clear(),
            2 => {
                let item = p
                    .types
                    .iter()
                    .find(|t| t.id.as_str() == crate::prelude::VEC_ID)
                    .unwrap()
                    .clone();
                p.types.push(item);
            }
            3 => {
                p.types
                    .iter_mut()
                    .find(|t| t.id.as_str() == crate::prelude::VEC_ID)
                    .unwrap()
                    .name = "forged".into()
            }
            _ => {
                if let ResolvedTypeDeclarationKind::Record { fields } = &mut p
                    .types
                    .iter_mut()
                    .find(|t| t.id.as_str() == "flat.Packet")
                    .unwrap()
                    .kind
                {
                    match mask {
                        4 => fields[1].name = fields[0].name.clone(),
                        5 => fields[0].index = u32::MAX,
                        6 => {
                            let field = fields[0].clone();
                            while fields.len() < 9 {
                                fields.push(field.clone());
                            }
                        }
                        7 => {
                            fields[0].ty = ResolvedType::Nominal {
                                declaration: DeclarationId::new("forged.nested"),
                                arguments: Vec::new(),
                            }
                        }
                        8 => fields[0].id = fields[1].id.clone(),
                        _ => {}
                    }
                }
            }
        }
        let f = p
            .functions
            .iter()
            .find(|f| f.id.as_str() == "flat.make")
            .unwrap();
        let context = ForgedTypes(&p);
        for ty in [
            ResolvedType::Nominal {
                declaration: DeclarationId::new("flat.Packet"),
                arguments: Vec::new(),
            },
            ResolvedType::Nominal {
                declaration: DeclarationId::new(crate::prelude::VEC_ID),
                arguments: vec![ResolvedType::I64],
            },
        ] {
            let old = crate::cleanup_plan::replay::type_needs_drop(&context, f, &ty);
            let mut budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
            let new = (|| -> Result<bool, EngineError> {
                let permit = Permit::acquire::<bool>(f, &mut budget)?;
                Ok(
                    crate::cleanup_plan::replay::selected_type_drop::type_needs_drop!(
                        permit, &context, f, &ty
                    ),
                )
            })();
            match (old, new) {
                (Ok(a), Ok(b)) => assert_eq!(a, b),
                (Err(a), Err(b)) => same(&a, &render(b, f)),
                _ => panic!("forged directory verdict differs mask={mask}"),
            }
        }
    }
}
