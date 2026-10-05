//! MC-06: the Graphify adapter's complete result, produced by its real handlers, is admitted by the host's
//! result-size gate (`ResultEnvelope::parse_for`) at the default and a small budget (fixture prefix `hp-mc06`).

use semaprax_harness::contract::*;
use std::process::Command;

const D1: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";

/// Runs the shipped `adapter.py` `orient` handler on a zero-item index with 200 long-path skipped documents and
/// prints one `<budget> <result envelope json>` line per budget, serialized exactly as the SDK sends it.
const DRIVER: &str = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
import adapter, semaprax_harness_adapter as sdk
ix = adapter.Index("/nonexistent", "/nonexistent")
ix.identity = "0.9.25"
docs = [{"path": "docs/%s/%s/file-%03d.md" % (chr(97 + i % 26) * 200, chr(65 + i % 26) * 200, i),
         "reason": "document or media: skipped by --code-only (no model-backed ingestion)"} for i in range(200)]
docs[0]["path"] = "docs/é漢\"q\"/f.md"
st = {"digest": "d" * 64, "graph": adapter.Graph({"nodes": [], "links": []}, "0.9.25"), "indexed": [], "skipped": docs, "errors": []}
ix.ensure = lambda refresh: (st, False)
handler = adapter.make_handlers(ix)[(adapter.KIND, "orient")]
prov = {"provider_id": adapter.PROVIDER_ID, "adapter_version": adapter.ADAPTER_VERSION, "upstream_version": "0.9.25"}
for budget in map(int, sys.argv[2:]):
    req = {"invocation_id": "inv-000001", "project": {"id": "p1", "worktree": "w1", "revision": "r1"},
           "capability": {"kind": "context.repository", "version": 1}, "budget": {"max_result_bytes": budget}, "payload": {}}
    status, payload, diags = handler(req)
    print(budget, json.dumps(sdk.result(req, status, payload, prov, diags), separators=(",", ":"), sort_keys=True))
"#;

fn request(max_result_bytes: usize) -> RequestEnvelope {
    RequestEnvelope {
        invocation_id: "inv-000001".into(),
        project: ProjectBinding {
            id: "p1".into(),
            worktree: "w1".into(),
            revision: "r1".into(),
        },
        lock_digest: D1.into(),
        capability: CapabilityRef {
            kind: CapabilityKind::ContextRepository,
            version: 1,
        },
        operation: "orient".into(),
        deadline_ms: 30_000,
        max_result_bytes,
        remaining_calls: 8,
        lineage: vec![],
        payload: serde_json::json!({}),
    }
}

fn adapter_results(budgets: &[usize]) -> Vec<(usize, Vec<u8>)> {
    let dir = crate::support::repo_root().join("packages/semaprax-harness-adapters/graphify");
    let mut cmd = Command::new("python3");
    cmd.arg("-c").arg(DRIVER).arg(&dir);
    cmd.args(budgets.iter().map(|b| b.to_string()));
    let out = cmd
        .output()
        .expect("python3 not found: the Graphify result-budget cell cannot run here");
    assert!(
        out.status.success(),
        "driver failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| {
            let (b, json) = l.split_once(' ').unwrap();
            (b.parse().unwrap(), json.as_bytes().to_vec())
        })
        .collect()
}

#[test]
fn hp_mc06_graphify_zero_item_skipped_docs_result_is_admitted_within_budget() {
    for (budget, bytes) in adapter_results(&[65_536, 4_096]) {
        assert!(
            bytes.len() <= budget,
            "{budget}: adapter emitted {} bytes",
            bytes.len()
        );
        let env = ResultEnvelope::parse_for(&request(budget), &bytes)
            .unwrap_or_else(|e| panic!("{budget}: host refused the adapter result: {e}"));
        let v = env.to_json();
        assert_eq!(v["status"], "partial", "{budget}");
        let payload = &v["payload"];
        assert_eq!(payload["items"].as_array().unwrap().len(), 0);
        assert_eq!(payload["coverage"]["complete"], false);
        assert_eq!(payload["coverage"]["exhaustive"], false);
        let kept = payload["coverage"]["skipped"].as_array().unwrap().len();
        assert!(kept < 200, "{budget}: omissions must be explicit");
        assert_eq!(payload["metadata"]["omitted"]["skipped"], 200 - kept);
        // The exact-size boundary: the host admits exactly this many bytes and refuses one fewer.
        assert!(ResultEnvelope::parse_for(&request(bytes.len()), &bytes).is_ok());
        assert_eq!(
            ResultEnvelope::parse_for(&request(bytes.len() - 1), &bytes)
                .unwrap_err()
                .code,
            "SPX-HPA002"
        );
    }
}
