# economic_agent/policy.rs

- limits_json · function · L23-L27 — pub(super) fn limits_json(limits: &Limits) -> String
- write_limits · function · L28-L30 — pub(super) fn write_limits<W: fmt::Write>(output: &mut W, limits: &Limits) -> fmt::Result
- render_policy · function · L32-L50 — pub(super) fn render_policy(policy: &Policy) -> String
- parse_policy · function · L52-L410 — pub(super) fn parse_policy(source: &str) -> Result<Policy, Diagnostic>
- valid_recipient · function · L412-L424 — pub(super) fn valid_recipient(rail: EconomicRail, value: &str) -> bool
- valid_origin · function · L425-L441 — pub(super) fn valid_origin(value: &str) -> bool
- valid_resource · function · L443-L475 — pub(super) fn valid_resource(value: &str) -> bool
