---
covers: []
---
# source_verify.rs

- arguments · module · L28-L28 — mod arguments;
- binding · module · L29-L29 — mod binding;
- capacity · module · L30-L30 — mod capacity;
- closure · module · L31-L31 — pub(crate) mod closure;
- declaration · module · L32-L32 — mod declaration;
- declared_type · module · L33-L33 — mod declared_type;
- diagnostics · module · L34-L34 — mod diagnostics;
- function_value_inventory · module · L35-L35 — pub(crate) mod function_value_inventory;
- generic_inference · module · L36-L36 — mod generic_inference;
- hints · module · L37-L37 — mod hints;
- iterative · module · L38-L38 — mod iterative;
- loans · module · L39-L39 — mod loans;
- owned_buffer · module · L40-L40 — mod owned_buffer;
- owning_closure · module · L41-L41 — mod owning_closure;
- place · module · L42-L42 — mod place;
- scope · module · L43-L43 — mod scope;
- type_table · module · L44-L44 — mod type_table;
- high_water · module · L47-L47 — mod high_water;
- oracle · module · L49-L49 — mod oracle;
- IterativeVerifier · struct · L77-L95 — struct IterativeVerifier<'a, 'p>
- new · function · L102-L134 — fn new(
- note_owned_buffer_reopen · function · L138-L151 — fn note_owned_buffer_reopen(&mut self, statement: &Statement)
- iterative_verifier_tests · module · L156-L156 — mod iterative_verifier_tests;
- generic_variant_profile · function · L160-L162 — pub(crate) fn generic_variant_profile(program: &Program, function: &Function) -> bool
