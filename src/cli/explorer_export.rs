//! Deterministic, source-free review projections for an authenticated Explorer snapshot.
//!
//! This module deliberately accepts the already-authenticated snapshot object, not a path or
//! arbitrary report bytes.  The caller owns source authentication and destination publication;
//! the renderer has no filesystem, process, network, or source authority.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const SNAPSHOT_SCHEMA: &str = "semaprax.explorer-snapshot.v1";
const VIEW_SCHEMA: &str = "semaprax.explorer-view.v1";
const MAX_EXPORT_NODES: usize = 256;
const MAX_DISPLAY_STRING_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Format {
    Markdown,
    Svg,
}

impl Format {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "markdown" => Some(Self::Markdown),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }
}

/// Renders one complete, compiler-authenticated Explorer snapshot.
///
/// The result has no source snippets and contains no executable SVG/HTML constructs.  Its digest
/// identifies the canonical snapshot bytes only; callers must not present it as provenance,
/// approval, freshness, or a candidate object.
pub(crate) fn render(snapshot: &Value, format: Format) -> Result<Vec<u8>, &'static str> {
    let projection = Projection::parse(snapshot)?;
    let text = match format {
        Format::Markdown => projection.markdown(),
        Format::Svg => projection.svg()?,
    };
    Ok(text.into_bytes())
}

struct Projection<'a> {
    snapshot_digest: String,
    subject: &'a Value,
    base_subject: Option<&'a Value>,
    mode: &'a str,
    target: Option<&'a str>,
    truncated: bool,
    truncation_reason: Option<&'a str>,
    declarations: Vec<&'a Value>,
    relations: Vec<&'a Value>,
    frontier_count: usize,
    changed_roots: Vec<&'a Value>,
    evidence_availability: &'a str,
    evidence_states: Vec<String>,
}

