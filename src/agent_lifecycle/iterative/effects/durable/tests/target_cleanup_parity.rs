//! Durable target cleanup-event parity for the explicit metered profile.
//!
//! The fixture discards one owned Bytes value in observe. That gives the
//! target backends a source-predicted physical cleanup event, while the
//! interpreter continues to report null. Checkpoint and evidence parsing are
//! deliberately separate read-only oracles: neither parsed row grants an
//! execution capability.

use super::*;

type FinalizerVectors = Vec<Option<Vec<(String, u32)>>>;

const OBSERVE_FINALIZER: &str = "fixture.agent.fn.observe";

fn source() -> String {
    super::super::super::tests::typed_effect_source()
        // Make the ordinary and migration-seeded paths each perform one
        // observe/authorize/effect/reduce pass. The seed starts at epoch 2;
        // the ordinary path initializes epoch 1.
        .replace("if state.epoch < 3", "if state.epoch < 1")
        // scratch is deliberately not moved into Observation, so a physical
        // target must settle it when observe returns.
        .replace(
            "    let tag = [79u8, 66u8];",
            "    let tag = [79u8, 66u8];\n    let scratch = bytes_copy(array_as_slice(tag));",
        )
}

fn json_vectors(rows: &[serde_json::Value]) -> FinalizerVectors {
    rows.iter()
        .map(|row| {
            row["finalizer_events"].as_array().map(|events| {
                events
                    .iter()
                    .map(|event| {
                        let pair = event.as_array().expect("canonical finalizer pair");
                        assert_eq!(pair.len(), 2, "canonical finalizer pair width");
                        (
                            pair[0]
                                .as_str()
                                .expect("canonical finalizer function")
                                .to_owned(),
                            u32::try_from(pair[1].as_u64().expect("canonical finalizer liveness"))
                                .expect("canonical u32 finalizer liveness"),
                        )
                    })
                    .collect()
            })
        })
        .collect()
}

/// Parse both durable serializations independently from the in-memory
/// observations, retaining their authored array order. Recovery retains the
/// verified prefix and appends newly charged deterministic replay rows, while
/// its outward evidence covers only the current observations.
fn canonical_vectors(
    run: &super::super::super::MeteredDurableTypedRun,
    retained_prefix: &[Option<Vec<(String, u32)>>],
) -> FinalizerVectors {
    let observed = run
        .observations()
        .iter()
        .map(|observation| {
            observation.work().finalizer_events.as_ref().map(|events| {
                events
                    .iter()
                    .map(|event| (event.function.as_str().to_owned(), event.liveness_flag))
                    .collect()
            })
        })
        .collect::<FinalizerVectors>();

    let evidence: serde_json::Value =
        serde_json::from_str(run.evidence()).expect("canonical durable semantic-work evidence");
    let evidence_rows = evidence["stages"]
        .as_array()
        .expect("durable semantic-work evidence stages");
    let evidence = json_vectors(evidence_rows);

    let checkpoint: serde_json::Value =
        serde_json::from_str(run.run().checkpoint()).expect("canonical metered checkpoint");
    let checkpoint_rows = checkpoint["entries"]
        .as_array()
        .expect("metered checkpoint entries")
        .iter()
        .filter_map(|entry| {
            (entry["event"]["kind"].as_str() == Some("semantic_work"))
                .then(|| entry["event"].clone())
        })
        .collect::<Vec<_>>();
    let checkpoint = json_vectors(&checkpoint_rows);

    assert_eq!(evidence, observed, "evidence keeps target event order");
    assert_eq!(
        checkpoint.len(),
        retained_prefix.len() + observed.len(),
        "checkpoint retains prior rows and appends exactly one current receipt per observation"
    );
    assert_eq!(
        &checkpoint[..retained_prefix.len()],
        retained_prefix,
        "checkpoint changed the retained target-event prefix"
    );
    assert_eq!(
        &checkpoint[retained_prefix.len()..],
        observed.as_slice(),
        "checkpoint appended a target event out of canonical order"
    );
    observed
}

fn performed(vectors: &FinalizerVectors) -> Vec<(String, u32)> {
    vectors.iter().flatten().flatten().cloned().collect()
}

fn assert_target_oracle(label: &str, vectors: &FinalizerVectors, target: bool) {
    if !target {
        assert!(
            vectors.iter().all(Option::is_none),
            "{label}: interpreter must not invent physical cleanup"
        );
        return;
    }
    assert!(
        vectors.iter().all(Option::is_some),
        "{label}: target must report every stage cleanup vector"
    );
    let events = performed(vectors);
    assert!(
        !events.is_empty(),
        "{label}: injected cleanup is non-vacuous"
    );
    assert!(
        events
            .iter()
            .any(|(function, _)| function == OBSERVE_FINALIZER),
        "{label}: the source-level observe cleanup oracle must be present"
    );
}

