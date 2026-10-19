//! Conservative capacity accounting for the post-HIR fact set and the
//! owned allocations the specification and facts retain.

use super::*;

pub(super) fn checked_spec_owned_capacity(spec: &Spec) -> Option<usize> {
    std::mem::size_of::<Spec>()
        .checked_add(spec.module.capacity())
        .and_then(|bytes| {
            bytes.checked_add(spec.source_revision.as_ref().map_or(0, String::capacity))
        })
        .and_then(|bytes| bytes.checked_add(spec.target.triple.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.endian.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.thread_policy.capacity()))
        .and_then(|bytes| {
            [&spec.exports, &spec.imports, &spec.capabilities]
                .into_iter()
                .try_fold(bytes, |bytes, values| {
                    bytes
                        .checked_add(
                            values
                                .capacity()
                                .checked_mul(std::mem::size_of::<String>())?,
                        )
                        .and_then(|bytes| {
                            values
                                .iter()
                                .try_fold(bytes, |bytes, value| bytes.checked_add(value.capacity()))
                        })
                })
        })
}

pub(super) fn prepared_spec_transfer_capacity(spec: &Spec) -> Option<usize> {
    spec.source_revision
        .as_ref()
        .map_or(0, String::capacity)
        .checked_add(spec.target.triple.capacity())
        .and_then(|bytes| bytes.checked_add(spec.target.endian.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.thread_policy.capacity()))
}

#[derive(Clone, Copy)]
pub(super) struct PostHirFactsCapacity {
    pub(super) retained_upper: usize,
    pub(super) facts_scratch_upper: usize,
    pub(super) render_scratch_upper: usize,
    pub(super) replay_scratch_upper: usize,
    pub(super) traversal_pending_capacity: usize,
}

impl PostHirFactsCapacity {
    fn scratch_upper(self) -> usize {
        self.facts_scratch_upper
            .max(self.render_scratch_upper)
            .max(self.replay_scratch_upper)
    }

    pub(super) fn complete(self) -> Option<usize> {
        self.retained_upper.checked_add(self.scratch_upper())
    }
}

fn checked_btree_allocation_upper<K, V>(len: usize) -> Option<usize> {
    len.checked_mul(
        std::mem::size_of::<(K, V)>().checked_add(std::mem::size_of::<BTreeMap<K, V>>())?,
    )
}

#[cfg(test)]
pub(super) fn checked_owned_string_vec(values: &[String], capacity: usize) -> Option<usize> {
    values.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<String>())?,
        |bytes, value| bytes.checked_add(value.capacity()),
    )
}

#[cfg(test)]
pub(super) fn checked_owned_string_pairs(values: &Vec<(String, String)>) -> Option<usize> {
    values.iter().try_fold(
        values
            .capacity()
            .checked_mul(std::mem::size_of::<(String, String)>())?,
        |bytes, (left, right)| {
            bytes
                .checked_add(left.capacity())?
                .checked_add(right.capacity())
        },
    )
}

#[cfg(test)]
pub(super) fn checked_u16_vec(values: &Vec<u16>) -> Option<usize> {
    values.capacity().checked_mul(std::mem::size_of::<u16>())
}

#[cfg(test)]
pub(super) fn note_post_hir_facts_live(_baseline: usize, scratch: usize) {
    note_post_hir_facts_scratch(scratch);
    note_post_hir_facts_capacity(_baseline.saturating_add(scratch));
}

#[cfg(test)]
pub(super) fn checked_owned_string_set(values: &BTreeSet<String>) -> Option<usize> {
    values.iter().try_fold(
        checked_btree_allocation_upper::<String, ()>(values.len())?,
        |bytes, value| bytes.checked_add(value.capacity()),
    )
}

#[cfg(test)]
pub(super) fn checked_json_value_owned(value: &Value) -> Option<usize> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Some(0),
        Value::String(value) => Some(value.capacity()),
        Value::Array(values) => values.iter().try_fold(
            values
                .capacity()
                .checked_mul(std::mem::size_of::<Value>())?,
            |bytes, value| bytes.checked_add(checked_json_value_owned(value)?),
        ),
        Value::Object(values) => values.iter().try_fold(
            checked_btree_allocation_upper::<String, Value>(values.len())?,
            |bytes, (key, value)| {
                bytes
                    .checked_add(key.capacity())?
                    .checked_add(checked_json_value_owned(value)?)
            },
        ),
    }
}

