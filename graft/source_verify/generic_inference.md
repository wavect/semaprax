# source_verify/generic_inference.rs

- MAX_EVIDENCE_DEPTH · constant · L12-L12 — const MAX_EVIDENCE_DEPTH: usize = 128;
- MAX_EVIDENCE_NODES · constant · L13-L13 — const MAX_EVIDENCE_NODES: usize = 4096;
- EvidenceBudget · struct · L16-L18 — struct EvidenceBudget
- visit · function · L21-L24 — fn visit(&mut self) -> Option<()>
- arguments · function · L27-L48 — pub(super) fn arguments(
- expression_type · function · L53-L71 — pub(super) fn expression_type(
- infer_arguments · function · L76-L135 — fn infer_arguments(
- numeric · function · L137-L142 — fn numeric(ty: &Type) -> bool
- ordered · function · L144-L149 — fn ordered(ty: &Type) -> bool
- evidence · function · L154-L295 — fn evidence(
- generic_arguments_are_admitted · function · L297-L308 — fn generic_arguments_are_admitted(
