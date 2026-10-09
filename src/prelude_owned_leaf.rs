//! Additive v14 binding; older collection and iterator contracts stay frozen.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes() -> Vec<u8> {
    let old = crate::prelude::contract_bytes_v13();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude is UTF-8")
        .replacen(crate::prelude::SCHEMA_V13, crate::prelude::SCHEMA_V14, 1)
        .into_bytes();
    bytes.extend_from_slice(b"rule owned_leaf_vec String_or_explicit_flat_record fields:1..8 owned_String_Bytes:1..2 scalar_storage:8192 owned_carriers:131072 payload_caps_unchanged\noperation core.vec.clone-at vec_clone_at <T>(borrow:Vec<T>,value:usize)->own:T\noperation core.vec.replace vec_replace <T>(own:Vec<T>,value:usize,own:T)->own:Vec<T>\noperation core.vec.reserve-owned vec_reserve_owned <T>(own:Vec<T>,value:usize)->own:Vec<T>\noperation core.vec.sort-owned vec_sort_owned <T>(own:Vec<T>)->own:Vec<T>\nrule owned_leaf_clone fresh_leaves_declaration_order partial_cleanup sticky_failure no_borrow_escape\nrule owned_leaf_sort stable_declared_field_lexicographic unsigned_UTF8_Bytes scalar_total_order whole_carriers no_clones\nrule owned_leaf_replace_reserve preflight_then_group_commit same_owner_renewal legacy_refusals_unchanged\n");
    bytes
}
pub(crate) fn digest_text() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}
fn ast_new_element(program: &crate::ast::Program, element: &crate::ast::Type) -> bool {
    crate::source_verify::owned_leaf_source_admitted(program, element)
        && !crate::source_verify::owned_leaf_legacy_source_element(program, element)
}
pub(crate) fn program_uses(program: &crate::ast::Program) -> bool {
    let uses = |f: &crate::ast::Function| {
        let carrier = |ty: &crate::ast::Type| matches!(ty, crate::ast::Type::Named { name, arguments } if matches!(name.as_str(), "Vec" | "Iter" | "IterStep") && matches!(arguments.as_slice(), [element] if ast_new_element(program, element)));
        if carrier(&f.return_type) || f.params.iter().any(|p| carrier(&p.ty)) {
            return true;
        }
        let mut pending = f
            .requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
            .collect::<Vec<_>>();
        while let Some(expr) = pending.pop() {
            if let crate::ast::ExprKind::Call {
                name,
                type_arguments,
                ..
            } = &expr.kind
            {
                if crate::vec_ops::by_name(name).is_some_and(|op| op.owned_leaf_only()
                    || matches!(type_arguments.as_slice(), [element] if ast_new_element(program, element)))
                    || (crate::iterator_ops::by_name(name).is_some() && matches!(type_arguments.as_slice(), [element] if ast_new_element(program, element))) { return true; }
            }
            if matches!(&expr.kind, crate::ast::ExprKind::ConstructVariant { type_name, type_arguments, .. } if type_name == "IterStep" && matches!(type_arguments.as_slice(), [element] if ast_new_element(program, element)))
            {
                return true;
            }
            let mut index = 0;
            while let Some(child) = expr.child(index) {
                pending.push(child);
                index += 1;
            }
        }
        false
    };
    program.functions.iter().any(uses) || program.types.iter().any(|ty| matches!(&ty.kind, crate::ast::TypeDeclarationKind::Class { methods, .. } if methods.iter().any(uses)))
}
pub(crate) fn resolved_program_uses(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|f| crate::hir::owned_leaf_collection::function_requires_profile(program, f))
}
