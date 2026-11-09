//! Runtime interpretation of the exact immutable `List<i64>` source profile.

use super::*;
use crate::immutable_list::{ImmutableList, ListStep};
use crate::list_ops::{self, ListOp};

impl Evaluator<'_> {
    pub(super) fn evaluate_list_op(
        &mut self,
        op: ListOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        if !type_arguments.is_empty() || args.len() != op.argument_count() {
            return Err(Flow::Guard("invalid immutable list operation shape"));
        }
        match op {
            ListOp::Nil => Ok(Value::List(ImmutableList::nil())),
            ListOp::Cons => {
                // Both arguments are immutable values. The source remains live
                // on capacity refusal, and physical tail sharing is read-only.
                let Value::Int(head) = self.evaluate(&args[0], environment, depth)? else {
                    return Err(Flow::Guard("invalid immutable list head"));
                };
                let Value::List(tail) = self.evaluate(&args[1], environment, depth)? else {
                    return Err(Flow::Guard("invalid immutable list tail"));
                };
                let Ok(list) = ImmutableList::cons(head, tail) else {
                    return Err(Flow::Guard("immutable list length limit"));
                };
                Ok(Value::List(list))
            }
            ListOp::Uncons => {
                let Value::List(list) = self.evaluate(&args[0], environment, depth)? else {
                    return Err(Flow::Guard("invalid immutable list carrier"));
                };
                let mut fields = BTreeMap::new();
                let case = match list.uncons() {
                    ListStep::Nil => hir::CompilerDeclaration::ListNil,
                    ListStep::Cons { head, tail } => {
                        fields.insert(
                            hir::DeclarationId::compiler(hir::CompilerDeclaration::ListHead),
                            Value::Int(head),
                        );
                        fields.insert(
                            hir::DeclarationId::compiler(hir::CompilerDeclaration::ListTail),
                            Value::List(tail),
                        );
                        hir::CompilerDeclaration::ListCons
                    }
                };
                Ok(Value::Variant(Arc::new(OwnedVariantValue {
                    ty: list_ops::resolved_step(),
                    variant: hir::DeclarationId::compiler(hir::CompilerDeclaration::ListStep),
                    case: hir::DeclarationId::compiler(case),
                    fields,
                })))
            }
        }
    }
}
