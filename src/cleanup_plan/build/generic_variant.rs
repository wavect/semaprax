//! Canonical owned-result join for an exact authored generic variant match.
use super::*;

pub(super) fn destination(
    builder: &mut PlanBuilder<'_>,
    expression: &ResolvedExpr,
    region: CleanupRegionId,
) -> Result<Option<CleanupPlace>, Diagnostic> {
    let ResolvedExprKind::Match { mode, .. } = expression.kind else {
        return Ok(None);
    };
    if expression.ownership != OwnershipMode::Own {
        return Ok(None);
    }
    if !crate::hir::generic_variant::match_result(
        builder.program,
        builder.function,
        mode,
        &expression.ty,
        expression.ownership,
    ) {
        return Err(plan_error(
            "owning variant match result is not an exact generic template",
        ));
    }
    builder.expression_slot(expression, region)
}

pub(super) fn finish_arm(
    builder: &mut PlanBuilder<'_>,
    expression: &ResolvedExpr,
    arm: &ResolvedExpr,
    result: &mut EvalResult,
    region: CleanupRegionId,
) -> Result<(), Diagnostic> {
    if let Some(source) = result.owned_source.take() {
        if arm.ty != expression.ty || arm.ownership != OwnershipMode::Own {
            return Err(plan_error(
                "owned variant arm result type or ownership differs",
            ));
        }
        let target = destination(builder, expression, region)?
            .ok_or_else(|| plan_error("owning variant match has no destination"))?;
        builder.transfer(
            result.block,
            arm.id.clone(),
            source,
            target.clone(),
            &mut result.state,
            true,
        )?;
        result.owned_source = Some(target);
    } else if expression.ownership == OwnershipMode::Own {
        return Err(plan_error("owning variant arm has no cleanup source"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_variant_match_oracle_and_forged_arm_ownership() {
        let mut source = String::from(
            r#"
module cleanup.generic.variant;
@id("v.choice") variant Choice<P,T> {
 @id("v.data") Data { @id("v.payload") payload:P, @id("v.marker") marker:T, },
 @id("v.empty") Empty { @id("v.empty.marker") marker:T, },
}
@id("v.rebuild") fn rebuild<T>(value:own Choice<Bytes,T>)->Choice<Bytes,T>{
 match own value {
  Choice::Data {payload,marker} => Choice<Bytes,T>::Data {payload:payload,marker:marker},
  Choice::Empty {marker} => Choice<Bytes,T>::Empty {marker:marker},
 }
}
"#,
        );
        for ty in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
            source.push_str(&format!("@id(\"v.invoke.{ty}\") fn invoke_{ty}(value:own Choice<Bytes,{ty}>)->Choice<Bytes,{ty}>{{rebuild<{ty}>(value)}}\n"));
        }
        source.push_str("@id(\"v.main\") fn main()->i64{0}");
        let checked = crate::check(&source, "cleanup-generic-variant.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        assert_eq!(program.function_instances.len(), 8);
        for instance in &program.function_instances {
            let function = &instance.function;
            assert_expression_lowering_oracle(&program, function, &function.body);
            assert_eq!(
                build_plan(&program, function).unwrap(),
                function.cleanup_plan
            );
            let mut hostile = function.body.clone();
            let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
                panic!("function body block");
            };
            let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
                panic!("owning match");
            };
            arms[0].value.ownership = OwnershipMode::Value;
            for lower in [
                PlanBuilder::lower_expr_iterative,
                PlanBuilder::lower_expr_recursive_reference,
            ] {
                let mut builder = PlanBuilder::new(&program, function).unwrap();
                let state = builder.initial_state.clone();
                assert!(lower(
                    &mut builder,
                    &hostile,
                    BlockId(0),
                    state,
                    CleanupRegionId(0)
                )
                .is_err());
            }
        }
    }
}
