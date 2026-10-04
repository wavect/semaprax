//! Machine-local state under `$SEMAPRAX_HARNESS_HOME`: adopted installations,
//! user preferences, and (via `trust.rs`) trust records. None of it is ever
//! committed or copied into the lock.

use super::trust::{TrustRecord, TRUST_SCHEMA};
use crate::cli::Environment;
use crate::contract::{CapabilityKind, Descriptor, PermissionRequest, Runtime};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::grant::GrantedPermissions;
use crate::json::{self, parse_strict, JsonLimits};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const INSTALLATIONS_SCHEMA: &str = "semaprax.harness-installations.v1";
pub const PREFERENCES_SCHEMA: &str = "semaprax.harness-preferences.v1";

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpstreamRecord {
    pub path: PathBuf,
    pub digest: String,
    /// Version reported by the explicit identity probe, if it ran and parsed.
    pub version: Option<String>,
    /// Probed version is one the descriptor lists.
    pub compatible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installation {
    pub provider_id: String,
    pub descriptor_path: PathBuf,
    pub descriptor_digest: String,
    pub entry_digest: Option<String>,
    pub upstream: Option<UpstreamRecord>,
    /// Explicit adapter runtime executable (`node`/`python`), machine-local.
    pub runtime: Option<PathBuf>,
}

/// A machine-local approved skill root (never read from a project).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRootRecord {
    pub path: PathBuf,
    pub origin: String,
}

/// Digests of what exists on disk right now; trust is bound to these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentDigests {
    pub descriptor_digest: String,
    pub entry_digest: Option<String>,
    pub upstream_digest: Option<String>,
    /// The descriptor declares an upstream executable.
    pub requires_upstream: bool,
    pub requested: PermissionRequest,
}

/// An installation re-read from disk.
#[derive(Clone, Debug)]
pub struct Inspected {
    pub descriptor: Descriptor,
    pub current: CurrentDigests,
    pub entry_path: Option<PathBuf>,
    pub upstream_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct LocalState {
    pub home: Option<PathBuf>,
    pub installations: BTreeMap<String, Installation>,
    pub trust: BTreeMap<String, TrustRecord>,
    pub preferences: BTreeMap<CapabilityKind, String>,
    pub skill_roots: Vec<SkillRootRecord>,
}

pub fn file_digest(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!(
        "sha256:{}",
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}

/// Entry file of a non-builtin descriptor (`entry[0]` relative to the descriptor directory).
pub fn entry_path(descriptor_path: &Path, d: &Descriptor) -> Option<PathBuf> {
    if d.runtime == Runtime::Builtin {
        return None;
    }
    Some(
        descriptor_path
            .parent()
            .unwrap_or(Path::new("/"))
            .join(d.entry.first()?),
    )
}

impl Installation {
    /// Re-read the descriptor and hash entry and upstream as they are now.
    pub fn inspect(&self) -> HarnessResult<Inspected> {
        let bytes = std::fs::read(&self.descriptor_path).map_err(|e| {
            bad(
                "SPX-HPB022",
                format!(
                    "descriptor of `{}` unreadable at its adopted path: {e}",
                    self.provider_id
                ),
            )
        })?;
        let descriptor = Descriptor::parse(&bytes)?;
        if descriptor.provider_id != self.provider_id {
            return Err(bad(
                "SPX-HPB022",
                format!(
                    "descriptor at the adopted path now declares `{}`, not `{}`",
                    descriptor.provider_id, self.provider_id
                ),
            ));
        }
        let entry = entry_path(&self.descriptor_path, &descriptor);
        let entry_digest = match &entry {
            Some(p) => Some(file_digest(p).map_err(|e| {
                bad(
                    "SPX-HPB022",
                    format!("adapter entry of `{}` unreadable: {e}", self.provider_id),
                )
            })?),
            None => None,
        };
        let upstream_path = self.upstream.as_ref().map(|u| u.path.clone());
        let upstream_digest = upstream_path.as_ref().and_then(|p| file_digest(p).ok());
        let current = CurrentDigests {
            descriptor_digest: descriptor.digest().to_string(),
            entry_digest,
            upstream_digest,
            requires_upstream: descriptor.upstream.as_ref().is_some_and(|u| !is_bundled(u)),
            requested: descriptor.permissions.clone(),
        };
        Ok(Inspected {
            descriptor,
            current,
            entry_path: entry,
            upstream_path,
        })
    }
}

pub fn permissions_to_json(p: &GrantedPermissions) -> Value {
    json!({"read": p.read, "write": p.write, "network": p.network, "process": p.process, "secrets": p.secrets})
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> HarnessResult<()> {
    let io = |e: std::io::Error| {
        bad(
            "SPX-HPB020",
            format!("cannot write {}: {e}", path.display()),
        )
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

const LIMITS: JsonLimits = JsonLimits {
    max_bytes: 4 * 1024 * 1024,
    max_depth: 8,
    max_nodes: 100_000,
};

pub(super) fn read_doc(home: &Path, file: &str, schema: &str) -> HarnessResult<Option<Value>> {
    let path = home.join(file);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(bad(
                "SPX-HPB020",
                format!("cannot read {}: {e}", path.display()),
            ))
        }
    };
    let doc = parse_strict(&bytes, &LIMITS)
        .map_err(|e| bad("SPX-HPB020", format!("{file}: {}", e.message)))?;
    if doc.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(bad(
            "SPX-HPB020",
            format!("{file}: expected schema `{schema}`"),
        ));
    }
    Ok(Some(doc))
}

pub(super) fn obj<'a>(
    v: &'a Value,
    file: &str,
    what: &str,
) -> HarnessResult<&'a Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| bad("SPX-HPB020", format!("{file}: {what} must be an object")))
}

