use super::*;

/// Run one hostile mode; the outcome must be a quarantine with `code`, the
/// adapter process must be gone, and later calls must not reach the adapter.
fn quarantined(mode: &str, code: &str, contains: &str) {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, hostile(&fx, mode)).unwrap();
    let c = CancelToken::new();
    let t = Instant::now();
    let out = h.invoke(&decide("inv-1"), InvocationClass::SafeRead, &c);
    let Outcome::Quarantined(d) = &out else {
        panic!("{mode}: expected quarantine, got {out:?}")
    };
    assert_eq!(d.code, code, "{mode}: {d}");
    assert!(d.message.contains(contains), "{mode}: {d}");
    assert!(
        t.elapsed() < Duration::from_secs(8),
        "{mode} took {:?}",
        t.elapsed()
    );
    assert!(matches!(h.state(), AdapterState::Quarantined(_)), "{mode}");
    assert_eq!(h.pid(), None, "{mode}: process must be settled");
    let again = h.invoke(&decide("inv-2"), InvocationClass::SafeRead, &c);
    assert!(
        matches!(again, Outcome::Quarantined(_)),
        "{mode}: {again:?}"
    );
}

#[test]
fn wrong_protocol_version_quarantines() {
    quarantined("wrong_protocol", "SPX-HPC006", "rpc.v2");
}

#[test]
fn flood_of_unknown_ids_quarantines_on_the_first_frame() {
    quarantined("flood", "SPX-HPC010", "unknown or already-answered id");
}

#[test]
fn malformed_frame_quarantines() {
    quarantined("malformed_frame", "SPX-HPC009", "unparseable frame");
}

#[test]
fn oversized_frame_quarantines_without_buffering_it() {
    quarantined("oversized_frame", "SPX-HPC012", "exceeds");
}

#[test]
fn unsolicited_host_request_quarantines() {
    quarantined("unsolicited_request", "SPX-HPC011", "host/readFile");
}

#[test]
fn mcp_style_sampling_request_quarantines() {
    quarantined("sampling_request", "SPX-HPC011", "sampling/createMessage");
}

#[test]
fn spoofed_envelope_fields_quarantine() {
    quarantined("spoof_invocation", "SPX-HPC015", "SPX-HPA031");
    quarantined("spoof_project", "SPX-HPC015", "SPX-HPA032");
    quarantined("fake_revision", "SPX-HPC015", "SPX-HPA032");
}

#[test]
fn payload_level_refusals_discard_the_result_but_keep_the_adapter() {
    // forbidden_model: a decision choice outside the offered options.
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, hostile(&fx, "forbidden_model")).unwrap();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::Decision,
        &CancelToken::new(),
    );
    assert!(
        matches!(&out, Outcome::Refused(d) if d.code == "SPX-HPA043"),
        "{out:?}"
    );
    assert_eq!(h.state(), AdapterState::Active);

    // path_escape / absolute_path: context items must be project-relative.
    for mode in ["path_escape", "absolute_path"] {
        let fx = fixture();
        let m = mgr(|_| {});
        let h = m.prepare(PROJECT, hostile(&fx, mode)).unwrap();
        let out = h.invoke(
            &search("i1", "x"),
            InvocationClass::SafeRead,
            &CancelToken::new(),
        );
        assert!(
            matches!(&out, Outcome::Refused(d) if d.code == "SPX-HPA041"),
            "{mode}: {out:?}"
        );
    }
}

#[test]
fn well_behaved_hostile_baseline_completes() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    let o = h.invoke(
        &decide("i1"),
        InvocationClass::Decision,
        &CancelToken::new(),
    );
    assert!(matches!(o, Outcome::Completed(_)), "{o:?}");
}
