//! Curated first-party catalog of third-party skills (HN-04). The catalog data
//! (`packages/semaprax-harness-adapters/skills/catalog.json`) and the byte-exact
//! upstream snapshots are embedded in the binary, so discovery and loading
//! work offline from an installed binary and never read a repository checkout.
//! Snapshots are materialized into the content-addressed store under the
//! harness home on first use.

use super::d;
use super::inventory::{Bounds, ScanRules};
use super::snapshot::{self, Snapshot};
use crate::diag::HarnessResult;
use crate::json;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

const CATALOG_JSON: &str =
    include_str!("../../../../packages/semaprax-harness-adapters/skills/catalog.json");

const fn bytes(b: &'static [u8]) -> &'static [u8] {
    b
}

macro_rules! asset {
    ($id:literal, $ver:literal, $file:literal) => {
        (
            $id,
            $file,
            bytes(include_bytes!(concat!(
                "../../../../packages/semaprax-harness-adapters/skills/official/",
                $id,
                "/",
                $ver,
                "/",
                $file
            ))),
        )
    };
}

/// (skill id, file path inside the bundle, exact upstream bytes).
const ASSETS: &[(&str, &str, &[u8])] = &[
    asset!("ponytail", "v4.10.3", "SKILL.md"),
    asset!("ponytail", "v4.10.3", "LICENSE"),
    asset!("caveman", "v3.1.0", "SKILL.md"),
    asset!("caveman", "v3.1.0", "README.md"),
    asset!("caveman", "v3.1.0", "LICENSE"),
];

static SERIAL: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug)]
pub struct OfficialFile {
    pub path: String,
    pub upstream_path: String,
    pub git_blob_sha: String,
    /// Plain `sha256:<hex>` of the exact upstream bytes.
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub name: String,
    pub status: String,
    pub note: String,
}

/// Curated applicability metadata; lives outside the immutable upstream files.
#[derive(Clone, Debug, Default)]
pub struct Applicability {
    pub task_families: Vec<String>,
    pub excluded_families: Vec<String>,
    pub automatic: bool,
    pub shipped_mode: String,
    pub modes: Vec<String>,
    pub explicit_triggers: Vec<String>,
    pub stop_triggers: Vec<String>,
    pub mode_triggers: BTreeMap<String, Vec<String>>,
    pub unsupported_triggers: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct OfficialSkill {
    pub id: String,
    pub aliases: Vec<String>,
    /// `upstream-authored` or `semaprax-authored`.
    pub authorship: String,
    pub repo: String,
    pub subpath: Option<String>,
    pub channel: String,
    pub tag: Option<String>,
    pub commit: Option<String>,
    pub re_resolved: Option<String>,
    pub version: String,
    pub license: String,
    pub license_file: Option<String>,
    /// Artifact-v2 digest of the bundle (all files); the snapshot address.
    pub bundle_digest: Option<String>,
    pub files: Vec<OfficialFile>,
    pub compatibility: String,
    pub applicability: Option<Applicability>,
    pub features: Vec<Feature>,
    /// Bytes ship inside the binary.
    pub embedded: bool,
}

impl OfficialSkill {
    pub fn matches(&self, name: &str) -> bool {
        self.id == name || self.aliases.iter().any(|a| a == name)
    }

    pub fn feature(&self, name: &str) -> Option<&Feature> {
        self.features.iter().find(|f| f.name == name)
    }
}

/// The curated set: catalog data plus the exact bytes of every embedded file.
#[derive(Clone, Debug)]
pub struct OfficialSet {
    pub skills: Vec<OfficialSkill>,
    pub presets: BTreeMap<String, BTreeMap<String, String>>,
    assets: BTreeMap<(String, String), Vec<u8>>,
}

fn bad(msg: impl Into<String>) -> crate::diag::HarnessDiagnostic {
    d("SPX-HPM037", msg)
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(String::from)
}

fn str_map(v: &Value) -> BTreeMap<String, Vec<String>> {
    v.as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), strs(v))).collect())
        .unwrap_or_default()
}

