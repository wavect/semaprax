# assurance_manifest/derive.rs

- CONTRACT_GUARD_TOOL · constant · L19-L19 — const CONTRACT_GUARD_TOOL: &str = "semaprax-runtime-contract-guard";
- OWNERSHIP_CHECKER_TOOL · constant · L23-L23 — const OWNERSHIP_CHECKER_TOOL: &str = "semaprax-ownership-checker";
- derive_obligations · function · L29-L53 — pub(super) fn derive_obligations(program: &Program) -> Vec<Obligation>
- contract_obligation · function · L55-L77 — fn contract_obligation(
- ownership_parameter_obligation · function · L79-L96 — fn ownership_parameter_obligation(declaration_id: &str, index: usize) -> Obligation
- tests · module · L99-L222 — mod tests
- program · function · L102-L104 — fn program(source: &str) -> Program
- derives_one_obligation_per_clause_and_parameter · function · L107-L149 — fn derives_one_obligation_per_clause_and_parameter()
- contract_obligations_are_runtime_guarded_not_compiler_proved · function · L152-L172 — fn contract_obligations_are_runtime_guarded_not_compiler_proved()
- ownership_parameter_obligations_are_compiler_proved · function · L175-L190 — fn ownership_parameter_obligations_are_compiler_proved()
- a_function_with_no_clauses_and_no_parameters_derives_nothing · function · L193-L203 — fn a_function_with_no_clauses_and_no_parameters_derives_nothing()
- derivation_is_deterministic_across_repeated_calls · function · L206-L221 — fn derivation_is_deterministic_across_repeated_calls()
