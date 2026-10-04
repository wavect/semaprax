//! Progressive resource loads and immutable activation for [`SkillService`]
//! (HN-03 / HN-19). A resource is loaded only by exact skill digest, relative
//! path and content digest; it is bounded, text-only, quoted as data and
//! charged once per presentation. Scripts are inventoried and never loaded
//! or executed. Activation copies a bundle into the snapshot store and the
//! session then reads only from that snapshot.

use super::bundle;
use super::d;
use super::inventory::{self, Bounds, FileKind, ScanRules};
use super::load::SkillService;
use super::policy::{self, Warning};
use super::snapshot::{self, Snapshot};
use crate::diag::HarnessResult;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ResourceLoad {
    /// Digest (artifact v2) of the skill that owns the resource.
    pub skill_digest: String,
    pub path: String,
    /// Plain `sha256:` of the resource bytes.
    pub digest: String,
    pub kind: FileKind,
    /// Exact framed text exposed to a model.
    pub text: String,
    pub bytes: usize,
    /// Model-visible bytes charged by this call: `text.len()` on first
    /// presentation, 0 when the same resource was already presented.
    pub charged_bytes: usize,
    pub already_presented: bool,
    pub warnings: Vec<Warning>,
}

impl ResourceLoad {
    /// `skill.catalog/v1` `load` result payload for a resource request.
    pub fn payload(&self) -> Value {
        json!({"digest": self.skill_digest,
               "artifact_refs": [{"path": self.path, "digest": self.digest}],
               "text": self.text})
    }
}

/// A skill whose source moved on while an older revision is active.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
    pub name: String,
    pub active_digest: String,
    /// Digest now present in the approved roots; `None` when removed.
    pub source_digest: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Activation {
    pub name: String,
    pub digest: String,
    pub snapshot_dir: PathBuf,
    pub already_active: bool,
    pub drift: Vec<Drift>,
}