impl<'a> Projection<'a> {
    fn parse(snapshot: &'a Value) -> Result<Self, &'static str> {
        let envelope = object(snapshot, "explorer export requires a snapshot object")?;
        let mut fields = vec![
            "schema",
            "generator",
            "views",
            "focus",
            "focus_sides",
            "snapshot_digest",
            "source_included",
            "evidence_availability",
            "confidentiality",
        ];
        if snapshot.get("evidence").is_some() {
            fields.push("evidence");
        }
        if snapshot.get("changes").is_some() {
            fields.push("changes");
        }
        closed_keys(
            envelope,
            &fields,
            "explorer export snapshot has unsupported fields",
        )?;
        if snapshot.get("snapshot_digest").and_then(Value::as_str)
            != Some(snapshot_digest(snapshot).as_str())
        {
            return Err("explorer snapshot digest does not match canonical content");
        }
        required_str(
            snapshot,
            "schema",
            SNAPSHOT_SCHEMA,
            "unsupported explorer snapshot schema",
        )?;
        if snapshot.get("source_included") != Some(&Value::Bool(false)) {
            return Err("explorer export only admits source-free snapshots");
        }
        required_str(
            snapshot,
            "confidentiality",
            "names_ids_and_paths_may_be_confidential",
            "explorer snapshot confidentiality label is invalid",
        )?;
        required_str(
            snapshot,
            "generator",
            "semaprax explore",
            "explorer snapshot generator is invalid",
        )?;
        match snapshot
            .get("evidence_availability")
            .and_then(Value::as_str)
        {
            Some("not_bundled") if snapshot.get("evidence").is_none() => {}
            Some("not_requested") if snapshot.get("evidence").is_some() => {}
            Some("available") if snapshot.get("evidence").is_some() => {}
            _ => return Err("explorer snapshot evidence availability is invalid"),
        }
        let focus = snapshot.get("focus").and_then(Value::as_str);
        if !snapshot
            .get("focus")
            .is_some_and(|value| value.is_null() || value.is_string())
        {
            return Err("explorer snapshot focus is invalid");
        }
        let views = snapshot
            .get("views")
            .and_then(Value::as_array)
            .ok_or("explorer snapshot views are invalid")?;
        if let Some(evidence) = snapshot.get("evidence") {
            validate_evidence(evidence, focus, views)?;
        }
        let focus_sides = snapshot
            .get("focus_sides")
            .and_then(Value::as_array)
            .ok_or("explorer snapshot focus sides are invalid")?;
        if focus_sides.len() > 2 || focus_sides.iter().any(|side| !side.is_string()) {
            return Err("explorer snapshot focus sides are invalid");
        }
        let sides = views
            .iter()
            .map(|view| {
                view.get("query")
                    .and_then(|query| query.get("side"))
                    .and_then(Value::as_str)
                    .ok_or("explorer snapshot side is invalid")
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if !(sides == BTreeSet::from(["current"]) || sides == BTreeSet::from(["base", "candidate"]))
            || views.len() != sides.len() + focus_sides.len()
        {
            return Err("explorer snapshot views are incomplete");
        }
        if focus.is_none() && !focus_sides.is_empty() || focus.is_some() && focus_sides.is_empty() {
            return Err("explorer snapshot focus availability is invalid");
        }
        let selected_side_set = focus_sides
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        if selected_side_set.len() != focus_sides.len()
            || selected_side_set.iter().any(|side| !sides.contains(side))
        {
            return Err("explorer snapshot focus side is invalid");
        }
        let selected_views = views
            .iter()
            .filter(|view| {
                view.get("query")
                    .and_then(|query| query.get("target"))
                    .and_then(Value::as_str)
                    == focus
            })
            .collect::<Vec<_>>();
        let overview_count = views
            .iter()
            .filter(|view| {
                view.get("query")
                    .and_then(|query| query.get("target"))
                    .is_some_and(Value::is_null)
            })
            .count();
        if selected_views.len()
            != if focus.is_some() {
                focus_sides.len()
            } else {
                sides.len()
            }
            || overview_count != sides.len()
        {
            return Err("explorer snapshot selected views are invalid");
        }
        let selected = selected_views
            .iter()
            .copied()
            .find(|view| {
                matches!(
                    view.get("query")
                        .and_then(|query| query.get("side"))
                        .and_then(Value::as_str),
                    Some("candidate" | "current")
                )
            })
            .or_else(|| selected_views.first().copied())
            .ok_or("explorer snapshot has no selected view")?;
        let base_subject = views
            .iter()
            .find(|view| {
                view.get("query")
                    .and_then(|query| query.get("side"))
                    .and_then(Value::as_str)
                    == Some("base")
            })
            .and_then(|view| view.get("summary"))
            .and_then(|summary| summary.get("subject"))
            .filter(|subject| subject.is_object());
        let selected_fields = object(selected, "explorer snapshot view is invalid")?;
        closed_keys(
            selected_fields,
            &["query", "summary", "pages"],
            "explorer snapshot view has unsupported fields",
        )?;
        let query = selected
            .get("query")
            .filter(|value| value.is_object())
            .ok_or("explorer snapshot query is invalid")?;
        if query.get("target").and_then(Value::as_str) != focus {
            return Err("explorer snapshot selected view does not bind focus");
        }
        let summary = selected
            .get("summary")
            .ok_or("explorer snapshot has no summary")?;
        let summary_object = object(summary, "explorer snapshot summary is invalid")?;
        closed_keys(
            summary_object,
            &[
                "schema",
                "kind",
                "subject",
                "mode",
                "target",
                "query",
                "artifact_digest",
                "truncation",
                "coverage",
                "inventories",
                "source_authority",
                "execution",
                "publication_authority",
                "nonclaims",
            ],
            "explorer summary has unsupported fields",
        )?;
        required_str(
            summary,
            "schema",
            VIEW_SCHEMA,
            "explorer summary schema is invalid",
        )?;
        required_str(
            summary,
            "kind",
            "summary",
            "explorer summary kind is invalid",
        )?;
        require_false(summary, "source_authority")?;
        require_false(summary, "execution")?;
        require_false(summary, "publication_authority")?;
        let subject = summary
            .get("subject")
            .filter(|value| value.is_object())
            .ok_or("explorer summary subject is invalid")?;
        let mut changed_roots = Vec::new();
        if let Some(changes) = snapshot.get("changes") {
            if !sides.contains("candidate") {
                return Err("changes require a candidate snapshot");
            }
            let changes_object = object(changes, "explorer changes are invalid")?;
            closed_keys(
                changes_object,
                &["catalog", "details"],
                "explorer changes have unsupported fields",
            )?;
            if !changes
                .get("details")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
            {
                return Err("explorer export requires compact change details only");
            }
            let catalog = changes
                .get("catalog")
                .ok_or("explorer change catalog is absent")?;
            required_str(
                catalog,
                "schema",
                "semaprax.project-candidate-semantic-delta-catalog.v1",
                "explorer change catalog schema is invalid",
            )?;
            if catalog.get("candidate_digest") != subject.get("candidate_revision") {
                return Err("explorer change catalog belongs to another candidate");
            }
            let roots = catalog
                .get("roots")
                .and_then(Value::as_array)
                .ok_or("explorer change roots are invalid")?;
            if roots.len() > 65_536 {
                return Err("explorer change roots exceed their bound");
            }
            for root in roots {
                let row = object(root, "explorer change root is invalid")?;
                closed_keys(
                    row,
                    &["target", "change", "base", "candidate"],
                    "explorer change root fields are invalid",
                )?;
                string(root, "target", "explorer change target is invalid")?;
                if !["added", "removed", "modified", "moved"].contains(&string(
                    root,
                    "change",
                    "explorer change category is invalid",
                )?) {
                    return Err("explorer change category is invalid");
                }
                changed_roots.push(root);
            }
        }
        let mode = string(summary, "mode", "explorer summary mode is invalid")?;
        let target = summary.get("target").and_then(Value::as_str);
        if !summary
            .get("target")
            .is_some_and(|value| value.is_null() || value.is_string())
        {
            return Err("explorer summary target is invalid");
        }
        if query.get("mode").and_then(Value::as_str) != Some(mode)
            || query.get("target") != summary.get("target")
        {
            return Err("explorer snapshot query does not bind summary");
        }
        let truncation = summary
            .get("truncation")
            .and_then(Value::as_object)
            .ok_or("explorer summary truncation is invalid")?;
        if mode == "overview" {
            closed_keys(
                truncation,
                &["truncated", "reason"],
                "explorer truncation has unsupported fields",
            )?;
        } else {
            closed_keys(
                truncation,
                &[
                    "truncated",
                    "reasons",
                    "omitted_known_nodes",
                    "deferred_known_nodes",
                ],
                "explorer truncation has unsupported fields",
            )?;
            natural(
                summary.get("truncation").unwrap(),
                "omitted_known_nodes",
                "explorer omitted count is invalid",
            )?;
            natural(
                summary.get("truncation").unwrap(),
                "deferred_known_nodes",
                "explorer deferred count is invalid",
            )?;
            let reasons = truncation
                .get("reasons")
                .and_then(Value::as_array)
                .ok_or("explorer truncation reasons are invalid")?;
            if reasons.len() > 64 || reasons.iter().any(|reason| !reason.is_string()) {
                return Err("explorer truncation reasons are invalid");
            }
        }
        let truncated = truncation
            .get("truncated")
            .and_then(Value::as_bool)
            .ok_or("explorer truncation flag is invalid")?;
        let truncation_reason = truncation
            .get("reason")
            .and_then(Value::as_str)
            .or_else(|| {
                truncation
                    .get("reasons")
                    .and_then(Value::as_array)
                    .and_then(|reasons| reasons.first())
                    .and_then(Value::as_str)
            });
        let coverage = summary
            .get("coverage")
            .filter(|value| value.is_object())
            .ok_or("explorer summary coverage is invalid")?;

        let inventories = summary
            .get("inventories")
            .and_then(Value::as_array)
            .ok_or("explorer summary inventories are invalid")?;
        let mut inventory_totals = BTreeMap::new();
        for row in inventories {
            let object = object(row, "explorer inventory is invalid")?;
            closed_keys(
                object,
                &["view", "handle", "total_items"],
                "explorer inventory has unsupported fields",
            )?;
            let view = string(row, "view", "explorer inventory view is invalid")?;
            if !matches!(view, "modules" | "declarations" | "relations" | "frontier")
                || inventory_totals
                    .insert(
                        view,
                        natural(row, "total_items", "explorer inventory count is invalid")?,
                    )
                    .is_some()
            {
                return Err("explorer inventories are not the closed canonical set");
            }
            string(row, "handle", "explorer inventory handle is invalid")?;
        }
        if inventory_totals.len() != 4 {
            return Err("explorer inventories are incomplete");
        }

        let pages = selected
            .get("pages")
            .and_then(Value::as_array)
            .ok_or("explorer snapshot pages are invalid")?;
        let mut page_items = BTreeMap::<&str, Vec<&Value>>::new();
        let mut page_offsets = BTreeMap::<&str, usize>::new();
        let mut cursors = BTreeSet::new();
        for page in pages {
            let fields = object(page, "explorer snapshot page is invalid")?;
            closed_keys(
                fields,
                &[
                    "schema",
                    "kind",
                    "subject",
                    "mode",
                    "target",
                    "query",
                    "artifact_digest",
                    "truncation",
                    "coverage",
                    "view",
                    "handle",
                    "cursor",
                    "offset",
                    "total_items",
                    "page_size",
                    "max_bytes",
                    "next_cursor",
                    "items",
                    "source_authority",
                    "execution",
                    "publication_authority",
                    "nonclaims",
                ],
                "explorer page has unsupported fields",
            )?;
            required_str(
                page,
                "schema",
                VIEW_SCHEMA,
                "explorer page schema is invalid",
            )?;
            required_str(page, "kind", "page", "explorer page kind is invalid")?;
            require_false(page, "source_authority")?;
            require_false(page, "execution")?;
            require_false(page, "publication_authority")?;
            if page.get("subject") != Some(subject)
                || page.get("mode").and_then(Value::as_str) != Some(mode)
                || page.get("target") != summary.get("target")
                || page.get("truncation") != summary.get("truncation")
                || page.get("coverage") != Some(coverage)
                || page.get("query") != summary.get("query")
            {
                return Err("explorer page does not bind the selected snapshot");
            }
            let view = string(page, "view", "explorer page view is invalid")?;
            let items = page
                .get("items")
                .and_then(Value::as_array)
                .ok_or("explorer page items are invalid")?;
            let offset = natural(page, "offset", "explorer page offset is invalid")?;
            let expected_offset = page_offsets.get(view).copied().unwrap_or(0);
            let total = natural(page, "total_items", "explorer page count is invalid")?;
            if inventory_totals.get(view) != Some(&total) || offset != expected_offset {
                return Err("explorer page is partial or out of order");
            }
            let next = page.get("next_cursor");
            if offset == 0 && page.get("cursor") != Some(&Value::Null) {
                return Err("explorer first page cursor is invalid");
            }
            if offset != 0 && page.get("cursor").and_then(Value::as_str).is_none() {
                return Err("explorer continued page cursor is invalid");
            }
            let end = offset
                .checked_add(items.len())
                .ok_or("explorer page count overflows")?;
            if let Some(cursor) = next.and_then(Value::as_str) {
                if end >= total {
                    return Err("explorer page cursor continues past its inventory");
                }
                if !cursors.insert(cursor) {
                    return Err("explorer page cursor repeats");
                }
            } else if next != Some(&Value::Null) || end != total {
                return Err("explorer next cursor is invalid");
            }
            page_offsets.insert(view, end);
            page_items.entry(view).or_default().extend(items.iter());
        }
        let declarations = page_items
            .remove("declarations")
            .ok_or("explorer declarations page is missing")?
            .into_iter()
            .collect::<Vec<_>>();
        let relations = page_items
            .remove("relations")
            .ok_or("explorer relations page is missing")?
            .into_iter()
            .collect::<Vec<_>>();
        let frontier_count = page_items
            .remove("frontier")
            .ok_or("explorer frontier page is missing")?
            .len();
        if page_items.remove("modules").is_none()
            || !page_items.is_empty()
            || page_offsets
                .iter()
                .any(|(view, offset)| inventory_totals.get(view) != Some(offset))
            || declarations.len() > MAX_EXPORT_NODES
        {
            return Err("explorer snapshot is too large or incomplete for review export");
        }
        validate_display_data(&declarations)?;
        validate_display_data(&relations)?;
        let evidence_availability = snapshot["evidence_availability"]
            .as_str()
            .ok_or("explorer snapshot evidence availability is invalid")?;
        let evidence_states = snapshot["evidence"]["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| {
                let side = entry["subject"]["side"].as_str().unwrap_or("unknown");
                let states = [
                    "function_summary",
                    "dependency_summary",
                    "analysis_coverage",
                    "contract_delta",
                    "ownership_delta",
                ]
                .into_iter()
                .filter_map(|key| entry["states"][key].as_str().map(|state| (key, state)))
                .map(|(key, state)| format!("{key}={state}"))
                .collect::<Vec<_>>()
                .join(", ");
                format!("{side}: {states}")
            })
            .collect();
        Ok(Self {
            snapshot_digest: snapshot_digest(snapshot),
            subject,
            base_subject,
            mode,
            target,
            truncated,
            truncation_reason,
            declarations,
            relations,
            frontier_count,
            changed_roots,
            evidence_availability,
            evidence_states,
        })
    }

    fn markdown(&self) -> String {
        let mut out = String::from("# SEMAPRAX semantic review\n\n");
        out.push_str("This source-free review artifact may contain confidential names, identifiers, and relative paths.\n\n");
        out.push_str("## Scope\n\n");
        out.push_str(&format!("- Integrity identity: `{}` (canonical snapshot byte identity; not provenance, approval, or freshness)\n", self.snapshot_digest));
        out.push_str(&format!(
            "- Project revision: `{}`\n",
            inline(self.subject, "project_revision")
        ));
        if let Some(base) = self.base_subject {
            out.push_str(&format!(
                "- Base project revision: `{}`\n",
                inline(base, "project_revision")
            ));
            out.push_str(&format!(
                "- Candidate revision: `{}`\n",
                inline(self.subject, "candidate_revision")
            ));
        }
        out.push_str(&format!(
            "- Workspace revision: `{}`\n",
            inline(self.subject, "workspace_revision")
        ));
        out.push_str(&format!(
            "- Graph identity: `{}`\n",
            inline(self.subject, "project_graph_digest")
        ));
        out.push_str(&format!("- Side: `{}`\n", inline(self.subject, "side")));
        out.push_str(&format!("- View mode: `{}`\n", markdown_escape(self.mode)));
        if let Some(target) = self.target {
            out.push_str(&format!(
                "- Selected target: `{}`\n",
                markdown_escape(target)
            ));
        }
        out.push_str(&format!(
            "- Scope status: {}\n",
            if self.truncated {
                "incomplete"
            } else {
                "complete within the retained compiler view"
            }
        ));
        if let Some(reason) = self.truncation_reason.filter(|_| self.truncated) {
            out.push_str(&format!(
                "- Incomplete reason: {}\n",
                markdown_escape(reason)
            ));
        }
        out.push_str(&format!(
            "- Loaded structural relation sites on {} side: {}\n",
            inline(self.subject, "side"),
            self.relations.len()
        ));
        out.push_str(&format!(
            "- Potential dependents: {}\n",
            if self.mode == "impact" {
                format!(
                    "{} retained relation sites within selected impact query",
                    self.relations.len()
                )
            } else {
                "not bundled as an impact query".to_owned()
            }
        ));
        out.push_str(&format!(
            "- Changed declarations: {}\n",
            if self.base_subject.is_some() {
                self.changed_roots.len().to_string()
            } else {
                "not applicable to an image snapshot".to_owned()
            }
        ));
        out.push_str(&format!(
            "- Hidden frontier entries: {}\n",
            self.frontier_count
        ));
        out.push_str(&format!("- Tests run: unavailable in this snapshot\n\n"));
        out.push_str("## Bundled evidence\n\n");
        out.push_str(&format!(
            "- Evidence availability: `{}`\n",
            markdown_escape(self.evidence_availability)
        ));
        if self.evidence_states.is_empty() {
            out.push_str("- Focused compiler reports: not requested or not bundled\n\n");
        } else {
            for state in &self.evidence_states {
                out.push_str(&format!("- {}\n", markdown_escape(state)));
            }
            out.push('\n');
        }
        if self.base_subject.is_some() {
            let mut counts = BTreeMap::new();
            for row in &self.changed_roots {
                *counts.entry(inline(row, "change")).or_insert(0usize) += 1;
            }
            out.push_str("## Changed declarations\n\n");
            out.push_str(&format!("Categories: added {}, removed {}, modified {}, moved {}. A move can also include changed facts; inspect the selected compiler delta.\n\n", counts.get("added").copied().unwrap_or(0), counts.get("removed").copied().unwrap_or(0), counts.get("modified").copied().unwrap_or(0), counts.get("moved").copied().unwrap_or(0)));
            out.push_str("| Stable ID | Kind | Status | Base path | Candidate path |\n| --- | --- | --- | --- | --- |\n");
            for row in &self.changed_roots {
                let base = row.get("base").filter(|value| value.is_object());
                let candidate = row.get("candidate").filter(|value| value.is_object());
                out.push_str(&format!(
                    "| `{}` | {} | {} | `{}` | `{}` |\n",
                    markdown_escape(inline(row, "target")),
                    markdown_escape(
                        candidate
                            .or(base)
                            .map(|value| inline(value, "kind"))
                            .unwrap_or("unavailable")
                    ),
                    markdown_escape(inline(row, "change")),
                    markdown_escape(base.map(|value| inline(value, "path")).unwrap_or("—")),
                    markdown_escape(candidate.map(|value| inline(value, "path")).unwrap_or("—"))
                ));
            }
            out.push('\n');
        }
        out.push_str("## Declarations in selected scope\n\n");
        out.push_str("| Stable ID | Kind | Path |\n| --- | --- | --- |\n");
        for declaration in &self.declarations {
            out.push_str(&format!(
                "| `{}` | {} | `{}` |\n",
                markdown_escape(required_display(declaration, "id")),
                markdown_escape(required_display(declaration, "kind")),
                markdown_escape(optional_display(declaration, "path")),
            ));
        }
        out.push_str("\n## Limitations\n\n");
        out.push_str("This view reports compiler structure only. It does not claim behavioral equivalence, runtime coverage, deployment risk, source authority, execution authority, publication approval, or external provenance. Test execution evidence is not bundled.\n");
        out
    }

    fn svg(&self) -> Result<String, &'static str> {
        let target = self
            .target
            .ok_or("SVG export requires a declaration target")?;
        let width = 1100usize;
        let height = 190usize
            .checked_add(
                self.declarations
                    .len()
                    .checked_mul(72)
                    .ok_or("SVG export size overflow")?,
            )
            .ok_or("SVG export size overflow")?;
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">SEMAPRAX dependency slice for {}</title><desc id=\"desc\">Target {}, side {}, integrity identity {}. {}</desc><rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/><text x=\"28\" y=\"34\" font-family=\"sans-serif\" font-size=\"20\" fill=\"#17202a\">SEMAPRAX dependency slice</text><text x=\"28\" y=\"60\" font-family=\"sans-serif\" font-size=\"13\" fill=\"#34495e\">target: {} · side: {} · scope: {}</text><text x=\"28\" y=\"84\" font-family=\"sans-serif\" font-size=\"11\" fill=\"#566573\">integrity identity: {}</text><text x=\"28\" y=\"108\" font-family=\"sans-serif\" font-size=\"11\" fill=\"#566573\">project revision: {} · candidate revision: {}</text><text x=\"28\" y=\"130\" font-family=\"sans-serif\" font-size=\"11\" fill=\"#566573\">changed declarations: {} · blue: declaration · gray: potential relationship · {} hidden frontier entries</text>",
            xml_escape(target),
            xml_escape(target),
            xml_escape(inline(self.subject, "side")),
            xml_escape(&self.snapshot_digest),
            if self.truncated {
                "incomplete selected scope"
            } else {
                "complete within retained compiler view"
            },
            xml_escape(target),
            xml_escape(inline(self.subject, "side")),
            if self.truncated {
                "incomplete"
            } else {
                "complete within retained compiler view"
            },
            xml_escape(&self.snapshot_digest),
            xml_escape(inline(self.subject, "project_revision")),
            xml_escape(inline(self.subject, "candidate_revision")),
            self.changed_roots.len(),
            self.frontier_count,
        );
        let mut keys = BTreeMap::new();
        out.push_str("<defs><marker id=\"spx-arrow\" markerWidth=\"8\" markerHeight=\"8\" refX=\"7\" refY=\"4\" orient=\"auto\"><path d=\"M 0 0 L 8 4 L 0 8\" fill=\"none\" stroke=\"#566573\"/></marker></defs>");
        for (index, declaration) in self.declarations.iter().enumerate() {
            keys.insert(required_display(declaration, "node_key").to_owned(), index);
        }
        for relation in &self.relations {
            let from = optional_display(relation, "from");
            let to = optional_display(relation, "to");
            if let (Some(from), Some(to)) = (keys.get(from), keys.get(to)) {
                let y1 = 176 + from * 72 + 28;
                let y2 = 176 + to * 72 + 28;
                let family = xml_escape(optional_display(relation, "family"));
                out.push_str(&format!("<path d=\"M 840 {y1} L 930 {y2}\" stroke=\"#566573\" stroke-width=\"1\" fill=\"none\" marker-end=\"url(#spx-arrow)\"><title>Potential {family} relationship</title></path>"));
            }
        }
        for (index, declaration) in self.declarations.iter().enumerate() {
            let y = 176 + index * 72;
            out.push_str(&format!("<rect x=\"28\" y=\"{y}\" width=\"812\" height=\"54\" rx=\"5\" fill=\"#eaf2f8\" stroke=\"#2874a6\"/><text x=\"44\" y=\"{}\" font-family=\"sans-serif\" font-size=\"14\" fill=\"#17202a\">{}</text><text x=\"44\" y=\"{}\" font-family=\"sans-serif\" font-size=\"11\" fill=\"#34495e\">{} · {}</text>", y + 22, xml_escape(required_display(declaration, "display_name")), y + 41, xml_escape(required_display(declaration, "kind")), xml_escape(optional_display(declaration, "path"))));
        }
        out.push_str("</svg>");
        Ok(out)
    }
}

