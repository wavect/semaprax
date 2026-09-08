//! Checked directory wire validation before an owned result is published.
use super::*;

impl Emitter<'_> {
    fn list_get_local(&mut self, local: u32) {
        self.output.push(0x20);
        write_u32(self.output, local);
    }
    fn list_set_local(&mut self, local: u32) {
        self.output.push(0x21);
        write_u32(self.output, local);
    }
    fn list_integer(&mut self, value: i64) {
        self.output.push(0x42);
        write_i64(self.output, value);
    }
    fn list_byte(&mut self, carrier: u32, index: u32) {
        self.list_get_local(carrier);
        self.list_get_local(index);
        self.output.push(0x10);
        write_u32(self.output, BYTE_GET_IMPORT);
        self.output.push(0xad);
    }
    fn list_failure_if(
        &mut self,
        carrier: u32,
        expression: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        self.output.extend([0x04, 0x40]);
        // The result passed membership validation, but is not a language owner
        // yet. Release this authenticated carrier before ordinary failure cleanup.
        self.list_get_local(carrier);
        self.output.push(0x10);
        write_u32(self.output, BYTE_DROP_IMPORT);
        self.output.push(0x41);
        write_i64(self.output, crate::filesystem_ops::IO_FAILURE as i64);
        self.list_set_local(self.plan.status);
        self.emit_filesystem_exit(expression)?;
        self.output.push(0x0b);
        Ok(())
    }
    pub(super) fn validate_filesystem_list(
        &mut self,
        carrier: u32,
        expression: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        let [index, start, previous_start, previous_end, count, compare] = self
            .plan
            .filesystem_list_scan
            .ok_or_else(|| error("filesystem directory scan locals are absent"))?;
        let (left, right, byte) = self
            .plan
            .filesystem_scan
            .ok_or_else(|| error("filesystem scan locals are absent"))?;
        for local in [index, start, previous_start, previous_end, count] {
            self.list_integer(0);
            self.list_set_local(local);
        }
        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.list_get_local(index);
        self.emit_carrier_length(carrier);
        self.output.extend([0x5a, 0x0d, 0x01]);
        self.list_byte(carrier, index);
        self.list_set_local(byte);
        self.list_get_local(byte);
        self.list_integer(255);
        self.output.push(0x56);
        self.list_failure_if(carrier, expression)?;
        self.list_get_local(byte);
        self.list_integer(47);
        self.output.push(0x51);
        self.list_failure_if(carrier, expression)?;
        self.list_get_local(byte);
        self.output.extend([0x50, 0x04, 0x40]);
        self.control_depth += 1;
        self.list_get_local(index);
        self.list_get_local(start);
        self.output.push(0x7d);
        self.list_set_local(left);
        self.list_get_local(left);
        self.output.push(0x50);
        self.list_get_local(left);
        self.list_integer(crate::filesystem_ops::MAX_PATH_BYTES as i64);
        self.output.extend([0x56, 0x72]);
        self.list_failure_if(carrier, expression)?;
        // Dot and dot-dot names never belong to a directory result.
        self.list_byte(carrier, start);
        self.list_integer(46);
        self.output.push(0x51);
        self.list_get_local(left);
        self.list_integer(1);
        self.output.extend([0x51, 0x71]);
        self.list_failure_if(carrier, expression)?;
        self.list_get_local(left);
        self.list_integer(2);
        self.output.extend([0x51, 0x04, 0x40]);
        self.control_depth += 1;
        self.list_get_local(start);
        self.list_integer(1);
        self.output.push(0x7c);
        self.list_set_local(compare);
        self.list_byte(carrier, start);
        self.list_integer(46);
        self.output.push(0x51);
        self.list_byte(carrier, compare);
        self.list_integer(46);
        self.output.extend([0x51, 0x71]);
        self.list_failure_if(carrier, expression)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        self.list_get_local(count);
        self.list_integer(1024);
        self.output.push(0x5a);
        self.list_failure_if(carrier, expression)?;
        self.list_get_local(count);
        self.output.extend([0x50, 0x45, 0x04, 0x40]);
        self.control_depth += 1;
        // Compare each adjacent pair exactly once; equal names are rejected.
        self.list_integer(0);
        self.list_set_local(compare);
        self.output.extend([0x02, 0x40, 0x03, 0x40]);
        self.control_depth += 2;
        self.list_get_local(compare);
        self.list_get_local(previous_end);
        self.list_get_local(previous_start);
        self.output.extend([0x7d, 0x5a, 0x04, 0x40]);
        self.control_depth += 1;
        self.list_get_local(compare);
        self.list_get_local(index);
        self.list_get_local(start);
        self.output.extend([0x7d, 0x5a]);
        self.list_failure_if(carrier, expression)?;
        self.output.extend([0x0c, 0x02]);
        self.control_depth -= 1;
        self.output.push(0x0b);
        self.list_get_local(compare);
        self.list_get_local(index);
        self.list_get_local(start);
        self.output.extend([0x7d, 0x5a]);
        self.list_failure_if(carrier, expression)?;
        self.list_get_local(previous_start);
        self.list_get_local(compare);
        self.output.push(0x7c);
        self.list_set_local(left);
        self.list_byte(carrier, left);
        self.list_set_local(left);
        self.list_get_local(start);
        self.list_get_local(compare);
        self.output.push(0x7c);
        self.list_set_local(right);
        self.list_byte(carrier, right);
        self.list_set_local(right);
        self.list_get_local(left);
        self.list_get_local(right);
        self.output.extend([0x54, 0x04, 0x40, 0x0c, 0x02, 0x0b]);
        self.list_get_local(left);
        self.list_get_local(right);
        self.output.push(0x56);
        self.list_failure_if(carrier, expression)?;
        self.list_get_local(compare);
        self.list_integer(1);
        self.output.push(0x7c);
        self.list_set_local(compare);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.control_depth -= 2;
        self.output.push(0x0b);
        self.control_depth -= 1;
        self.list_get_local(start);
        self.list_set_local(previous_start);
        self.list_get_local(index);
        self.list_set_local(previous_end);
        self.list_get_local(index);
        self.list_integer(1);
        self.output.push(0x7c);
        self.list_set_local(start);
        self.list_get_local(count);
        self.list_integer(1);
        self.output.push(0x7c);
        self.list_set_local(count);
        self.output.push(0x0b);
        self.control_depth -= 1;
        self.list_get_local(index);
        self.list_integer(1);
        self.output.push(0x7c);
        self.list_set_local(index);
        self.output.extend([0x0c, 0x00, 0x0b, 0x0b]);
        self.control_depth -= 2;
        self.list_get_local(start);
        self.emit_carrier_length(carrier);
        self.output.push(0x52);
        self.list_failure_if(carrier, expression)
    }
}