fn parse_skill(v: &Value) -> HarnessResult<OfficialSkill> {
    let id = s(v, "id").ok_or_else(|| bad("catalog skill without `id`"))?;
    let src = &v["source"];
    let files = v["files"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| OfficialFile {
                    path: s(f, "path").unwrap_or_default(),
                    upstream_path: s(f, "upstream_path").unwrap_or_default(),
                    git_blob_sha: s(f, "git_blob_sha").unwrap_or_default(),
                    sha256: format!("sha256:{}", s(f, "sha256").unwrap_or_default()),
                    bytes: f["bytes"].as_u64().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    let applicability = v.get("applicability").map(|a| Applicability {
        task_families: strs(&a["task_families"]),
        excluded_families: strs(&a["excluded_families"]),
        automatic: a["automatic"].as_bool().unwrap_or(false),
        shipped_mode: s(a, "shipped_mode").unwrap_or_else(|| "off".into()),
        modes: strs(&a["modes"]),
        explicit_triggers: strs(&a["explicit_triggers"]),
        stop_triggers: strs(&a["stop_triggers"]),
        mode_triggers: str_map(&a["mode_triggers"]),
        unsupported_triggers: str_map(&a["unsupported_triggers"]),
    });
    let features = v["features"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|f| Feature {
                    name: s(f, "name").unwrap_or_default(),
                    status: s(f, "status").unwrap_or_default(),
                    note: s(f, "note").unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(OfficialSkill {
        aliases: strs(&v["aliases"]),
        authorship: s(v, "authorship").unwrap_or_default(),
        repo: s(src, "repo").unwrap_or_default(),
        subpath: s(src, "subpath"),
        channel: s(src, "channel").unwrap_or_default(),
        tag: s(src, "tag"),
        commit: s(src, "commit"),
        re_resolved: s(src, "re_resolved"),
        version: s(v, "version").unwrap_or_default(),
        license: s(v, "license").unwrap_or_default(),
        license_file: s(v, "license_file"),
        bundle_digest: s(v, "bundle_digest"),
        files,
        compatibility: s(&v["compatibility"], "status").unwrap_or_default(),
        applicability,
        features,
        embedded: v["embedded"].as_bool().unwrap_or(true),
        id,
    })
}

impl OfficialSet {
    /// The catalog and bytes compiled into this binary.
    pub fn embedded() -> OfficialSet {
        let assets = ASSETS
            .iter()
            .map(|(id, path, bytes)| ((id.to_string(), path.to_string()), bytes.to_vec()))
            .collect();
        Self::from_parts(CATALOG_JSON, assets).expect("embedded skill catalog is valid")
    }

    /// Build a set from catalog JSON and owned asset bytes (tests, alternate catalogs).
    pub fn from_parts(
        catalog_json: &str,
        assets: BTreeMap<(String, String), Vec<u8>>,
    ) -> HarnessResult<OfficialSet> {
        let v: Value = serde_json::from_str(catalog_json)
            .map_err(|e| bad(format!("skill catalog is not valid JSON: {e}")))?;
        if v["schema"] != "semaprax.curated-skill-catalog.v1" {
            return Err(bad("unknown skill catalog schema"));
        }
        let skills = v["skills"]
            .as_array()
            .ok_or_else(|| bad("catalog needs `skills`"))?
            .iter()
            .map(parse_skill)
            .collect::<HarnessResult<Vec<_>>>()?;
        let presets = v["presets"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, p)| {
                        let m = p
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .filter_map(|(a, b)| b.as_str().map(|b| (a.clone(), b.into())))
                                    .collect()
                            })
                            .unwrap_or_default();
                        (k.clone(), m)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(OfficialSet {
            skills,
            presets,
            assets,
        })
    }

    pub fn find(&self, name: &str) -> Option<&OfficialSkill> {
        self.skills.iter().find(|k| k.matches(name))
    }

    /// Skills whose bytes ship in the binary, in catalog order.
    pub fn embedded_skills(&self) -> impl Iterator<Item = &OfficialSkill> {
        self.skills.iter().filter(|k| k.embedded)
    }

    /// Every name (id or alias) an approved project root may not claim.
    pub fn reserved_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .embedded_skills()
            .flat_map(|k| std::iter::once(k.id.clone()).chain(k.aliases.iter().cloned()))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Exact upstream bytes of one embedded file, verified against the catalog.
    pub fn file_bytes(&self, id: &str, path: &str) -> HarnessResult<&[u8]> {
        let skill = self.find(id).ok_or_else(|| unknown(id))?;
        let f = skill
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| bad(format!("`{path}` is not a recorded file of `{id}`")))?;
        let bytes = self
            .assets
            .get(&(skill.id.clone(), path.to_string()))
            .ok_or_else(|| bad(format!("`{id}/{path}` is not embedded")))?;
        if json::sha256_plain(bytes) != f.sha256 || bytes.len() as u64 != f.bytes {
            return Err(bad(format!(
                "embedded `{id}/{path}` does not match its recorded sha256"
            )));
        }
        Ok(bytes)
    }

    /// Materialize the skill's recorded revision into the content-addressed
    /// `store`, staging under `scratch`; idempotent. Every file is verified
    /// against the catalog and the snapshot digest against `bundle_digest`.
    pub fn materialize(&self, id: &str, store: &Path, scratch: &Path) -> HarnessResult<Snapshot> {
        let skill = self.find(id).ok_or_else(|| unknown(id))?;
        let want = skill
            .bundle_digest
            .clone()
            .ok_or_else(|| bad(format!("`{id}` has no embedded bundle")))?;
        if let Ok(snap) = snapshot::open(store, &want) {
            return Ok(snap);
        }
        let tmp = scratch.join(format!(
            "stage-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let io = |e: std::io::Error| d("SPX-HPM035", format!("cannot stage `{id}`: {e}"));
        std::fs::create_dir_all(&tmp).map_err(io)?;
        let result = (|| {
            for f in &skill.files {
                std::fs::write(tmp.join(&f.path), self.file_bytes(id, &f.path)?).map_err(io)?;
            }
            snapshot::publish(store, &tmp, &ScanRules::skill(), &Bounds::SKILL)
        })();
        let _ = std::fs::remove_dir_all(&tmp);
        let snap = result?;
        if snap.digest != want {
            return Err(bad(format!(
                "`{id}` materialized as {} but the catalog records {want}",
                snap.digest
            )));
        }
        Ok(snap)
    }
}

pub(crate) fn unknown(name: &str) -> crate::diag::HarnessDiagnostic {
    d(
        "SPX-HPM038",
        format!("`{name}` is not a curated skill id or alias"),
    )
}

/// Process-wide parsed embedded set (config validation, CLI).
pub fn embedded_cached() -> &'static OfficialSet {
    static SET: std::sync::OnceLock<OfficialSet> = std::sync::OnceLock::new();
    SET.get_or_init(OfficialSet::embedded)
}
