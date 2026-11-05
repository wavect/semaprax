//! Manual complete-wire diagnosis of the unchanged owning Operations fixture.
use super::*;

#[test]
#[ignore = "manual complete-wire capture; does not qualify the original literal KATs"]
fn capture_all_original_operations_fixture_receipts() {
    assert_eq!(std::env::var("SEMAPRAX_CAPTURE_PUBLIC_WIRES").unwrap(), "1");
    let fixture = ManagedOperationsFixture::new("complete-wire-capture");
    let before = fixture.inventory();
    let bundle = generate_evidence(&fixture.root, &fixture.proposal_path).unwrap();
    for (label, document) in [
        ("operations-derivation", bundle.derivation()),
        ("operations-change", bundle.derived_change_proposal()),
        (
            "operations-child-evidence",
            bundle.workspace_change_evidence(),
        ),
        ("operations-evidence", bundle.operations_evidence()),
    ] {
        let _: Value = serde_json::from_str(document).unwrap();
        crate::source_reflows::record_wire_fixture(label, document.as_bytes());
    }
    assert_eq!(fixture.inventory(), before);
    let path = fixture.root.join("operations-evidence.json");
    std::fs::write(&path, bundle.operations_evidence()).unwrap();
    let verified = verify(&fixture.root, &fixture.proposal_path, &path).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&verified).unwrap()["result"],
        "exact_replay"
    );
    crate::source_reflows::record_wire_fixture("operations-verification", verified.as_bytes());
    let applied = apply(&fixture.root, &fixture.proposal_path, &path).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&applied).unwrap()["result"],
        "applied"
    );
    crate::source_reflows::record_wire_fixture("operations-application", applied.as_bytes());
    fixture.assert_exclusive_reacquire();
}
