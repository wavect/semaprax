//! Compiler-owned bridges for private authenticated native identity/moves profiles.
//! No body is inferred from a descriptor: replay the selected checked HIR, then
//! use the ordinary native emitter and its own symbols and aggregate layout.

use super::*;
use crate::public_generic_abi::descriptor::verify::{
    verify_public_generic_descriptor, VerificationOptions, VerifiedPublicGenericDescriptor,
};
use crate::public_generic_type::{owned_bytes_leaf_field_paths, render_leaf_path, TypeInventory};

mod allocating;
pub(crate) use allocating::emit_public_generic_allocating_bridge;

pub(crate) fn emit_public_generic_identity_bridge(
    program: &ResolvedProgram,
    revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<(String, String), Diagnostic> {
    emit_bridge(program, revision, descriptor, false)
}

pub(crate) fn emit_public_generic_moves_bridge(
    program: &ResolvedProgram,
    revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<(String, String), Diagnostic> {
    emit_bridge(program, revision, descriptor, true)
}

// This is admission, not evaluation or an ownership checker. HIR validation
// supplies the move/cleanup proof, and the ordinary emitter executes the body.
// In particular, fresh records are allowed; fresh Bytes are not: their runtime
// allocator aborts on failure and cannot settle through this private bridge.
fn movement_body(expression: &ResolvedExpr, roots: &BTreeSet<ValueId>) -> bool {
    match &expression.kind {
        ResolvedExprKind::Place(place) => roots.contains(&place.root),
        ResolvedExprKind::ConstructRecord { fields, .. } => fields
            .iter()
            .all(|field| movement_body(&field.value, roots)),
        ResolvedExprKind::Block { statements, tail } => {
            let mut locals = roots.clone();
            for statement in statements {
                let hir::ResolvedStatement::Let {
                    mutable: false,
                    binding,
                    value,
                    ..
                } = statement
                else {
                    return false;
                };
                if !movement_body(value, &locals) || !locals.insert(binding.id.clone()) {
                    return false;
                }
            }
            movement_body(tail, &locals)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            matches!(condition.kind, ResolvedExprKind::Bool(_))
                && movement_body(then_branch, roots)
                && movement_body(else_branch, roots)
        }
        _ => false,
    }
}

fn admit<'a>(
    program: &'a ResolvedProgram,
    revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
    moves: bool,
) -> Result<&'a ResolvedFunction, Diagnostic> {
    hir::validate(program)?;
    verify_public_generic_descriptor(
        program,
        revision,
        descriptor.export_id(),
        descriptor.program_root_digest(),
        descriptor.accepted_bytes(),
        &VerificationOptions::default(),
    )?;
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == descriptor.export_id())
        .ok_or_else(|| backend_error("authenticated native identity endpoint is missing"))?;
    let mut body = &function.body;
    while let ResolvedExprKind::Block { statements, tail } = &body.kind {
        if !statements.is_empty() {
            break;
        }
        body = tail;
    }
    let input = &descriptor.input_facts();
    if function.params.len() != 1
        || input.instance_digest != descriptor.result_facts().instance_digest
        || input.fields.is_empty()
        || input.fields.iter().any(|field| field.term != "bytes")
        || !(if moves {
            movement_body(
                &function.body,
                &BTreeSet::from([function.params[0].id.clone()]),
            )
        } else {
            matches!(&body.kind, ResolvedExprKind::Place(place)
                if place.root == function.params[0].id && place.projections.is_empty())
        })
        || function
            .requires
            .iter()
            .chain(&function.ensures)
            .any(|guard| !matches!(guard.kind, ResolvedExprKind::Bool(_)))
    {
        return Err(backend_error(if moves {
            "authenticated-native-moves.v1 requires a flat Bytes movement body and literal boolean contracts"
        } else {
            "authenticated-native-identity.v1 requires a flat Bytes identity body and literal boolean contracts"
        }));
    }
    Ok(function)
}

