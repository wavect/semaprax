//! TC-08: opt-in cost-aware skill activation. Offline; no model calls.

use crate::support::*;
use semaprax_harness::skills::cost_profile::{report_for, CostPolicy};
use semaprax_harness::skills::defaults::{DefaultSelection, DefaultSkills, TaskInput};
use semaprax_harness::skills::official::OfficialSet;
use semaprax_harness::skills::task_tags;

const BIG: usize = 64 * 1024;

fn ds(policy: CostPolicy, tag: &str) -> DefaultSkills {
    DefaultSkills::embedded(Some(fixture_dir(tag)), "p", "s")
        .unwrap()
        .with_cost_policy(policy)
}

fn pick(s: &mut DefaultSkills, family: &str, instr: Option<&str>) -> DefaultSelection {
    s.select_for_task(
        &TaskInput {
            family,
            instruction: instr,
        },
        BIG,
    )
    .unwrap()
}

fn sel_ids(s: &DefaultSelection) -> Vec<String> {
    s.reports
        .iter()
        .filter(|r| r.selected)
        .map(|r| r.id.clone())
        .collect()
}

#[test]
fn tc08_tiny_structured_task_skips_automatic_skills_in_compact_only() {
    let std = pick(&mut ds(CostPolicy::default(), "tc08-a"), "mechanical", None);
    assert_eq!(sel_ids(&std), vec!["ponytail"]);
    let mut c = ds(CostPolicy::compact(), "tc08-b");
    let tiny = pick(&mut c, "mechanical", None);
    assert!(tiny.text.is_empty() && sel_ids(&tiny).is_empty());
    let p = tiny.reports.iter().find(|r| r.id == "ponytail").unwrap();
    assert_eq!(
        p.omitted.as_deref(),
        Some("cost-profile:tiny-structured-task")
    );
    assert!(tiny.model_visible_bytes < std.model_visible_bytes);
    // A larger coding task keeps the default behaviour under compact.
    let big = pick(&mut c, "refactor", None);
    assert_eq!(sel_ids(&big), vec!["ponytail"]);
}

#[test]
fn tc08_explicit_selection_uses_pinned_artifact_and_requested_mode() {
    let set = OfficialSet::embedded();
    let skill_md =
        String::from_utf8(set.file_bytes("ponytail", "SKILL.md").unwrap().to_vec()).unwrap();
    let mut c = ds(CostPolicy::compact(), "tc08-c");
    let sel = pick(&mut c, "mechanical", Some("/ponytail lite"));
    assert_eq!(sel_ids(&sel), vec!["ponytail"]);
    assert!(sel.text.contains("active-mode: lite") && sel.text.contains("explicit-instruction"));
    // Every upstream body line is present verbatim (quoted), none shortened.
    let body = skill_md.split("---").nth(2).unwrap_or(&skill_md);
    for line in body.lines().filter(|l| !l.trim().is_empty()) {
        assert!(sel.text.contains(&format!("> {line}")), "missing: {line}");
    }
    let rep = report_for(&set, &CostPolicy::compact(), "mechanical", &sel);
    let d = rep.decisions.iter().find(|d| d.id == "ponytail").unwrap();
    assert_eq!(d.state, "forced");
    assert!(d.snapshot_bytes > 0 && d.rendered_bytes > d.snapshot_bytes - 1000);
}

#[test]
fn tc08_stop_triggers_and_unknown_modes_behave_as_before() {
    for policy in [CostPolicy::default(), CostPolicy::compact()] {
        let mut s = ds(policy, "tc08-d");
        let off = pick(&mut s, "refactor", Some("stop ponytail"));
        assert!(off.text.is_empty() && sel_ids(&off).is_empty());
        let bad = pick(&mut s, "refactor", Some("use ultracave"));
        assert!(bad.diagnostics.iter().any(|d| d.code == "SPX-HPM039"));
        assert!(!bad.text.contains("ultracave mode"));
    }
}

