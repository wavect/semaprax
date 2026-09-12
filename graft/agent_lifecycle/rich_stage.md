# agent_lifecycle/rich_stage.rs

- invariant · function · L101-L106 — fn invariant(field: &str) -> Diagnostic
- persistent · function · L108-L117 — fn persistent(program: &ResolvedProgram, id: &str, field: &str) -> Result<(), Diagnostic>
- nominal · function · L119-L124 — fn nominal(id: &str) -> ResolvedType
- find_type · function · L126-L141 — fn find_type<'a>(
- find_function · function · L143-L158 — fn find_function<'a>(
- require_param · function · L160-L178 — fn require_param(
- TwoCaseShape · struct · L195-L201 — struct TwoCaseShape
- two_case_shape · function · L203-L286 — fn two_case_shape(
- RichProposalStages · struct · L304-L321 — pub struct RichProposalStages
- schema · function · L327-L329 — pub fn schema(&self) -> &CompiledInteractionSchema
- bind_rich_proposal_stages · function · L341-L471 — pub fn bind_rich_proposal_stages(
- RichTurnOutcome · enum · L475-L484 — pub enum RichTurnOutcome
- refused · function · L486-L491 — fn refused(field: &str) -> Diagnostic
- run_rich_turn · function · L504-L609 — pub fn run_rich_turn(
- tests · module · L612-L612 — mod tests;