fn q(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

impl SkillService {
    /// Enable immutable activation under `store` (`<harness_home>/artifacts`).
    pub fn with_snapshot_store(mut self, store: PathBuf) -> Self {
        self.store = Some(store);
        self
    }

    /// Model-visible bytes charged so far for resource presentations.
    pub fn resource_bytes_charged(&self) -> usize {
        self.charged.values().sum()
    }

    /// Activate a listed skill: extract and validate its immutable snapshot.
    /// Later source edits do not affect this session; activating again reports
    /// drift (`SPX-HPM036` is the warning code carried in the message).
    pub fn activate(&mut self, digest: &str) -> HarnessResult<Activation> {
        let store = self.store.clone().ok_or_else(|| {
            d(
                "SPX-HPM035",
                "no snapshot store configured; activation needs one",
            )
        })?;
        if !self.config.enabled {
            return Err(d(
                "SPX-HPM007",
                "skills are disabled; nothing can be activated",
            ));
        }
        let cat = self.scan_catalog();
        let existing = self
            .active
            .iter()
            .find(|(_, a)| a.digest == digest)
            .map(|(n, a)| (n.clone(), a.snap.digest.clone(), a.entry.clone()));
        let (name, snap, entry, already) = if let Some((name, sd, entry)) = existing {
            (name, snapshot::open(&store, &sd)?, entry, true)
        } else {
            let Some(e) = cat.entries.iter().find(|e| e.digest == digest) else {
                return Err(if self.snapshot.contains_key(digest) {
                    d(
                        "SPX-HPM006",
                        format!("skill digest {digest} is stale: the bundle changed since listing"),
                    )
                } else {
                    d(
                        "SPX-HPM007",
                        format!("no approved skill has digest {digest}"),
                    )
                });
            };
            let snap = snapshot::publish(&store, &e.dir, &ScanRules::skill(), &Bounds::SKILL)?;
            if snap.digest != digest {
                return Err(d("SPX-HPM006", format!("skill digest {digest} is stale: the bundle changed while it was snapshotted")));
            }
            let mut entry = e.clone();
            entry.dir = snap.files_dir.clone();
            (e.name.clone(), snap, entry, false)
        };
        let dir = snap.files_dir.clone();
        self.active.insert(
            name.clone(),
            super::load::Active {
                digest: digest.to_string(),
                snap,
                entry,
            },
        );
        let drift = cat
            .entries
            .iter()
            .filter(|e| e.name == name && e.digest != digest)
            .map(|e| Drift {
                name: name.clone(),
                active_digest: digest.to_string(),
                source_digest: Some(e.digest.clone()),
            })
            .chain(
                (!cat.entries.iter().any(|e| e.name == name)).then(|| Drift {
                    name: name.clone(),
                    active_digest: digest.to_string(),
                    source_digest: None,
                }),
            )
            .collect();
        Ok(Activation {
            name,
            digest: digest.to_string(),
            snapshot_dir: dir,
            already_active: already,
            drift,
        })
    }

    /// Active skills whose approved source differs from the active revision.
    pub fn drift(&mut self) -> Vec<Drift> {
        let cat = self.scan_catalog();
        let mut out = Vec::new();
        for (name, a) in &self.active {
            let digest = &a.digest;
            let cur: Vec<&str> = cat
                .entries
                .iter()
                .filter(|e| &e.name == name)
                .map(|e| e.digest.as_str())
                .collect();
            if cur.contains(&digest.as_str()) {
                continue;
            }
            out.push(Drift {
                name: name.clone(),
                active_digest: digest.clone(),
                source_digest: cur.first().map(|s| s.to_string()),
            });
        }
        out
    }

    /// Load one approved text resource by exact skill digest, path and content
    /// digest. Refusals are `SPX-HPM033`; a changed source is `SPX-HPM006`.
    pub fn load_resource(
        &mut self,
        skill_digest: &str,
        path: &str,
        want: &str,
    ) -> HarnessResult<ResourceLoad> {
        let b = self.bundle_for(skill_digest)?;
        let refuse = |m: String| d("SPX-HPM033", format!("resource `{path}`: {m}"));
        let entry = b
            .inventory
            .get(path)
            .ok_or_else(|| refuse("not in the skill's inventory".into()))?;
        if entry.kind == FileKind::ExecutableScript {
            return Err(refuse(
                "scripts are inventoried but never loaded or executed".into(),
            ));
        }
        if entry.sha256 != want {
            return Err(refuse(format!(
                "digest {want} differs from the inventory digest {}",
                entry.sha256
            )));
        }
        if entry.bytes as usize > self.config.max_resource_bytes {
            return Err(refuse(format!(
                "{} bytes exceeds the {} byte resource bound",
                entry.bytes, self.config.max_resource_bytes
            )));
        }
        let bytes = inventory::read_file(&b.dir, path, self.config.max_resource_bytes as u64)
            .map_err(|e| refuse(e.message))?;
        if crate::json::sha256_plain(&bytes) != entry.sha256 {
            return Err(d(
                "SPX-HPM006",
                format!("resource `{path}` changed since the skill was listed"),
            ));
        }
        let text = String::from_utf8(bytes).map_err(|_| refuse("not valid UTF-8 text".into()))?;
        if text.contains('\0') {
            return Err(refuse("contains NUL; not a text resource".into()));
        }
        let warnings = policy::scan(&text);
        let mut out = format!(
            "BEGIN SKILL RESOURCE skill={} path={} digest={}\n{}\n",
            q(&b.name),
            q(path),
            entry.sha256,
            policy::PRECEDENCE
        );
        for w in &warnings {
            out.push_str(&format!(
                "warning {} {} line {} (flagged, not obeyed)\n",
                w.code, w.kind, w.line
            ));
        }
        out.push_str("quoted data:\n");
        for line in text.lines() {
            out.push_str("> ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&format!("END SKILL RESOURCE path={}\n", q(path)));
        let key = (skill_digest.to_string(), path.to_string());
        let first = !self.charged.contains_key(&key);
        if first {
            self.charged.insert(key, out.len());
        }
        Ok(ResourceLoad {
            skill_digest: skill_digest.to_string(),
            path: path.to_string(),
            digest: entry.sha256.clone(),
            kind: entry.kind,
            bytes: entry.bytes as usize,
            charged_bytes: if first { out.len() } else { 0 },
            already_presented: !first,
            text: out,
            warnings,
        })
    }

    /// The bundle behind an exact digest: the active snapshot when activated,
    /// otherwise the live approved source (stale if it changed).
    pub(super) fn bundle_for(&mut self, digest: &str) -> HarnessResult<bundle::Bundle> {
        if !self.config.enabled {
            return Err(d(
                "SPX-HPM007",
                "skills are disabled; nothing can be loaded",
            ));
        }
        if let Some(a) = self.active.values().find(|a| a.digest == digest) {
            let snap: &Snapshot = &a.snap;
            return match bundle::read_bundle(&snap.files_dir) {
                Ok(Some(b)) if b.digest == digest => Ok(b),
                _ => Err(d(
                    "SPX-HPM035",
                    format!("active snapshot of {digest} no longer validates"),
                )),
            };
        }
        let cat = self.scan_catalog();
        let stale = || {
            d(
                "SPX-HPM006",
                format!("skill digest {digest} is stale: the bundle changed since listing"),
            )
        };
        let Some(entry) = cat.entries.iter().find(|e| e.digest == digest) else {
            return Err(if self.snapshot.contains_key(digest) {
                stale()
            } else {
                d(
                    "SPX-HPM007",
                    format!("no approved skill has digest {digest}"),
                )
            });
        };
        match bundle::read_bundle(&entry.dir) {
            Ok(Some(b)) if b.digest == digest => Ok(b),
            _ => Err(stale()),
        }
    }
}
