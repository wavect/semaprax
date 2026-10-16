//! Historical raw-wire domains, never a prediction for the next compiler bill.
use sha2::{Digest, Sha256};
const ACTUAL: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-main75b-independent-20261016/actual55e-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-main75b-independent-20261016/actual55e-calculator-renamed.json"),
];
const HOSTED: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-graph-pins-75b314f70/current-hosted-calculator-renamed.json"),
];
fn hash(raw: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(raw.as_bytes()))
    )
}
fn restore(actual: &str, hosted: &str) -> String {
    let a: serde_json::Value = serde_json::from_str(actual).unwrap();
    let h: serde_json::Value = serde_json::from_str(hosted).unwrap();
    assert_eq!(super::graph_digest(actual), a["graph_digest"]);
    assert_eq!(super::graph_digest(hosted), h["graph_digest"]);
    assert_eq!(a["budget"]["used_builder_bytes"], 3_439_179);
    assert_eq!(h["budget"]["used_builder_bytes"], 2_954_152);
    let bill = "\"used_builder_bytes\":3439179";
    assert_eq!(actual.matches(bill).count(), 1);
    let restored = actual.replacen(bill, "\"used_builder_bytes\":2954152", 1);
    let field = format!("\"graph_digest\":{}", a["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", h["graph_digest"]),
        1,
    );
    assert_eq!(
        restored, hosted,
        "every hosted75b byte, with no reserialization"
    );
    restored
}
#[test]
fn actual55e_bill_and_digest_restore_every_hosted75b_byte() {
    let facts: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/browser-main75b-independent-20261016/before-domain-facts.json"
    ))
    .unwrap();
    for (index, (actual, hosted)) in ACTUAL.into_iter().zip(HOSTED).enumerate() {
        assert_eq!(hash(actual), facts["facts"][index]["actual55e_raw_sha256"]);
        assert_eq!(hash(hosted), facts["facts"][index]["hosted75b_raw_sha256"]);
        let restored = restore(
            actual.strip_suffix('\n').unwrap(),
            hosted.strip_suffix('\n').unwrap(),
        );
        let expected = [
            "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
            "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
        ];
        assert_eq!(super::main_75b::digest(&restored), expected[index]);
    }
}
#[test]
fn rehashed_non_builder_drift_cannot_cross_the_historical_domains() {
    for (actual, hosted) in ACTUAL.into_iter().zip(HOSTED) {
        let actual = actual.strip_suffix('\n').unwrap();
        let hosted = hosted.strip_suffix('\n').unwrap();
        let parsed: serde_json::Value = serde_json::from_str(actual).unwrap();
        let field = "\"used_call_sites\":10";
        assert_eq!(actual.matches(field).count(), 1);
        let changed = actual.replacen(field, "\"used_call_sites\":11", 1);
        let digest = format!("\"graph_digest\":{}", parsed["graph_digest"]);
        let changed = changed.replacen(
            &digest,
            &format!("\"graph_digest\":\"{}\"", super::graph_digest(&changed)),
            1,
        );
        assert!(std::panic::catch_unwind(|| restore(&changed, hosted)).is_err());
    }
}
#[test]
fn complete_corrected_main75b_and_prior_helpers_remain_source_bound() {
    for (source, original) in [
        ("tests/wasm/scalar_browser_ci_contract/historical_main_75b.rs",
         include_str!("../../../docs/evidence/browser-main75b-independent-20261016/e47-historical_builder.rs.txt")),
        ("tests/wasm/scalar_browser_ci_contract/before_75b.rs",
         include_str!("../../../docs/evidence/browser-main75b-independent-20261016/e47-before_75b.rs.txt")),
    ] {
        let raw = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(source)).unwrap();
        let restored = crate::scalar_browser_ci_contract::browser_custody::before_e47_input(source, &raw);
        assert_eq!(restored, original, "complete incoming Main source predicate");
    }
}