fn object<'a>(
    value: &'a Value,
    error: &'static str,
) -> Result<&'a serde_json::Map<String, Value>, &'static str> {
    value.as_object().ok_or(error)
}

fn closed_keys(
    value: &serde_json::Map<String, Value>,
    expected: &[&str],
    error: &'static str,
) -> Result<(), &'static str> {
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    (actual == expected).then_some(()).ok_or(error)
}

fn validate_evidence(
    evidence: &Value,
    focus: Option<&str>,
    views: &[Value],
) -> Result<(), &'static str> {
    let fields = object(evidence, "explorer evidence index is invalid")?;
    closed_keys(
        fields,
        &["schema", "entries"],
        "explorer evidence index has unsupported fields",
    )?;
    required_str(
        evidence,
        "schema",
        "semaprax.explorer-evidence-index.v1",
        "explorer evidence schema is invalid",
    )?;
    let entries = evidence
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("explorer evidence entries are invalid")?;
    let contexts = views
        .iter()
        .filter(|view| view["query"]["mode"] == "context")
        .collect::<Vec<_>>();
    if entries.len() > 2
        || (focus.is_none() && !entries.is_empty())
        || (focus.is_some() && entries.len() != contexts.len())
    {
        return Err("explorer evidence entry count is invalid");
    }
    for entry in entries {
        let fields = object(entry, "explorer evidence entry is invalid")?;
        closed_keys(
            fields,
            &["subject", "target", "states", "compact", "omitted"],
            "explorer evidence entry has unsupported fields",
        )?;
        if entry.get("target").and_then(Value::as_str) != focus || focus.is_none() {
            return Err("explorer evidence target does not bind focus");
        }
        let subject = entry
            .get("subject")
            .filter(|value| value.is_object())
            .ok_or("explorer evidence subject is invalid")?;
        if !contexts.iter().any(|view| {
            view["query"]["target"] == entry["target"] && view["summary"]["subject"] == *subject
        }) {
            return Err("explorer evidence subject is not bound to a selected context view");
        }
        let states = object(
            entry
                .get("states")
                .ok_or("explorer evidence states are missing")?,
            "explorer evidence states are invalid",
        )?;
        let compact = object(
            entry
                .get("compact")
                .ok_or("explorer evidence compact record is missing")?,
            "explorer evidence compact record is invalid",
        )?;
        let allowed = [
            "function_summary",
            "dependency_summary",
            "analysis_coverage",
            "contract_delta",
            "ownership_delta",
        ];
        let allowed = allowed.into_iter().collect::<BTreeSet<_>>();
        if states.keys().any(|key| !allowed.contains(key.as_str())) {
            return Err("explorer evidence states have unsupported fields");
        }
        if compact.keys().any(|key| !allowed.contains(key.as_str())) {
            return Err("explorer evidence compact record has unsupported fields");
        }
        for slot in [
            "function_summary",
            "dependency_summary",
            "analysis_coverage",
        ] {
            if !states.contains_key(slot) || !compact.contains_key(slot) {
                return Err("explorer evidence required slot is missing");
            }
        }
        for (slot, state) in states {
            if !matches!(
                state.as_str(),
                Some("available" | "not applicable" | "error")
            ) || !compact.contains_key(slot)
            {
                return Err("explorer evidence state or compact slot is invalid");
            }
            if (state == "available" && !compact[slot].is_object())
                || (state != "available" && !compact[slot].is_null())
            {
                return Err("explorer evidence state does not match its compact slot");
            }
        }
        if states.len() != compact.len() {
            return Err("explorer evidence state and compact slots differ");
        }
        if !entry
            .get("omitted")
            .and_then(Value::as_array)
            .is_some_and(|omitted| omitted.iter().all(Value::is_string))
        {
            return Err("explorer evidence omission inventory is invalid");
        }
    }
    Ok(())
}

