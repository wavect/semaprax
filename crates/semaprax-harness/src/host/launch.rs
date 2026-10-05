//! Launch specification and pre-launch verification: the grant must name this
//! descriptor and the entry/upstream bytes on disk right now, and requested
//! isolation must be enforceable. Environments are rebuilt from nothing.

use super::grant::Grant;
use super::isolation::{IsolationBackend, IsolationMode, IsolationRequest, Wrapper};
use crate::contract::{Descriptor, Runtime};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Everything the host needs to start one adapter. Constructed by the host
/// from trusted configuration, never from adapter or model output.
#[derive(Clone, Debug)]
pub struct LaunchSpec {
    pub descriptor: Descriptor,
    pub descriptor_dir: PathBuf,
    /// Absolute path of the node/python executable; unused for native adapters.
    pub runtime_executable: Option<PathBuf>,
    pub upstream_executable: Option<PathBuf>,
    pub grant: Grant,
    pub project_root: PathBuf,
    pub cache_dir: PathBuf,
    pub retention_dir: PathBuf,
    pub isolation: IsolationRequest,
    /// The only caller-chosen environment (granted secrets/config); reserved
    /// `SEMAPRAX_HARNESS_*`, `PATH`, `HOME` and `TMPDIR` keys are refused.
    pub forward_env: BTreeMap<String, String>,
}

/// A verified, ready-to-spawn launch.
pub(crate) struct Prepared {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
    pub mode: IsolationMode,
}

