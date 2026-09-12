# source_verify/declared_type/generic_result.rs

- slot · function · L5-L12 — pub(in crate::source_verify) fn slot(function: &Function, ty: &Type) -> bool
- profile · function · L14-L26 — pub(in crate::source_verify) fn profile(function: &Function) -> bool
- arguments · function · L28-L44 — pub(in crate::source_verify) fn arguments(function: &Function, arguments: &[Type]) -> bool
- body · function · L46-L75 — pub(in crate::source_verify) fn body(function: &Function, expression: &Expr) -> bool
- copy_success · function · L77-L79 — pub(in crate::source_verify) fn copy_success(function: &Function) -> bool
- substitutions · function · L81-L87 — pub(in crate::source_verify) fn substitutions(function: &Function) -> Vec<Vec<Type>>
- concrete_try · function · L89-L94 — pub(in crate::source_verify) fn concrete_try(operand: &Type, result: &Type) -> bool
