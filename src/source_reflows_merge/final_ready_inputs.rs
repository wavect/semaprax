//! New unqualified outer endpoints; the99 and DB2 projected contracts are fixed.
use sha2::{Digest, Sha256};
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
// Only these NEW endpoint spec constants are generated after the final Rust fixes.
const ENDPOINT_LENGTH: usize = 646003;
const ENDPOINT_SHA256: &str = "09b6663692b9ab9118a2622eee9f30a927a7f6e3de50367305314eab36c8a286";
const PREFIX: &str = "docs/evidence/final-ready-outer-joins-20261017";
#[derive(Clone, Copy)]
enum Domain {
    Owner99,
    Canonical344,
    Projection344,
}
impl Domain {
    fn tag(self) -> &'static str {
        match self {
            Self::Owner99 => "owner99",
            Self::Canonical344 => "canonical344",
            Self::Projection344 => "projection344",
        }
    }
    fn source(self) -> &'static str {
        match self {
            Self::Owner99 => "src/workspace_graph/identity_resolution.rs",
            Self::Canonical344 => "src/cleanup_plan/replay/factored/canonical_shared.rs",
            Self::Projection344 => "expected_projection::root_join",
        }
    }
    fn projected(self) -> (&'static str, usize, &'static str) {
        match self {
            Self::Owner99 => (
                "projected/owner99.rs.txt",
                6729,
                "413a71132e9c87cd41f5f7d79b6ca41f679b553d0a086a0251cbd5ce8cf3b6e4",
            ),
            Self::Canonical344 => (
                "projected/canonical-db2.rs.txt",
                62576,
                "6568da26e2376af6e713fc396db3e9390907d8dedc7074c3dc314b76da348d6b",
            ),
            Self::Projection344 => (
                "projected/projection-db2.txt",
                535292,
                "0c1b20aa901a342923a52a51f1e7b5bc71330c785e4baeee92185ebaf6e64aa4",
            ),
        }
    }
}
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
fn fixed(path: &str, length: usize, sha256: &str) -> String {
    reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(PREFIX)
            .join(path),
        length,
        sha256,
        2_097_152,
    )
    .expect("fixed complete outer source input")
}
fn records() -> serde_json::Value {
    let text = fixed("endpoints.json", ENDPOINT_LENGTH, ENDPOINT_SHA256);
    let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    rows
}
fn lines(
    current: &str,
    edits: &serde_json::Value,
    start: &str,
    end: &str,
    present: &str,
    previous: &str,
) -> String {
    assert!(current.len() <= 1_048_576);
    let edits = edits.as_array().unwrap();
    assert!(edits.len() <= 512);
    let mut lines: Vec<String> = current.split_inclusive('\n').map(str::to_owned).collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        let a = edit[start].as_u64().unwrap() as usize;
        let z = edit[end].as_u64().unwrap() as usize;
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit[present].as_str().unwrap());
        lines.splice(
            a..z,
            edit[previous]
                .as_str()
                .unwrap()
                .split_inclusive('\n')
                .map(str::to_owned),
        );
        last = a;
    }
    lines.concat()
}
fn project(rows: &serde_json::Value, domain: Domain, current: &str) -> String {
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows.iter().filter(|r| r["domain"] == domain.tag()).count(),
        1
    );
    let row = rows.iter().find(|r| r["domain"] == domain.tag()).unwrap();
    assert_eq!(row["source"], domain.source());
    assert_eq!(digest(current), row["current_sha256"]);
    assert_eq!(current, row["current"].as_str().unwrap());
    let mut restored = lines(
        current,
        &row["inverse"],
        "start",
        "end",
        "current",
        "previous",
    );
    if matches!(domain, Domain::Canonical344) {
        let intermediate = fixed(
            "projected/canonical-831-ready.rs.txt",
            63214,
            "b71323aed5dda3f03ca6215b17c2e192a5782dabbeccca40ce5bf280f168f482",
        );
        assert_eq!(
            restored, intermediate,
            "complete immutable831 ready checkpoint"
        );
        let text = fixed(
            "projected/831-record.json",
            1213,
            "35751d69d29946366175deb6b55d35dcc3065eb0016fce62dc8b671b55379331",
        );
        let record: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(digest(&restored), record["current_sha256"]);
        restored = lines(
            &restored,
            &record["edits"],
            "current_start",
            "current_end",
            "current",
            "original",
        );
        assert_eq!(digest(&restored), record["sha256"]);
    }
    let (path, length, sha256) = domain.projected();
    let projected = fixed(path, length, sha256);
    assert_eq!(digest(&restored), sha256);
    assert_eq!(
        restored, projected,
        "every complete immutable projected source byte"
    );
    restored
}
pub(crate) fn source_owner(domain: &str, source: &str, current: &str) -> String {
    if domain == "source-followups" && source == Domain::Owner99.source() {
        project(&records(), Domain::Owner99, current)
    } else {
        current.to_owned()
    }
}
pub(crate) fn current344_input(source: &str, current: &str) -> String {
    let domain = match source {
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => Domain::Canonical344,
        "expected_projection::root_join" => Domain::Projection344,
        _ => return current.to_owned(),
    };
    project(&records(), domain, current)
}
pub(crate) fn controls() {
    reader::controls();
    let rows = records();
    for domain in [Domain::Owner99, Domain::Canonical344, Domain::Projection344] {
        let row = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["domain"] == domain.tag())
            .unwrap();
        let current = row["current"].as_str().unwrap();
        let projected = project(&rows, domain, current);
        let through_owner = match domain {
            Domain::Owner99 => source_owner("source-followups", domain.source(), current),
            Domain::Canonical344 | Domain::Projection344 => {
                current344_input(domain.source(), current)
            }
        };
        assert_eq!(through_owner, projected);
        for wrong in [
            String::new(),
            format!("{current} "),
            format!("{current}{current}"),
        ] {
            assert!(std::panic::catch_unwind(|| project(&rows, domain, &wrong)).is_err());
        }
        if projected != current {
            assert!(std::panic::catch_unwind(|| project(&rows, domain, &projected)).is_err());
        }
    }
    assert_eq!(current344_input("unlisted", "unchanged"), "unchanged");
    assert_eq!(
        source_owner("unlisted", "unlisted", "unchanged"),
        "unchanged"
    );
}
