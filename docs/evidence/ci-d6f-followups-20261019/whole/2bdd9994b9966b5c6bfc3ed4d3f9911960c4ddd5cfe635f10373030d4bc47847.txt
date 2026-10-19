//! New unqualified outer endpoints; the99 and DB2 projected contracts are fixed.
use sha2::{Digest, Sha256};
#[path = "../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
// Only these NEW endpoint spec constants are generated after the final Rust fixes.
const ENDPOINT_LENGTH: usize = 646446;
const ENDPOINT_SHA256: &str = "ba88bc9f7766f2e0ae848ecc3400b3c20eb787d8750a94aa0e1713a14d041ca2";
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
// This named bridge is only for the original historical facade assertion.
// The owning test audits the complete current projection before calling it.
fn projection344_stage(supplied: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = include_str!(
        "../../docs/evidence/current339-source-custody-repair-20261017/projection344-stage.json"
    );
    assert_eq!(data.len(), 9752);
    assert_eq!(
        digest(data),
        "ab922dd2f58ea4dc32b7e5ddfbae934a8353b7fe9eefbb2927414cbbdf7e4b3b"
    );
    let read = |path: &str, length: usize, sha: &str| {
        reader::read(&root.join(path), length, sha, 2_097_152)
            .or_else(|error| {
                crate::source_reflows_merge::main_e177_final_inputs::file(
                    &root.join(path),
                    length,
                    sha,
                )
                .ok_or(error)
            })
            .unwrap()
    };
    assert_eq!(
        read(
            "docs/evidence/current339-source-custody-repair-20261017/projection344-stage.json",
            data.len(),
            &digest(data)
        ),
        data
    );
    let row: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        row["schema"],
        "semaprax.current339.projection344-whole-stage.v1"
    );
    let image = |name: &str| {
        let view = &row[name];
        read(
            view["archive"].as_str().unwrap(),
            view["length"].as_u64().unwrap() as usize,
            view["sha256"].as_str().unwrap(),
        )
    };
    let current = image("current");
    let previous = image("prior");
    assert!(
        supplied == current || supplied == previous,
        "foreign historical projection input"
    );
    let carrier = &row["carrier"];
    let carrier_path = root.join(carrier["source"].as_str().unwrap());
    let carrier_length = carrier["length"].as_u64().unwrap() as usize;
    let carrier_sha = carrier["sha256"].as_str().unwrap();
    let carrier = crate::source_reflows_merge::main_e177_final_inputs::combined_side_file(
        &carrier_path,
        carrier_length,
        carrier_sha,
    )
    .unwrap_or_else(|| {
        read(
            carrier["source"].as_str().unwrap(),
            carrier_length,
            carrier_sha,
        )
    });
    assert_eq!(carrier.len(), carrier_length);
    assert_eq!(digest(&carrier), carrier_sha);
    let mut actual = String::new();
    for part in row["parts"].as_array().unwrap() {
        if let Some(literal) = part["literal"].as_str() {
            actual.push_str(literal);
        } else {
            actual.push_str(&read(
                part["source"].as_str().unwrap(),
                part["length"].as_u64().unwrap() as usize,
                part["sha256"].as_str().unwrap(),
            ));
        }
    }
    assert_eq!(
        actual, current,
        "every current projection source and literal in owning macro order"
    );
    previous
}

pub(crate) fn current344_input(source: &str, current: &str) -> String {
    let domain = match source {
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => Domain::Canonical344,
        "expected_projection::root_join" => Domain::Projection344,
        _ => return current.to_owned(),
    };
    let stage;
    let current = if matches!(domain, Domain::Canonical344) {
        stage = canonical870_stage(current);
        stage.as_str()
    } else if matches!(domain, Domain::Projection344) {
        stage = projection344_stage(current);
        stage.as_str()
    } else {
        current
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
            Domain::Canonical344 => current344_input(
                domain.source(),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/src/cleanup_plan/replay/factored/canonical_shared.rs"
                )),
            ),
            Domain::Projection344 => current344_input(domain.source(), current),
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

// Separate new raw endpoint; the immutable Canonical344 row remains staged51.
// Old stage bytes are never an alternative live870 input.
fn canonical870_stage(current: &str) -> String {
    let outer = super::main67107_inputs::project("canonical870", current);
    let current = outer.as_str();
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/opt870-current-stage-custody-20261017/raw-to-stage.json"
    ));
    assert_eq!(
        digest(data),
        "6523645c1445ceffafe43ff87118f3f2d6f94686a39b444ff6f87cedf7460bc0"
    );
    let row: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(row["schema"], "semaprax.canonical870-stage.v1");
    assert_eq!(row["source"], Domain::Canonical344.source());
    assert_eq!(current.len() as u64, row["raw_length"].as_u64().unwrap());
    assert_eq!(digest(current), row["raw_sha256"], "complete live870 root");
    let inverse = row["inverse"].as_array().unwrap();
    assert!(!inverse.is_empty() && inverse.len() <= 64);
    let stage = lines(
        current,
        &row["inverse"],
        "start",
        "end",
        "current",
        "previous",
    );
    assert_eq!(stage.len() as u64, row["stage_length"].as_u64().unwrap());
    assert_eq!(
        digest(&stage),
        row["stage_sha256"],
        "complete immutable51 stage"
    );
    stage
}

#[test]
fn canonical870_raw_endpoint_preserves_full51_stage_and_rejects_other_domains() {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/factored/canonical_shared.rs"
    ));
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/opt870-current-stage-custody-20261017/canonical51.stage.rs.txt"
    ));
    assert_eq!(canonical870_stage(raw), old);
    let rows = records();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["domain"] == "canonical344")
        .unwrap();
    assert_eq!(
        row["current"], old,
        "binder must retain the immutable stage domain"
    );
    assert_eq!(
        current344_input(Domain::Canonical344.source(), raw),
        project(&rows, Domain::Canonical344, old)
    );
    for wrong in [
        String::new(),
        format!("{raw} "),
        format!("{raw}{raw}"),
        old.to_owned(),
    ] {
        assert!(std::panic::catch_unwind(|| current344_input(
            Domain::Canonical344.source(),
            &wrong
        ))
        .is_err());
    }
}

#[test]
fn observer_projection_stage_keeps_whole_current_and_prior_and_refuses_foreign_joins() {
    let current=include_str!("../../docs/evidence/current339-source-custody-repair-20261017/projection344-current.whole.txt");
    let previous=include_str!("../../docs/evidence/current339-source-custody-repair-20261017/projection344-prior.whole.txt");
    assert_ne!(current, previous);
    assert_eq!(projection344_stage(current), previous);
    assert_eq!(projection344_stage(previous), previous);
    let rows = records();
    assert_eq!(
        current344_input("expected_projection::root_join", current),
        project(&rows, Domain::Projection344, previous)
    );
    for foreign in [
        String::new(),
        format!("{current} "),
        format!("{current}{current}"),
        current[..current.len() - 1].to_owned(),
        "foreign".to_owned(),
    ] {
        assert!(std::panic::catch_unwind(|| projection344_stage(&foreign)).is_err());
    }
}
