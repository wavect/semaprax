//! Closed typed collection calls share ordinary owned argument staging.
use super::*;
use crate::map_ops::MapOp;
#[allow(clippy::too_many_arguments)]
pub(super) fn schedule<'expr>(resolver: &Resolver<'_>, _function: &FunctionExecutionId,
    frames: &mut Vec<Frame<'expr>>, type_arguments: &[Type], args: &'expr [Expr],
    bindings: Rc<BTreeMap<String, Binding>>, path: String, span: Span, op: MapOp,
) -> Result<(), Diagnostic> {
    let collection = op.ast_type(type_arguments).and_then(|ty|crate::map_ops::ast_resolved(&ty))
        .ok_or_else(||resolver.error("SPX-T274", "typed collection requires admitted key/value arguments", span))?;
    if args.len()!=op.arity() {return Err(resolver.error("SPX-H006", "invalid typed collection call arity", span));}
    frames.push(Frame::FinishOwnedGenericOp { span, path:path.clone(), op:OwnedGenericCallSite::Map(op), element:collection, argument_count:args.len() });
    frames.push(Frame::ChildNext {children:args,index:0,bindings,path,segment:"arg"});
    Ok(())
}
pub(super) fn finish(function:&FunctionExecutionId,path:&str,span:Span,op:MapOp,collection:ResolvedType,args:Vec<ResolvedExpr>) -> Result<ResolvedExpr,Diagnostic> {
    let type_arguments=op.resolved_arguments(&collection).ok_or_else(||Diagnostic::io("SPX-H006","invalid typed collection instance"))?;
    let (params,ty)=op.resolved_signature(&type_arguments).ok_or_else(||Diagnostic::io("SPX-H006","invalid typed collection signature"))?;
    if args.len()!=params.len() || args.iter().zip(&params).any(|(arg,param)|arg.ty!=param.ty) {
        return Err(Diagnostic::io("SPX-H006","typed collection arguments do not match exact signature"));
    }
    let ownership=if crate::map_ops::is_collection(&ty)||ty==ResolvedType::String {OwnershipMode::Own}else{OwnershipMode::Value};
    Ok(ResolvedExpr{id:ExpressionId::new(function,path),ownership,ty,span,kind:ResolvedExprKind::Call {callee:DeclarationId::new(op.id()),type_arguments,instance:None,args}})
}
#[cfg(test)]
pub(super) fn reference(resolver:&Resolver<'_>,function:&FunctionExecutionId,call:ReferenceCall<'_>,op:MapOp)->Result<ResolvedExpr,Diagnostic> {
    let collection=op.ast_type(call.type_arguments).and_then(|ty|crate::map_ops::ast_resolved(&ty)).ok_or_else(||Diagnostic::io("SPX-H006","invalid typed collection arguments"))?;
    let mut args=Vec::new();
    for (index,arg) in call.args.iter().enumerate() {args.push(resolver.resolve_expr_recursive_reference(function,arg,call.bindings,&format!("{}.arg{index}",call.path))?);}
    finish(function,call.path,call.span,op,collection,args)
}