pub(super) fn str_field(m: &Map<String, Value>, file: &str, key: &str) -> HarnessResult<String> {
    m.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| bad("SPX-HPB020", format!("{file}: `{key}` must be a string")))
}

pub(super) fn opt_str(
    m: &Map<String, Value>,
    file: &str,
    key: &str,
) -> HarnessResult<Option<String>> {
    match m.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(bad(
            "SPX-HPB020",
            format!("{file}: `{key}` must be a string or null"),
        )),
    }
}

pub(super) fn abs_path(s: String, file: &str, key: &str) -> HarnessResult<PathBuf> {
    let p = PathBuf::from(s);
    if p.is_absolute() {
        Ok(p)
    } else {
        Err(bad(
            "SPX-HPB020",
            format!("{file}: `{key}` must be absolute"),
        ))
    }
}

impl LocalState {
    /// Load every machine-local file; missing files are an empty (first-run) state.
    pub fn load(env: &Environment) -> HarnessResult<LocalState> {
        let mut st = LocalState {
            home: env.harness_home.clone(),
            ..Default::default()
        };
        let Some(home) = env.harness_home.clone() else {
            return Ok(st);
        };
        const F: &str = "installations.json";
        if let Some(doc) = read_doc(&home, F, INSTALLATIONS_SCHEMA)? {
            let list = obj(
                doc.get("installations").unwrap_or(&Value::Null),
                F,
                "`installations`",
            )?;
            for (id, v) in list {
                let m = obj(v, F, "an installation")?;
                let upstream = match m.get("upstream") {
                    None | Some(Value::Null) => None,
                    Some(u) => {
                        let u = obj(u, F, "`upstream`")?;
                        Some(UpstreamRecord {
                            path: abs_path(str_field(u, F, "path")?, F, "upstream.path")?,
                            digest: str_field(u, F, "digest")?,
                            version: opt_str(u, F, "version")?,
                            compatible: u.get("compatible").and_then(Value::as_bool).ok_or_else(
                                || {
                                    bad(
                                        "SPX-HPB020",
                                        format!("{F}: `compatible` must be a boolean"),
                                    )
                                },
                            )?,
                        })
                    }
                };
                st.installations.insert(
                    id.clone(),
                    Installation {
                        provider_id: id.clone(),
                        descriptor_path: abs_path(
                            str_field(m, F, "descriptor_path")?,
                            F,
                            "descriptor_path",
                        )?,
                        descriptor_digest: str_field(m, F, "descriptor_digest")?,
                        entry_digest: opt_str(m, F, "entry_digest")?,
                        upstream,
                        runtime: opt_str(m, F, "runtime")?
                            .map(|r| abs_path(r, F, "runtime"))
                            .transpose()?,
                    },
                );
            }
            if let Some(roots) = doc.get("skill_roots") {
                let arr = roots.as_array().ok_or_else(|| {
                    bad("SPX-HPB020", format!("{F}: `skill_roots` must be an array"))
                })?;
                for r in arr {
                    let m = obj(r, F, "a skill root")?;
                    st.skill_roots.push(SkillRootRecord {
                        path: abs_path(str_field(m, F, "path")?, F, "skill_roots.path")?,
                        origin: str_field(m, F, "origin")?,
                    });
                }
            }
        }
        const P: &str = "preferences.json";
        if let Some(doc) = read_doc(&home, P, PREFERENCES_SCHEMA)? {
            let list = obj(
                doc.get("preferences").unwrap_or(&Value::Null),
                P,
                "`preferences`",
            )?;
            for (k, v) in list {
                let kind = CapabilityKind::parse(k).ok_or_else(|| {
                    bad("SPX-HPB020", format!("{P}: unknown capability kind `{k}`"))
                })?;
                let id = v.as_str().ok_or_else(|| {
                    bad("SPX-HPB020", format!("{P}: preference must be a string"))
                })?;
                st.preferences.insert(kind, id.to_string());
            }
        }
        st.trust = super::trust::load_records(&home)?;
        Ok(st)
    }

