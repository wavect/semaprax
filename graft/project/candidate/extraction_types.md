# project/candidate/extraction_types.rs

- Types · struct · L7-L15 — pub(super) struct Types<'a>
- new · function · L18-L49 — pub(super) fn new(
- check · function · L51-L90 — pub(super) fn check(&mut self, ty: &ResolvedType) -> Result<()>
- ast · function · L92-L95 — pub(super) fn ast(&mut self, ty: &ResolvedType) -> Result<Type>
- result · function · L99-L112 — pub(super) fn result(&mut self, ty: &ResolvedType, mode: OwnershipMode) -> Result<Type>
- project · function · L114-L150 — fn project(&mut self, ty: &ResolvedType) -> Result<Type>
- internal · function · L154-L190 — pub(super) fn internal(&mut self, ty: &ResolvedType, mode: OwnershipMode) -> Result<bool>
- charge · function · L193-L201 — fn charge(nodes: &mut usize, count: usize) -> Result<()>
- scalar · function · L203-L212 — fn scalar(ty: &ResolvedType) -> Option<Type>