fn refuse(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn digest_of(path: &Path) -> HarnessResult<String> {
    std::fs::read(path).map(|b| sha256_plain(&b)).map_err(|e| {
        refuse(
            "SPX-HPC002",
            format!("cannot read {} for digest check: {e}", path.display()),
        )
    })
}

fn require_abs(what: &str, p: &Path) -> HarnessResult<()> {
    if p.is_absolute() {
        Ok(())
    } else {
        Err(refuse(
            "SPX-HPC004",
            format!("{what} must be an absolute path"),
        ))
    }
}

impl LaunchSpec {
    /// First security-relevant launch input that differs from `other`, or
    /// `None` when the complete launch identity is the same. The descriptor is
    /// compared by digest; everything else by value.
    pub(crate) fn launch_difference(&self, other: &Self) -> Option<&'static str> {
        if self.descriptor.digest() != other.descriptor.digest() {
            return Some("descriptor");
        }
        let checks: [(&'static str, bool); 9] = [
            ("isolation request", self.isolation == other.isolation),
            ("grant", self.grant == other.grant),
            (
                "runtime executable",
                self.runtime_executable == other.runtime_executable,
            ),
            (
                "upstream executable",
                self.upstream_executable == other.upstream_executable,
            ),
            (
                "descriptor directory",
                self.descriptor_dir == other.descriptor_dir,
            ),
            ("project root", self.project_root == other.project_root),
            ("cache directory", self.cache_dir == other.cache_dir),
            (
                "retention directory",
                self.retention_dir == other.retention_dir,
            ),
            (
                "forwarded environment",
                self.forward_env == other.forward_env,
            ),
        ];
        checks.iter().find(|(_, same)| !same).map(|(n, _)| *n)
    }

    /// Verify grant, digests, paths and isolation; derive the command.
    pub(crate) fn prepare(&self, backend: &IsolationBackend) -> HarnessResult<Prepared> {
        let d = &self.descriptor;
        let g = &self.grant;
        if g.provider_id() != d.provider_id || g.descriptor_digest() != d.digest() {
            return Err(refuse(
                "SPX-HPC001",
                format!(
                    "grant does not match descriptor `{}`; launch refused",
                    d.provider_id
                ),
            ));
        }
        for (what, p) in [
            ("descriptor_dir", &self.descriptor_dir),
            ("project_root", &self.project_root),
            ("cache_dir", &self.cache_dir),
            ("retention_dir", &self.retention_dir),
        ] {
            require_abs(what, p)?;
        }
        let entry0 = d.entry.first().ok_or_else(|| {
            refuse(
                "SPX-HPC004",
                "descriptor has no adapter entry (builtin providers are not process-launched)",
            )
        })?;
        if d.runtime == Runtime::Builtin {
            return Err(refuse(
                "SPX-HPC004",
                "builtin providers are not process-launched",
            ));
        }
        let dir = self
            .descriptor_dir
            .canonicalize()
            .map_err(|e| refuse("SPX-HPC004", format!("descriptor_dir: {e}")))?;
        let entry = dir
            .join(entry0)
            .canonicalize()
            .map_err(|e| refuse("SPX-HPC004", format!("adapter entry: {e}")))?;
        if !entry.starts_with(&dir) {
            return Err(refuse(
                "SPX-HPC002",
                "adapter entry resolves outside the descriptor directory",
            ));
        }
        match g.entry_digest() {
            // artifact-v2: the grant binds every adapter file under the
            // descriptor directory, not only the entry.
            Some(want) if crate::skills::inventory::is_v2_label(want) => {
                let rel = entry
                    .strip_prefix(&dir)
                    .ok()
                    .and_then(|p| p.to_str())
                    .unwrap_or(entry0.as_str());
                let have = crate::skills::inventory::adapter_closure_label(&dir, rel, &[])
                    .map_err(|e| {
                        refuse(
                            "SPX-HPC002",
                            format!("adapter closure unreadable: {} ({})", e.message, e.code),
                        )
                    })?;
                if have != want {
                    return Err(refuse(
                        "SPX-HPC002",
                        "adapter closure (entry or helper files) changed since it was granted",
                    ));
                }
            }
            Some(want) if want == digest_of(&entry)? => {}
            _ => {
                return Err(refuse(
                    "SPX-HPC002",
                    "adapter entry changed since it was granted (or no entry digest was granted)",
                ))
            }
        }
        let upstream = match (&self.upstream_executable, g.upstream_digest()) {
            (None, None) => None,
            (Some(p), Some(want)) => {
                require_abs("upstream_executable", p)?;
                if want != digest_of(p)? {
                    return Err(refuse(
                        "SPX-HPC002",
                        "upstream executable changed since it was granted",
                    ));
                }
                Some(
                    p.canonicalize()
                        .map_err(|e| refuse("SPX-HPC004", format!("upstream executable: {e}")))?,
                )
            }
            _ => {
                return Err(refuse(
                    "SPX-HPC002",
                    "upstream executable and its granted digest must both be present",
                ))
            }
        };

        let mut exec = vec![dir.clone()];
        let mut read = vec![dir.clone()];
        let (mut program, args) = match d.runtime {
            Runtime::Native => (
                entry.clone(),
                d.entry[1..].iter().map(OsString::from).collect::<Vec<_>>(),
            ),
            _ => {
                let rt = self.runtime_executable.as_ref().ok_or_else(|| {
                    refuse(
                        "SPX-HPC004",
                        "runtime_executable is required for node/python adapters",
                    )
                })?;
                require_abs("runtime_executable", rt)?;
                let canon = rt
                    .canonicalize()
                    .map_err(|e| refuse("SPX-HPC004", format!("runtime executable: {e}")))?;
                // Interpreter prefix: `<prefix>/bin/<exe>` -> `<prefix>`.
                let root = canon
                    .parent()
                    .and_then(|b| b.parent())
                    .unwrap_or(&canon)
                    .to_path_buf();
                exec.push(root.clone());
                read.push(root);
                let mut a = vec![entry.clone().into_os_string()];
                a.extend(d.entry[1..].iter().map(OsString::from));
                (
                    if matches!(self.isolation, IsolationRequest::None) {
                        rt.clone()
                    } else {
                        canon
                    },
                    a,
                )
            }
        };
        if let Some(u) = &upstream {
            let parent = u.parent().unwrap_or(u).to_path_buf();
            exec.push(parent.clone());
            read.push(parent);
        }
        if g.permissions().read.iter().any(|r| r == "project") {
            read.push(self.project_root.clone());
        }
        std::fs::create_dir_all(self.cache_dir.join("home"))
            .and_then(|_| std::fs::create_dir_all(self.cache_dir.join("tmp")))
            .map_err(|e| refuse("SPX-HPC004", format!("cache_dir: {e}")))?;
        read.push(self.cache_dir.clone());
        let mut write = vec![self.cache_dir.clone()];
        if g.permissions().write.iter().any(|w| w == "retention") {
            std::fs::create_dir_all(&self.retention_dir)
                .map_err(|e| refuse("SPX-HPC004", format!("retention_dir: {e}")))?;
            write.push(self.retention_dir.clone());
        }

        let (mode, wrapper) = backend.plan(&self.isolation, &read, &write, &exec)?;
        let mut args = args;
        if let Some(Wrapper {
            program: wp,
            args: mut wa,
        }) = wrapper
        {
            wa.push(program.into_os_string());
            wa.append(&mut args);
            program = wp;
            args = wa;
        }
        Ok(Prepared {
            program,
            args,
            env: self.environment(&upstream, d.runtime)?,
            cwd: self.cache_dir.clone(),
            mode,
        })
    }

    fn environment(
        &self,
        upstream: &Option<PathBuf>,
        runtime: Runtime,
    ) -> HarnessResult<BTreeMap<String, String>> {
        let mut env = BTreeMap::new();
        let s = |p: &Path| p.to_string_lossy().into_owned();
        env.insert("PATH".into(), "/usr/bin:/bin".into());
        env.insert("HOME".into(), s(&self.cache_dir.join("home")));
        env.insert("TMPDIR".into(), s(&self.cache_dir.join("tmp")));
        env.insert(
            "SEMAPRAX_HARNESS_PROJECT_ROOT".into(),
            s(&self.project_root),
        );
        env.insert("SEMAPRAX_HARNESS_CACHE_DIR".into(), s(&self.cache_dir));
        env.insert(
            "SEMAPRAX_HARNESS_RETENTION_DIR".into(),
            s(&self.retention_dir),
        );
        if let Some(u) = upstream {
            env.insert("SEMAPRAX_HARNESS_UPSTREAM".into(), s(u));
        }
        if runtime == Runtime::Python {
            env.insert("PYTHONDONTWRITEBYTECODE".into(), "1".into());
            env.insert("PYTHONNOUSERSITE".into(), "1".into());
        }
        for (k, v) in &self.forward_env {
            // `SEMAPRAX_HARNESS_CFG_*` is the host's own channel for validated adapter config.
            let reserved = env.contains_key(k)
                || (k.starts_with("SEMAPRAX_HARNESS_") && !k.starts_with("SEMAPRAX_HARNESS_CFG_"));
            if reserved || k.is_empty() || k.contains(['=', '\0']) || v.contains('\0') {
                return Err(refuse(
                    "SPX-HPC001",
                    format!("forward_env key `{k}` is reserved or malformed"),
                ));
            }
            env.insert(k.clone(), v.clone());
        }
        Ok(env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::grant::GrantedPermissions;
    use crate::skills::inventory::adapter_closure_label;
    use serde_json::json;

    fn descriptor() -> Descriptor {
        let caps = json!([{"kind": "context.repository", "version": 1, "required": true,
            "operations": crate::contract::CapabilityKind::ContextRepository.operations()}]);
        Descriptor::parse(
            json!({
                "schema": "semaprax.harness-provider.v1",
                "provider": {"id": "org.example/closure", "version": "0.1.0"},
                "adapter": {"runtime": "native", "entry": ["adapter.sh"], "version": "0.1.0"},
                "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
                "capabilities": caps,
                "platforms": ["macos-aarch64", "linux-x86_64"],
                "permissions": {"read": ["project"], "write": [], "network": [], "process": [], "secrets": []},
                "resources": {"handshake_timeout_ms": 5000, "invoke_timeout_ms": 30000, "max_frame_bytes": 1048576,
                              "max_concurrency": 1, "idle_shutdown_ms": 60000},
                "cancellation": "cooperative",
                "support": {"license": "MIT", "isolation": "subprocess", "tested": []}
            })
            .to_string()
            .as_bytes(),
        )
        .expect("descriptor")
    }

    fn spec(root: &Path, entry_digest: Option<String>) -> LaunchSpec {
        let d = descriptor();
        let grant = Grant::issue(
            d.provider_id.clone(),
            d.digest().to_string(),
            entry_digest,
            None,
            GrantedPermissions::default(),
        );
        LaunchSpec {
            descriptor: d,
            descriptor_dir: root.join("adapter"),
            runtime_executable: None,
            upstream_executable: None,
            grant,
            project_root: root.join("project"),
            cache_dir: root.join("cache"),
            retention_dir: root.join("retention"),
            isolation: IsolationRequest::None,
            forward_env: BTreeMap::new(),
        }
    }

    #[test]
    fn v2_grant_binds_helper_files_but_legacy_grant_does_not() {
        let root = std::env::temp_dir().join(format!("hp-hn19-launch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("adapter");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(root.join("project")).unwrap();
        std::fs::write(dir.join("adapter.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::write(dir.join("helper.py"), "VALUE = 1\n").unwrap();
        let backend = IsolationBackend::unavailable();
        let v2 = adapter_closure_label(&dir, "adapter.sh", &[]).unwrap();
        let legacy = digest_of(&dir.join("adapter.sh")).unwrap();
        assert!(spec(&root, Some(v2.clone())).prepare(&backend).is_ok());
        assert!(spec(&root, Some(legacy.clone())).prepare(&backend).is_ok());
        std::fs::write(dir.join("helper.py"), "VALUE = 2\n").unwrap();
        let e = spec(&root, Some(v2.clone()))
            .prepare(&backend)
            .err()
            .expect("refused");
        assert_eq!(e.code, "SPX-HPC002");
        assert!(e.message.contains("closure"), "{}", e.message);
        // legacy-v1 binds only the entry bytes: the helper edit rides along.
        assert!(spec(&root, Some(legacy)).prepare(&backend).is_ok());
        // a new file under the descriptor directory is also a closure change
        std::fs::write(dir.join("helper.py"), "VALUE = 1\n").unwrap();
        assert!(spec(&root, Some(v2.clone())).prepare(&backend).is_ok());
        std::fs::write(dir.join("extra.js"), "x\n").unwrap();
        assert!(spec(&root, Some(v2)).prepare(&backend).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
