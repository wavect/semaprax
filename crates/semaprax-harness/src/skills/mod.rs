//! Lazy bounded skill catalog (HP-13, `skill.catalog/v1`).
//!
//! Skills are passive, approved-root-only instruction data. The catalog lists
//! cheap metadata, loads bodies lazily by exact digest, and renders them as
//! quoted data below host/compiler authority. See `docs/HARNESS-SKILLS-V1.md`.

pub mod agentskills;
pub mod bundle;
pub mod catalog;
pub mod cli;
pub mod inventory;
pub mod legacy;
pub mod load;
pub mod plain;
pub mod policy;
pub mod resources;
pub mod select;
pub mod snapshot;
pub mod yaml;

pub use catalog::{ApprovedRoot, Catalog, Conflict, SkillEntry};
pub use cli::cli_skills;
pub use load::{ListOutput, ListedSkill, Omitted, PromptOutput, Rendered, SkillService};
pub use plain::PlainSkills;
pub use policy::Warning;
pub use resources::{Activation, Drift, ResourceLoad};
pub use select::{task_tags, NoRecommender, Recommender, Selection};

use std::collections::BTreeSet;

/// Default total model-visible budget (catalog descriptions + loaded content).
pub const DEFAULT_MAX_BYTES: usize = 16_384;
/// Default cap on one progressive resource load.
pub const DEFAULT_MAX_RESOURCE_BYTES: usize = 64 * 1024;
/// Longest description shown in a catalog line.
pub const DESCRIPTION_MAX_CHARS: usize = 120;
/// Most skills chosen by task tags (explicit selections are not capped).
pub const MAX_TAG_SELECTED: usize = 3;

/// Skill catalog configuration. hp02's `[skills]` table maps into this at
/// integration time. `Default` is disabled: skills are strictly opt-in.
#[derive(Clone, Debug)]
pub struct SkillCatalogConfig {
    pub enabled: bool,
    /// Explicit user selection: skill names or exact digests.
    pub select: Vec<String>,
    pub max_bytes: usize,
    /// Host-provided tools that satisfy `requires_host_tool` dependencies
    /// (script file names such as `scripts/x.sh` or tool names).
    pub host_tools: BTreeSet<String>,
    /// Most bytes one resource load may return.
    pub max_resource_bytes: usize,
}

impl Default for SkillCatalogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            select: Vec::new(),
            max_bytes: DEFAULT_MAX_BYTES,
            host_tools: BTreeSet::new(),
            max_resource_bytes: DEFAULT_MAX_RESOURCE_BYTES,
        }
    }
}

pub(crate) fn d(code: &'static str, msg: impl Into<String>) -> crate::diag::HarnessDiagnostic {
    crate::diag::HarnessDiagnostic::new(code, msg)
}
