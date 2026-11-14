//! Generated-module receipt acquired only from the original checked source owner.
use super::*;

/// No constructor accepts caller module bytes, mutable HIR, or a closure receipt.
pub(in crate::wasm) struct PartitionModule {
    closure: Closure,
}
impl PartitionModule {
    pub(in crate::wasm) fn emit(
        owner: &crate::hir::byte_partition::ValidatedPartitionProgram<'_>,
        receiving_work: &mut usize,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        let module = super::super::aggregate::emit_partition(owner, receiving_work)?;
        let closure = closure_impl(&module, Host::PrivateOrdinary, true).map_err(|refusal| {
            crate::diagnostic::Diagnostic::io(
                "SPX-W110",
                format!("private partition consumer closure: {refusal:?}"),
            )
        })?;
        Ok(Self { closure })
    }
    /// Compiler-paired private entry; caller supplies only finite account bytes.
    pub(in crate::wasm) fn entry_source(&self) -> String {
        use std::fmt::Write;
        let mut module_hex = String::with_capacity(self.bytes().len() * 2);
        for byte in self.bytes() {
            write!(module_hex, "{byte:02x}").expect("String writer");
        }
        let mut statuses = Vec::new();
        use super::super::aggregate as guest;
        use crate::cleanup_plan::StatusCase;
        for (raw, case) in [
            (guest::STATUS_ADD_OVERFLOW, StatusCase::AddOverflow),
            (guest::STATUS_SUB_OVERFLOW, StatusCase::SubOverflow),
            (guest::STATUS_MUL_OVERFLOW, StatusCase::MulOverflow),
            (guest::STATUS_DIV_ZERO, StatusCase::DivisionByZero),
            (guest::STATUS_DIV_OVERFLOW, StatusCase::DivisionOverflow),
            (guest::STATUS_REM_ZERO, StatusCase::RemainderByZero),
            (guest::STATUS_REM_OVERFLOW, StatusCase::RemainderOverflow),
            (guest::STATUS_NEG_OVERFLOW, StatusCase::NegationOverflow),
        ] {
            statuses.push((
                raw as u32,
                crate::conformance::ARITHMETIC_STATUS_DOMAIN_V1,
                case.code(),
            ));
        }
        statuses.extend([
            (
                guest::STATUS_REQUIRES_FALSE as u32,
                crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
                crate::conformance::CONTRACT_REQUIRES_FALSE_CODE,
            ),
            (
                guest::STATUS_ENSURES_FALSE as u32,
                crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
                crate::conformance::CONTRACT_ENSURES_FALSE_CODE,
            ),
            (
                guest::STATUS_BYTE_RANGE_START_AFTER_END as u32,
                crate::byte_ops::RANGE_STATUS_DOMAIN,
                crate::byte_ops::RANGE_START_AFTER_END_CODE,
            ),
            (
                guest::STATUS_BYTE_RANGE_END_OUT_OF_BOUNDS as u32,
                crate::byte_ops::RANGE_STATUS_DOMAIN,
                crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
            ),
            (
                guest::STATUS_VEC_PUSH_FULL as u32,
                crate::vec_ops::STATUS_DOMAIN,
                crate::vec_ops::PUSH_FULL_CODE,
            ),
            (
                guest::STATUS_VEC_GET_OUT_OF_BOUNDS as u32,
                crate::vec_ops::STATUS_DOMAIN,
                crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
            ),
            (
                guest::STATUS_VEC_ALLOCATION_FAILURE as u32,
                crate::vec_ops::STATUS_DOMAIN,
                crate::vec_ops::ALLOCATION_FAILURE_CODE,
            ),
            (
                guest::STATUS_BYTE_BUFFER_INDEX_OUT_OF_BOUNDS as u32,
                crate::byte_ops::SET_STATUS_DOMAIN,
                crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
            ),
            (
                guest::STATUS_BYTE_BUFFER_CAPACITY_EXCEEDED as u32,
                crate::byte_ops::SET_STATUS_DOMAIN,
                crate::byte_ops::CAPACITY_EXCEEDED_CODE,
            ),
        ]);
        statuses.extend(PARTITION_STATUS_CODES.iter().map(|(_, code)| {
            (
                PARTITION_GUEST_STATUS_BASE + code,
                PARTITION_STATUS_DOMAIN,
                *code,
            )
        }));
        let table = statuses
            .iter()
            .map(|(raw, domain, code)| {
                format!(
                    "[{raw},{{\"domain\":{},\"code\":{code}}}]",
                    crate::diagnostic::quote_json(domain)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut entry = include_str!("partition/entry.mjs").to_owned();
        for (placeholder, status) in [
            ("$ADD_OVERFLOW", guest::STATUS_ADD_OVERFLOW),
            ("$SUB_OVERFLOW", guest::STATUS_SUB_OVERFLOW),
            ("$MUL_OVERFLOW", guest::STATUS_MUL_OVERFLOW),
            ("$DIV_ZERO", guest::STATUS_DIV_ZERO),
            ("$DIV_OVERFLOW", guest::STATUS_DIV_OVERFLOW),
            ("$REM_ZERO", guest::STATUS_REM_ZERO),
            ("$REM_OVERFLOW", guest::STATUS_REM_OVERFLOW),
            ("$NEG_OVERFLOW", guest::STATUS_NEG_OVERFLOW),
        ] {
            entry = entry.replace(placeholder, &status.to_string());
        }
        entry
            .replace("$STATUS_TABLE", &format!("[{table}]"))
            .replace("$MODULE_HEX", &module_hex)
    }
    pub(in crate::wasm) fn bytes(&self) -> &[u8] {
        &self.closure.module
    }
}

#[cfg(test)]
mod tests;
