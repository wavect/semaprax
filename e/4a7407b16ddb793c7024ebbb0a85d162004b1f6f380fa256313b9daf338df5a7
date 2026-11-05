//! Complete incoming flat target receiver; test-only, not shipping storage.

use super::*;
type SharedWeighted = Rc<Weighted>;
use super::before_inline_weighted::{own_weighted, weighted_successor};

pub(super) enum Targets {
    Legacy(BTreeMap<usize, u64>),
    Paid(Weighted),
}

impl Targets {
    pub(super) fn new(paid: bool) -> Self {
        if paid {
            Self::Paid(Vec::new())
        } else {
            Self::Legacy(BTreeMap::new())
        }
    }

    /// False retains the original checked route-multiplicity overflow verdict.
    pub(super) fn add(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        child: usize,
        weight: u64,
    ) -> Result<bool, Diagnostic> {
        match self {
            Self::Legacy(targets) => {
                if !targets.contains_key(&child) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored successor target node",
                    )?;
                }
                let slot = targets.entry(child).or_insert(0);
                let Some(sum) = slot.checked_add(weight) else {
                    return Ok(false);
                };
                *slot = sum;
            }
            Self::Paid(targets) => {
                budget.reserve_bytes(function, 0, "factored target authentication")?;
                // Binary search performs at most floor(log2(len)) + 2 probes.
                let comparisons = (usize::BITS - targets.len().leading_zeros()) as usize + 1;
                budget.charge(function, comparisons, "factored target search")?;
                match targets.binary_search_by_key(&child, |(node, _)| *node) {
                    Ok(index) => {
                        let Some(sum) = targets[index].1.checked_add(weight) else {
                            return Ok(false);
                        };
                        targets[index].1 = sum;
                    }
                    Err(index) => {
                        budget.charge(
                            function,
                            targets.len() - index + 1,
                            "factored target insertion and shifts",
                        )?;
                        budget.reserve_vec(function, targets, 1, "factored target weights")?;
                        targets.insert(index, (child, weight));
                    }
                }
            }
        }
        Ok(true)
    }

    pub(super) fn share(
        self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<SharedWeighted, Diagnostic> {
        match self {
            Self::Legacy(targets) => weighted_successor(function, budget, targets),
            Self::Paid(targets) => own_weighted(function, budget, targets),
        }
    }
}

const JOINED_LENGTH: usize = 116861;
const JOINED_SHA256: &str = "10d30cbdc8f8f8b655ad1c02cf3eb615ef5e5ee3edc761ee25e633d520144abc";
const JOINED_PATH: &str = "docs/evidence/mainbe8-flat-targets-joined-20261017/joined-inputs.json";
fn joined_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn joined_data() -> String {
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut file = std::fs::File::open(root.join(JOINED_PATH)).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert!(JOINED_LENGTH <= 2 * 1024 * 1024);
    assert_eq!(metadata.len(), JOINED_LENGTH as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(JOINED_LENGTH).unwrap();
    bytes.resize(JOINED_LENGTH, 0);
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(file.read(&mut [0]).unwrap(), 0);
    assert_eq!(joined_digest(&bytes), JOINED_SHA256);
    String::from_utf8(bytes).unwrap()
}
fn joined_checked(data: &str, path: &str, current: &str) -> Option<String> {
    if data.len() != JOINED_LENGTH || joined_digest(data.as_bytes()) != JOINED_SHA256 {
        return None;
    }
    let rows: serde_json::Value = serde_json::from_str(data).ok()?;
    let rows = rows.as_array()?;
    if rows.len() != 3 || rows.iter().filter(|row| row["path"] == path).count() != 1 {
        return None;
    }
    let row = rows.iter().find(|row| row["path"] == path)?;
    if current.len() != row["current_length"].as_u64()? as usize
        || joined_digest(current.as_bytes()) != row["current_sha256"].as_str()?
    {
        return None;
    }
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array()?.iter().rev() {
        let start = usize::try_from(edit["start"].as_u64()?).ok()?;
        let end = usize::try_from(edit["end"].as_u64()?).ok()?;
        if restored.get(start..end)? != edit["current"].as_str()? {
            return None;
        }
        restored.replace_range(start..end, edit["incoming"].as_str()?);
    }
    if restored.len() != row["incoming_length"].as_u64()? as usize
        || joined_digest(restored.as_bytes()) != row["incoming_sha256"].as_str()?
    {
        return None;
    }
    Some(restored)
}
pub(super) fn incoming_input(path: &str, current: &str) -> String {
    let rows = callback_rows();
    let joined = relocated_callback_input(&rows, path, current);
    let result = joined_checked(&joined_data(), path, &joined)
        .expect("complete joined-to-incoming source domain");
    relocated_callback_result(&rows, path, current, &result)
}

// A distinct, closed caller-input domain before the unchanged joined callback.
// Reuse the existing regular-handle bounded reader; no old cap/table is changed.
#[path = "../../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod callback_reader;
const CALLBACK_PREFIX: &str = "docs/evidence/c874-factored-callback-input-20261017";
const CALLBACK_DATA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/c874-factored-callback-input-20261017/inputs.json"
));
fn callback_rows() -> serde_json::Value {
    assert_eq!(CALLBACK_DATA.len(), CALLBACK_LENGTH);
    assert_eq!(joined_digest(CALLBACK_DATA.as_bytes()), CALLBACK_SHA);
    let rows: serde_json::Value = serde_json::from_str(CALLBACK_DATA).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    rows
}
fn callback_archive(row: &serde_json::Value) -> String {
    callback_reader::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(CALLBACK_PREFIX)
            .join(row["archive"].as_str().unwrap()),
        usize::try_from(row["length"].as_u64().unwrap()).unwrap(),
        row["sha256"].as_str().unwrap(),
        2_097_152,
    )
    .expect("complete fixed callback input archive")
}
fn relocated_callback_input(rows: &serde_json::Value, path: &str, current: &str) -> String {
    assert_eq!(rows, &callback_rows(), "whole fixed callback domain");
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows.iter().filter(|r| r["path"] == path).count(),
        1,
        "one closed callback subject"
    );
    let row = rows.iter().find(|r| r["path"] == path).unwrap();
    let stage = callback_archive(&row["stage"]);
    let joined = callback_archive(&row["joined"]);
    assert!(
        current == stage || current == joined,
        "complete admitted callback stage"
    );
    joined
}

