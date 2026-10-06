//! Payload-free variant equality: both operands are planned Copy variant
//! aggregates of one declaration, so `==` and `!=` compare the `i32` case tags
//! stored at offset zero.

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
}
