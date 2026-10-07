//! Authenticated Copy variant guards use authored arm order and ordinary fuel.
use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_guarded_copy_variant(
        &mut self,
        mode: hir::ResolvedMatchMode,
        scrutinee: &ResolvedExpr,
        arms: &[hir::ResolvedMatchArm],
        variant: &OwnedVariantValue,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        if !crate::variant_guards::copy_variant(self.declarations, &scrutinee.ty)
            || !nested_owned::variant_ok(self.declarations, mode, &scrutinee.ty, arms)
            || mode != hir::ResolvedMatchMode::Value
            || variant.ty != scrutinee.ty
            || !matches!(&scrutinee.ty, ResolvedType::Nominal { declaration, .. } if declaration == &variant.variant)
        {
            return Err(Flow::Guard(
                "guarded Copy variant runtime carrier is not authenticated",
            ));
        }
        let declared = concrete_variant_case_fields(self.declarations, &variant.ty, &variant.case)
            .ok_or(Flow::Guard(
                "guarded Copy variant has an unknown active case",
            ))?;
        if declared.len() != variant.fields.len() {
            return Err(Flow::Guard(
                "guarded Copy variant payload inventory disagrees",
            ));
        }
        for arm in arms {
            if let Some(guard) = &arm.guard {
                if guard.ty != ResolvedType::Bool
                    || !crate::variant_guards::admitted(
                        self.declarations,
                        &scrutinee.ty,
                        mode,
                        &arm.pattern,
                        guard,
                    )
                {
                    return Err(Flow::Guard(
                        "guarded Copy variant arm is outside the checked profile",
                    ));
                }
            }
            let bindings = if matches!(arm.pattern, hir::ResolvedMatchPattern::Wildcard) {
                Vec::new()
            } else {
                let Some(pattern) = nested_owned::arm_case_patterns(&arm.pattern)
                    .and_then(|patterns| patterns.iter().find(|pattern| matches!(pattern, hir::ResolvedMatchPattern::Variant { case, .. } if case == &variant.case))) else { continue; };
                let hir::ResolvedMatchPattern::Variant {
                    variant: pattern_variant,
                    case,
                    fields,
                } = pattern
                else {
                    unreachable!()
                };
                if pattern_variant != &variant.variant
                    || case != &variant.case
                    || fields.len() != declared.len()
                {
                    return Err(Flow::Guard(
                        "guarded Copy variant pattern disagrees with its carrier",
                    ));
                }
                nested_owned::bc_bind_fields(self, &declared, fields, variant)?
            };
            let base = environment.len();
            environment.extend(bindings);
            // Always remove arm bindings, including on terminal guard failure.
            let outcome = (|| {
                if let Some(guard) = &arm.guard {
                    match self.evaluate(guard, environment, depth)? {
                        Value::Bool(false) => return Ok(None),
                        Value::Bool(true) => {}
                        _ => return Err(Flow::Guard("non-boolean Copy variant guard")),
                    }
                }
                self.evaluate(&arm.value, environment, depth).map(Some)
            })();
            environment.truncate(base);
            if let Some(value) = outcome? {
                return Ok(value);
            }
        }
        Err(Flow::Guard(
            "guarded Copy variant selected no exhaustive arm",
        ))
    }
}
