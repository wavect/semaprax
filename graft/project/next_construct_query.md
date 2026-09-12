# project/next_construct_query.rs

- MAX_CANDIDATES_PER_LIST · constant · L63-L63 — const MAX_CANDIDATES_PER_LIST: usize = 128;
- Result · type · L65-L65 — type Result<T> = std::result::Result<T, Vec<crate::diagnostic::Diagnostic>>;
- ParamFact · struct · L68-L72 — struct ParamFact
- TargetFact · struct · L75-L82 — struct TargetFact
- next_constructs_payload · function · L84-L301 — pub(super) fn next_constructs_payload(
- CallFact · struct · L303-L308 — struct CallFact
- index_call_candidate · function · L310-L340 — fn index_call_candidate(
- target_fact · function · L342-L379 — fn target_fact<'a>(
- ownership_name · function · L381-L388 — fn ownership_name(mode: OwnershipMode) -> &'static str
- expression_kind · function · L390-L401 — fn expression_kind(kind: &ResolvedExprKind) -> &'static str
