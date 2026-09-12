---
covers: []
---
# public_generic_settlement.rs

- SETTLEMENT_PLAN_SCHEMA · constant · L44-L44 — pub const SETTLEMENT_PLAN_SCHEMA: &str = "semaprax.public-generic-settlement-plan.v1";
- PLAN_DOMAIN · constant · L46-L46 — const PLAN_DOMAIN: &[u8] = b"semaprax.public-generic-settlement-plan.v1\0";
- UNSUPPORTED_PARAMETER · constant · L49-L49 — pub const UNSUPPORTED_PARAMETER: &str = "SPX-PG501";
- SETTLEMENT_DISAGREEMENT · constant · L51-L51 — pub const SETTLEMENT_DISAGREEMENT: &str = "SPX-PG502";
- unsupported · function · L53-L58 — fn unsupported(subject: &str) -> Diagnostic
- disagreement · function · L60-L65 — fn disagreement(subject: &str) -> Diagnostic
- Obligation · struct · L69-L78 — pub struct Obligation
- SettlementPlan · struct · L82-L89 — pub struct SettlementPlan
- export · function · L93-L95 — pub fn export(&self) -> &str
- instance_term · function · L98-L100 — pub fn instance_term(&self) -> &str
- obligations · function · L104-L106 — pub fn obligations(&self) -> &[Obligation]
- transfer_unit · function · L111-L113 — pub fn transfer_unit(&self) -> &str
- release_order · function · L118-L124 — pub fn release_order(&self) -> Vec<&str>
- digest · function · L127-L129 — pub fn digest(&self) -> &str
- plan · function · L139-L276 — pub fn plan(
- collect · function · L280-L307 — fn collect(
- render_path · function · L309-L318 — fn render_path(fields: &[String]) -> String
- render_inventory_place · function · L320-L328 — fn render_inventory_place(place: &InventoryPlace) -> String
- frame · function · L330-L333 — fn frame(preimage: &mut Vec<u8>, bytes: &[u8])
- digest · function · L335-L341 — fn digest(domain: &[u8], bytes: &[u8]) -> String
- tests · module · L344-L344 — mod tests;