fn string<'a>(value: &'a Value, key: &str, error: &'static str) -> Result<&'a str, &'static str> {
    let value = value.get(key).and_then(Value::as_str).ok_or(error)?;
    (value.len() <= MAX_DISPLAY_STRING_BYTES)
        .then_some(value)
        .ok_or(error)
}

fn required_str(
    value: &Value,
    key: &str,
    expected: &str,
    error: &'static str,
) -> Result<(), &'static str> {
    (string(value, key, error)? == expected)
        .then_some(())
        .ok_or(error)
}

fn natural(value: &Value, key: &str, error: &'static str) -> Result<usize, &'static str> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(error)
}

fn require_false(value: &Value, key: &str) -> Result<(), &'static str> {
    (value.get(key) == Some(&Value::Bool(false)))
        .then_some(())
        .ok_or("explorer export refuses authority-bearing content")
}

fn validate_display_data(rows: &[&Value]) -> Result<(), &'static str> {
    for row in rows {
        validate_value(row, 0)?;
    }
    Ok(())
}

fn validate_value(value: &Value, depth: usize) -> Result<(), &'static str> {
    if depth > 32 {
        return Err("explorer export display data is too deeply nested");
    }
    match value {
        Value::String(value) if value.len() > MAX_DISPLAY_STRING_BYTES => {
            Err("explorer export display string is too long")
        }
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_value(value, depth + 1)),
        Value::Object(values) => values
            .values()
            .try_for_each(|value| validate_value(value, depth + 1)),
        _ => Ok(()),
    }
}

