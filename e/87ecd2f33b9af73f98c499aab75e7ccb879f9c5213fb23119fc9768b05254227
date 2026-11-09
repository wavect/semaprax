//! Constructor/lender controls use real loans; no forged certificate constructor.
use super::super::WORK;
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::workspace_graph::AuthoredDeclaration;
const SOURCE: &str = "module constructor.loan; @id(\"helper\") fn helper(x:i64)->i64{x} @id(\"other\") fn other()->i64{0} @id(\"main\") fn main()->i64{helper(1)}";
fn index(programs: &[Program]) -> Index<'_> {
    let mut result = Index::new();
    for program in programs {
        for function in &program.functions {
            result.insert(
                function.stable_id.as_str(),
                AuthoredDeclaration {
                    path: &program.path,
                    module: &program.module,
                    explicit: function.explicit_id,
                    kind: crate::workspace_graph::AuthoredKind::Function,
                    function: Some(function),
                    ty: None,
                },
            );
        }
    }
    result
}

#[test]
fn foreign_function_source_context_and_budget_cannot_reuse_constructor_loans() {
    let programs = [crate::parse(SOURCE, "constructor.spx").unwrap()];
    let authored = index(&programs);
    let foreign = index(&programs);
    let target_copy = programs[0].functions[0].clone();
    let ctx = Context {
        program: &programs[0],
        authored: &authored,
        programs: &programs,
    };
    let owner = &programs[0].functions[2];
    let target = &programs[0].functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut current = None;
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        assert_eq!(Session::checkpoint(&current), 1);
        Session::observe(&mut current, ctx, owner, &target_copy, 1).unwrap();
        assert_eq!(
            Session::checkpoint(&current),
            1,
            "copied header is not the live Function"
        );
        let wrong = Context {
            authored: &foreign,
            ..ctx
        };
        assert!(Session::observe(&mut current, wrong, owner, target, 1).is_err());
        assert_eq!(Session::checkpoint(&current), 1);
        let complete = Session::finish(current, ctx).unwrap();
        assert_eq!(complete.slots, 1);
        assert!(complete.bytes > 0);
        let mut current = None;
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        assert!(Session::finish(current, wrong).is_err());
    });
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut current = None;
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        let ((), inner_refused, _, inner_first) = with_limit_usage_refusal(usize::MAX, || {
            assert!(Session::observe(&mut current, ctx, owner, target, 1).is_err());
        });
        assert!(
            !inner_refused && inner_first.is_none(),
            "foreign current lender is untouched"
        );
        assert_eq!(Session::checkpoint(&current), 1);
        assert!(Session::finish(current, ctx).is_err());
        assert!(
            !reserve_active_required(0),
            "restoring scope cannot revive old credit"
        );
    });
    assert!(refused && first.is_some());
}

#[test]
fn incomplete_body_rollback_and_exhausted_optional_work_cannot_create_credit() {
    let programs = [crate::parse(SOURCE, "constructor.spx").unwrap()];
    let authored = index(&programs);
    let ctx = Context {
        program: &programs[0],
        authored: &authored,
        programs: &programs,
    };
    let owner = &programs[0].functions[2];
    let target = &programs[0].functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut current = None;
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        assert!(Session::rollback(&mut current, 2).is_err());
        assert_eq!(Session::checkpoint(&current), 1);
        let before = current.as_ref().unwrap().invoice.bytes();
        assert_eq!(
            current.as_ref().unwrap().invoice.work(WORK),
            Err(Failure::Work)
        );
        Session::observe(&mut current, ctx, owner, &programs[0].functions[1], 0).unwrap();
        assert_eq!(Session::checkpoint(&current), 0);
        Session::rollback(&mut current, 1).unwrap();
        Session::observe(&mut current, ctx, owner, target, 1).unwrap();
        assert_eq!(Session::checkpoint(&current), 0);
        let final_cost = Session::finish(current, ctx).unwrap();
        assert_eq!(final_cost.slots, 0);
        assert!(final_cost.bytes > before);
        // Bounded optional source work is distinct from physical exhaustion.
        assert!(reserve_active_required(0));
    });
}

#[test]
fn direct_constructor_rejects_alias_and_forged_authored_source() {
    let mut programs = [crate::parse(SOURCE, "constructor.spx").unwrap()];
    // Copy a real parsed alias declaration rather than inventing an authority.
    let mut imported = crate::parse("module alias.guard; use function @id(\"external.helper\") from x.y as hidden; @id(\"m\") fn main()->i64{0}", "alias.spx").unwrap();
    {
        programs[0].module_uses = std::mem::take(&mut imported.module_uses);
        assert!(!programs[0].module_uses.is_empty());
        let authored = index(&programs);
        let ctx = Context {
            program: &programs[0],
            authored: &authored,
            programs: &programs,
        };
        with_limit_usage_refusal(usize::MAX, || {
            let mut current = None;
            Session::observe(
                &mut current,
                ctx,
                &programs[0].functions[2],
                &programs[0].functions[0],
                1,
            )
            .unwrap();
            let cost = Session::finish(current, ctx).unwrap();
            assert_eq!(cost.slots, 0);
            assert!(cost.bytes > 0);
        });
    }
    let programs = [crate::parse(SOURCE, "constructor.spx").unwrap()];
    let mut authored = index(&programs);
    authored.get_mut("helper").unwrap().path = "foreign.spx";
    let ctx = Context {
        program: &programs[0],
        authored: &authored,
        programs: &programs,
    };
    with_limit_usage_refusal(usize::MAX, || {
        let mut current = None;
        Session::observe(
            &mut current,
            ctx,
            &programs[0].functions[2],
            &programs[0].functions[0],
            1,
        )
        .unwrap();
        assert_eq!(Session::finish(current, ctx).unwrap().slots, 0);
    });
}
