# source_verify/owning_closure.rs

- SENTINEL_NAME · constant · L29-L29 — const SENTINEL_NAME: &str = "\u{0}owning-closure.v1";
- sentinel_type · function · L31-L42 — fn sentinel_type(target: &str, result: &Type) -> Type
- is_sentinel · function · L47-L49 — pub(super) fn is_sentinel(ty: &Type) -> bool
- sentinel_parts · function · L51-L69 — fn sentinel_parts(ty: &Type) -> Option<(&str, &Type)>
- check_construction · function · L77-L234 — pub(super) fn check_construction(
- check_call · function · L243-L295 — pub(super) fn check_call(
- reject_escaping_read · function · L302-L326 — pub(super) fn reject_escaping_read(
