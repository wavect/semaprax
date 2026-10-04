//! Approved-root scanning: metadata only, never bodies.

use super::bundle::{self, Bundle};
use super::{d, SkillCatalogConfig};
use crate::diag::HarnessDiagnostic;
use crate::json;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Repository instruction files: ignored wherever found, never skills or policy.
const INSTRUCTION_FILES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md", ".cursorrules"];

/// A skill root the host or user approved. Passed in by the host; nothing in
/// the project can create one.
#[derive(Clone, Debug)]
pub struct ApprovedRoot {
    /// Absolute, machine-local directory.
    pub path: PathBuf,
    /// Origin label recorded on every skill from this root.
    pub origin: String,
    /// When set, only bundles whose digest equals this are adopted.
    pub approved_digest: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
    pub origin: String,
    pub digest: String,
    /// Body bytes a load would expose.
    pub bytes: usize,
    /// Whitespace-separated word count of the body.
    pub lexical_size: usize,
    pub format: &'static str,
    pub scripts: Vec<String>,
    /// Unsatisfied `requires_host_tool` / skill dependencies.
    pub missing_dependencies: Vec<String>,
    /// Same name with a different digest exists; isolated from selection.
    pub conflict: bool,
    pub dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub name: String,
    pub digests: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    /// Sorted by (name, digest).
    pub entries: Vec<SkillEntry>,
    pub conflicts: Vec<Conflict>,
    pub diagnostics: Vec<HarnessDiagnostic>,
    /// Digest of the project's skill authorization (the approved roots).
    pub authz_digest: String,
}

pub fn authz_digest(roots: &[ApprovedRoot]) -> String {
    let mut rs: Vec<_> = roots
        .iter()
        .map(|r| json!({"path": r.path.to_string_lossy(), "origin": r.origin, "approved_digest": r.approved_digest}))
        .collect();
    rs.sort_by_key(json::canonical);
    json::digest("semaprax.skill-authz.v1", &json!({ "roots": rs }))
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub(crate) fn words(s: &str) -> usize {
    s.split_whitespace().count()
}

impl Catalog {
    /// Scan approved roots. A disabled config yields an empty catalog, no error.
    pub fn scan(roots: &[ApprovedRoot], config: &SkillCatalogConfig) -> Catalog {
        let mut cat = Catalog {
            authz_digest: authz_digest(roots),
            ..Catalog::default()
        };
        if !config.enabled {
            return cat;
        }
        let mut found: Vec<(Bundle, String)> = Vec::new();
        for root in roots {
            cat.scan_root(root, &mut found);
        }
        cat.finish(found, config);
        cat
    }

    fn scan_root(&mut self, root: &ApprovedRoot, found: &mut Vec<(Bundle, String)>) {
        if !root.path.is_absolute() || !root.path.is_dir() {
            self.diagnostics.push(d(
                "SPX-HPM004",
                format!(
                    "approved root {} must be an existing absolute directory",
                    root.path.display()
                ),
            ));
            return;
        }
        for f in INSTRUCTION_FILES {
            if root.path.join(f).exists() {
                self.diagnostics.push(d(
                    "SPX-HPM005",
                    format!("{}/{f} ignored: repository instruction files never register as skills or policy", root.path.display()),
                ));
            }
        }
        let candidates = if bundle::is_bundle_dir(&root.path) {
            vec![root.path.clone()]
        } else {
            subdirs(&root.path)
        };
        if candidates.first() != Some(&root.path) {
            self.check_unsupported(&root.path);
        }
        for dir in candidates {
            match bundle::read_bundle(&dir) {
                Ok(Some(b)) => {
                    if let Some(want) = &root.approved_digest {
                        if *want != b.digest {
                            self.diagnostics.push(d(
                                "SPX-HPM012",
                                format!(
                                    "bundle `{}` digest {} differs from the approved digest",
                                    b.name, b.digest
                                ),
                            ));
                            continue;
                        }
                    }
                    found.push((b, root.origin.clone()));
                }
                Ok(None) => self.check_unsupported(&dir),
                Err(e) => self.diagnostics.push(e),
            }
        }
    }

    fn check_unsupported(&mut self, dir: &Path) {
        if let Some((marker, eco)) = bundle::detect_unsupported(dir) {
            self.diagnostics.push(d(
                "SPX-HPM008",
                format!(
                    "{}/{marker}: {eco} are an unsupported skill ecosystem",
                    dir.display()
                ),
            ));
        }
    }

    fn finish(&mut self, found: Vec<(Bundle, String)>, config: &SkillCatalogConfig) {
        let mut by_name: BTreeMap<String, Vec<(Bundle, String)>> = BTreeMap::new();
        for (b, origin) in found {
            let group = by_name.entry(b.name.clone()).or_default();
            if !group.iter().any(|(g, _)| g.digest == b.digest) {
                group.push((b, origin));
            }
        }
        let names: Vec<String> = by_name.keys().cloned().collect();
        for (name, group) in by_name {
            let conflict = group.len() > 1;
            if conflict {
                let mut digests: Vec<String> =
                    group.iter().map(|(b, _)| b.digest.clone()).collect();
                digests.sort();
                self.diagnostics.push(d(
                    "SPX-HPM010",
                    format!("skill `{name}` has {} different bundles; all isolated, none selected by name", digests.len()),
                ));
                self.conflicts.push(Conflict {
                    name: name.clone(),
                    digests,
                });
            }
            for (b, origin) in group {
                let missing = missing_dependencies(&b, &names, config);
                if !missing.is_empty() {
                    self.diagnostics.push(d(
                        "SPX-HPM011",
                        format!(
                            "skill `{}` has missing dependencies: {}",
                            b.name,
                            missing.join(", ")
                        ),
                    ));
                }
                self.entries.push(SkillEntry {
                    id: format!("{}#{}", b.name, &b.digest[7..19]),
                    bytes: b.body.len(),
                    lexical_size: words(&b.body),
                    name: b.name,
                    description: b.description,
                    version: b.version,
                    license: b.license,
                    tags: b.tags,
                    dependencies: b.dependencies,
                    origin,
                    digest: b.digest,
                    format: b.format.as_str(),
                    scripts: b.scripts,
                    missing_dependencies: missing,
                    conflict,
                    dir: b.dir,
                });
            }
        }
        self.entries
            .sort_by(|a, b| (&a.name, &a.digest).cmp(&(&b.name, &b.digest)));
    }
}

fn missing_dependencies(b: &Bundle, names: &[String], config: &SkillCatalogConfig) -> Vec<String> {
    let have = |t: &str| config.host_tools.contains(t);
    let mut out = Vec::new();
    for s in &b.scripts {
        let file = s.rsplit('/').next().unwrap_or(s);
        if !have(s) && !have(file) {
            out.push(format!(
                "requires_host_tool script {s}: missing script dependency"
            ));
        }
    }
    for dep in &b.dependencies {
        match dep.strip_prefix("tool:") {
            Some(t) if !have(t) => {
                out.push(format!("requires_host_tool {t}: missing script dependency"))
            }
            Some(_) => {}
            None if !names.contains(dep) => {
                out.push(format!("skill {dep}: not in approved catalog"))
            }
            None => {}
        }
    }
    out
}
