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
const JOINED_PATH: &str =
    "docs/evidence/mainbe8-flat-targets-joined-20261017/joined-inputs.json";
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
    joined_checked(&joined_data(), path, current)
        .expect("complete joined-to-incoming source domain")
}
