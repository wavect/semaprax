//! Derived skill bundles: distinct canonical identity (artifact inventory v2),
//! parent/source digests, scope and evaluation record. The official parent
//! snapshot is only ever read.

use super::w;
use crate::diag::HarnessResult;
use crate::json::canonical;
use crate::skills::inventory::{self, Bounds, ScanRules};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const PROVENANCE_SCHEMA: &str = "semaprax.derived-skill.v1";

/// Derived names always carry the `derived-` prefix, so they can never equal an
/// official skill name.
pub fn derived_name(proposed: &str) -> Option<String> {
    let slug: String = proposed
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let slug = slug.trim_start_matches("derived-").to_string();
    if slug.is_empty() {
        return None;
    }
    let name = format!("derived-{slug}");
    Some(
        name.chars()
            .take(64)
            .collect::<String>()
            .trim_end_matches('-')
            .to_string(),
    )
}

pub fn skill_md(
    name: &str,
    description: &str,
    parent_name: &str,
    parent_digest: &str,
    family: &str,
    body: &str,
) -> String {
    let desc = Value::String(description.replace('\n', " ")).to_string();
    format!(
        "---\nname: {name}\ndescription: {desc}\n---\n> Derived skill, not an official snapshot. Parent `{parent_name}` {parent_digest}; scope `{family}`; promotion is a separate explicit action.\n\n{body}\n"
    )
}

pub fn digest_of(dir: &Path) -> HarnessResult<String> {
    Ok(inventory::scan(dir, &ScanRules::skill(), &Bounds::SKILL)?.digest())
}

/// Write the candidate `SKILL.md` into `<ws>/derived/<name>/`.
pub fn write_candidate(ws: &Path, name: &str, md: &str) -> HarnessResult<PathBuf> {
    let dir = ws.join("derived").join(name);
    std::fs::create_dir_all(&dir).map_err(|e| w("SPX-HPW011", format!("derived dir: {e}")))?;
    std::fs::write(dir.join("SKILL.md"), md)
        .map_err(|e| w("SPX-HPW011", format!("derived SKILL.md: {e}")))?;
    Ok(dir)
}

/// Add the provenance record and return the final v2 identity digest.
pub fn finalize(dir: &Path, provenance: &Value) -> HarnessResult<String> {
    std::fs::write(dir.join("provenance.json"), canonical(provenance) + "\n")
        .map_err(|e| w("SPX-HPW011", format!("provenance: {e}")))?;
    digest_of(dir)
}

/// Remove only the derived skill (wiki, evidence and everything else stay).
pub fn rollback(ws: &Path, name: &str) {
    let _ = std::fs::remove_dir_all(ws.join("derived").join(name));
}

pub fn copy_bundle(src: &Path, dst: &Path) -> HarnessResult<()> {
    std::fs::create_dir_all(dst).map_err(|e| w("SPX-HPW009", format!("{}: {e}", dst.display())))?;
    for f in ["SKILL.md", "provenance.json"] {
        std::fs::copy(src.join(f), dst.join(f))
            .map_err(|e| w("SPX-HPW009", format!("copy {f}: {e}")))?;
    }
    Ok(())
}