fn relocated_callback_result(
    rows: &serde_json::Value,
    path: &str,
    current: &str,
    result: &str,
) -> String {
    assert_eq!(rows, &callback_rows(), "whole fixed callback output domain");
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
        .unwrap();
    let original = callback_archive(&row["joined_result"]);
    assert_eq!(result, original, "complete original callback result");
    let stage = callback_archive(&row["stage"]);
    if current == stage {
        // The relocated OURS caller owns a separate immutable afterimage.
        // The original callback has run in full above; preserve its old direct
        // joined-input/output contract and bind only this exact caller stage.
        callback_archive(&row["ours_after"])
    } else {
        assert_eq!(current, callback_archive(&row["joined"]));
        result.to_owned()
    }
}

#[test]
fn complete_relocated_callback_inputs_preserve_original_joined_receiver_and_refuse_drift() {
    callback_reader::controls();
    let rows = callback_rows();
    let old = joined_data();
    let old_rows: serde_json::Value = serde_json::from_str(&old).unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let stage = callback_archive(&row["stage"]);
        let joined = callback_archive(&row["joined"]);
        assert_ne!(stage, joined);
        assert_eq!(relocated_callback_input(&rows, path, &stage), joined);
        assert_eq!(relocated_callback_input(&rows, path, &joined), joined);
        let original = old_rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["path"] == path)
            .unwrap();
        assert_eq!(joined_digest(joined.as_bytes()), original["current_sha256"]);
        let result = joined_checked(&old, path, &joined).unwrap();
        let ours_after = callback_archive(&row["ours_after"]);
        assert_eq!(incoming_input(path, &stage), ours_after);
        assert_eq!(incoming_input(path, &joined), result);
        assert_eq!(
            joined_digest(result.as_bytes()),
            original["incoming_sha256"]
        );
        for bad in [
            String::new(),
            stage[..stage.len() - 1].to_owned(),
            format!("{stage} "),
            format!("{stage}{stage}"),
            result,
        ] {
            assert!(std::panic::catch_unwind(|| incoming_input(path, &bad)).is_err());
        }
        assert!(std::panic::catch_unwind(|| incoming_input("foreign", &stage)).is_err());
        let mut forged = rows.clone();
        forged[0]["stage"]["sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(
            std::panic::catch_unwind(|| relocated_callback_input(&forged, path, &stage)).is_err()
        );
    }
}
const CALLBACK_LENGTH: usize = 2889;
const CALLBACK_SHA: &str = "8cb7bd2f9e1891bf21e9f7405e9dd00ab0a6470557b2c6969430f97912ef88b2";
