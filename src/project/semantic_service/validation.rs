//! Revision invalidation and retained receipt validation helpers.

use super::*;

pub(super) fn invalidation(
    before: &ProjectRevision,
    after: &ProjectRevision,
) -> (BTreeSet<String>, BTreeSet<String>, bool, bool) {
    let old = before
        .sources()
        .iter()
        .map(|source| (source.path(), source))
        .collect::<BTreeMap<_, _>>();
    let new = after
        .sources()
        .iter()
        .map(|source| (source.path(), source))
        .collect::<BTreeMap<_, _>>();
    let paths = old
        .keys()
        .chain(new.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let changed = paths
        .iter()
        .filter(|path| match (old.get(**path), new.get(**path)) {
            (Some(left), Some(right)) => left.source() != right.source(),
            _ => true,
        })
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    let manifest_changed =
        before.manifest().to_canonical_toml() != after.manifest().to_canonical_toml();
    let inventory_changed = old.keys().ne(new.keys());
    let mut invalidated = if manifest_changed || inventory_changed {
        paths.iter().map(|path| (*path).to_owned()).collect()
    } else {
        changed.clone()
    };
    let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();
    for revision in [before, after] {
        for edge in revision.semantic.image_edges() {
            if matches!(edge.kind(), "function_import" | "type_import") {
                reverse
                    .entry(edge.target_path().to_owned())
                    .or_default()
                    .insert(edge.caller_path().to_owned());
            }
        }
    }
    let mut pending = invalidated.iter().cloned().collect::<Vec<_>>();
    while let Some(path) = pending.pop() {
        if let Some(consumers) = reverse.get(&path) {
            for consumer in consumers {
                if invalidated.insert(consumer.clone()) {
                    pending.push(consumer.clone());
                }
            }
        }
    }
    (changed, invalidated, manifest_changed, inventory_changed)
}

pub(super) fn same_revision(left: &ProjectRevision, right: &ProjectRevision) -> bool {
    left.project_revision() == right.project_revision()
        && left.workspace_revision() == right.workspace_revision()
        && left.manifest().to_canonical_toml() == right.manifest().to_canonical_toml()
        && left.workspace_manifest() == right.workspace_manifest()
        && left.semantic_graph() == right.semantic_graph()
        && left.sources().len() == right.sources().len()
        && left
            .sources()
            .iter()
            .zip(right.sources())
            .all(|(left, right)| {
                left.path() == right.path()
                    && left.source() == right.source()
                    && left.source_revision() == right.source_revision()
                    && left.source_digest() == right.source_digest()
            })
}

pub(super) fn parse_value(source: &str) -> Result<Value> {
    serde_json::from_str(source)
        .map_err(|_| invalid("semantic workspace service retained work is not valid JSON"))
}

pub(super) fn render(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut json = serde_json::to_string(&value)
        .map_err(|_| invalid("semantic workspace service receipt cannot be rendered"))?;
    json.push('\n');
    if json.len() > MAX_SEMANTIC_WORKSPACE_SERVICE_RECEIPT_BYTES {
        return Err(capacity(
            "semantic workspace service receipt exceeds its byte limit",
        ));
    }
    Ok(json)
}

pub(super) fn hash(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

pub(super) fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "semantic workspace service revision digest is invalid",
        ));
    }
    Ok(())
}

pub(super) fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G528", message)]
}

pub(super) fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G529", message)]
}

pub(super) fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G530", message)]
}
