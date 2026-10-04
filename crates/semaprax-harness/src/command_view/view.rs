//! Provider access for `command.view/v1`: the post-execution `view` operation
//! and the pre-execution `wrap` operation. A provider is any adopted and
//! trusted adapter the profile resolved; nothing here names a product.

use super::lineage::LINEAGE_VAR;
use super::policy::Policy;
use crate::cli::Environment;
use crate::contract::{
    CapabilityKind, CapabilityRef, ProjectBinding, RequestEnvelope, ResultStatus, Runtime,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::{
    AdapterHandle, AdapterManager, CancelToken, HostConfig, InvocationClass, IsolationRequest,
    LaunchSpec, Outcome,
};
use crate::profile::{check_grant_current, Resolution, ResolvedLaunch};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Diagnostic a wrapper plan must carry to declare that raw output of the
/// wrapped route stays recoverable without re-execution.
pub const RAW_RECOVERY_DIAG: &str = "wrapper.raw-recovery";

pub struct Provider {
    _manager: AdapterManager,
    handle: Arc<AdapterHandle>,
    launch: ResolvedLaunch,
    env: Environment,
    project: ProjectBinding,
    lock_digest: String,
    deadline_ms: u64,
    max_frame: usize,
    seq: std::cell::Cell<u32>,
    pub provider_id: String,
}

pub struct ProviderView {
    pub text: String,
    pub lossless: bool,
    pub omissions: u64,
}

pub struct WrapperPlan {
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    pub raw_recovery_declared: bool,
    pub upstream: Option<PathBuf>,
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn note(d: &HarnessDiagnostic) -> String {
    format!("{}: {}", d.code, d.message)
}

impl Provider {
    /// Prepare (not start) the provider the profile resolved for
    /// `command.view`. `Ok(None)`: the profile resolved no external provider.
    pub fn open(
        env: &Environment,
        project: &Path,
        project_id: &str,
        res: &Resolution,
        policy: &Policy,
    ) -> HarnessResult<Option<Provider>> {
        let Some(launch) = res.launches.get(&CapabilityKind::CommandView) else {
            return Ok(None);
        };
        let d = &launch.descriptor;
        let err = |m: String| HarnessDiagnostic::new("SPX-HPI002", m);
        let home = env
            .harness_home
            .clone()
            .ok_or_else(|| err("no harness home; cannot prepare a provider".into()))?;
        let runtime = match d.runtime {
            Runtime::Python => Some(
                policy
                    .runtimes
                    .get("python")
                    .cloned()
                    .or_else(|| env.vars.get("HARNESS_PYTHON").map(PathBuf::from)),
            ),
            Runtime::Node => Some(
                policy
                    .runtimes
                    .get("node")
                    .cloned()
                    .or_else(|| env.vars.get("HARNESS_NODE").map(PathBuf::from)),
            ),
            _ => None,
        };
        let runtime_executable = match runtime {
            Some(Some(p)) => Some(p),
            Some(None) => {
                return Err(err(format!(
                    "`{}` needs a {} runtime path (policy `runtimes` or environment)",
                    d.provider_id,
                    d.runtime.as_str()
                )))
            }
            None => None,
        };
        check_grant_current(env, &launch.grant)
            .map_err(|e| HarnessDiagnostic::new("SPX-HPI002", note(&e)))?;
        let key = sanitize(&d.provider_id);
        let pid = project_id.trim_start_matches("sha256:");
        let spec = LaunchSpec {
            descriptor: d.clone(),
            descriptor_dir: launch
                .descriptor_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            runtime_executable,
            upstream_executable: launch.upstream_path.clone(),
            grant: launch.grant.clone(),
            project_root: project.to_path_buf(),
            cache_dir: home.join("cache").join(pid).join(&key),
            retention_dir: home
                .join("retention")
                .join(pid)
                .join("providers")
                .join(&key),
            isolation: IsolationRequest::None,
            forward_env: BTreeMap::new(),
        };
        let manager = AdapterManager::new(HostConfig::default());
        let handle = manager
            .prepare(project_id, spec)
            .map_err(|e| HarnessDiagnostic::new("SPX-HPI002", note(&e)))?;
        Ok(Some(Provider {
            _manager: manager,
            handle,
            launch: launch.clone(),
            env: env.clone(),
            project: ProjectBinding {
                id: project_id.to_string(),
                worktree: project_id.to_string(),
                revision: "command-view".into(),
            },
            lock_digest: res.profile.lock_digest(),
            deadline_ms: policy.provider_timeout_ms,
            max_frame: d.resources.max_frame_bytes.min(4 << 20),
            seq: std::cell::Cell::new(0),
            provider_id: d.provider_id.clone(),
        }))
    }

    pub fn declares(&self, op: &str) -> bool {
        self.launch.descriptor.capabilities.iter().any(|c| {
            c.kind == Some(CapabilityKind::CommandView) && c.operations.iter().any(|o| o == op)
        })
    }

    /// Largest stdout+stderr this provider's frame can carry.
    pub fn input_limit(&self) -> usize {
        self.max_frame / 2
    }

    pub fn upstream(&self) -> Option<&Path> {
        self.launch.upstream_path.as_deref()
    }

    fn call(
        &self,
        op: &str,
        payload: Value,
        lineage: &[String],
    ) -> Result<crate::contract::ResultEnvelope, String> {
        check_grant_current(&self.env, &self.launch.grant).map_err(|e| note(&e))?;
        let n = self.seq.get() + 1;
        self.seq.set(n);
        let req = RequestEnvelope {
            invocation_id: format!("inv-cv-{n:06}"),
            project: self.project.clone(),
            lock_digest: self.lock_digest.clone(),
            capability: CapabilityRef {
                kind: CapabilityKind::CommandView,
                version: 1,
            },
            operation: op.to_string(),
            deadline_ms: self.deadline_ms,
            max_result_bytes: self.max_frame,
            remaining_calls: 4,
            lineage: lineage.to_vec(),
            payload,
        };
        // Nothing here is side-effecting for the repository: a provider only
        // reads bytes the host already captured, so it may be retried or skipped.
        match self
            .handle
            .invoke(&req, InvocationClass::SafeRead, &CancelToken::new())
        {
            Outcome::Completed(r) if r.status == ResultStatus::Complete && r.payload.is_some() => {
                Ok(r)
            }
            Outcome::Completed(r) => Err(format!(
                "provider answered `{}` without a usable view",
                r.status.as_str()
            )),
            Outcome::Refused(d) | Outcome::Quarantined(d) | Outcome::Uncertain(d) => Err(note(&d)),
            Outcome::Unavailable { reason, .. } => Err(note(&reason)),
            Outcome::Cancelled => Err("provider call cancelled".into()),
        }
    }

    /// Post-execution transform of already-captured (and redacted) output.
    pub fn view(
        &self,
        argv: &[String],
        stdout: &str,
        stderr: &str,
        lineage: &[String],
    ) -> Result<ProviderView, String> {
        let payload =
            json!({"form": "post-execution", "argv": argv, "stdout": stdout, "stderr": stderr});
        let r = self.call("view", payload, lineage)?;
        let v = &r.payload.as_ref().expect("checked")["view"];
        Ok(ProviderView {
            text: v["text"].as_str().unwrap_or_default().to_string(),
            lossless: v["lossless"].as_bool().unwrap_or(false),
            omissions: v["omissions"].as_u64().unwrap_or(0),
        })
    }

    /// Ask for a pre-execution wrapper plan. Unvalidated: the caller must
    /// authorize it before any dispatch.
    pub fn wrap(&self, argv: &[String], lineage: &[String]) -> Result<WrapperPlan, String> {
        let r = self.call("wrap", json!({"form": "wrapper", "argv": argv}), lineage)?;
        let p = &r.payload.as_ref().expect("checked")["plan"];
        Ok(WrapperPlan {
            argv: p["argv"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            cwd: p.get("cwd").and_then(Value::as_str).map(String::from),
            raw_recovery_declared: r.diagnostics.iter().any(|(c, _)| c == RAW_RECOVERY_DIAG),
            upstream: self.launch.upstream_path.clone(),
        })
    }
}

/// Environment variable carrying the lineage chain into the command.
pub fn lineage_env(chain: &[String]) -> (String, String) {
    (LINEAGE_VAR.to_string(), chain.join(","))
}
