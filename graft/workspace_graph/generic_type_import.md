# workspace_graph/generic_type_import.rs

- template_is_admitted · function · L10-L34 — pub(super) fn template_is_admitted(declaration: &TypeDeclaration) -> bool
- rewrite_declaration_runtime_cost · function · L36-L56 — pub(super) fn rewrite_declaration_runtime_cost(
- rewrite_declaration · function · L58-L71 — pub(super) fn rewrite_declaration(
- parameter_names · function · L73-L79 — fn parameter_names(declaration: &TypeDeclaration) -> BTreeSet<String>
- is_parameter · function · L81-L84 — fn is_parameter(ty: &Type, parameters: &BTreeSet<String>) -> bool
- declaration_types · function · L86-L106 — fn declaration_types(declaration: &TypeDeclaration) -> Vec<&Type>
- declaration_types_mut · function · L108-L130 — fn declaration_types_mut(declaration: &mut TypeDeclaration) -> Vec<&mut Type>
