//! Same-invocation empty projection backing; all shape work stays independent.
use super::*;
use std::mem::size_of;
// Five Vec roles: constructor return, initializer, owned binding and explicit
// drop actual/formal. The two reserve-byte receiver/result packets are NEW,
// paid before any new owner. Existing collect_shape argument/formal loans,
// reserve_vec growth, identity clones and all final leaf/index fees stay paid.
pub(super) const ENTRY: usize = 5 * size_of::<Vec<DeclarationId>>()
    + 2 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>();

#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn before_selected() -> bool {
    BEFORE.with(|v| v.get())
}
#[cfg(test)]
pub(super) struct Before(bool);
#[cfg(test)]
impl Before {
    pub(super) fn hold() -> Self {
        Self(BEFORE.with(|v| v.replace(true)))
    }
}
#[cfg(test)]
impl Drop for Before {
    fn drop(&mut self) {
        BEFORE.with(|v| v.set(self.0));
    }
}

#[cfg(test)]
#[rustfmt::skip]
macro_rules! before_body {
    ($function:ident, $budget:ident) => {{
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    let mut pending = shape_cursor::Pending::new();
    for slot in &$function.cleanup_plan.slots {
        collect_shape(
            $function,
            &slot.storage,
            &mut Vec::new(),
            &slot.field_liveness_shape,
            &mut leaves,
            &mut places,
            ($budget, &mut pending),
        )?;
    }
    for (expected, flag) in leaves.keys().enumerate() {
        if flag.0 != u32_index($function, expected, "liveness flag")? {
            return Err(replay_error(
                $function,
                "cleanup liveness flags are not contiguous from zero",
            ));
        }
    }
    Leaves::new_with_budget($function, leaves, $budget)
    }};
}
#[cfg(test)]
pub(super) use before_body;

#[cfg(test)]
#[path = "../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
#[cfg(test)]
pub(super) fn prior_source(compiled: &str) -> String {
    let metadata = include_str!("projection_backing_inverse.json");
    let data: serde_json::Value = serde_json::from_str(metadata).unwrap();
    let row = &data["owners"][0];
    let digest = |b: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(sha2::Sha256::digest(b)));
    use sha2::Digest;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["source"].as_str().unwrap());
    let metadata_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/cleanup_plan/replay/projection_backing_inverse.json");
    assert_eq!(
        reader::read(
            &metadata_path,
            metadata.len(),
            &digest(metadata.as_bytes()),
            2 << 20
        )
        .unwrap(),
        metadata
    );
    assert_eq!(
        compiled.len() as u64,
        row["current"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        digest(compiled.as_bytes()),
        row["current"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        reader::read(&path, compiled.len(), &digest(compiled.as_bytes()), 2 << 20).unwrap(),
        compiled
    );
    let mut out = compiled.to_owned();
    for e in row["edits"].as_array().unwrap().iter().rev() {
        let a: usize = e["start"].as_u64().unwrap().try_into().unwrap();
        let b: usize = e["end"].as_u64().unwrap().try_into().unwrap();
        assert_eq!(&out[a..b], e["after"].as_str().unwrap());
        out.replace_range(a..b, e["before"].as_str().unwrap());
    }
    assert_eq!(out.len() as u64, row["before"]["length"].as_u64().unwrap());
    assert_eq!(
        digest(out.as_bytes()),
        row["before"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        reader::read(&path, compiled.len(), &digest(compiled.as_bytes()), 2 << 20).unwrap(),
        compiled
    );
    assert_eq!(
        reader::read(
            &metadata_path,
            metadata.len(),
            &digest(metadata.as_bytes()),
            2 << 20
        )
        .unwrap(),
        metadata
    );
    out
}

#[cfg(test)]
mod tests;