    fn home_dir(&self) -> HarnessResult<&Path> {
        self.home
            .as_deref()
            .ok_or_else(|| bad("SPX-HPB020", "no harness home: set SEMAPRAX_HARNESS_HOME (or HOME) so machine-local state can be stored"))
    }

    pub fn save_installations(&self) -> HarnessResult<()> {
        let mut list = Map::new();
        for (id, i) in &self.installations {
            let upstream = i.upstream.as_ref().map_or(Value::Null, |u| {
                json!({"path": u.path.to_string_lossy(), "digest": u.digest, "version": u.version, "compatible": u.compatible})
            });
            let mut rec = json!({"descriptor_path": i.descriptor_path.to_string_lossy(), "descriptor_digest": i.descriptor_digest,
                       "entry_digest": i.entry_digest, "upstream": upstream});
            if let Some(r) = &i.runtime {
                rec["runtime"] = json!(r.to_string_lossy());
            }
            list.insert(id.clone(), rec);
        }
        let mut doc = json!({"schema": INSTALLATIONS_SCHEMA, "installations": list});
        if !self.skill_roots.is_empty() {
            doc["skill_roots"] = Value::Array(
                self.skill_roots
                    .iter()
                    .map(|r| json!({"path": r.path.to_string_lossy(), "origin": r.origin}))
                    .collect(),
            );
        }
        write_atomic(
            &self.home_dir()?.join("installations.json"),
            format!("{}\n", json::canonical(&doc)).as_bytes(),
        )
    }

    pub fn save_preferences(&self) -> HarnessResult<()> {
        let list: Map<String, Value> = self
            .preferences
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), json!(v)))
            .collect();
        let doc = json!({"schema": PREFERENCES_SCHEMA, "preferences": list});
        write_atomic(
            &self.home_dir()?.join("preferences.json"),
            format!("{}\n", json::canonical(&doc)).as_bytes(),
        )
    }

    pub fn save_trust(&self) -> HarnessResult<()> {
        let mut list = Map::new();
        for (id, t) in &self.trust {
            list.insert(id.clone(), t.to_json());
        }
        let doc = json!({"schema": TRUST_SCHEMA, "trust": list});
        write_atomic(
            &self.home_dir()?.join("trust.json"),
            format!("{}\n", json::canonical(&doc)).as_bytes(),
        )
    }

    pub fn home(&self) -> HarnessResult<&Path> {
        self.home_dir()
    }
}

/// A `local:` upstream with no identity probe is code bundled beside the
/// adapter entry; the entry digest already binds it, so no separate upstream
/// executable is adopted or required.
pub fn is_bundled(upstream: &crate::contract::UpstreamIdentity) -> bool {
    upstream.package.starts_with("local:") && upstream.identity_probe.is_empty()
}
