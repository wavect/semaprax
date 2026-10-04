//! Authorization of a provider's pre-execution wrapper plan. The plan is
//! untrusted data: both the original intent and the effective argv must pass
//! before anything launches, and the host runs the result exactly once.

use super::intent::check_syntax;
use super::view::WrapperPlan;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use std::path::{Path, PathBuf};

fn refuse(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Returns the accepted effective argv and the absolute executable to run.
///
/// Accepted shapes: the original executable with the original arguments, or the
/// provider's own granted upstream executable followed by the complete
/// original argv. Anything else is substitution or permission widening.
pub fn authorize(
    plan: &WrapperPlan,
    original: &[String],
    original_exe: &Path,
    cwd: &Path,
    project: &Path,
) -> HarnessResult<(Vec<String>, PathBuf)> {
    if !plan.raw_recovery_declared {
        return Err(refuse(
            "SPX-HPH017",
            "wrapper plan does not declare raw recovery for its route; refusing it (run unwrapped)",
        ));
    }
    check_syntax(&plan.argv)?;
    let first = Path::new(&plan.argv[0]);
    if !first.is_absolute() {
        return Err(refuse(
            "SPX-HPH014",
            "wrapper plan must name an absolute executable; PATH is never searched for a wrapper",
        ));
    }
    let exe = first
        .canonicalize()
        .map_err(|_| refuse("SPX-HPH014", "wrapper executable does not exist"))?;
    let same_args = plan.argv[1..] == original[1..];
    if exe == original_exe {
        if !same_args {
            return Err(refuse(
                "SPX-HPH016",
                "wrapper plan changes the arguments of the authorized executable",
            ));
        }
    } else if plan
        .upstream
        .as_deref()
        .and_then(|u| u.canonicalize().ok())
        .as_deref()
        == Some(exe.as_path())
    {
        if plan.argv[1..] != *original {
            return Err(refuse(
                "SPX-HPH016",
                "wrapper plan adds, drops or reorders arguments of the original command",
            ));
        }
    } else {
        return Err(refuse(
            "SPX-HPH014",
            format!("wrapper plan substitutes executable `{}`; only the original or the provider's granted upstream may run", exe.display()),
        ));
    }
    if let Some(rel) = &plan.cwd {
        let want = project.join(rel).canonicalize().ok();
        if want.as_deref() != Some(cwd) {
            return Err(refuse(
                "SPX-HPH018",
                "wrapper plan changes the working directory",
            ));
        }
    }
    Ok((plan.argv.clone(), exe))
}
