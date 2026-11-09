//! Exact finite inverse for the source fragments owned by OPT#898.
//! Existing older inverse tests still inspect their entire original subject.
pub(in crate::workspace_graph) fn pre898(source: &str) -> String {
    let mut restored = source.to_owned();
    for (before, after) in [
        (
            r####"#[path = "expected_projection/cached_exact.rs"]
pub(super) mod cached_exact;
#[path = "expected_projection/call_identity.rs"]
mod call_identity;
"####,
            r####"#[path = "expected_projection/cached_exact.rs"]
pub(super) mod cached_exact;
#[path = "expected_projection/call_carriers.rs"]
mod call_carriers;
#[cfg(test)]
#[path = "expected_projection/call_carriers_original.rs"]
#[allow(dead_code, reason = "complete independent pre-898 forecaster control")]
mod call_carriers_original;
#[path = "expected_projection/call_identity.rs"]
mod call_identity;
"####,
        ),
        (
            r####"    let mut raw = StructuralCost::raw_ast(layout_mode >= 1).with_inline_values(layout_mode >= 2);
    ast_program_cost(program, &mut raw, include_imports)?;
    let fresh;
    let held;
"####,
            r####"    let mut raw = StructuralCost::raw_ast(layout_mode >= 1).with_inline_values(layout_mode >= 2);
    ast_program_cost(program, &mut raw, include_imports)?;
    let call_proof = if layout_mode >= 5 {
        let proof = super::call_carriers::count(program, authored, include_imports);
        raw.exact_carrier_free_calls = proof.roots;
        proof.bytes
    } else {
        0
    };
    let fresh;
    let held;
"####,
        ),
        (
            r####"    )?;
    let hir_upper = fixed_hir_upper
        .checked_add(prelude_hir_upper)
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
        .and_then(|bytes| bytes.checked_add(identity_occurrence_upper))
"####,
            r####"    )?;
    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
        .and_then(|bytes| bytes.checked_add(identity_occurrence_upper))
"####,
        ),
        (
            r####"                include_str!("../../src/workspace_graph/expected_projection/vec_constructor.rs"),
                include_str!("../../src/workspace_graph/expected_projection/held_projection.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast_tests.rs"),
"####,
            r####"                include_str!("../../src/workspace_graph/expected_projection/vec_constructor.rs"),
                include_str!("../../src/workspace_graph/expected_projection/held_projection.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_tests.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_inverse.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_original.rs"),
                include_str!("../../src/workspace_graph/expected_projection/call_carriers_historical_tests.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast.rs"),
                include_str!("../../src/workspace_graph/expected_projection/prelude_forecast_tests.rs"),
"####,
        ),
    ] {
        if restored.contains(after) {
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, before, 1);
        }
    }
    restored
}
// Complete authenticated acquisition stage, before the unchanged pre-898 inverse.
pub(in crate::workspace_graph) fn pre916_held(source: &str) -> String {
    let paired_stage =
        super::super::shared_local_callees::paired::tests::restore("held_projection.rs", source);
    let source = paired_stage.as_str();
    use sha2::{Digest, Sha256};
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    assert_eq!(
        (source.len(), hash(source)),
        (
            22889,
            "8443e4086088b9a623df9f7b7cd9755a0e69664fc4a60066bf5e8da2e6797710".to_owned()
        )
    );
    let mut restored = source.to_owned();
    for (after, before) in [
        (
            r######"    Ok(synthetic)
}

#[path = "shared_local_callees/coupled_projection.rs"]
mod coupled_projection;
pub(super) use coupled_projection::estimate as coupled_uncached_estimate;
"######,
            r######"    Ok(synthetic)
}
"######,
        ),
        (
            r######"    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(shared_callees.bytes))
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
"######,
            r######"    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
"######,
        ),
        (
            r######"            .expect("literal storage discount retains the complete reduced fixed bundle")
    };
    // Only the final uncached route has no semantic-cache serialization owner.
    // This independently paid source certificate concerns local callee backing,
    // not the auxiliary-carrier absence counter, BASE, or an expression ID.
    let shared_callees = if layout_mode == 6 && retention == cached_exact::Retention::Parsed {
        super::shared_local_callees::costs(super::shared_local_callees::Context {
            program,
            authored,
            programs,
        })?
    } else {
        super::shared_local_callees::Costs { slots: 0, bytes: 0 }
    };
    let identity_slots = identity_slots
        .checked_sub(shared_callees.slots)
        .expect("a certified raw call retains its callee slot until this separate proof");
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
"######,
            r######"            .expect("literal storage discount retains the complete reduced fixed bundle")
    };
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
"######,
        ),
    ] {
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, before, 1);
    }
    assert_eq!(
        (restored.len(), hash(&restored)),
        (
            21916,
            "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942".to_owned()
        )
    );
    restored
}

#[test]
fn complete_forecaster_inverse_reconstructs_entire_pre898_algorithm() {
    let actual_stage = pre916_held(include_str!("held_projection.rs"));
    let actual = actual_stage.as_str();
    let original = include_str!("call_carriers_original.rs");
    let original = original
        .split("// Finite helper wiring")
        .next()
        .unwrap()
        .trim_end();
    assert_eq!(pre898(actual).trim_end(), original);
}

// Complete independent dc45 source inverse; the live NEXT gateway remains strict.
pub(in crate::workspace_graph) fn pre916_held_dc45(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = |raw: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
        )
    };
    assert_eq!(
        (source.len(), hash(source)),
        (
            22889,
            "8443e4086088b9a623df9f7b7cd9755a0e69664fc4a60066bf5e8da2e6797710".to_owned()
        )
    );
    let mut restored = source.to_owned();
    for (after, before) in [
        (
            r######"    Ok(synthetic)
}

#[path = "shared_local_callees/coupled_projection.rs"]
mod coupled_projection;
pub(super) use coupled_projection::estimate as coupled_uncached_estimate;
"######,
            r######"    Ok(synthetic)
}
"######,
        ),
        (
            r######"    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(shared_callees.bytes))
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
"######,
            r######"    let hir_upper = fixed_hir_upper
        .checked_add(call_proof)
        .and_then(|bytes| bytes.checked_add(prelude_hir_upper))
        .and_then(|bytes| bytes.checked_add(identity_carrier_bytes))
"######,
        ),
        (
            r######"            .expect("literal storage discount retains the complete reduced fixed bundle")
    };
    // Only the final uncached route has no semantic-cache serialization owner.
    // This independently paid source certificate concerns local callee backing,
    // not the auxiliary-carrier absence counter, BASE, or an expression ID.
    let shared_callees = if layout_mode == 6 && retention == cached_exact::Retention::Parsed {
        super::shared_local_callees::costs(super::shared_local_callees::Context {
            program,
            authored,
            programs,
        })?
    } else {
        super::shared_local_callees::Costs { slots: 0, bytes: 0 }
    };
    let identity_slots = identity_slots
        .checked_sub(shared_callees.slots)
        .expect("a certified raw call retains its callee slot until this separate proof");
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
"######,
            r######"            .expect("literal storage discount retains the complete reduced fixed bundle")
    };
    let identity_occurrence_upper = identity_slots
        .checked_mul(maximum_identity_bytes)
"######,
        ),
    ] {
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, before, 1);
    }
    assert_eq!(
        (restored.len(), hash(&restored)),
        (
            21916,
            "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942".to_owned()
        )
    );
    restored
}
