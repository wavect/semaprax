//! Alias authenticated record projections without an owning materialization.
use super::*;

impl Emitter<'_> {
    pub(super) fn borrowed_call_argument(
        &self,
        argument: &ResolvedExpr,
        borrowed_bytes: bool,
    ) -> Result<Value, Diagnostic> {
        let ResolvedExprKind::Place(place) = &argument.kind else {
            return Err(error("borrowed owner call argument is not an exact place"));
        };
        if !borrowed_bytes
            && !place.projections.is_empty()
            && !projected_call_place_is_admitted(self.program, place, &argument.ty)
        {
            return Err(error(
                "borrowed aggregate call field path is unauthenticated",
            ));
        }
        // HIR validates the lexical root and full loan lifetime. Preserve its
        // carrier alias: ordinary emit_expr would stage an owned field move.
        self.place_value(place)
    }
}

fn projected_call_place_is_admitted(
    program: &ResolvedProgram,
    place: &crate::hir::Place,
    ty: &ResolvedType,
) -> bool {
    crate::hir::owned_collection_record::projected_field(&program.declarations, place, ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projected_borrowed_vec_calls_alias_exact_fields_and_refuse_forged_paths() {
        let source = r#"module borrowed.call;
@id("r") record Row {@id("r.text") text:string,}
@id("report") record Report {@id("report.rows") rows:Vec<Row>,}
@id("read") fn read(rows:borrow Vec<Row>)->usize{vec_len<Row>(rows)}
@id("inspect") fn inspect(report:borrow Report)->usize{read(report.rows)}
@id("main") fn main()->i64{let report=Report{rows:vec_with_capacity<Row>(0usize)};i64_from_usize(inspect(report)+inspect(report))}
"#;
        let program =
            crate::hir::resolve(&crate::check(source, "borrowed-call.spx").unwrap()).unwrap();
        crate::hir::validate(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "inspect")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("block")
        };
        let ResolvedExprKind::Call { args, .. } = &tail.kind else {
            panic!("call")
        };
        let ResolvedExprKind::Place(place) = &args[0].kind else {
            panic!("place")
        };
        assert!(projected_call_place_is_admitted(
            &program,
            place,
            &args[0].ty
        ));
        let mut forged = place.clone();
        forged.projections[0] = PlaceProjection::Field(DeclarationId::new("r.text"));
        assert!(!projected_call_place_is_admitted(
            &program,
            &forged,
            &args[0].ty
        ));
        assert!(!projected_call_place_is_admitted(
            &program,
            place,
            &ResolvedType::String
        ));
        let bytes = crate::wasm::emit_resolved_module(&program).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        assert_eq!(bytes, crate::wasm::emit_resolved_module(&program).unwrap());
    }
}