fn snapshot_digest(snapshot: &Value) -> String {
    let mut canonical = snapshot.clone();
    canonical
        .as_object_mut()
        .expect("snapshot object")
        .remove("snapshot_digest");
    canonical.sort_all_objects();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.explorer-snapshot.v1\0");
    hash.update(canonical.to_string().as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn required_display<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("unknown")
}

fn optional_display<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("unavailable")
}

fn inline<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("unavailable")
}

fn markdown_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('|', "\\|")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('!', "\\!")
        .replace('\n', " ")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snapshot(target: Option<&str>, truncated: bool) -> Value {
        let subject = json!({"kind":"image","image_revision":"sha256:image","project_revision":"sha256:project","workspace_revision":"sha256:workspace","project_graph_digest":"sha256:graph","candidate_revision":null,"side":"current"});
        let truncation = json!({"truncated":truncated,"reasons":if truncated {vec!["node_budget"]} else {Vec::<&str>::new()},"omitted_known_nodes":usize::from(truncated),"deferred_known_nodes":0});
        let coverage =
            json!({"owner":"workspace_graph","complete_within_retained_graph":!truncated});
        let inventories = ["modules", "declarations", "relations", "frontier"].into_iter().map(|view| json!({"view":view,"handle":format!("sha256:{view}"),"total_items":if view == "declarations" {2} else if view == "relations" {1} else {0}})).collect::<Vec<_>>();
        let mut pages = inventories.iter().map(|inventory| {
            let view = inventory["view"].as_str().unwrap();
            let items = match view {
                "declarations" => vec![json!({"node_key":"one","id":"safe.<svg>","display_name":"<script>alert(1)</script>","kind":"function","path":"src/</script>.spx"}), json!({"node_key":"two","id":"two","display_name":"two","kind":"function","path":"src/two.spx"})],
                "relations" => vec![json!({"from":"one","to":"two","family":"call"})],
                _ => vec![],
            };
            json!({"schema":VIEW_SCHEMA,"kind":"page","subject":subject,"mode":"context","target":target,"query":{"direction":"both","depth":1,"max_nodes":256,"max_bytes":262144},"artifact_digest":"sha256:artifact","truncation":truncation,"coverage":coverage,"view":view,"handle":inventory["handle"],"cursor":null,"offset":0,"total_items":items.len(),"page_size":256,"max_bytes":524288,"next_cursor":null,"items":items,"source_authority":false,"execution":false,"publication_authority":false,"nonclaims":[]})
        }).collect::<Vec<_>>();
        let query =
            json!({"mode":"context","target":target,"direction":"both","depth":1,"side":"current"});
        let summary = json!({"schema":VIEW_SCHEMA,"kind":"summary","subject":subject,"mode":"context","target":target,"query":{"direction":"both","depth":1,"max_nodes":256,"max_bytes":262144},"artifact_digest":"sha256:artifact","truncation":truncation,"coverage":coverage,"inventories":inventories,"source_authority":false,"execution":false,"publication_authority":false,"nonclaims":[]});
        for page in pages.iter_mut() {
            page["query"] = summary["query"].clone();
        }
        let mut views = vec![json!({"query":query,"summary":summary,"pages":pages})];
        if target.is_some() {
            views.insert(0, json!({"query":{"mode":"overview","target":null,"direction":"both","depth":1,"side":"current"},"summary":Value::Null,"pages":[]}));
        }
        let mut value = json!({"schema":SNAPSHOT_SCHEMA,"generator":"semaprax explore","views":views,"focus":target,"focus_sides":if target.is_some() {vec!["current"]} else {Vec::<&str>::new()},"source_included":false,"evidence_availability":"not_bundled","confidentiality":"names_ids_and_paths_may_be_confidential"});
        value["snapshot_digest"] = json!(snapshot_digest(&value));
        value
    }

    #[test]
    fn markdown_and_svg_are_deterministic_and_keep_hostile_display_text_inert() {
        let value = snapshot(Some("safe.<svg>"), false);
        let markdown = String::from_utf8(render(&value, Format::Markdown).unwrap()).unwrap();
        let svg = String::from_utf8(render(&value, Format::Svg).unwrap()).unwrap();
        assert_eq!(
            markdown,
            String::from_utf8(render(&value, Format::Markdown).unwrap()).unwrap()
        );
        assert_eq!(
            svg,
            String::from_utf8(render(&value, Format::Svg).unwrap()).unwrap()
        );
        assert!(markdown.contains("source-free"));
        assert!(svg.contains("changed declarations: 0"));
        assert!(svg.contains("project revision: sha256:project"));
        assert!(svg.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!svg.contains("<script>alert"));
        assert!(!svg.contains("foreignObject"));
        assert!(!svg.contains("href="));
    }

    #[test]
    fn export_rejects_partial_and_authority_bearing_snapshots() {
        let mut partial = snapshot(Some("target"), true);
        partial["views"][1]["pages"][0]["next_cursor"] = json!("sha256:continued");
        partial["snapshot_digest"] = json!(snapshot_digest(&partial));
        assert_eq!(
            render(&partial, Format::Markdown),
            Err("explorer page cursor continues past its inventory")
        );
        let mut authority = snapshot(Some("target"), false);
        authority["views"][1]["summary"]["source_authority"] = json!(true);
        authority["snapshot_digest"] = json!(snapshot_digest(&authority));
        assert_eq!(
            render(&authority, Format::Markdown),
            Err("explorer export refuses authority-bearing content")
        );
    }

    #[test]
    fn svg_requires_a_bounded_target() {
        assert_eq!(
            render(&snapshot(None, false), Format::Svg),
            Err("SVG export requires a declaration target")
        );
    }
}