#[test]
fn selected_targets_preserve_cleanup_vectors_across_durable_fresh_recovery_and_seeded_migration() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let native_fixture = crate::agent_lifecycle::tests::native_stage_host()
        .expect("durable cleanup parity needs a held native compiler");
    let native = super::super::super::NativeTargetHost::open(native_fixture.compiler_path())
        .expect("durable cleanup parity reopens the held native compiler");
    let wasm = held_wasm_target();
    let source = source();
    let compiled = super::super::super::tests::compile_from_source(&source);
    let mut compiled_reference: Option<Vec<(String, u32)>> = None;

    for (label, selected, target) in [
        (
            "interpreter",
            super::super::super::TargetStageBackend::Interpreter,
            false,
        ),
        (
            "native",
            super::super::super::TargetStageBackend::Native(&native),
            true,
        ),
        (
            "Core Wasm",
            super::super::super::TargetStageBackend::CoreWasmHeld(&wasm),
            true,
        ),
    ] {
        let mut handler = Handler::default();
        let mut store = Store::default();
        let fresh = run_metered_selected(&compiled, &mut handler, &mut store, None, selected)
            .unwrap_or_else(|error| panic!("{label}: fresh durable run: {error:?}"));
        let fresh_vectors = canonical_vectors(&fresh, &[]);
        assert_target_oracle(label, &fresh_vectors, target);
        let retained = fresh.run().checkpoint().to_owned();
        let delivered = handler.calls;
        let recovered = run_metered_selected(
            &compiled,
            &mut handler,
            &mut store,
            Some(&retained),
            selected,
        )
        .unwrap_or_else(|error| panic!("{label}: durable recovery: {error:?}"));
        assert_eq!(
            canonical_vectors(&recovered, &fresh_vectors),
            fresh_vectors,
            "{label}: recovery changed the canonical cleanup vectors"
        );
        assert_eq!(
            handler.calls, delivered,
            "{label}: retained recovery redelivered host work"
        );

        let mut seeded_handler = Handler::default();
        let mut seeded_store = Store::default();
        let seeded = run_migration_seed_metered_selected(
            &compiled,
            &mut seeded_handler,
            &mut seeded_store,
            None,
            selected,
        )
        .unwrap_or_else(|error| panic!("{label}: migration-seeded durable run: {error:?}"));
        let seeded_vectors = canonical_vectors(&seeded, &[]);
        assert_target_oracle(label, &seeded_vectors, target);
        let seeded_retained = seeded.run().checkpoint().to_owned();
        let seeded_delivered = seeded_handler.calls;
        let seeded_recovered = run_migration_seed_metered_selected(
            &compiled,
            &mut seeded_handler,
            &mut seeded_store,
            Some(&seeded_retained),
            selected,
        )
        .unwrap_or_else(|error| panic!("{label}: migration-seeded recovery: {error:?}"));
        assert_eq!(
            canonical_vectors(&seeded_recovered, &seeded_vectors),
            seeded_vectors,
            "{label}: seeded recovery changed the canonical cleanup vectors"
        );
        assert_eq!(
            seeded_handler.calls, seeded_delivered,
            "{label}: seeded recovery redelivered host work"
        );

        let events = performed(&fresh_vectors);
        assert_eq!(
            performed(&seeded_vectors),
            events,
            "{label}: migration-seeded cleanup differs from fresh durable cleanup"
        );
        if target {
            if let Some(reference) = &compiled_reference {
                assert_eq!(
                    &events, reference,
                    "{label}: target cleanup vector differs from the independent native target"
                );
            } else {
                compiled_reference = Some(events);
            }
        }
    }
}

#[test]
fn selected_target_cleanup_vector_mutation_refuses_before_recovery_work() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let native_fixture = crate::agent_lifecycle::tests::native_stage_host()
        .expect("durable cleanup hostility needs a held native compiler");
    let native = super::super::super::NativeTargetHost::open(native_fixture.compiler_path())
        .expect("durable cleanup hostility reopens the held native compiler");
    let source = source();
    let compiled = super::super::super::tests::compile_from_source(&source);
    let mut handler = Handler::default();
    let mut store = Store::default();
    let fresh = run_metered_selected(
        &compiled,
        &mut handler,
        &mut store,
        None,
        super::super::super::TargetStageBackend::Native(&native),
    )
    .expect("native target records the cleanup vector");
    assert_target_oracle("native", &canonical_vectors(&fresh, &[]), true);
    let retained = fresh.run().checkpoint();
    let forged = retained.replacen(
        r#""finalizer_events":[["fixture.agent.fn.observe""#,
        r#""finalizer_events":[["forged.agent.fn.observe""#,
        1,
    );
    assert_ne!(
        forged, retained,
        "fixture must retain an observe cleanup row"
    );
    let before = (handler.calls, store.commits);
    assert!(
        run_metered_selected(
            &compiled,
            &mut handler,
            &mut store,
            Some(&forged),
            super::super::super::TargetStageBackend::Native(&native),
        )
        .is_err(),
        "forged cleanup vector entered durable recovery"
    );
    assert_eq!(
        (handler.calls, store.commits),
        before,
        "forged cleanup evidence dispatched or persisted work"
    );
}
