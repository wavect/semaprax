use super::*;
use crate::interpreter::retained_call::owned_handoff::staged_count;

#[test]
fn cached_bound_handoff_matches_explicitly_authenticated_delivery() {
    let boundary = Boundary::derive().unwrap();
    let mut exact_limit = [b'x'; MAX_BYTES];
    exact_limit[0] = 0;
    exact_limit[1] = 0xc3;
    exact_limit[2] = 0xa9;
    for input in [b"".as_slice(), b"matcher", exact_limit.as_slice()] {
        let cached = boundary.deliver_bound(input, MAX_FUEL).unwrap();
        let authenticated = boundary
            .deliver(
                &boundary.authenticated_bytes,
                boundary.digest,
                input,
                MAX_FUEL,
            )
            .unwrap();
        assert_eq!(cached, input);
        assert_eq!(cached, authenticated);
    }

    let oversized = [0u8; MAX_BYTES + 1];
    let before_refusals = staged_count();
    assert_eq!(boundary.deliver_bound(&oversized, MAX_FUEL), Err(()));
    assert_eq!(
        boundary.deliver(
            &boundary.authenticated_bytes,
            boundary.digest,
            &oversized,
            MAX_FUEL,
        ),
        Err(())
    );
    assert_eq!(boundary.deliver_bound(b"fuel", 0), Err(()));
    assert_eq!(
        boundary.deliver(
            &boundary.authenticated_bytes,
            boundary.digest,
            b"fuel",
            0,
        ),
        Err(())
    );
    assert_eq!(staged_count(), before_refusals);

    let recovered = boundary.deliver_bound(b"reentry", MAX_FUEL).unwrap();
    let authenticated = boundary
        .deliver(
            &boundary.authenticated_bytes,
            boundary.digest,
            b"reentry",
            MAX_FUEL,
        )
        .unwrap();
    assert_eq!(recovered, b"reentry");
    assert_eq!(recovered, authenticated);
}

#[test]
fn owned_handoff_binding_is_reproducible_and_real_bytes_settle_before_output() {
    let first = Boundary::derive().unwrap();
    let second = Boundary::derive().unwrap();
    assert_eq!(first.authenticated_bytes, second.authenticated_bytes);
    assert_eq!(first.digest, second.digest);
    let canonical = super::super::rung_two_authority::in_candidate_scope(|| {
        crate::format::canonical(&crate::parse(SOURCE, "owned-handoff.spx").unwrap())
    });
    assert_eq!(canonical, SOURCE);
    let parsed = crate::parse(SOURCE, "owned-handoff.spx").unwrap();
    let graph = crate::graph::to_json(&parsed).unwrap();
    crate::graph::verify_json(&parsed, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    let owner = document["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == ENTRY)
        .unwrap();
    assert_eq!(owner["params"][0]["ownership_mode"], "own");
    assert_eq!(owner["result"]["ownership_mode"], "own");
    assert_eq!(owner["cleanup"]["schema"], "semaprax.cleanup-plan.v2");
    assert_eq!(
        owner["cleanup"]["entry_state"]["live_owned_parameters"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut forged = document.clone();
    let owner = forged["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == ENTRY)
        .unwrap();
    owner["params"][0]["ownership_mode"] = "borrow".into();
    assert!(crate::graph::verify_json(&parsed, &serde_json::to_string(&forged).unwrap()).is_err());
    for input in [b"".as_slice(), b"x", b"abcdefghijklmnopqrst"] {
        let before = staged_count();
        assert_eq!(
            first
                .deliver(&first.authenticated_bytes, first.digest, input, MAX_FUEL)
                .unwrap(),
            input
        );
        assert_eq!(staged_count(), before + 1);
    }
}

#[test]
fn reminted_owned_handoff_substitutions_refuse_before_owner_allocation() {
    let boundary = Boundary::derive().unwrap();
    let mut mutations = Vec::new();
    let mut changed = boundary.binding.clone();
    changed.source[0] ^= 1;
    mutations.push(("source", changed));
    let mut changed = boundary.binding.clone();
    changed.entry.push('x');
    mutations.push(("entry", changed));
    let mut changed = boundary.binding.clone();
    changed.maximum = 21;
    mutations.push(("maximum", changed));
    let mut changed = boundary.binding.clone();
    changed.core_sources.swap(0, 1);
    mutations.push(("source order", changed));
    let mut changed = boundary.binding.clone();
    changed.core_entries[3] = "format.render-byte".to_owned();
    mutations.push(("core entry", changed));
    let mut changed = boundary.binding.clone();
    changed.core_terms[0][0] ^= 1;
    mutations.push(("core term", changed));
    let mut changed = boundary.binding.clone();
    changed.c_source[0] ^= 1;
    mutations.push(("native target", changed));
    let mut changed = boundary.binding.clone();
    changed.wasm[0] ^= 1;
    mutations.push(("Wasm target", changed));
    let mut changed = boundary.binding.clone();
    changed.descriptor[0] ^= 1;
    mutations.push(("descriptor", changed));
    for (name, binding) in mutations {
        let bytes = binding.bytes().unwrap();
        let before = staged_count();
        assert!(
            boundary
                .deliver(&bytes, checksum(&bytes), b"abc", MAX_FUEL)
                .is_err(),
            "{name}"
        );
        assert_eq!(
            staged_count(),
            before,
            "{name}: no physical interpreter owner or dispatch"
        );
        eprintln!("owned handoff reminted {name}: staged=0 dispatch=0");
    }
    for bytes in [
        &boundary.authenticated_bytes[..boundary.authenticated_bytes.len() - 1],
        b"legacy flattened bytes",
    ] {
        let before = staged_count();
        assert!(boundary
            .deliver(bytes, checksum(bytes), b"abc", MAX_FUEL)
            .is_err());
        assert_eq!(staged_count(), before);
    }
    assert_eq!(
        boundary
            .deliver(
                &boundary.authenticated_bytes,
                boundary.digest,
                b"reentry",
                MAX_FUEL
            )
            .unwrap(),
        b"reentry"
    );
}

#[test]
fn owned_handoff_exhaustion_has_no_candidate_and_next_invocation_recovers() {
    let boundary = Boundary::derive().unwrap();
    let before = handoffs();
    assert!(boundary
        .deliver(&boundary.authenticated_bytes, boundary.digest, b"abc", 1)
        .is_err());
    assert_eq!(handoffs(), before);
    assert_eq!(
        boundary
            .deliver(
                &boundary.authenticated_bytes,
                boundary.digest,
                b"abc",
                MAX_FUEL
            )
            .unwrap(),
        b"abc"
    );
    assert_eq!(handoffs(), before + 1);
}
