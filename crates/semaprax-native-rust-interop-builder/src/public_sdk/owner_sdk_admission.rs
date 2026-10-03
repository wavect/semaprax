//! Replay the authenticated admission status before ownership commitment.
use super::*;
use semaprax::cleanup_plan::{StatusLane, StatusSourceId};

impl Emitter<'_> {
    pub(super) fn admission_fields(&self) -> String {
        if self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .any(|source| source.id.lane == StatusLane::OwnerAdmission)
        {
            format!(
                " uint8_t admission_done[{}]; int32_t admission_status[{}];",
                self.expressions.len(),
                self.expressions.len()
            )
        } else {
            String::new()
        }
    }

    pub(super) fn render_admission(&self, out: &mut String) -> Result<(), Diagnostic> {
        for source in self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .filter(|source| source.id.lane == StatusLane::OwnerAdmission)
        {
            let index = self.index(&source.id.expression)?;
            let commits = self
                .function
                .cleanup_plan
                .blocks
                .iter()
                .flat_map(|block| &block.transitions)
                .filter_map(|transition| match transition {
                    CleanupTransition::CallCommit { call, arguments }
                        if call == &source.id.expression =>
                    {
                        Some(arguments)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [arguments] = commits.as_slice() else {
                return Err(sdk_error(
                    "owner admission requires one authenticated commit",
                ));
            };
            if arguments.is_empty() {
                return Err(sdk_error("owner admission has no owned arguments"));
            }
            writeln!(out, "int32_t spx_admit_{index}(uint64_t context,spx_frame *f) {{ if(f->admission_done[{index}]) return f->admission_status[{index}]; f->admission_done[{index}]=1; int32_t status=0;").unwrap();
            for argument in *arguments {
                let slot = self.slot(&argument.source)?;
                writeln!(out, "if(!f->live[{slot}] || spx_owner_validate(context,f->owners[{slot}])) {{ status=7; goto admission_done; }}").unwrap();
            }
            writeln!(
                out,
                "admission_done: f->admission_status[{index}]=status; return status; }}"
            )
            .unwrap();
        }
        Ok(())
    }

    pub(super) fn status_value(&self, source: &StatusSourceId) -> Result<String, Diagnostic> {
        let index = self.index(&source.expression)?;
        match source.lane {
            StatusLane::OwnerAdmission => Ok(format!("spx_admit_{index}(context,f)")),
            StatusLane::OperationFailure => Ok(format!("spx_eval_{index}(context,f).status")),
            StatusLane::ContractFalse => Err(sdk_error(
                "native owner contract status is outside the closed profile",
            )),
        }
    }
}
