//! Typed Map/Set lowering. Pointer owners obey the canonical call-commit plan.
use super::*;
use crate::map_ops::MapOp;
pub(super) fn emit_runtime(output:&mut impl COutput,program:&ResolvedProgram) {
    if crate::map_ops::resolved_program_uses(program) {output.push_str(include_str!("map_runtime.c"));}
}
fn atom_member(ty:&ResolvedType)->Result<&'static str,Diagnostic> {
    Ok(match ty {ResolvedType::String=>"text",ResolvedType::I64=>"i64",ResolvedType::I32=>"i32",ResolvedType::U8=>"u8",ResolvedType::Usize=>"usize",ResolvedType::Char=>"character",ResolvedType::F32=>"f32",ResolvedType::F64=>"f64",ResolvedType::Bool=>"boolean",_=>return Err(backend_error("invalid collection atom type"))})
}
fn tag(ty:&ResolvedType)->Result<u32,Diagnostic> {
    Ok(match ty {ResolvedType::String=>1,ResolvedType::I64=>2,ResolvedType::Bool=>3,ResolvedType::I32=>4,ResolvedType::U8=>5,ResolvedType::Usize=>6,ResolvedType::Char=>7,ResolvedType::F32=>8,ResolvedType::F64=>9,_=>return Err(backend_error("invalid collection atom tag"))})
}
impl<O:COutput> CEmitter<'_,O> {
    pub(super) fn emit_typed_map_op(&mut self,op:MapOp,types:&[ResolvedType],args:&[ResolvedExpr],result_type:&ResolvedType,expression:&ExpressionId)->Result<CValue,Diagnostic> {
        if let Some(legacy)=op.legacy(types) {return self.emit_string_op(legacy,args,result_type,expression);}
        let (params,result)=op.resolved_signature(types).ok_or_else(||backend_error("invalid typed collection signature"))?;
        self.require_type(result_type,&result,"typed collection result")?;
        let mut arguments=Vec::new();
        for (index,arg) in args.iter().enumerate() {
            let value=self.emit_expr(arg)?;
            self.require_type(&value.ty,&params[index].ty,"typed collection operand")?;
            arguments.push(if params[index].ownership==hir::OwnershipMode::Own {self.stage_bytes_call_argument(expression,index,arg,hir::OwnershipMode::Own,value)?} else {value});
        }
        let temporary=if is_direct_plan_owned(self.program,result_type) {self.bytes_plan.ok_or_else(||backend_error("collection producer has no cleanup plan"))?.value(&crate::cleanup_plan::StorageId::Temporary(expression.clone()))?.to_owned()}else{self.temporary(result_type)?};
        let atom=|i:usize|->Result<String,Diagnostic>{Ok(format!("(spx_map_atom_v2){{.{} = {}}}",atom_member(&arguments[i].ty)?,arguments[i].code))};
        let owner=||arguments[0].code.as_str();
        match op {
            MapOp::New|MapOp::SetNew=> {
                let value_tag=if op.is_set(){3}else{tag(&types[1])?};
                self.line(&format!("spx_status = spx_collection_new_v2(spx_ctx, {}, {}, {}, &{temporary});",tag(&types[0])?,value_tag,arguments[0].code));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            MapOp::Len|MapOp::SetLen=>self.line(&format!("{temporary} = {}->len;",owner())),
            MapOp::Has|MapOp::SetHas=>self.line(&format!("{temporary} = spx_collection_find_v2({}, {}, NULL);",owner(),atom(1)?)),
            MapOp::GetOr|MapOp::KeyAt|MapOp::ValueAt|MapOp::SetKeyAt=> {
                let out_atom=format!("{temporary}_atom");self.line(&format!("spx_map_atom_v2 {out_atom} = {{0}};"));
                if op==MapOp::GetOr {
                    self.line(&format!("{out_atom} = spx_collection_get_v2({}, {}, {});",owner(),atom(1)?,atom(2)?));
                }else{
                    self.line(&format!("spx_status = spx_collection_at_v2(spx_ctx, {}, {}, {}, &{out_atom});",owner(),arguments[1].code,if matches!(op,MapOp::KeyAt|MapOp::SetKeyAt){"true"}else{"false"}));
                    self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                }
                self.line(&format!("{temporary} = {out_atom}.{};",atom_member(result_type)?));
            }
            _=> {
                let plan=self.bytes_plan.ok_or_else(||backend_error("collection reopen has no cleanup plan"))?;
                let (source,flag,_)=plan.call_argument(expression,0)?;
                let (source,flag)=(source.to_owned(),flag.to_owned());
                if source!=owner(){return Err(backend_error("collection reopen operand is not canonical argument"));}
                if matches!(op,MapOp::Remove|MapOp::SetRemove) {
                    self.line(&format!("{temporary} = spx_collection_remove_v2({source}, {});",atom(1)?));
                }else{
                    let value=if op==MapOp::SetInsert {"(spx_map_atom_v2){.boolean=true}".into()}else{atom(2)?};
                    self.line(&format!("spx_status = spx_collection_put_v2(spx_ctx, {source}, {}, {value}, {}, &{temporary});",atom(1)?,if op==MapOp::Add{"true"}else{"false"}));
                    self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                }
                self.line(&format!("{flag} = false;"));self.line(&format!("{source} = NULL;"));
            }
        }
        Ok(CValue{code:temporary,ty:result_type.clone()})
    }
}
