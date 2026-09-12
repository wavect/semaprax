# economic_agent/intent.rs

- settlement_rail · function · L15-L22 — pub(super) fn settlement_rail(&self) -> EconomicRail
- recipient · function · L23-L30 — pub(super) fn recipient(&self) -> &str
- amount · function · L31-L38 — pub(super) fn amount(&self) -> u64
- max_fee · function · L39-L46 — pub(super) fn max_fee(&self) -> u64
- network_asset · function · L47-L54 — pub(super) fn network_asset(&self) -> (&str, &str)
- admit_intent · function · L57-L150 — pub(super) fn admit_intent(policy: &Policy, intent: &Intent) -> Result<(), Diagnostic>
- render_intent · function · L152-L164 — pub(super) fn render_intent(intent: &Intent) -> String
- parse_intent · function · L166-L360 — pub(super) fn parse_intent(source: &str) -> Result<Intent, Diagnostic>
