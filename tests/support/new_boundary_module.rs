//! Separate actual audited-boundary membership from complete source-role custody.
//! Every original class (including old test-only classes) remains a sealed root.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const CURRENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/source-boundary-domains-20261016/current-domains.json"
));
const HIR_BASE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/source-boundary-domains-20261016/hir-complete-a82e-manifest.json"
));
const PROJECTION_BASE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/source-boundary-domains-20261016/projection-complete-a82e-manifest.json"
));
const HIR_SHA: &str = "ed7cdfdf39c3cfe6a90ba84764ca643716b9ac4883757dbbd0efc3d6b9a53abd";
const PROJECTION_SHA: &str = "f943a9f16b7f4bb1f2b4f0574d8e07db57e275661211f523c9f79217922937ee";
#[derive(Clone, Copy)]
enum Domain {
    Hir,
    Projection,
}
impl Domain {
    fn name(self) -> &'static str {
        match self {
            Self::Hir => "hir",
            Self::Projection => "projection",
        }
    }
    fn baseline(self) -> (&'static str, &'static str, usize) {
        match self {
            Self::Hir => (HIR_BASE, HIR_SHA, 1046),
            Self::Projection => (PROJECTION_BASE, PROJECTION_SHA, 179),
        }
    }
    fn roots(self) -> BTreeSet<String> {
        let (source, expected, count) = self.baseline();
        assert_eq!(digest(source), expected, "complete original a82e manifest");
        let original: Value = serde_json::from_str(source).unwrap();
        assert_eq!(original["schema"], "semaprax.source-authority-partition.v1");
        let roots: BTreeSet<_> = original["classes"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            roots.len(),
            count,
            "ALL original classes, including old test-only"
        );
        roots
    }
}
fn digest(source: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn strings(value: &Value) -> BTreeSet<String> {
    let items: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    let result: BTreeSet<_> = items.iter().cloned().collect();
    assert_eq!(items.len(), result.len(), "unique typed boundary members");
    result
}
fn compiled_edge(text: &str) -> bool {
    let text = text.trim();
    // Data/custody transport cannot turn unrelated raw roots into compiler
    // descendants. Every ORIGINAL data class is already a sealed root above.
    if text.starts_with("include_str!") {
        return false;
    }
    if text.starts_with("include!") {
        return true;
    }
    // Module registration text ends at its actual header, after complete
    // attributes. Match only a final mod identifier followed by ; or {.
    // No filesystem-directory heuristic: #[path] may name another directory.
    for (at, _) in text.match_indices("mod") {
        if at != 0
            && text[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let tail = &text[at + 3..];
        if !tail.chars().next().is_some_and(char::is_whitespace) {
            continue;
        }
        let tail = tail.trim();
        if !tail.ends_with(';') && !tail.ends_with('{') {
            continue;
        }
        let name = tail[..tail.len() - 1]
            .trim()
            .strip_prefix("r#")
            .unwrap_or(tail[..tail.len() - 1].trim());
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return true;
        }
    }
    panic!("unrecognized actual registration edge: {text}")
}
fn members(full: &Value, roots: &BTreeSet<String>) -> BTreeSet<String> {
    let classes = full["classes"].as_object().unwrap();
    assert!(
        roots.iter().all(|p| classes.contains_key(p)),
        "an original audited source disappeared"
    );
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in classes.values() {
        for chain in row["chains"].as_array().unwrap() {
            for edge in chain["edges"].as_array().unwrap() {
                if compiled_edge(edge["text"].as_str().unwrap()) {
                    edges
                        .entry(edge["parent"].as_str().unwrap().to_owned())
                        .or_default()
                        .insert(edge["child"].as_str().unwrap().to_owned());
                }
            }
        }
    }
    let mut result = roots.clone();
    let mut pending: Vec<_> = roots.iter().cloned().collect();
    while let Some(parent) = pending.pop() {
        for child in edges.get(&parent).into_iter().flatten() {
            assert!(
                classes.contains_key(child),
                "actual registered descendant omitted: {parent} -> {child}"
            );
            if result.insert(child.clone()) {
                pending.push(child.clone());
            }
        }
    }
    result
}
fn validate_scope(full: &Value, domain: Domain, scope: &Value) -> BTreeSet<String> {
    assert_eq!(
        scope["baseline"],
        "a82e00cc83f99b16249181c45710d962598577db"
    );
    assert_eq!(
        scope["baseline_sha256"].as_str().unwrap(),
        domain.baseline().1
    );
    let roots = domain.roots();
    assert_eq!(strings(&scope["original_roots"]), roots);
    let current = members(full, &roots);
    assert_eq!(
        strings(&scope["current_members"]),
        current,
        "complete actual current registration/include closure"
    );
    let classes = full["classes"].as_object().unwrap();
    let production: BTreeSet<_> = current
        .iter()
        .filter(|p| classes[*p]["role"] == "production")
        .cloned()
        .collect();
    assert_eq!(
        strings(&scope["production_members"]),
        production,
        "membership does not change source role"
    );
    let outside: BTreeSet<_> = classes
        .iter()
        .filter(|(p, v)| v["role"] == "production" && !current.contains(*p))
        .map(|(p, _)| p.clone())
        .collect();
    assert_eq!(
        strings(&scope["fully_authenticated_outside_production"]),
        outside
    );
    current
}
fn source(path: &str, row: &Value) -> String {
    let path = std::path::Path::new(path);
    assert!(path
        .components()
        .all(|x| matches!(x, std::path::Component::Normal(_))));
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    assert_eq!(
        bytes.len(),
        usize::try_from(row["length"].as_u64().unwrap()).unwrap()
    );
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(
        digest(&text),
        row["sha256"].as_str().unwrap(),
        "no source drift after original authentication"
    );
    text
}
pub(super) fn production(raw: &str, manifest: &str, authenticated: &str) -> String {
    let full: Value = serde_json::from_str(manifest).unwrap();
    let domains: Value = serde_json::from_str(CURRENT).unwrap();
    assert_eq!(domains["schema"], "semaprax.source-boundary-domains.v1");
    let scopes = domains["domains"].as_object().unwrap();
    assert_eq!(
        scopes.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["hir".to_owned(), "projection".to_owned()])
    );
    let matching: Vec<_> = [Domain::Hir, Domain::Projection]
        .into_iter()
        .filter(|d| scopes[d.name()]["full_manifest_sha256"].as_str().unwrap() == digest(manifest))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "exact typed full-manifest binding; no global fallback"
    );
    let domain = matching[0];
    assert_eq!(
        digest(raw),
        scopes[domain.name()]["full_raw_sha256"].as_str().unwrap(),
        "complete authenticated raw binding"
    );
    assert_eq!(
        digest(authenticated),
        scopes[domain.name()]["full_authenticated_production_sha256"]
            .as_str()
            .unwrap(),
        "complete authenticated production binding"
    );
    let membership = validate_scope(&full, domain, &scopes[domain.name()]);
    let classes = full["classes"].as_object().unwrap();
    let sources: BTreeMap<_, _> = full["sources"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(p, row)| (p.as_str(), source(p, row)))
        .collect();
    let mut whole_raw = String::new();
    let mut whole_production = String::new();
    let mut boundary = String::new();
    for chunk in full["chunks"].as_array().unwrap() {
        if let Some(path) = chunk["path"].as_str() {
            let text = &sources[path];
            whole_raw.push_str(text);
            if classes[path]["role"] == "production" {
                whole_production.push_str(text);
                if membership.contains(path) {
                    boundary.push_str(text);
                }
            }
        } else {
            let literal = chunk["literal"].as_str().unwrap();
            whole_raw.push_str(literal);
            whole_production.push_str(literal);
            boundary.push_str(literal);
        }
    }
    for value in full["closure_only"].as_array().unwrap() {
        let path = value.as_str().unwrap();
        if classes[path]["role"] == "production" {
            whole_production.push_str(&sources[path]);
            if membership.contains(path) {
                boundary.push_str(&sources[path]);
            }
        }
    }
    assert_eq!(whole_raw, raw, "full original raw transport remains exact");
    assert_eq!(
        whole_production, authenticated,
        "unchanged original helper authenticated every production root"
    );
    boundary
}
#[cfg(test)]
mod tests {
    use super::*;
    const HIR: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/a3-e47-final-custody-20261016/hir.json"
    ));
    const PROJECTION: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/a3-e47-final-custody-20261016/projection.json"
    ));
    fn raw(full: &Value) -> String {
        full["chunks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                c["path"].as_str().map_or_else(
                    || c["literal"].as_str().unwrap().to_owned(),
                    |p| source(p, &full["sources"][p]),
                )
            })
            .collect()
    }
    #[test]
    fn complete_original_domains_and_current_roles_remain_separate_and_source_bound() {
        let domains: Value = serde_json::from_str(CURRENT).unwrap();
        for (domain, text) in [(Domain::Hir, HIR), (Domain::Projection, PROJECTION)] {
            let full: Value = serde_json::from_str(text).unwrap();
            let scope = &domains["domains"][domain.name()];
            let set = validate_scope(&full, domain, scope);
            assert!(domain.roots().is_subset(&set));
            let raw = raw(&full);
            let original = super::super::original::production(&raw, text);
            let current = production(&raw, text, &original);
            assert_eq!(current, super::super::production(&raw, text));
            for outside in strings(&scope["fully_authenticated_outside_production"]) {
                assert_eq!(full["classes"][&outside]["role"], "production");
            }
        }
    }
    #[test]
    fn complete_authentication_refuses_reclassification_source_drift_and_raw_drift() {
        for text in [HIR, PROJECTION] {
            let full: Value = serde_json::from_str(text).unwrap();
            let raw = raw(&full);
            let path = full["classes"]
                .as_object()
                .unwrap()
                .iter()
                .find(|(_, v)| v["role"] == "production")
                .unwrap()
                .0
                .clone();
            let mut changed = full.clone();
            changed["classes"][&path]["role"] = Value::String("test-only".to_owned());
            assert!(std::panic::catch_unwind(|| super::super::production(
                &raw,
                &changed.to_string()
            ))
            .is_err());
            let mut changed = full.clone();
            changed["sources"][&path]["sha256"] = Value::String("0".repeat(64));
            assert!(std::panic::catch_unwind(|| super::super::production(
                &raw,
                &changed.to_string()
            ))
            .is_err());
            assert!(std::panic::catch_unwind(|| super::super::production(
                &(raw.clone() + "\n"),
                text
            ))
            .is_err());
        }
    }
    #[test]
    fn old_test_roots_and_foreign_path_actual_children_cannot_escape_membership() {
        let domains: Value = serde_json::from_str(CURRENT).unwrap();
        for (domain, text) in [(Domain::Hir, HIR), (Domain::Projection, PROJECTION)] {
            let full: Value = serde_json::from_str(text).unwrap();
            let scope = &domains["domains"][domain.name()];
            let roots = domain.roots();
            let parent = roots
                .iter()
                .find(|p| full["classes"][*p]["role"] == "production")
                .unwrap();
            // A real module registration edge may name a different directory.
            // Membership uses its authenticated parent/child link, not a prefix.
            let mut changed = full.clone();
            let child = "src/foreign_boundary_child.rs";
            let mut chain = changed["classes"][parent]["chains"][0].clone();
            chain["edges"].as_array_mut().unwrap().push(serde_json::json!({"parent":parent,"child":child,"text":"#[path = \"../foreign_boundary_child.rs\"] mod relocated;","start":0,"attributes":["#[path = \"../foreign_boundary_child.rs\"]"]}));
            changed["classes"][child] = serde_json::json!({"role":"production","chains":[chain]});
            assert!(members(&changed, &roots).contains(child));
            assert!(std::panic::catch_unwind(|| validate_scope(&changed, domain, scope)).is_err());
            // Even a baseline class formerly test-only remains a root if a later
            // actual non-test registration promotes it; old roles are no filter.
            let original: Value = serde_json::from_str(domain.baseline().0).unwrap();
            let old_test = original["classes"]
                .as_object()
                .unwrap()
                .iter()
                .find(|(_, v)| v["role"] == "test-only")
                .unwrap()
                .0;
            assert!(roots.contains(old_test));
            let mut changed = full.clone();
            changed["classes"][old_test]["role"] = Value::String("production".to_owned());
            assert!(members(&changed, &roots).contains(old_test));
            assert!(std::panic::catch_unwind(|| validate_scope(&changed, domain, scope)).is_err());
        }
    }
    #[test]
    fn dropped_current_child_and_modified_typed_manifest_are_rejected() {
        let domains: Value = serde_json::from_str(CURRENT).unwrap();
        for (domain, text) in [(Domain::Hir, HIR), (Domain::Projection, PROJECTION)] {
            let full: Value = serde_json::from_str(text).unwrap();
            let scope = &domains["domains"][domain.name()];
            let actual = validate_scope(&full, domain, scope);
            let roots = domain.roots();
            let child = actual
                .difference(&roots)
                .next()
                .expect("current domain has actual new registered children");
            let mut broken = scope.clone();
            broken["current_members"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v.as_str().unwrap() != child);
            assert!(std::panic::catch_unwind(|| validate_scope(&full, domain, &broken)).is_err());
            let mut changed = full.clone();
            changed["classes"].as_object_mut().unwrap().remove(child);
            changed["closure_only"]
                .as_array_mut()
                .unwrap()
                .retain(|v| v.as_str().unwrap() != child);
            let raw = raw(&full);
            let original = super::super::original::production(&raw, text);
            assert!(
                std::panic::catch_unwind(|| production(&raw, &changed.to_string(), &original))
                    .is_err()
            );
        }
    }
}
