# agent_lifecycle/authorization.rs

- BINDING_DOMAIN · constant · L34-L34 — const BINDING_DOMAIN: &[u8] = b"semaprax.agent-lifecycle.authorization.v1\0";
- Authorized · struct · L43-L47 — pub struct Authorized
- binding · function · L52-L54 — pub fn binding(&self) -> &str
- granted_budget · function · L61-L63 — pub(in crate::agent_lifecycle) const fn granted_budget(&self) -> i64
- consume · function · L68-L74 — pub fn consume(self) -> AuthorizedRequest
- AuthorizedRequest · struct · L78-L82 — pub struct AuthorizedRequest
- binding · function · L86-L88 — pub fn binding(&self) -> &str
- budget · function · L93-L95 — pub const fn budget(&self) -> i64
- seal · function · L99-L101 — pub fn seal(&self) -> &[u8]
- AuthorizationOutcome · enum · L105-L111 — pub(super) enum AuthorizationOutcome
- binding · function · L118-L137 — pub(super) fn binding(
- mint · function · L140-L146 — fn mint(binding: String, budget: i64, seal: Vec<u8>) -> Authorized
- run_authorize_stage · function · L151-L231 — pub(super) fn run_authorize_stage(