type ScanLocals = (Option<(u32, u32, u32)>, Option<[u32; 6]>);
pub(super) fn allocate_scan_locals(
    program: &ResolvedProgram,
    standalone_strings: bool,
    add_local: &mut impl FnMut(u8) -> Result<u32, Diagnostic>,
) -> Result<ScanLocals, Diagnostic> {
    let filesystem_scan = (!standalone_strings
        && program.permits.iter().any(|effect| {
            effect == crate::filesystem_ops::READ_EFFECT
                || effect == crate::filesystem_ops::WRITE_EFFECT
        }))
    .then(|| Ok((add_local(I64)?, add_local(I64)?, add_local(I64)?)))
    .transpose()?;
    let filesystem_list_scan = super::super::filesystem_v2::needs_list_scan(program)
        .then(|| {
            Ok([
                add_local(I64)?,
                add_local(I64)?,
                add_local(I64)?,
                add_local(I64)?,
                add_local(I64)?,
                add_local(I64)?,
            ])
        })
        .transpose()?;
    Ok((filesystem_scan, filesystem_list_scan))
}

pub(super) fn append_command_exports(
    exports: &mut Vec<u8>,
    command_io: Option<&super::super::command_io::CommandPlan>,
    line_command_io: bool,
    network_io: bool,
    http_io: bool,
    filesystem_ops: bool,
) {
    if command_io.is_some() {
        super::super::host_output::append_stderr_exports(exports);
        write_name(exports, super::super::command_io::INPUT_STATUS_EXPORT);
        exports.push(0x03);
        write_u32(exports, super::super::command_io::INPUT_STATUS_GLOBAL);
        if line_command_io {
            super::super::line_command_io::append_export(exports);
        } else if network_io {
            super::super::network_io::append_export(exports);
        } else if http_io {
            super::super::http_io::append_export(exports);
        } else if filesystem_ops {
            if command_io.is_some_and(super::super::command_io::CommandPlan::is_filesystem_v2) {
                super::super::filesystem_v2::append_export(exports);
            } else {
                super::super::filesystem_ops::append_export(exports);
            }
        }
    }
}
