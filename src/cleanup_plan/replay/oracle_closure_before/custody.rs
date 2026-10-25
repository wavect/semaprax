//! Exact live-source binding followed by a complete immediate-predecessor inverse.
//! Only source tests use this adapter. No replay algorithm reads this packet.
use sha2::{Digest, Sha256};
#[path = "../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
const SOURCE_CAP: usize = 2_097_152;
fn exact(path: &std::path::Path, length: usize, expected: &str) -> String {
    reader::read(path, length, expected, SOURCE_CAP).unwrap()
}
pub(crate) fn exact_compiled(path: &std::path::Path, compiled: &str) -> String {
    exact(path, compiled.len(), &digest(compiled.as_bytes()))
}
fn authenticate_adapter(root: &std::path::Path) {
    for (path, compiled) in [
        (
            "src/cleanup_plan/replay/oracle_closure_before/custody.rs",
            include_str!("custody.rs"),
        ),
        (
            "src/workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs",
            include_str!(
                "../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"
            ),
        ),
    ] {
        assert_eq!(
            exact(
                &root.join(path),
                compiled.len(),
                &digest(compiled.as_bytes())
            ),
            compiled
        );
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn compiled(path: &str) -> &'static str {
    match path {
        "src/cleanup_plan/replay/factored/oracle_order.rs" => {
            include_str!("../factored/oracle_order.rs")
        }
        "src/cleanup_plan/replay/factored/oracle_order/tests.rs" => {
            include_str!("../factored/oracle_order/tests.rs")
        }
        "src/cleanup_plan/replay/factored/oracle_order/before_flat_identity.rs" => {
            include_str!("../factored/oracle_order/before_flat_identity.rs")
        }
        "src/cleanup_plan/replay/factored/oracle_order/flat_identity_tests.rs" => {
            include_str!("../factored/oracle_order/flat_identity_tests.rs")
        }
        "src/cleanup_plan/replay/continuation_storage.rs" => {
            include_str!("../continuation_storage.rs")
        }
        "src/cleanup_plan/replay/continuation_storage/oracle_reached.rs" => {
            include_str!("../continuation_storage/oracle_reached.rs")
        }
        "src/cleanup_plan/replay/continuation_storage/oracle_reached_tests.rs" => {
            include_str!("../continuation_storage/oracle_reached_tests.rs")
        }
        "src/cleanup_plan/replay/continuation_storage/reached.rs" => {
            include_str!("../continuation_storage/reached.rs")
        }
        "src/cleanup_plan/replay/continuation_storage/reached_tests.rs" => {
            include_str!("../continuation_storage/reached_tests.rs")
        }
        "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs" => {
            include_str!("../path_join/frontier_tests/custody.rs")
        }
        "docs/SCOPED-REPLAY-ORACLE-ACCOUNT-V1.md" => {
            include_str!("../../../../docs/SCOPED-REPLAY-ORACLE-ACCOUNT-V1.md")
        }
        "docs/REPLAY-CONTINUATION-STORAGE-V1.md" => {
            include_str!("../../../../docs/REPLAY-CONTINUATION-STORAGE-V1.md")
        }
        _ => panic!("foreign Oracle closure source"),
    }
}
pub(crate) fn restore(path: &str, supplied: &str) -> String {
    let physical = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    authenticate_adapter(physical);
    let packet = include_str!("inverse.json");
    assert_eq!(
        digest(packet.as_bytes()),
        "abdb9cbe9163644ccd78d425597e9a006f941285f0c807365e21282c4f727a58"
    );
    let packet_path = physical.join("src/cleanup_plan/replay/oracle_closure_before/inverse.json");
    assert_eq!(
        exact(
            &packet_path,
            83_853,
            "abdb9cbe9163644ccd78d425597e9a006f941285f0c807365e21282c4f727a58"
        ),
        packet
    );
    let rows: serde_json::Value = serde_json::from_str(packet).unwrap();
    let rows = rows.as_array().unwrap();
    // Authenticate every current module, including the complete executable old
    // algorithm and new helper/test children; shrinking a root audits no less.
    for row in rows {
        let name = row["path"].as_str().unwrap();
        let actual = compiled(name);
        assert_eq!(
            exact(
                &physical.join(name),
                actual.len(),
                row["current_sha256"].as_str().unwrap()
            ),
            actual
        );
        assert_eq!(actual.len() as u64, row["current_length"]);
        assert_eq!(
            digest(actual.as_bytes()),
            row["current_sha256"].as_str().unwrap()
        );
    }
    assert_eq!(supplied, compiled(path));
    let row = rows.iter().find(|row| row["path"] == path).unwrap();
    let mut restored = supplied.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
    }
    assert_eq!(restored.len() as u64, row["before_length"]);
    assert_eq!(
        digest(restored.as_bytes()),
        row["before_sha256"].as_str().unwrap()
    );
    if let Some(name) = row["before_fixture"].as_str() {
        assert_eq!(
            exact(
                &physical
                    .join("src/cleanup_plan/replay/oracle_closure_before")
                    .join(name),
                usize::try_from(row["before_length"].as_u64().unwrap()).unwrap(),
                row["before_sha256"].as_str().unwrap()
            ),
            restored
        );
    } else {
        assert!(restored.is_empty(), "new module has no predecessor");
    }
    // Double-read exact live modules and packet after inversion too.
    for row in rows {
        let name = row["path"].as_str().unwrap();
        let actual = compiled(name);
        assert_eq!(
            exact(
                &physical.join(name),
                actual.len(),
                row["current_sha256"].as_str().unwrap()
            ),
            actual
        );
    }
    assert_eq!(
        exact(
            &packet_path,
            83_853,
            "abdb9cbe9163644ccd78d425597e9a006f941285f0c807365e21282c4f727a58"
        ),
        packet
    );
    authenticate_adapter(physical);
    restored
}
