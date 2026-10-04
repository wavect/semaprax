//! Opt-in cost-aware skill activation (TC-08). The default profile is the
//! existing behaviour. The `compact` profile, for a fixed invocation that cannot
//! load another skill mid-run, (1) emits no discoverable catalog block, (2) does
//! not pay for automatic (shipped-default) skills on tiny structured-intent
//! tasks, and (3) never injects a skill the outer host already delivered.
//! Explicit user selection, configured presets/modes and stop triggers are
//! honoured exactly. Upstream skill bytes are never shortened: a skill is either
//! loaded in full (quoted, with its host policy frame) or not loaded at all.
//! Full digests, versions and provenance stay in the host-side [`CostReport`].

use super::catalog::SkillEntry;
use super::defaults::{DefaultSelection, SkillReport};
use super::load::{Omitted, PromptOutput, SkillService};
use super::official::OfficialSet;
use super::select;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const REPORT_SCHEMA: &str = "semaprax.skill-cost-report/v1";

/// Families treated as tiny structured-intent tasks unless configured otherwise.
pub const DEFAULT_TINY_FAMILIES: &[&str] = &["structured_intent", "mechanical", "mechanical_edit"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CostProfile {
    /// Current behaviour: automatic skills by family, catalog block included.
    #[default]
    Standard,
    /// Opt-in: see the module docs.
    Compact,
}

impl CostProfile {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "standard" => Some(Self::Standard),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Compact => "compact",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CostPolicy {
    pub profile: CostProfile,
    /// Overrides [`DEFAULT_TINY_FAMILIES`] when non-empty.
    pub tiny_families: Vec<String>,
    /// Skill ids the outer host already delivers (bridge-owned injection).
    pub host_delivered: BTreeSet<String>,
}

impl CostPolicy {
    pub fn compact() -> Self {
        Self {
            profile: CostProfile::Compact,
            ..Self::default()
        }
    }

    pub fn is_compact(&self) -> bool {
        self.profile == CostProfile::Compact
    }

    pub fn is_tiny(&self, family: &str) -> bool {
        if self.tiny_families.is_empty() {
            DEFAULT_TINY_FAMILIES.contains(&family)
        } else {
            self.tiny_families.iter().any(|f| f == family)
        }
    }

    /// Automatic (shipped-default) skills are suppressed for a tiny task.
    pub fn suppresses_automatic(&self, family: &str) -> bool {
        self.is_compact() && self.is_tiny(family)
    }

    pub fn delivered_by_host(&self, id: &str) -> bool {
        self.host_delivered.contains(id)
    }
}

/// One skill's decision with its reason and sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    /// `selected`, `forced`, `omitted` or `already-delivered-by-host`.
    pub state: &'static str,
    pub reason: String,
    pub version: String,
    pub digest: String,
    /// Bytes of the pinned artifact files; never what a model saw.
    pub snapshot_bytes: usize,
    /// Bytes actually rendered into the prompt (quoted body plus host frame).
    pub rendered_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostReport {
    pub profile: CostProfile,
    pub family: String,
    pub tiny_task: bool,
    pub decisions: Vec<Decision>,
    /// Total model-visible bytes of the skill block (decisions plus framing).
    pub rendered_bytes: usize,
    /// Model-visible catalog bytes (always 0 for the compact profile).
    pub catalog_bytes: usize,
}

impl CostReport {
    pub fn to_json(&self) -> Value {
        let snap: usize = self.decisions.iter().map(|d| d.snapshot_bytes).sum();
        json!({
            "schema": REPORT_SCHEMA,
            "profile": self.profile.as_str(),
            "task_family": self.family,
            "tiny_task": self.tiny_task,
            "decisions": self.decisions.iter().map(|d| json!({
                "id": d.id, "state": d.state, "reason": d.reason,
                "version": d.version, "digest": d.digest,
                "snapshot_bytes": d.snapshot_bytes, "rendered_bytes": d.rendered_bytes,
            })).collect::<Vec<_>>(),
            "snapshot_bytes": snap,
            "rendered_bytes": self.rendered_bytes,
            "catalog_bytes": self.catalog_bytes,
            "provider_input_tokens": null,
            "tokens_note": "bytes are not tokens; provider input tokens come from the provider observation",
        })
    }
}

fn reason_for(r: &SkillReport) -> (&'static str, String) {
    if let Some(o) = &r.omitted {
        let state = if o == "already-delivered-by-host" {
            "already-delivered-by-host"
        } else {
            "omitted"
        };
        return (state, o.clone());
    }
    if let Some(dis) = &r.disabled {
        return ("omitted", dis.clone());
    }
    if r.selected {
        return match r.source {
            "explicit-instruction" => ("forced", "explicit-user-instruction".into()),
            super::modes::SHIPPED => ("selected", "automatic-default".into()),
            s => ("selected", format!("configured:{s}")),
        };
    }
    ("omitted", "not-selected".into())
}

/// Build the host-side report for a default-skill selection.
pub fn report_for(
    set: &OfficialSet,
    policy: &CostPolicy,
    family: &str,
    sel: &DefaultSelection,
) -> CostReport {
    let decisions = sel
        .reports
        .iter()
        .map(|r| {
            let (state, reason) = reason_for(r);
            let snapshot_bytes = set
                .find(&r.id)
                .map(|k| k.files.iter().map(|f| f.bytes as usize).sum())
                .unwrap_or(0);
            Decision {
                id: r.id.clone(),
                state,
                reason,
                version: r.version.clone(),
                digest: r.digest.clone(),
                snapshot_bytes,
                rendered_bytes: r.model_visible_bytes,
            }
        })
        .collect();
    CostReport {
        profile: policy.profile,
        family: family.into(),
        tiny_task: policy.is_tiny(family),
        decisions,
        rendered_bytes: sel.model_visible_bytes,
        catalog_bytes: 0,
    }
}

impl SkillService {
    /// Prompt section for a fixed invocation: selected skills only, no catalog
    /// block. Selection is `select::select` over the same snapshot as
    /// `render_prompt`; the omitted/unresolved bookkeeping is identical.
    pub fn render_prompt_compact(&mut self, tags: &[String]) -> PromptOutput {
        let budget = self.config.max_bytes;
        let mut out = PromptOutput {
            budget,
            ..PromptOutput::default()
        };
        let mut cat = self.scan_catalog();
        for (name, a) in &self.active {
            cat.entries
                .retain(|e| &e.name != name || e.digest == a.digest);
            if !cat.entries.iter().any(|e| e.digest == a.digest) {
                cat.entries.push(a.entry.clone());
            }
        }
        cat.entries
            .sort_by(|a, b| (&a.name, &a.digest).cmp(&(&b.name, &b.digest)));
        out.diagnostics = cat.diagnostics.clone();
        self.snapshot = cat
            .entries
            .iter()
            .map(|e| (e.digest.clone(), e.name.clone()))
            .collect();
        if !self.config.enabled || cat.entries.is_empty() {
            return out;
        }
        let sel = select::select(&cat, tags, &self.config.select);
        out.unresolved = sel.unresolved.clone();
        for e in cat.entries.iter().filter(|e| e.conflict) {
            out.omitted.push(Omitted {
                name: e.name.clone(),
                digest: e.digest.clone(),
                reason: "conflict",
            });
        }
        let mut text = String::new();
        let picked: Vec<SkillEntry> = sel
            .selected
            .iter()
            .map(|i| cat.entries[*i].clone())
            .collect();
        for e in &picked {
            match self.load(&e.digest) {
                Ok(r) if text.len() + r.text.len() <= budget => {
                    text.push_str(&r.text);
                    out.loaded.push((r.name.clone(), r.digest.clone()));
                    out.warnings
                        .extend(r.warnings.iter().map(|w| (r.name.clone(), w.clone())));
                }
                Ok(_) => {
                    out.omitted.push(Omitted {
                        name: e.name.clone(),
                        digest: e.digest.clone(),
                        reason: "content-budget",
                    });
                    out.diagnostics.push(super::d(
                        "SPX-HPM009",
                        format!(
                            "skill `{}` ({} bytes) does not fit the remaining budget",
                            e.name, e.bytes
                        ),
                    ));
                }
                Err(err) => out.diagnostics.push(err),
            }
        }
        out.model_visible_bytes = text.len();
        out.text = text;
        out
    }
}
