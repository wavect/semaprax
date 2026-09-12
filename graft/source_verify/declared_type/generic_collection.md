# source_verify/declared_type/generic_collection.rs

- parameter · function · L4-L9 — fn parameter(function: &Function, index: usize) -> Option<&str>
- direct_parameter · function · L11-L13 — fn direct_parameter(ty: &Type, parameter: &str) -> bool
- carrier · function · L15-L18 — fn carrier(ty: &Type, carrier: &str, parameter: &str) -> bool
- slot · function · L20-L32 — pub(in crate::source_verify) fn slot(function: &Function, ty: &Type) -> bool
- callback · function · L34-L46 — pub(in crate::source_verify) fn callback(function: &Function, ty: &Type) -> bool
- bounded_profile · function · L48-L74 — fn bounded_profile(function: &Function) -> bool
- profile · function · L76-L78 — pub(crate) fn profile(function: &Function) -> bool
- arguments · function · L80-L86 — pub(in crate::source_verify) fn arguments(function: &Function, arguments: &[Type]) -> bool
