//! Ordinary profile selection is not authority. The separate provisioned
//! collector reports its opaque settled observation without activating discovery.
use super::{
    append_tool_checks, base_checks, parse, report, validate_host_fact, Check, DoctorError,
    DoctorHost, DoctorOutcome, DoctorTarget,
};

pub(crate) trait OfflineProfileHost {
    fn os(&self) -> &str;
    fn arch(&self) -> &str;
    fn acquire(&self, selector: &str) -> Result<AdmittedProfile<'_>, DoctorError>;
}

/// Holds one admission for the complete report, including every tool check.
/// A selector string alone never constructs a production admission.
pub(crate) struct AdmittedProfile<'a> {
    pub(crate) selector: String,
    pub(crate) host: Box<dyn DoctorHost + 'a>,
}

pub(super) struct RealOfflineProfileHost;

impl OfflineProfileHost for RealOfflineProfileHost {
    fn os(&self) -> &str {
        std::env::consts::OS
    }

    fn arch(&self) -> &str {
        std::env::consts::ARCH
    }

    fn acquire(&self, selector: &str) -> Result<AdmittedProfile<'_>, DoctorError> {
        // Deliberately no environment, cwd, filesystem, registry or process
        // discovery. This ordinary CLI route has no provisioner-owned handoff.
        Err(DoctorError::new(format!(
            "offline profile `{selector}` is unavailable on this host"
        )))
    }
}

pub(super) fn validate_selector(selector: &str) -> Result<(), DoctorError> {
    let bytes = selector.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 64
        || !bytes[0].is_ascii_lowercase()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        return Err(DoctorError::new(
            "invalid doctor profile identifier; expected [a-z][a-z0-9-]{0,63}",
        ));
    }
    Ok(())
}

pub(crate) fn run_with_profile_host(
    arguments: &[String],
    host: &dyn OfflineProfileHost,
) -> Result<DoctorOutcome, DoctorError> {
    // Complete argument validation precedes even host-fact callbacks.
    let options = parse(arguments)?;
    inspect_profile(
        host,
        options.target,
        options.json,
        !cfg!(debug_assertions),
        options.profile.as_deref(),
    )
}

pub(crate) fn inspect_profile(
    host: &dyn OfflineProfileHost,
    target: DoctorTarget,
    json: bool,
    release_build: bool,
    selector: Option<&str>,
) -> Result<DoctorOutcome, DoctorError> {
    if let Some(selector) = selector {
        validate_selector(selector)?;
    }
    let os = host.os();
    let arch = host.arch();
    let mut checks = base_checks(os, arch, release_build)?;
    let Some(selector) = selector else {
        unavailable(
            &mut checks,
            target,
            "an explicit offline profile is required; use --profile <id>",
        );
        return Ok(report(target, json, &checks));
    };
    let admitted = match host.acquire(selector) {
        Ok(admitted) => admitted,
        Err(error) => {
            unavailable(&mut checks, target, &error.to_string());
            return Ok(report(target, json, &checks));
        }
    };
    if admitted.selector != selector {
        return Err(DoctorError::new(
            "offline profile host returned a mismatched selector",
        ));
    }
    let admitted_os = admitted.host.os();
    let admitted_arch = admitted.host.arch();
    validate_host_fact("operating system", admitted_os)?;
    validate_host_fact("architecture", admitted_arch)?;
    if admitted_os != os || admitted_arch != arch {
        return Err(DoctorError::new(
            "offline profile host returned mismatched platform facts",
        ));
    }
    checks.push(Check::ok(
        "profile",
        format!("offline profile `{selector}`; checks describe this profile only"),
    ));
    append_tool_checks(&mut checks, admitted.host.as_ref(), target);
    Ok(report(target, json, &checks))
}

fn unavailable(checks: &mut Vec<Check>, target: DoctorTarget, reason: &str) {
    checks.push(Check::failed("profile", reason));
    let detail = "not probed: no admitted offline profile";
    if matches!(target, DoctorTarget::Native | DoctorTarget::All) {
        checks.push(Check::failed("clang", detail));
    }
    if matches!(target, DoctorTarget::Web | DoctorTarget::All) {
        checks.push(Check::failed("node", detail));
    }
    if matches!(target, DoctorTarget::Contributor | DoctorTarget::All) {
        checks.push(Check::failed("rust", detail));
    }
}
