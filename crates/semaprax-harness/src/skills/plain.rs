//! Builtin `semaprax/plain-skills` provider: `skill.catalog/v1` answered
//! in-process by [`SkillService`] over machine-approved roots. No process, no
//! authority; payloads satisfy the contract's `skill.catalog` validators.

use super::{ApprovedRoot, SkillCatalogConfig, SkillService};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::Value;

pub const PROVIDER_ID: &str = "semaprax/plain-skills";

pub struct PlainSkills {
    svc: SkillService,
}

impl PlainSkills {
    pub fn new(roots: Vec<ApprovedRoot>, config: SkillCatalogConfig) -> Self {
        Self {
            svc: SkillService::new(roots, config),
        }
    }

    /// Answer one `list` or `load` request payload with a result payload.
    pub fn handle(&mut self, operation: &str, request: &Value) -> HarnessResult<Value> {
        match operation {
            "list" => Ok(self.svc.list().payload()),
            "load" => {
                let digest = request["digest"].as_str().ok_or_else(|| {
                    HarnessDiagnostic::new("SPX-HPM001", "load needs a skill `digest`")
                })?;
                Ok(self.svc.load(digest)?.payload())
            }
            other => Err(HarnessDiagnostic::new(
                "SPX-HPM001",
                format!("unknown skill.catalog operation `{other}`"),
            )),
        }
    }

    pub fn service(&mut self) -> &mut SkillService {
        &mut self.svc
    }
}
