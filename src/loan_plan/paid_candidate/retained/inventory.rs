//! Opaque actual maps, with exact subject and actual affine reservation ownership.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
type Parts<'a> = (
    cfg_directory::Map<ValueId, Place>,
    cfg_directory::Map<ExpressionId, ValueId>,
    cfg_directory::Map<ValueId, OwnershipMode>,
    crate::hir::ByteTraversalSlot<'a>,
);
pub(super) const PARTS_BYTES: usize = std::mem::size_of::<Result<Parts<'static>, Diagnostic>>()
    + std::mem::size_of::<Parts<'static>>();
pub(in crate::loan_plan) struct Inventory<'a, 'p> {
    pub(super) program: &'a ResolvedProgram,
    pub(super) function: &'a ResolvedFunction,
    pub(super) payer: &'p CfgReservation,
    pub(super) aliases: cfg_directory::Map<ValueId, Place>,
    pub(super) bound: cfg_directory::Map<ExpressionId, ValueId>,
    pub(super) ownership: cfg_directory::Map<ValueId, OwnershipMode>,
    pub(super) byte_census: crate::hir::ByteTraversalSlot<'a>,
}
impl<'a> Inventory<'a, '_> {
    pub(in crate::loan_plan) fn into_parts(
        self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        payer: Option<&CfgReservation>,
    ) -> Result<Parts<'a>, Diagnostic> {
        // This affine result and receiving tuple were fully prepaid by the
        // producer before returning Inventory. A foreign caller pays no account.
        if !std::ptr::eq(self.program, program)
            || !std::ptr::eq(self.function, function)
            || !payer.is_some_and(|payer| std::ptr::eq(self.payer, payer))
        {
            return Err(Diagnostic::io_static(
                "SPX-H006",
                "candidate inventories belong to another subject or reservation",
            ));
        }
        // Zero still authenticates the exact ACTIVE and retains first refusal.
        self.payer.reserve(0).map_err(|_| storage::marker())?;
        Ok((self.aliases, self.bound, self.ownership, self.byte_census))
    }
}