#[test]
fn tc08_host_delivered_skill_is_not_injected_twice_unless_forced() {
    let mut policy = CostPolicy::default();
    policy.host_delivered.insert("ponytail".into());
    let mut s = ds(policy.clone(), "tc08-e");
    let sel = pick(&mut s, "refactor", None);
    assert!(sel.text.is_empty());
    let rep = report_for(&OfficialSet::embedded(), &policy, "refactor", &sel);
    let d = rep.decisions.iter().find(|d| d.id == "ponytail").unwrap();
    assert_eq!(d.state, "already-delivered-by-host");
    assert_eq!(d.rendered_bytes, 0);
    assert!(
        d.snapshot_bytes > 0,
        "snapshot bytes stay distinct from rendered"
    );
    let forced = pick(&mut s, "refactor", Some("/ponytail"));
    assert_eq!(sel_ids(&forced), vec!["ponytail"]);
    assert_eq!(
        forced.text.matches("BEGIN SKILL name=\"ponytail\"").count(),
        1
    );
}

#[test]
fn tc08_report_separates_snapshot_rendered_and_provider_tokens() {
    let set = OfficialSet::embedded();
    let policy = CostPolicy::compact();
    let mut s = ds(policy.clone(), "tc08-f");
    let sel = pick(&mut s, "refactor", None);
    let j = report_for(&set, &policy, "refactor", &sel).to_json();
    assert_eq!(j["profile"], "compact");
    assert_eq!(j["tiny_task"], false);
    assert_eq!(j["catalog_bytes"], 0);
    assert_eq!(j["rendered_bytes"], sel.model_visible_bytes as u64);
    assert!(j["snapshot_bytes"].as_u64().unwrap() > 0);
    assert!(j["provider_input_tokens"].is_null());
    let p = j["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "ponytail")
        .unwrap();
    assert_eq!(p["state"], "selected");
    assert_eq!(p["reason"], "automatic-default");
    assert!(p["digest"].as_str().unwrap().starts_with("sha256:"));
}

#[test]
fn tc08_compact_render_has_no_catalog_and_interactive_render_is_unchanged() {
    let dir = fixture_dir("tc08-root");
    for i in 0..5 {
        super::skill(
            &dir,
            &format!("other-{i}"),
            "Unrelated helper",
            "other",
            "OTHER-BODY",
        );
    }
    super::skill(&dir, "wanted", "Wanted", "api-reuse", "WANTED-BODY");
    let mut s = super::svc(&dir, 16384);
    let full = s.render_prompt(&task_tags("api-reuse"));
    assert!(full.text.starts_with("skill catalog"));
    assert!(full.text.contains("other-0"));
    let c = s.render_prompt_compact(&task_tags("api-reuse"));
    assert!(!c.text.contains("skill catalog") && !c.text.contains("other-0"));
    assert!(c.text.contains("WANTED-BODY") && c.loaded.len() == 1);
    assert!(c.model_visible_bytes < full.model_visible_bytes);
    // Tiny task: no tag matching, nothing loaded without an explicit selection.
    assert!(s.render_prompt_compact(&[]).text.is_empty());
}

#[test]
fn tc08_config_parses_opt_in_keys_and_rejects_unknown_profile() {
    use semaprax_harness::profile::config::parse;
    let head = "schema=\"semaprax.harness-config.v1\"\n";
    assert_eq!(parse(head.as_bytes()).unwrap().skills.cost_profile, None);
    let c = parse(
        format!("{head}[skills]\ncost_profile=\"compact\"\nhost_delivered=[\"ponytail\"]\n")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(c.skills.cost_profile.as_deref(), Some("compact"));
    assert_eq!(c.skills.host_delivered, vec!["ponytail".to_string()]);
    assert!(parse(format!("{head}[skills]\ncost_profile=\"tiny\"\n").as_bytes()).is_err());
}
