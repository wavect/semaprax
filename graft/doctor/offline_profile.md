# doctor/offline_profile.rs

- OfflineProfileHost · interface · L8-L12 — pub trait OfflineProfileHost
- os · function · L9-L9 — fn os(&self) -> &str;
- arch · function · L10-L10 — fn arch(&self) -> &str;
- acquire · function · L11-L11 — fn acquire(&self, selector: &str) -> Result<AdmittedProfile<'_>, DoctorError>;
- AdmittedProfile · struct · L16-L19 — pub struct AdmittedProfile<'a>
- RealOfflineProfileHost · struct · L21-L21 — pub(super) struct RealOfflineProfileHost;
- os · function · L24-L26 — fn os(&self) -> &str
- arch · function · L28-L30 — fn arch(&self) -> &str
- acquire · function · L32-L38 — fn acquire(&self, selector: &str) -> Result<AdmittedProfile<'_>, DoctorError>
- validate_selector · function · L41-L55 — pub(super) fn validate_selector(selector: &str) -> Result<(), DoctorError>
- run_with_profile_host · function · L57-L70 — pub fn run_with_profile_host(
- inspect_profile · function · L72-L120 — pub fn inspect_profile(
- unavailable · function · L122-L134 — fn unavailable(checks: &mut Vec<Check>, target: DoctorTarget, reason: &str)