#[cfg(test)]
pub(super) fn checked_json_string_payload(value: &Value) -> Option<usize> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Some(0),
        Value::String(value) => Some(value.capacity()),
        Value::Array(values) => values.iter().try_fold(0usize, |bytes, value| {
            bytes.checked_add(checked_json_string_payload(value)?)
        }),
        Value::Object(values) => values.iter().try_fold(0usize, |bytes, (key, value)| {
            bytes
                .checked_add(key.capacity())?
                .checked_add(checked_json_string_payload(value)?)
        }),
    }
}

pub(super) fn post_hir_facts_capacity(
    _source_bytes: usize,
    spec_bytes: usize,
    resolved: &ResolvedProgram,
    closure: &[&ResolvedFunction],
    spec: &Spec,
) -> Result<PostHirFactsCapacity, Diagnostic> {
    let selected = closure.len().max(1);
    let exports = spec.exports.len();
    let imports = spec.imports.len();
    let capabilities = spec.capabilities.len();
    let resolved_import_count = resolved
        .interfaces
        .iter()
        .try_fold(0usize, |count, interface| {
            count.checked_add(interface.imports.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let digest_text_capacity = "sha256:"
        .len()
        .checked_add(64)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let parameter_slots = closure
        .iter()
        .try_fold(0usize, |count, function| {
            count.checked_add(function.params.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // This census executes before the post-HIR reservation. Traverse borrowed
    // imports directly: no Vec or map may be materialized until `complete()`
    // has been admitted.
    let (
        import_parameter_slots,
        import_retained_payload,
        import_effect_entries,
        selected_import_id_bytes,
        selected_import_effect_bytes,
    ) = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| spec.imports.iter().any(|id| id == import.id.as_str()))
        .try_fold((0usize, 0usize, 0usize, 0usize, 0usize), |state, import| {
            let parameter_names = import
                .parameters
                .iter()
                .try_fold(0usize, |total, parameter| {
                    total.checked_add(parameter.name.capacity())
                })?;
            let effects = import
                .effects
                .iter()
                .try_fold(0usize, |total, effect| total.checked_add(effect.capacity()))?;
            let failure = match &import.failure {
                ResolvedImportFailure::Infallible => 0,
                ResolvedImportFailure::Status { domain_id, .. } => domain_id.len(),
            };
            let parameter_backing = import
                .parameters
                .len()
                .checked_mul(std::mem::size_of::<ParameterFact>())?;
            let effect_backing = import
                .effects
                .len()
                .checked_mul(std::mem::size_of::<String>())?
                .checked_mul(2)?;
            let retained = import
                .id
                .as_str()
                .len()
                .checked_add(import.interface.as_str().len())?
                .checked_add(import.import_key.capacity())?
                .checked_add("import_".len().checked_add(64)?)?
                .checked_add("spxnr1_i_".len().checked_add(64)?)?
                .checked_add(parameter_backing)?
                .checked_add(parameter_names)?
                .checked_add(effect_backing)?
                .checked_add(effects.checked_mul(2)?)?
                .checked_add(failure)?
                .checked_add(digest_text_capacity)?;
            Some((
                state.0.checked_add(import.parameters.len())?,
                state.1.checked_add(retained)?,
                state.2.checked_add(import.effects.len())?,
                state.3.checked_add(import.id.as_str().len())?,
                state.4.checked_add(effects)?,
            ))
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let import_effect_conversion_scratch = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| spec.imports.iter().any(|id| id == import.id.as_str()))
        .try_fold(0usize, |maximum, import| {
            let effect_payload = import
                .effects
                .iter()
                .try_fold(0usize, |bytes, effect| bytes.checked_add(effect.capacity()))?;
            let parameter_payload = import
                .parameters
                .iter()
                .try_fold(0usize, |bytes, parameter| {
                    bytes.checked_add(parameter.name.capacity())
                })?;
            let failure_payload = match &import.failure {
                ResolvedImportFailure::Infallible => 0,
                ResolvedImportFailure::Status { domain_id, .. } => domain_id.as_str().len(),
            };
            let scratch = checked_btree_allocation_upper::<String, ()>(import.effects.len())?
                .checked_add(effect_payload)?
                .checked_add(
                    import
                        .effects
                        .len()
                        .checked_mul(std::mem::size_of::<String>())?,
                )?
                .checked_add(effect_payload)?
                .checked_add(
                    import
                        .parameters
                        .len()
                        .checked_mul(std::mem::size_of::<ParameterFact>())?,
                )?
                .checked_add(parameter_payload)?
                .checked_add(failure_payload)?
                .checked_add(digest_text_capacity)?;
            Some(maximum.max(scratch))
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let checked_string_bytes = |values: &[String]| {
        values
            .iter()
            .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    };
    let import_id_bytes = checked_string_bytes(&spec.imports)?;
    let capability_bytes = checked_string_bytes(&spec.capabilities)?;
    let closure_id_bytes = closure
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes.checked_add(function.id.as_str().len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let export_retained_payload = spec
        .exports
        .iter()
        .try_fold(0usize, |bytes, id| {
            let function = closure
                .iter()
                .find(|function| function.id.as_str() == id)
                .copied()?;
            let parameter_payload = function
                .params
                .iter()
                .try_fold(0usize, |payload, parameter| {
                    payload.checked_add(parameter.name.capacity())
                })?;
            let parameter_backing = function
                .params
                .len()
                .checked_mul(std::mem::size_of::<ParameterFact>())?;
            let effect_payload = function
                .effects
                .iter()
                .try_fold(0usize, |payload, effect| {
                    payload.checked_add(effect.capacity())
                })?;
            let effect_backing = function
                .effects
                .len()
                .checked_mul(std::mem::size_of::<String>())?;
            let capability_backing = capabilities.checked_mul(std::mem::size_of::<String>())?;
            let required_import_backing = imports.checked_mul(std::mem::size_of::<String>())?;
            let status_ordinal_backing = imports
                .checked_add(3)?
                .checked_mul(std::mem::size_of::<u16>())?;
            let retained = id
                .len()
                .checked_add("export_".len().checked_add(64)?)?
                .checked_add("spxnr1_e_".len().checked_add(64)?)?
                .checked_add(parameter_backing)?
                .checked_add(parameter_payload)?
                .checked_add(effect_backing)?
                .checked_add(effect_payload)?
                .checked_add(capability_backing)?
                .checked_add(capability_bytes)?
                .checked_add(required_import_backing)?
                .checked_add(import_id_bytes)?
                .checked_add(status_ordinal_backing)?
                .checked_add(digest_text_capacity)?;
            bytes.checked_add(retained)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let target_retained_payload = spec
        .target
        .triple
        .capacity()
        .checked_add(spec.target.endian.capacity())
        .and_then(|bytes| bytes.checked_add(spec.target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.thread_policy.capacity()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let final_digest_payload = digest_text_capacity
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_upper = exports
        .checked_mul(std::mem::size_of::<ExportFact>())
        .and_then(|bytes| {
            bytes.checked_add(imports.checked_mul(std::mem::size_of::<ImportFact>())?)
        })
        .and_then(|bytes| bytes.checked_add(import_retained_payload))
        .and_then(|bytes| bytes.checked_add(export_retained_payload))
        .and_then(|bytes| {
            bytes.checked_add(closure.len().checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(closure_id_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                spec.source_revision
                    .as_ref()
                    .map_or(SHA256_TEXT_BYTES, String::capacity),
            )
        })
        .and_then(|bytes| bytes.checked_add(final_digest_payload))
        .and_then(|bytes| bytes.checked_add(target_retained_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let (maximum_c_nodes, maximum_c_depth, maximum_parameter_owned) = closure
        .iter()
        .try_fold((1usize, 1usize, 0usize), |maximum, function| {
            let function_shape = function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .try_fold((1usize, 1usize), |current, expression| {
                    let (nodes, depth) = c_expression_shape(expression).ok()?;
                    Some((current.0.max(nodes), current.1.max(depth)))
                })?;
            let parameter_owned = function
                .params
                .iter()
                .try_fold(0usize, |bytes, parameter| {
                    bytes.checked_add(parameter.name.len())
                })?
                .checked_add(
                    function
                        .params
                        .len()
                        .checked_mul(std::mem::size_of::<ParameterFact>())?,
                )?;
            Some((
                maximum.0.max(function_shape.0),
                maximum.1.max(function_shape.1),
                maximum.2.max(parameter_owned),
            ))
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let selected_effects_backing =
        checked_btree_allocation_upper::<&str, ()>(import_effect_entries)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let source_function_backing =
        checked_btree_allocation_upper::<&str, &ResolvedFunction>(resolved.functions.len())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolved_import_backing = resolved_import_count
        .checked_mul(std::mem::size_of::<(&str, &ResolvedImport)>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let by_function_backing =
        checked_btree_allocation_upper::<&str, &ResolvedFunction>(closure.len())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let selection_scratch = selected_effects_backing
        .checked_add(source_function_backing)
        .and_then(|bytes| bytes.checked_add(resolved_import_backing))
        .and_then(|bytes| bytes.checked_add(by_function_backing))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

    let traversal_calls = traversal_call_site_census(closure)?;
    let traversal_pending_capacity = traversal_calls
        .function_sites
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let current_id_payload = closure
        .iter()
        .map(|function| function.id.as_str().len())
        .max()
        .unwrap_or(0);
    let pending_backing = traversal_pending_capacity
        .checked_mul(std::mem::size_of::<String>())
        .and_then(|bytes| bytes.checked_add(traversal_calls.function_id_bytes))
        .and_then(|bytes| bytes.checked_add(current_id_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let visited_backing = checked_btree_allocation_upper::<String, ()>(closure.len())
        .and_then(|bytes| bytes.checked_add(closure_id_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let direct_function_backing =
        checked_btree_allocation_upper::<DeclarationId, ()>(traversal_calls.function_sites)
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(traversal_calls.function_id_bytes.checked_mul(2)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let direct_import_backing =
        checked_btree_allocation_upper::<DeclarationId, ()>(traversal_calls.import_sites)
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(traversal_calls.import_id_bytes.checked_mul(2)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let transitive_import_backing = checked_btree_allocation_upper::<String, ()>(imports)
        .and_then(|bytes| bytes.checked_add(selected_import_id_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let traversal_scratch = pending_backing
        .checked_add(visited_backing)
        .and_then(|bytes| bytes.checked_add(direct_function_backing))
        .and_then(|bytes| bytes.checked_add(direct_import_backing))
        .and_then(|bytes| bytes.checked_add(transitive_import_backing))
        .and_then(|bytes| bytes.checked_add(current_id_payload))
        .and_then(|bytes| {
            bytes.checked_add(
                (MAX_SEMANTIC_EXPRESSION_DEPTH + 1)
                    .checked_mul(std::mem::size_of::<(&ResolvedExpr, usize)>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_cross_product_entry = digest_text_capacity
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<(String, String)>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_cross_product = exports
        .checked_mul(imports)
        .and_then(|rows| rows.checked_mul(facts_cross_product_entry))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let export_construction_scratch = export_retained_payload
        .checked_add(facts_cross_product)
        .and_then(|bytes| bytes.checked_add(import_retained_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_general_scratch = selection_scratch
        .checked_add(traversal_scratch)
        .and_then(|bytes| bytes.checked_add(export_construction_scratch))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let import_phase_scratch = selection_scratch
        .checked_add(import_effect_conversion_scratch)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Status-domain canonicalization owns the complete BTreeSet while the
    // exact-capacity Vec is filled. Charge both container allocations and a
    // conservative copy of every bounded key payload; the actual conversion
    // moves each String, so this is an upper rather than an amortized claim.
    let status_payload = imports
        .checked_mul(MAX_IDENTIFIER_BYTES)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let selected_capabilities_owned = import_effect_entries
        .checked_mul(std::mem::size_of::<String>())
        .and_then(|bytes| bytes.checked_add(selected_import_effect_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_set_vec_scratch = checked_btree_allocation_upper::<String, ()>(imports)
        .and_then(|bytes| bytes.checked_add(status_payload))
        .and_then(|bytes| bytes.checked_add(imports.checked_mul(std::mem::size_of::<String>())?))
        .and_then(|bytes| bytes.checked_add(status_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_conversion_scratch = status_set_vec_scratch
        .checked_add(selection_scratch)
        .and_then(|bytes| bytes.checked_add(selected_capabilities_owned))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let fingerprint_action_scratch = FINGERPRINT_ACTION_SLOTS
        .checked_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let fingerprint_scratch = fingerprint_action_scratch
        .checked_add(fingerprint_type_scratch_upper(closure)?)
        .and_then(|bytes| bytes.checked_add(digest_text_capacity))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_scratch_upper = facts_general_scratch
        .max(import_phase_scratch)
        .max(status_conversion_scratch)
        .max(fingerprint_scratch);
    // Artifact outputs have independent retained reservations. These terms
    // authorize only simultaneously live renderer/replay scratch: final sink
    // plus branch/argument fragments for C, and the descriptor JSON DOM plus
    // exact-replay hash/escape temporaries. Fixed output maxima are admission
    // limits, not empirical multipliers.
    // The shared C generator/replay machine has one continuation per semantic
    // ancestor, one result slot per depth, and one flat argument slot per
    // expression node. A fixed line arena and the disjoint live value payload
    // each have the generated-C byte ceiling; neither can grow geometrically.
    let c_machine_scratch = maximum_c_depth
        .checked_add(1)
        .and_then(|slots| {
            slots.checked_mul(C_EXPRESSION_FRAME_BYTES.max(REPLAY_C_EXPRESSION_FRAME_BYTES))
        })
        .and_then(|bytes| {
            bytes.checked_add(
                maximum_c_depth
                    .checked_add(1)?
                    .checked_mul(std::mem::size_of::<String>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(maximum_c_nodes.checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(MAX_GENERATED_C_BYTES))
        .and_then(|bytes| bytes.checked_add(MAX_GENERATED_C_BYTES))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Persistent generator locals that coexist with the expression machine:
    // exact selected parameter facts, two borrowed import indexes, and the
    // bounded capability/parameter/hash strings. Final output is excluded.
    let c_outer_scratch = maximum_parameter_owned
        .checked_add(
            checked_btree_allocation_upper::<&String, ()>(imports)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(checked_btree_allocation_upper::<&str, usize>(imports)?)
        })
        .and_then(|bytes| bytes.checked_add(MAX_IDENTIFIER_BYTES.checked_mul(12)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let render_entries = selected
        .checked_add(exports)
        .and_then(|entries| entries.checked_add(imports))
        .and_then(|entries| entries.checked_add(capabilities))
        .and_then(|entries| entries.checked_add(parameter_slots))
        .and_then(|entries| entries.checked_add(import_parameter_slots))
        .and_then(|entries| entries.checked_add(imports.checked_add(4)?))
        .and_then(|entries| entries.checked_add(exports.checked_mul(imports.checked_add(3)?)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_collection_bytes = render_entries
        .checked_mul(
            std::mem::size_of::<String>()
                .checked_add(std::mem::size_of::<Value>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<Value>>()))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Descriptor row fragments coexist with their joined row strings before
    // the separately reserved final descriptor sink is materialized.
    let descriptor_render_scratch = MAX_DESCRIPTOR_BYTES
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(descriptor_collection_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The final generated-C output has its own retained reservation. One
    // MAX_C term here authorizes only the transient line payload; lines are
    // drained directly into the output so a second joined copy never exists.
    let c_render_scratch = c_machine_scratch
        .checked_add(c_outer_scratch)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Safe Rust keeps the quoted capability row plus at most one parameter or
    // argument row. Private FFI keeps its 32 digest-byte strings and import
    // table rows, then at most one callback/argument pair. Charge these Vec
    // headers fieldwise; MAX_RUST below covers only their joined/string
    // payloads, never either separately retained final sink.
    let safe_rust_vec_headers = capabilities
        .checked_add(MAX_PARAMETERS)
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<String>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let private_ffi_vec_headers = 32usize
        .checked_add(imports)
        .and_then(|entries| {
            entries.checked_add(
                MAX_PARAMETERS
                    .checked_mul(2)?
                    .max(imports.checked_add(MAX_PARAMETERS)?),
            )
        })
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<String>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let rust_render_scratch = MAX_GENERATED_RUST_BYTES
        .checked_add(safe_rust_vec_headers.max(private_ffi_vec_headers))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let render_scratch_upper = descriptor_render_scratch
        .max(c_render_scratch)
        .max(rust_render_scratch);
    // Descriptor replay owns one serde_json DOM plus independent expected
    // status/limit collections. Charge every schema-derived object entry as a
    // separately allocated BTree node, every admitted array Value slot at a
    // geometric two-times capacity upper, and the status Set→Vec overlap.
    // The two descriptor-byte terms cover decoded key/string payload capacity
    // and exact-replay escape/number temporaries; the final artifact is held by
    // its independent retained reservation.
    let descriptor_object_entries = 56usize
        .checked_add(
            exports
                .checked_mul(12)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|entries| entries.checked_add(imports.checked_mul(17)?))
        .and_then(|entries| {
            entries.checked_add(
                parameter_slots
                    .checked_add(import_parameter_slots)?
                    .checked_mul(3)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_array_values = imports
        .checked_add(4)
        .and_then(|values| values.checked_add(exports))
        .and_then(|values| values.checked_add(imports))
        .and_then(|values| values.checked_add(parameter_slots))
        .and_then(|values| values.checked_add(import_parameter_slots))
        .and_then(|values| values.checked_add(exports.checked_mul(3)?))
        .and_then(|values| values.checked_add(exports.checked_mul(capabilities.checked_mul(2)?)?))
        .and_then(|values| values.checked_add(exports.checked_mul(imports.checked_mul(2)?)?))
        .and_then(|values| values.checked_add(imports.checked_mul(capabilities.checked_mul(2)?)?))
        .and_then(|values| values.checked_add(NONCLAIMS.len()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_dom_backing =
        checked_btree_allocation_upper::<String, Value>(descriptor_object_entries)
            .and_then(|bytes| {
                bytes.checked_add(
                    descriptor_array_values
                        .checked_mul(2)?
                        .checked_mul(std::mem::size_of::<Value>())?,
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let expected_status_backing = imports
        .checked_add(4)
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<(u64, &str)>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Locked serde_json 1.0.151 owns one reusable `Deserializer::scratch`
    // Vec<u8> (`src/de.rs`) whose string/number paths clear and reuse the same
    // buffer (`src/read.rs`). Decoded bytes cannot exceed the admitted input;
    // geometric Vec capacity is therefore at most twice that input. Returned
    // DOM string/key payload is a distinct at-most-input term. Exact replay's
    // escape/hash temporary is separate and begins only after parsing ends.
    let descriptor_dom_string_payload = MAX_DESCRIPTOR_BYTES;
    let serde_parser_vec_scratch = MAX_DESCRIPTOR_BYTES
        .checked_mul(2)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exact_descriptor_replay_temp = MAX_DESCRIPTOR_BYTES;
    let descriptor_parse_scratch = descriptor_dom_backing
        .checked_add(descriptor_dom_string_payload)
        .and_then(|bytes| bytes.checked_add(serde_parser_vec_scratch))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_validation_scratch = descriptor_dom_backing
        .checked_add(descriptor_dom_string_payload)
        .and_then(|bytes| bytes.checked_add(status_set_vec_scratch))
        .and_then(|bytes| bytes.checked_add(expected_status_backing))
        .and_then(|bytes| bytes.checked_add(exact_descriptor_replay_temp))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_replay_scratch = descriptor_parse_scratch.max(descriptor_validation_scratch);
    let c_replay_scratch = c_machine_scratch
        .checked_add(c_outer_scratch)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exact_replay_scratch = spec_bytes
        .checked_add(
            MAX_IDENTIFIER_BYTES
                .checked_mul(4)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let replay_scratch_upper = descriptor_replay_scratch
        .max(c_replay_scratch)
        .max(exact_replay_scratch);
    Ok(PostHirFactsCapacity {
        retained_upper,
        facts_scratch_upper,
        render_scratch_upper,
        replay_scratch_upper,
        traversal_pending_capacity,
    })
}

pub(super) fn string_vec_owned_capacity(values: &[String], capacity: usize) -> usize {
    capacity * std::mem::size_of::<String>() + values.iter().map(String::capacity).sum::<usize>()
}

pub(super) fn parameter_facts_owned_capacity(values: &[ParameterFact], capacity: usize) -> usize {
    capacity * std::mem::size_of::<ParameterFact>()
        + values
            .iter()
            .map(|value| value.name.capacity())
            .sum::<usize>()
}

fn string_vec_owned_capacity_checked(values: &[String], capacity: usize) -> Option<usize> {
    values.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<String>())?,
        |bytes, value| bytes.checked_add(value.capacity()),
    )
}

fn parameter_facts_owned_capacity_checked(
    values: &[ParameterFact],
    capacity: usize,
) -> Option<usize> {
    values.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<ParameterFact>())?,
        |bytes, value| bytes.checked_add(value.name.capacity()),
    )
}

fn borrowed_string_set_owned_capacity(values: &BTreeSet<&str>) -> usize {
    values.len() * (std::mem::size_of::<(&str, ())>() + std::mem::size_of::<BTreeMap<&str, ()>>())
}

pub(super) fn owned_string_set_owned_capacity(values: &BTreeSet<String>) -> usize {
    values.len()
        * (std::mem::size_of::<(String, ())>() + std::mem::size_of::<BTreeMap<String, ()>>())
        + values.iter().map(String::capacity).sum::<usize>()
}

#[cfg(test)]
pub(super) fn borrowed_map_owned_capacity<K, V>(len: usize) -> usize {
    btree_allocation_upper::<K, V>(len)
}

#[cfg(test)]
pub(super) fn post_hir_selection_scratch_capacity(
    selected_effects: &BTreeSet<&str>,
    source_functions: &BTreeMap<&str, &ResolvedFunction>,
    resolved_imports: &Vec<(&str, &ResolvedImport)>,
) -> usize {
    borrowed_string_set_owned_capacity(selected_effects)
        .saturating_add(borrowed_map_owned_capacity::<&str, &ResolvedFunction>(
            source_functions.len(),
        ))
        .saturating_add(
            resolved_imports.capacity() * std::mem::size_of::<(&str, &ResolvedImport)>(),
        )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn post_hir_live_facts_capacity(
    export_facts: &Vec<ExportFact>,
    import_facts: &Vec<ImportFact>,
    selected_effects: &BTreeSet<&str>,
    source_functions: &BTreeMap<&str, &ResolvedFunction>,
    resolved_imports: &Vec<(&str, &ResolvedImport)>,
    selected_capabilities: &Vec<String>,
    status_domains: &Vec<String>,
    ordinals: &BTreeMap<&str, u16>,
    by_function: &BTreeMap<&str, &ResolvedFunction>,
) -> usize {
    post_hir_facts_owned_capacity(export_facts, import_facts)
        .saturating_add(post_hir_selection_scratch_capacity(
            selected_effects,
            source_functions,
            resolved_imports,
        ))
        .saturating_add(string_vec_owned_capacity(
            selected_capabilities,
            selected_capabilities.capacity(),
        ))
        .saturating_add(string_vec_owned_capacity(
            status_domains,
            status_domains.capacity(),
        ))
        .saturating_add(borrowed_map_owned_capacity::<&str, u16>(ordinals.len()))
        .saturating_add(borrowed_map_owned_capacity::<&str, &ResolvedFunction>(
            by_function.len(),
        ))
}

pub(super) fn post_hir_facts_owned_capacity(
    exports: &Vec<ExportFact>,
    imports: &Vec<ImportFact>,
) -> usize {
    let export_bytes = exports.iter().map(|fact| {
        fact.id.capacity()
            + fact.rust_method.capacity()
            + fact.c_symbol.capacity()
            + parameter_facts_owned_capacity(&fact.parameters, fact.parameters.capacity())
            + string_vec_owned_capacity(&fact.effects, fact.effects.capacity())
            + string_vec_owned_capacity(&fact.capabilities, fact.capabilities.capacity())
            + string_vec_owned_capacity(&fact.required_imports, fact.required_imports.capacity())
            + fact.status_domain_ordinals.capacity() * std::mem::size_of::<u16>()
            + fact.call_contract_digest.capacity()
    });
    let import_bytes = imports.iter().map(|fact| {
        fact.id.capacity()
            + fact.interface.capacity()
            + fact.import_key.capacity()
            + fact.rust_method.capacity()
            + fact.c_field.capacity()
            + parameter_facts_owned_capacity(&fact.parameters, fact.parameters.capacity())
            + string_vec_owned_capacity(&fact.effects, fact.effects.capacity())
            + string_vec_owned_capacity(&fact.capabilities, fact.capabilities.capacity())
            + fact.failure.as_ref().map_or(0, String::capacity)
            + fact.call_contract_digest.capacity()
    });
    exports.capacity() * std::mem::size_of::<ExportFact>()
        + imports.capacity() * std::mem::size_of::<ImportFact>()
        + export_bytes.sum::<usize>()
        + import_bytes.sum::<usize>()
}

pub(super) fn post_hir_facts_owned_capacity_checked(
    exports: &Vec<ExportFact>,
    imports: &Vec<ImportFact>,
) -> Option<usize> {
    let mut bytes = exports
        .capacity()
        .checked_mul(std::mem::size_of::<ExportFact>())?
        .checked_add(
            imports
                .capacity()
                .checked_mul(std::mem::size_of::<ImportFact>())?,
        )?;
    for fact in exports {
        bytes = bytes
            .checked_add(fact.id.capacity())?
            .checked_add(fact.rust_method.capacity())?
            .checked_add(fact.c_symbol.capacity())?
            .checked_add(parameter_facts_owned_capacity_checked(
                &fact.parameters,
                fact.parameters.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.effects,
                fact.effects.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.capabilities,
                fact.capabilities.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.required_imports,
                fact.required_imports.capacity(),
            )?)?
            .checked_add(
                fact.status_domain_ordinals
                    .capacity()
                    .checked_mul(std::mem::size_of::<u16>())?,
            )?
            .checked_add(fact.call_contract_digest.capacity())?;
    }
    for fact in imports {
        bytes = bytes
            .checked_add(fact.id.capacity())?
            .checked_add(fact.interface.capacity())?
            .checked_add(fact.import_key.capacity())?
            .checked_add(fact.rust_method.capacity())?
            .checked_add(fact.c_field.capacity())?
            .checked_add(parameter_facts_owned_capacity_checked(
                &fact.parameters,
                fact.parameters.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.effects,
                fact.effects.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.capabilities,
                fact.capabilities.capacity(),
            )?)?
            .checked_add(fact.failure.as_ref().map_or(0, String::capacity))?
            .checked_add(fact.call_contract_digest.capacity())?;
    }
    Some(bytes)
}

pub(super) fn string_slice_owned_capacity(values: &[String]) -> usize {
    std::mem::size_of_val(values) + values.iter().map(String::capacity).sum::<usize>()
}

fn spec_owned_capacity(spec: &Spec) -> usize {
    spec.module.capacity()
        + spec.source_revision.as_ref().map_or(0, String::capacity)
        + spec.target.triple.capacity()
        + spec.target.endian.capacity()
        + spec.target.panic_strategy.capacity()
        + spec.target.thread_policy.capacity()
        + string_slice_owned_capacity(&spec.exports)
        + string_slice_owned_capacity(&spec.imports)
        + string_slice_owned_capacity(&spec.capabilities)
}
