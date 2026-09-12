# hir/generic_result.rs

- slot · function · L4-L9 — pub(crate) fn slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool
- profile · function · L11-L27 — pub(crate) fn profile(template: &ResolvedFunctionTemplate) -> bool
- arguments · function · L29-L34 — pub(crate) fn arguments(result: &ResolvedType, arguments: &[ResolvedType]) -> bool
- copy_success · function · L36-L38 — pub(crate) fn copy_success(result: &ResolvedType) -> bool
- substitutions · function · L40-L56 — pub(crate) fn substitutions(result: &ResolvedType) -> Vec<Vec<ResolvedType>>
- concrete_signature · function · L59-L80 — pub(crate) fn concrete_signature(function: &super::ResolvedFunction) -> bool
