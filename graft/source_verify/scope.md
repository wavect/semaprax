# source_verify/scope.rs

- VerifierScope · struct · L16-L19 — pub(super) struct VerifierScope
- VerifierFrame · enum · L21-L258 — pub(super) enum VerifierFrame<'a>
- ScalarMatchState · struct · L261-L271 — pub(super) struct ScalarMatchState<'a>
- pattern_literal_type · function · L274-L283 — pub(super) fn pattern_literal_type(value: crate::ast::PatternLiteral) -> Type
- VariantMatchState · struct · L285-L301 — pub(super) struct VariantMatchState<'a>
- VerifierCallTarget · enum · L303-L309 — pub(super) enum VerifierCallTarget<'a>
- VerifierFunctionSignature · enum · L311-L318 — pub(super) enum VerifierFunctionSignature<'a>
- verifier_signature_owned_capacity · function · L321-L346 — pub(super) fn verifier_signature_owned_capacity(
- variant_match_state_owned_capacity · function · L349-L426 — pub(super) fn variant_match_state_owned_capacity(state: &VariantMatchState<'_>) -> usize
- diagnostics_owned_capacity · function · L429-L439 — pub(super) fn diagnostics_owned_capacity(diagnostics: &Vec<Diagnostic>) -> usize
- verifier_frame_owned_capacity · function · L442-L529 — pub(super) fn verifier_frame_owned_capacity(frame: &VerifierFrame<'_>) -> usize
- params · function · L532-L537 — pub(super) fn params(&self) -> &[Param]
- return_type · function · L539-L544 — pub(super) fn return_type(&self) -> &Type
- implicit_unique_ownership · function · L546-L554 — pub(super) fn implicit_unique_ownership(&self) -> bool
