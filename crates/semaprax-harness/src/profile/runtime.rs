//! The one adapter-runtime resolver shared by `run`, `context`, `exec`,
//! `conformance` and the bridges (HN-07). Precedence, strongest first: an
//! explicit flag, the machine-local adopted runtime (`adopt --runtime` or
//! `setup`), a policy file, then the `HARNESS_NODE` / `HARNESS_PYTHON`
//! variables. PATH is never searched.

use super::installations::LocalState;
use crate::cli::Environment;
use crate::contract::Runtime;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::path::{Path, PathBuf};

/// Environment variable naming the interpreter for `rt` (None: no interpreter).
pub fn env_var(rt: Runtime) -> Option<&'static str> {
    match rt {
        Runtime::Python => Some("HARNESS_PYTHON"),
        Runtime::Node => Some("HARNESS_NODE"),
        _ => None,
    }
}

/// Resolve the interpreter for an adapter of kind `rt` (None for native/builtin
/// adapters or when nothing names one).
pub fn pick(
    rt: Runtime,
    explicit: Option<&Path>,
    adopted: Option<&Path>,
    policy: Option<&Path>,
    env: &Environment,
) -> Option<PathBuf> {
    let var = env_var(rt)?;
    explicit
        .or(adopted)
        .or(policy)
        .map(Path::to_path_buf)
        .or_else(|| env.vars.get(var).map(PathBuf::from))
}

/// [`pick`], failing with `code` and an actionable message when an interpreter
/// adapter has no runtime. `Ok(None)` for adapters that need none.
pub fn require(
    code: &'static str,
    provider_id: &str,
    rt: Runtime,
    explicit: Option<&Path>,
    adopted: Option<&Path>,
    policy: Option<&Path>,
    env: &Environment,
) -> HarnessResult<Option<PathBuf>> {
    let Some(var) = env_var(rt) else {
        return Ok(None);
    };
    pick(rt, explicit, adopted, policy, env)
        .map(Some)
        .ok_or_else(|| {
            HarnessDiagnostic::new(
                code,
                format!(
                    "`{provider_id}` needs a {} runtime and none is adopted; run `semaprax harness setup` (or `adopt --runtime <abs path>`), or set {var}",
                    rt.as_str()
                ),
            )
        })
}

/// Adopted runtime and upstream for the installation whose descriptor is
/// `descriptor` (standalone verbs such as `conformance` use it as the default
/// for flags the user left out).
pub fn adopted_for_descriptor(
    env: &Environment,
    descriptor: &Path,
) -> Option<(Option<PathBuf>, Option<PathBuf>)> {
    let want = descriptor.canonicalize().ok()?;
    let state = LocalState::load(env).ok()?;
    state
        .installations
        .values()
        .find(|i| i.descriptor_path.canonicalize().ok().as_deref() == Some(want.as_path()))
        .map(|i| {
            (
                i.runtime.clone(),
                i.upstream.as_ref().map(|u| u.path.clone()),
            )
        })
}
