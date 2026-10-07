//! Lexical scope-exit anchors for native blocks.
//!
//! Every block settles the finalizable slots in its own canonical region,
//! including String operands nested in scalar statements and tails. Nested
//! blocks and match arms settle their own regions and are not entered.

use std::collections::BTreeSet;

use super::is_direct_plan_owned;
use crate::cleanup_plan::StorageId;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedStatement};

pub(super) fn block_anchors(
    program: &ResolvedProgram,
    plan: &crate::codegen::native_bytes::NativeBytesPlan,
    block: &ResolvedExpr,
) -> BTreeSet<StorageId> {
    let ResolvedExprKind::Block { statements, tail } = &block.kind else {
        return BTreeSet::new();
    };
    let owned = |storage: &StorageId, ty: &crate::hir::ResolvedType| {
        is_direct_plan_owned(program, ty) || plan.has_projected_leaves(storage)
    };
    let mut anchors = BTreeSet::new();
    for statement in statements {
        if let ResolvedStatement::Let { binding, .. } = statement {
            let storage = StorageId::Value(binding.id.clone());
            if owned(&storage, &binding.ty) {
                anchors.insert(storage);
            }
        }
        let value = match statement {
            ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => {
                Some(value)
            }
            ResolvedStatement::Unsafe { body, .. } => Some(body.as_ref()),
            ResolvedStatement::While { .. } => None,
        };
        if let Some(value) = value {
            let storage = StorageId::Temporary(value.id.clone());
            if owned(&storage, &value.ty) {
                anchors.insert(storage);
            }
            nested_temporaries(value, plan, &mut anchors);
        }
    }
    nested_temporaries(tail, plan, &mut anchors);
    anchors
}

/// Finalizable temporaries of `root` and its descendants in the same
/// lexical region.
fn nested_temporaries(
    root: &ResolvedExpr,
    plan: &crate::codegen::native_bytes::NativeBytesPlan,
    anchors: &mut BTreeSet<StorageId>,
) {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        let storage = StorageId::Temporary(expression.id.clone());
        if plan.is_region_slot(&storage) {
            anchors.insert(storage);
        }
        match &expression.kind {
            ResolvedExprKind::Block { .. } | ResolvedExprKind::Closure { .. } => {}
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                // Direct HIR arms share this region. A Block arm's result
                // slot belongs here too, but its children belong to its own
                // region and are excluded by the Block case above.
                pending.extend([
                    else_branch.as_ref(),
                    then_branch.as_ref(),
                    condition.as_ref(),
                ]);
            }
            ResolvedExprKind::Match { scrutinee, .. } => pending.push(scrutinee),
            _ => pending.extend(crate::interpreter::trace_child_expressions(expression)),
        }
    }
}

impl<'a, O: super::COutput> super::CEmitter<'a, O> {
    pub(super) fn emit_block_plan_scope_exit(
        &mut self,
        block: &ResolvedExpr,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        if let Some(plan) = self.bytes_plan {
            let cleanup = plan.scope_exit(&block_anchors(self.program, plan, block))?;
            for line in cleanup.lines() {
                self.line(line);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_hir_if_arms_remain_in_the_enclosing_block_region() {
        let ast = crate::parse(
            r#"module test.direct_if;
@id("app.main") fn main() -> i64 {
    let text = "ab";
    let mut i = 0;
    while i < 3 {
        let size = if i == 0 { string_len(text) } else { string_len("c") };
        i = i + size;
        0
    }
    i
}"#,
            "direct-if-arms.spx",
        )
        .unwrap();
        let mut program = crate::hir::resolve(&ast).unwrap();
        let function = &mut program.functions[0];
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!()
        };
        let ResolvedStatement::While { body, .. } = &mut statements[2] else {
            panic!()
        };
        let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
            panic!()
        };
        let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!()
        };
        let ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &mut value.kind
        else {
            panic!()
        };
        let mut expected = BTreeSet::new();
        for branch in [then_branch, else_branch] {
            let ResolvedExprKind::Block { statements, tail } = &branch.kind else {
                panic!()
            };
            assert!(statements.is_empty());
            *branch = tail.clone();
            let ResolvedExprKind::Call { args, .. } = &branch.kind else {
                panic!()
            };
            expected.insert(StorageId::Temporary(args[0].id.clone()));
        }
        // This admitted HIR shape has no lexical branch Blocks. Rebuild every
        // derived ownership proof, then require ordinary validation/replay.
        program.functions[0].loan_plan =
            crate::loan_plan::build_plan(&program, &program.functions[0]).unwrap();
        program.functions[0].cleanup =
            crate::cleanup::build_inventory(&program, &program.functions[0]).unwrap();
        program.functions[0].cleanup_plan =
            crate::cleanup_plan::build_plan(&program, &program.functions[0]).unwrap();
        crate::hir::validate(&program).unwrap();
        let function = &program.functions[0];
        let plan = crate::codegen::native_bytes::NativeBytesPlan::build(function)
            .unwrap()
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
            panic!()
        };
        let ResolvedStatement::While { body, .. } = &statements[2] else {
            panic!()
        };
        let anchors = block_anchors(&program, &plan, body);
        assert_eq!(anchors, expected);
        let region = function
            .cleanup_plan
            .regions
            .iter()
            .find(|region| region.slots.contains(anchors.first().unwrap()))
            .unwrap();
        assert!(anchors.iter().all(|anchor| region.slots.contains(anchor)));
        let exit = &function.cleanup_plan.exits[region.normal_scope_end.0 as usize];
        assert_eq!(exit.finalize_in_order.len(), 2);
        assert!(!plan.scope_exit(&anchors).unwrap().is_empty());
        crate::codegen::emit_hir_c(&program).unwrap();
    }
}