fn emit_bridge(
    program: &ResolvedProgram,
    revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
    moves: bool,
) -> Result<(String, String), Diagnostic> {
    let function = admit(program, revision, descriptor, moves)?;
    let functions = function_index(program)?;
    let symbol = &functions[&FunctionExecutionId::Monomorphic(function.id.clone())].symbol;
    let record = c_record_symbol(&function.params[0].ty);
    let fields: Vec<_> = descriptor
        .input_facts()
        .fields
        .iter()
        .map(|field| c_field_symbol(&DeclarationId::new(&field.id)))
        .collect();
    let source = format!(
        "#define SPX_NO_ENTRY_WRAPPER\n{}\n#undef SPX_NO_ENTRY_WRAPPER\n",
        // The existing provider-carrier profile emits Bytes even when the
        // selected function only moves an aggregate and contains no byte op.
        emit_hir_c_with_labels(
            program,
            &HashMap::new(),
            NativeOutputProfile::OwnedUtf8Provider,
            None
        )?
    );
    let endpoint = if moves {
        "spx_pg_endpoint_checked_moves_v1"
    } else {
        "spx_pg_endpoint_checked_identity_v1"
    };
    let mut bridge = format!(
        r#"
static spx_pg_status_v1 {endpoint}(uint32_t leaf_count,
    uint8_t *const *input_leaf_bytes, const size_t *input_leaf_lens,
    uint8_t **out_leaf_bytes, size_t *out_leaf_lens) {{
    if (leaf_count != {}u) return SPX_PG_STATUS_MALFORMED_CARRIER;
    struct {record} input = {{0}}, result = {{0}};
    struct spx_context context = {{0}};
    struct spx_status_entry entries[1];
    if (!spx_context_init(&context, UINT64_C(1), entries, 1, NULL, NULL, NULL))
        return SPX_PG_STATUS_CONTRACT_FAILURE;
    spx_pg_status_v1 status = SPX_PG_STATUS_OK;
    uint32_t completed = 0;
"#,
        fields.len()
    );
    for (i, field) in fields.iter().enumerate() {
        writeln!(bridge, "    input.{field}.len = input_leaf_lens[{i}];\n    input.{field}.ptr = input_leaf_lens[{i}] ? (uint8_t *)malloc(input_leaf_lens[{i}]) : NULL;\n    if (input_leaf_lens[{i}] && !input.{field}.ptr) {{ input.{field}.len = 0; status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto prepare_failed; }}\n    if (input_leaf_lens[{i}]) memcpy(input.{field}.ptr, input_leaf_bytes[{i}], input_leaf_lens[{i}]);").unwrap();
    }
    writeln!(bridge, "    SPX_PG_OBSERVE_ENDPOINT();\n    if ({symbol}(&context, &input, &result) != SPX_STATUS_SUCCESS) return SPX_PG_STATUS_CONTRACT_FAILURE;").unwrap();
    for (i, field) in fields.iter().enumerate() {
        writeln!(bridge, "    if (spx_pg_physical_phase(SPX_PG_PHASE_RESULT_ALLOCATION_STARTED, 1, {i})) {{ status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto result_failed; }}\n    out_leaf_lens[{i}] = (size_t)result.{field}.len;\n    out_leaf_bytes[{i}] = result.{field}.len ? (uint8_t *)spx_pg_alloc((size_t)result.{field}.len) : NULL;\n    if (result.{field}.len && !out_leaf_bytes[{i}]) {{ status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto result_failed; }}\n    completed = {i} + 1;\n    if (spx_pg_physical_phase(SPX_PG_PHASE_RESULT_ALLOCATION_COMMITTED, 1, {i})) {{ status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto result_failed; }}\n    if (result.{field}.len) memcpy(out_leaf_bytes[{i}], result.{field}.ptr, (size_t)result.{field}.len);\n    SPX_PG_OBSERVE_RESULT_PAYLOAD({i}, out_leaf_bytes[{i}], out_leaf_lens[{i}]);\n    if (spx_pg_physical_phase(SPX_PG_PHASE_RESULT_PAYLOAD_COPIED, 1, {i})) {{ status = SPX_PG_STATUS_CONTRACT_FAILURE; goto result_failed; }}").unwrap();
    }
    bridge.push_str("result_failed:\n    if (status != SPX_PG_STATUS_OK) {\n        spx_pg_select_primary_failure(status);\n        for (uint32_t i = completed; i-- > 0;) spx_pg_release_payload(out_leaf_bytes, out_leaf_lens, i, 1);\n    }\n");
    for field in fields.iter().rev() {
        writeln!(bridge, "    spx_bytes_drop(&result.{field});").unwrap();
    }
    bridge.push_str("    return status;\nprepare_failed:\n");
    for field in fields.iter().rev() {
        writeln!(bridge, "    spx_bytes_drop(&input.{field});").unwrap();
    }
    bridge.push_str("    return status;\n}\n");
    Ok((source, bridge))
}

/// Private additive constructor for a NEW, separately versioned physical
/// profile (`semaprax.authenticated-native-moves-nested.v1`, see
/// [`super::authenticated::render_authenticated_nested_moves_provider`]).
/// `moves-v1` itself ([`emit_public_generic_moves_bridge`]/[`admit`] above)
/// stays exactly as written: still only a flat, one-level owned-Bytes record,
/// same admission code path, same emitted bytes for every program it already
/// admitted. Nested owned records (a record whose fields are themselves
/// owned-Bytes-only record instances, to a bounded depth) are admitted only
/// under this distinct profile, never by silently widening `moves-v1`'s own
/// frozen contract.
pub(crate) fn emit_public_generic_nested_moves_bridge(
    program: &ResolvedProgram,
    revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<(String, String), Diagnostic> {
    let (function, paths) = admit_nested_moves(program, revision, descriptor)?;
    let functions = function_index(program)?;
    let symbol = &functions[&FunctionExecutionId::Monomorphic(function.id.clone())].symbol;
    let record = c_record_symbol(&function.params[0].ty);
    let fields: Vec<String> = paths
        .iter()
        .map(|chain| field_access_chain(chain))
        .collect();
    let source = format!(
        "#define SPX_NO_ENTRY_WRAPPER\n{}\n#undef SPX_NO_ENTRY_WRAPPER\n",
        // The existing provider-carrier profile emits Bytes even when the
        // selected function only moves an aggregate and contains no byte op.
        emit_hir_c_with_labels(
            program,
            &HashMap::new(),
            NativeOutputProfile::OwnedUtf8Provider,
            None
        )?
    );
    let endpoint = "spx_pg_endpoint_checked_nested_moves_v1";
    let mut bridge = format!(
        r#"
static spx_pg_status_v1 {endpoint}(uint32_t leaf_count,
    uint8_t *const *input_leaf_bytes, const size_t *input_leaf_lens,
    uint8_t **out_leaf_bytes, size_t *out_leaf_lens) {{
    if (leaf_count != {}u) return SPX_PG_STATUS_MALFORMED_CARRIER;
    struct {record} input = {{0}}, result = {{0}};
    struct spx_context context = {{0}};
    struct spx_status_entry entries[1];
    if (!spx_context_init(&context, UINT64_C(1), entries, 1, NULL, NULL, NULL))
        return SPX_PG_STATUS_CONTRACT_FAILURE;
    spx_pg_status_v1 status = SPX_PG_STATUS_OK;
    uint32_t completed = 0;
"#,
        fields.len()
    );
    for (i, field) in fields.iter().enumerate() {
        writeln!(bridge, "    input.{field}.len = input_leaf_lens[{i}];\n    input.{field}.ptr = input_leaf_lens[{i}] ? (uint8_t *)malloc(input_leaf_lens[{i}]) : NULL;\n    if (input_leaf_lens[{i}] && !input.{field}.ptr) {{ input.{field}.len = 0; status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto prepare_failed; }}\n    if (input_leaf_lens[{i}]) memcpy(input.{field}.ptr, input_leaf_bytes[{i}], input_leaf_lens[{i}]);").unwrap();
    }
    writeln!(bridge, "    SPX_PG_OBSERVE_ENDPOINT();\n    if ({symbol}(&context, &input, &result) != SPX_STATUS_SUCCESS) return SPX_PG_STATUS_CONTRACT_FAILURE;").unwrap();
    for (i, field) in fields.iter().enumerate() {
        writeln!(bridge, "    if (spx_pg_physical_phase(SPX_PG_PHASE_RESULT_ALLOCATION_STARTED, 1, {i})) {{ status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto result_failed; }}\n    out_leaf_lens[{i}] = (size_t)result.{field}.len;\n    out_leaf_bytes[{i}] = result.{field}.len ? (uint8_t *)spx_pg_alloc((size_t)result.{field}.len) : NULL;\n    if (result.{field}.len && !out_leaf_bytes[{i}]) {{ status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto result_failed; }}\n    completed = {i} + 1;\n    if (spx_pg_physical_phase(SPX_PG_PHASE_RESULT_ALLOCATION_COMMITTED, 1, {i})) {{ status = SPX_PG_STATUS_ALLOCATION_FAILURE; goto result_failed; }}\n    if (result.{field}.len) memcpy(out_leaf_bytes[{i}], result.{field}.ptr, (size_t)result.{field}.len);\n    SPX_PG_OBSERVE_RESULT_PAYLOAD({i}, out_leaf_bytes[{i}], out_leaf_lens[{i}]);\n    if (spx_pg_physical_phase(SPX_PG_PHASE_RESULT_PAYLOAD_COPIED, 1, {i})) {{ status = SPX_PG_STATUS_CONTRACT_FAILURE; goto result_failed; }}").unwrap();
    }
    bridge.push_str("result_failed:\n    if (status != SPX_PG_STATUS_OK) {\n        spx_pg_select_primary_failure(status);\n        for (uint32_t i = completed; i-- > 0;) spx_pg_release_payload(out_leaf_bytes, out_leaf_lens, i, 1);\n    }\n");
    for field in fields.iter().rev() {
        writeln!(bridge, "    spx_bytes_drop(&result.{field});").unwrap();
    }
    bridge.push_str("    return status;\nprepare_failed:\n");
    for field in fields.iter().rev() {
        writeln!(bridge, "    spx_bytes_drop(&input.{field});").unwrap();
    }
    bridge.push_str("    return status;\n}\n");
    Ok((source, bridge))
}

/// The dotted C field-access suffix (no leading `input`/`result`, no leading
/// `.`) for one leaf's field-declaration-id path: `["a", "b"]` becomes
/// `spx_field_<hex a>.spx_field_<hex b>`. A one-element chain renders
/// exactly like [`emit_bridge`]'s own flat single-symbol field name, so a
/// flat body's generated access expression is byte-identical either way.
fn field_access_chain(chain: &[String]) -> String {
    chain
        .iter()
        .map(|id| c_field_symbol(&DeclarationId::new(id)))
        .collect::<Vec<_>>()
        .join(".")
}

/// Admission for the nested-owned-record physical profile: everything
/// [`admit`]'s `moves` branch already requires (exactly one owned parameter
/// whose instance type matches the result's, a checked movement body, and
/// literal boolean contracts — [`movement_body`] already walks `ConstructRecord`
/// and place projections generically, so it needs no change for nesting),
/// plus the additional, stricter shape [`emit_public_generic_nested_moves_bridge`]'s
/// C bridge can actually marshal: every transitively reachable field of the
/// owned parameter's type is either a direct `Bytes` leaf or a further owned
/// record instance, bounded exactly the way the grammar itself already bounds
/// record nesting and leaf count (see
/// [`crate::public_generic_type::owned_bytes_leaf_field_paths`]).
fn admit_nested_moves<'a>(
    program: &'a ResolvedProgram,
    revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<(&'a ResolvedFunction, Vec<Vec<String>>), Diagnostic> {
    hir::validate(program)?;
    verify_public_generic_descriptor(
        program,
        revision,
        descriptor.export_id(),
        descriptor.program_root_digest(),
        descriptor.accepted_bytes(),
        &VerificationOptions::default(),
    )?;
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == descriptor.export_id())
        .ok_or_else(|| backend_error("authenticated native nested-moves endpoint is missing"))?;
    let shape_error = || {
        backend_error(
            "authenticated-native-moves-nested.v1 requires an owned Bytes-only movement body \
             (flat or nested) and literal boolean contracts",
        )
    };
    let input = &descriptor.input_facts();
    if function.params.len() != 1
        || input.instance_digest != descriptor.result_facts().instance_digest
        || input.fields.is_empty()
        || !movement_body(
            &function.body,
            &BTreeSet::from([function.params[0].id.clone()]),
        )
        || function
            .requires
            .iter()
            .chain(&function.ensures)
            .any(|guard| !matches!(guard.kind, ResolvedExprKind::Bool(_)))
    {
        return Err(shape_error());
    }
    // The stricter, additional check: `moves`'s own flat-fields-are-`bytes`
    // check generalized to every transitively reachable field, bounded the
    // same way the grammar itself bounds nesting and leaf count. A mixed
    // record (some Bytes leaves, some Copy scalar leaves) is refused here,
    // not silently narrowed to only its Bytes leaves.
    let inventory = TypeInventory::of(program);
    let paths = owned_bytes_leaf_field_paths(&inventory, &function.params[0].ty)?
        .ok_or_else(shape_error)?;
    // Defense in depth, and not merely a count check: this module's own
    // independently walked leaf-path chains, rendered the same way
    // `InstanceFacts::owned_leaves` itself is rendered, must equal the
    // descriptor's own independently derived leaf-path list *in order* --
    // on both the input and result side, since the C bridge below marshals
    // both the `input` and `result` structs through this one `fields` list.
    // An equal-length-only check would miss a reordered, renamed, or
    // substituted leaf that still totals the same count; the two walks can
    // only disagree on order or content if one of the two implementations
    // has drifted.
    if paths.is_empty()
        || !leaf_paths_match(&paths, &input.owned_leaves)
        || !leaf_paths_match(&paths, &descriptor.result_facts().owned_leaves)
    {
        return Err(shape_error());
    }
    Ok((function, paths))
}

/// `true` when `paths` (field-declaration-id chains, as returned by
/// [`crate::public_generic_type::owned_bytes_leaf_field_paths`]), each
/// rendered through [`render_leaf_path`], is positionally identical to
/// `expected` (a descriptor's own `owned_leaves`) -- same length, same
/// order, same content. Deliberately not a length-only or set/multiset
/// comparison: a caller relying on this to bind physical field access to a
/// descriptor's trusted leaf inventory needs the stronger, order-sensitive
/// guarantee. See this module's own tests for a permuted-list case a
/// length-only check would have let through.
fn leaf_paths_match(paths: &[Vec<String>], expected: &[String]) -> bool {
    paths.len() == expected.len()
        && paths
            .iter()
            .zip(expected)
            .all(|(chain, path)| render_leaf_path(chain) == *path)
}

#[cfg(test)]
mod tests {
    use super::leaf_paths_match;

    fn chain(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    /// A permuted expected list is refused: this is the exact property
    /// [`super::admit_nested_moves`] relies on to require positional, not
    /// merely count, equality against a descriptor's own leaf-path list.
    #[test]
    fn leaf_paths_match_refuses_a_permuted_expected_list() {
        let paths = vec![chain(&["left"]), chain(&["right"])];
        let expected = vec![
            crate::public_generic_type::render_leaf_path(&chain(&["left"])),
            crate::public_generic_type::render_leaf_path(&chain(&["right"])),
        ];
        assert!(leaf_paths_match(&paths, &expected));

        let mut permuted = expected.clone();
        permuted.swap(0, 1);
        assert!(
            !leaf_paths_match(&paths, &permuted),
            "a reordered expected list must not match, even though its length and \
             set of members are unchanged"
        );

        // A length-only check would also have missed a substituted (but
        // equal-count) path; confirm the same helper catches that too.
        let mut substituted = expected.clone();
        substituted[0] = "@30:forged.path".to_owned();
        assert!(!leaf_paths_match(&paths, &substituted));
    }
}
