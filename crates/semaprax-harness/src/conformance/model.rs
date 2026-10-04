//! `model.generate/v1`: reuses the root crate's existing ProviderAdapter
//! conformance; the harness does not re-implement it.

use super::report::{Case, Suite, Verdict};
use serde_json::json;

pub const DELEGATED: &str = "provider-adapter-conformance";

pub fn run() -> Suite {
    let mut s = Suite::new(
        "model.generate",
        "adapter",
        vec![Case {
            name: "delegated-provider-adapter-conformance".into(),
            verdict: Verdict::Unverified,
            evidence: json!({
                "reason": "model adapters reuse the existing ProviderAdapter conformance through the toolchain bridge; this runner does not execute it",
                "delegated": DELEGATED,
                "entry": "run_conformance_suite in src/provider_adapter_sdk/conformance.rs",
            }),
        }],
    );
    s.delegated = Some(DELEGATED);
    s
}
