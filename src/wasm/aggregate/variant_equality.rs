//! Case-tag tests over planned Copy variant aggregates, whose `i32` case tag
//! sits at offset zero: payload-free variant equality compares two tags, and
//! an or-pattern arm over payload-free cases tests one tag against each
//! alternative.

use super::*;

impl Emitter<'_> {
    pub(super) fn emit_case_equality(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        require_type(
            value_type(left),
            value_type(right),
            "variant equality operands",
        )?;
        for operand in [left, right] {
            let Value::Aggregate { pointer, .. } = operand else {
                return Err(error("variant equality operand is not a planned aggregate"));
            };
            self.emit_pointer(*pointer);
            self.output.extend([0x28, 0x02, 0x00]);
        }
        // i32.eq / i32.ne, then local.set the bool destination.
        self.output
            .push(if op == BinaryOp::Eq { 0x46 } else { 0x47 });
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }

    /// Emits one or-pattern arm of a plain value variant match: the disjunction
    /// of its case tests guards the arm value, and the remaining arms follow
    /// in the `else` branch exactly as after a single-case arm.
    pub(super) fn emit_case_or_arm(
        &mut self,
        emission: &VariantMatchEmission<'_>,
        index: usize,
        alternatives: &[crate::hir::ResolvedMatchPattern],
    ) -> Result<(), Diagnostic> {
        if emission.mode != crate::hir::ResolvedMatchMode::Value {
            return Err(error("or-pattern arm requires a plain value variant match"));
        }
        for (position, alternative) in alternatives.iter().enumerate() {
            let crate::hir::ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            } = alternative
            else {
                return Err(error("or-pattern alternative is not a case pattern"));
            };
            let case_layout = emission
                .layout
                .case(case)
                .filter(|_| *variant == emission.layout.variant && fields.is_empty())
                .ok_or_else(|| error(format!("or-pattern references foreign case `{case}`")))?;
            let tag = i64::from(case_layout.tag);
            self.emit_pointer(emission.scrutinee);
            // i32.load the tag, i32.const the case, i32.eq; i32.or folds the
            // alternatives after the first.
            self.output.extend([0x28, 0x02, 0x00, 0x41]);
            write_i64(self.output, tag);
            self.output.push(0x46);
            if position != 0 {
                self.output.push(0x72);
            }
        }
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        let saved = self.bindings.clone();
        let value = self.emit_expr(&emission.arms[index].value)?;
        self.copy_value(emission.destination, &value, "or-pattern match arm result")?;
        self.bindings = saved;
        self.output.push(0x05);
        self.emit_match_arms(emission, index + 1)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(())
    }
}
