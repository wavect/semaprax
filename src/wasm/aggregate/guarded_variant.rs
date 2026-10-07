//! Linear guarded Copy-variant chain: each guard and value is emitted once.
use super::*;
impl Emitter<'_> {
    pub(super) fn emit_guarded_variant_match(
        &mut self,
        emission: &VariantMatchEmission<'_>,
    ) -> Result<(), Diagnostic> {
        let VariantMatchEmission {
            destination,
            scrutinee,
            layout,
            arms,
            mode,
            ..
        } = emission;
        let scrutinee = *scrutinee;
        if *mode != crate::hir::ResolvedMatchMode::Value {
            return Err(error("guarded variant needs value mode"));
        }
        self.output.extend([0x02, 0x40]); // block $done
        self.control_depth += 1;
        for arm in *arms {
            self.output.extend([0x02, 0x40]); // block $reject
            self.control_depth += 1;
            let saved = self.bindings.clone();
            match &arm.pattern {
                crate::hir::ResolvedMatchPattern::Variant {
                    variant,
                    case,
                    fields,
                } => {
                    if *variant != layout.variant {
                        return Err(error("guarded variant has a foreign pattern"));
                    }
                    let case = layout
                        .case(case)
                        .cloned()
                        .ok_or_else(|| error("guarded variant has an unknown case"))?;
                    self.emit_pointer(scrutinee);
                    self.output.extend([0x28, 0x02, 0x00, 0x41]);
                    write_i64(self.output, i64::from(case.tag));
                    self.output.extend([0x47, 0x0d, 0x00]); // tag != case -> reject
                    self.bind_variant_match_fields(
                        fields,
                        &case,
                        scrutinee,
                        layout.payload_offset,
                        *mode,
                    )?;
                }
                crate::hir::ResolvedMatchPattern::Wildcard => {}
                crate::hir::ResolvedMatchPattern::Or(alternatives) => {
                    for (index, pattern) in alternatives.iter().enumerate() {
                        let crate::hir::ResolvedMatchPattern::Variant { case, fields, .. } =
                            pattern
                        else {
                            return Err(error("guarded chain has an invalid or pattern"));
                        };
                        if !fields.is_empty() {
                            return Err(error("guarded chain or pattern has payloads"));
                        }
                        let case = layout
                            .case(case)
                            .ok_or_else(|| error("guarded chain has an unknown or case"))?;
                        self.emit_pointer(scrutinee);
                        self.output.extend([0x28, 0x02, 0x00, 0x41]);
                        write_i64(self.output, i64::from(case.tag));
                        self.output.push(0x46);
                        if index != 0 {
                            self.output.push(0x72);
                        } // i32.or
                    }
                    self.output.extend([0x45, 0x0d, 0x00]); // no case -> reject
                }
                _ => return Err(error("guarded chain has a nonvariant pattern")),
            }
            if let Some(guard) = &arm.guard {
                if !crate::variant_guards::guard_shape(guard) {
                    return Err(error("invalid Copy variant guard"));
                }
                let flag = self.emit_expr(guard)?;
                require_type(
                    value_type(&flag),
                    &ResolvedType::Bool,
                    "variant match guard",
                )?;
                self.get_scalar(&flag);
                self.output.extend([0x45, 0x0d, 0x00]); // false -> reject
            }
            let value = self.emit_expr(&arm.value)?;
            self.copy_value(destination, &value, "guarded variant arm result")?;
            self.bindings = saved;
            self.output.extend([0x0c, 0x01, 0x0b]); // br $done; end $reject
            self.control_depth -= 1;
        }
        self.output.extend([0x00, 0x0b]); // impossible missing case; end $done
        self.control_depth -= 1;
        Ok(())
    }
}
