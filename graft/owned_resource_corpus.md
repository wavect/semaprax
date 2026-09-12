---
covers: []
---
# owned_resource_corpus.rs

- OWNED_RESOURCE_CORPUS_SOURCE_V1 · constant · L18-L65 — pub const OWNED_RESOURCE_CORPUS_SOURCE_V1: &str = r#"module test.owned_resource_corpus;
- OwnedResourceCorpusArgument · enum · L68-L72 — pub enum OwnedResourceCorpusArgument
- OwnedResourceCorpusCase · struct · L75-L81 — pub struct OwnedResourceCorpusCase
- OwnedResourceCorpus · struct · L84-L87 — pub struct OwnedResourceCorpus
- CasePlan · struct · L89-L97 — struct CasePlan
- build_owned_resource_corpus_v1 · function · L101-L147 — pub fn build_owned_resource_corpus_v1() -> Result<OwnedResourceCorpus, String>
- plans · function · L149-L289 — fn plans(program: &ResolvedProgram) -> Result<Vec<CasePlan>, String>
- case · function · L291-L306 — fn case(
- owned_case · function · L308-L318 — fn owned_case(
- owned · function · L320-L322 — const fn owned(payload: u64) -> OwnedResourceCorpusArgument
- function · function · L324-L330 — fn function<'a>(program: &'a ResolvedProgram, id: &str) -> Result<&'a ResolvedFunction, String>
- contract_source · function · L332-L347 — fn contract_source(
- arithmetic_source · function · L349-L359 — fn arithmetic_source(function: &ResolvedFunction) -> Result<StatusSourceId, String>
- tests · module · L362-L377 — mod tests
- canonical_corpus_builds_in_authoritative_order · function · L366-L376 — fn canonical_corpus_builds_in_authoritative_order()
