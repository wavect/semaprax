// Exact direct-reader bridges discovered by the current344 owning run.
// No compiler, budget, source authority, or executable qualification is granted.
use sha2::{Digest, Sha256};

const DATA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/current344-input-composition-20261016/bridges.json"
));
const DATA_LENGTH: usize = 4885;
const DATA_SHA256: [u8; 32] = [
    242, 39, 7, 178, 28, 159, 62, 174, 181, 225, 121, 126, 138, 32, 36, 40, 227, 209, 55, 21, 151,
    227, 188, 207, 185, 60, 62, 168, 167, 119, 255, 91,
];

fn assert_digest(source: &str, expected: &serde_json::Value) {
    let expected = expected.as_str().expect("fixed whole-source SHA256");
    assert_eq!(expected.len(), 64);
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|index| u8::from_str_radix(&expected[index..index + 2], 16).unwrap())
        .collect();
    assert_eq!(Sha256::digest(source.as_bytes()).as_slice(), bytes);
}

pub(crate) fn project(source: &str, current: &str) -> String {
    assert_eq!(DATA.len(), DATA_LENGTH, "fixed bridge metadata length");
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(DATA.as_bytes())),
        DATA_SHA256
    );
    let rows: serde_json::Value = serde_json::from_str(DATA).unwrap();
    let rows = rows.as_array().unwrap();
    let matching: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    assert!(matching.len() <= 1, "unique direct-reader bridge");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    assert_digest(current, &row["current_sha256"]);
    let mut previous = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert!(!new.is_empty(), "nonempty exact preimage");
        assert_eq!(
            previous.matches(new).count(),
            1,
            "complete unique bridge preimage"
        );
        previous = previous.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_digest(&previous, &row["original_sha256"]);
    previous
}

#[cfg(test)]
pub(crate) fn refuses_drift(source: &str, current: &str) {
    let mut changed = current.to_owned();
    changed.push(' ');
    assert!(std::panic::catch_unwind(|| project(source, &changed)).is_err());
    assert!(std::panic::catch_unwind(|| project(source, "")).is_err());
    let previous = project(source, current);
    assert_ne!(previous, current);
    assert!(std::panic::catch_unwind(|| project(source, &previous)).is_err());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_direct_reader_preimages_refuse_stale_truncated_and_historical_inputs() {
        let sources = [
            (
                "src/cleanup_plan.rs",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan.rs")),
            ),
            (
                "src/hir/ids.rs",
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/ids.rs")),
            ),
            (
                "tests/native/string_settlement/codec_allocation/string_vectors.rs",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/native/string_settlement/codec_allocation/string_vectors.rs"
                )),
            ),
        ];
        for (source, current) in sources {
            refuses_drift(source, current);
        }
        assert_eq!(project("unlisted", "unchanged"), "unchanged");
    }
}
