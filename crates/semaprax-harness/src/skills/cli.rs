//! `skills [list|load <digest>] --root <abs-dir>... [--tags t1,t2] [--max-bytes N] [--select a,b] [--json]`

use super::catalog::ApprovedRoot;
use super::load::{ListOutput, PromptOutput, SkillService};
use super::select::task_tags;
use super::{d, SkillCatalogConfig, DEFAULT_MAX_BYTES};
use crate::cli::{Environment, Outcome};
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn cli_skills(args: &[String], _env: &Environment) -> Outcome {
    let mut action = None::<String>;
    let mut load_digest = None::<String>;
    let mut roots = Vec::new();
    let mut tags = Vec::new();
    let mut select = Vec::new();
    let mut max_bytes = DEFAULT_MAX_BYTES;
    let mut as_json = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        let r: Result<(), String> = match a.as_str() {
            "--json" => {
                as_json = true;
                Ok(())
            }
            "--root" => val("--root").map(|v| roots.push(v)),
            "--tags" => val("--tags")
                .map(|v| tags.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string))),
            "--select" => val("--select")
                .map(|v| select.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string))),
            "--max-bytes" => val("--max-bytes")
                .and_then(|v| v.parse().map_err(|_| format!("invalid --max-bytes `{v}`")))
                .map(|n| max_bytes = n),
            "list" | "load" if action.is_none() => {
                action = Some(a.clone());
                Ok(())
            }
            other
                if action.as_deref() == Some("load")
                    && load_digest.is_none()
                    && !other.starts_with("--") =>
            {
                load_digest = Some(other.to_string());
                Ok(())
            }
            other => Err(format!("unexpected argument `{other}`")),
        };
        if let Err(m) = r {
            return Outcome::usage(m);
        }
    }
    let action = action.unwrap_or_else(|| "list".into());
    let mut approved = Vec::new();
    for r in roots {
        let path = PathBuf::from(&r);
        if !path.is_absolute() {
            return Outcome::refused(&d(
                "SPX-HPM004",
                format!("--root {r} must be an absolute directory"),
            ));
        }
        approved.push(ApprovedRoot {
            origin: format!("cli:{r}"),
            path,
            approved_digest: None,
        });
    }
    let want_prompt = !tags.is_empty() || !select.is_empty();
    let config = SkillCatalogConfig {
        enabled: true,
        select,
        max_bytes,
        ..Default::default()
    };
    let mut svc = SkillService::new(approved, config);
    if action == "load" {
        let Some(digest) = load_digest else {
            return Outcome::usage("load needs a digest");
        };
        return match svc.load(&digest) {
            Ok(r) if as_json => Outcome::ok(format!(
                "{}\n",
                json!({"digest": r.digest, "text": r.text, "model_visible_bytes": r.model_visible_bytes,
                       "warnings": r.warnings.iter().map(|w| w.json()).collect::<Vec<_>>()})
            )),
            Ok(r) => Outcome::ok(r.text),
            Err(e) => Outcome::refused(&e),
        };
    }
    let listing = svc.list();
    let prompt = want_prompt.then(|| {
        let all: Vec<String> = tags.iter().flat_map(|t| task_tags(t)).collect();
        svc.render_prompt(&all)
    });
    Outcome::ok(render_list(&listing, prompt.as_ref(), as_json))
}

fn render_list(l: &ListOutput, p: Option<&PromptOutput>, as_json: bool) -> String {
    if !as_json {
        let mut s = l.text.clone();
        for o in &l.omitted {
            s.push_str(&format!("omitted {} ({})\n", o.name, o.reason));
        }
        for dg in &l.diagnostics {
            s.push_str(&format!("{dg}\n"));
        }
        if let Some(p) = p {
            s.push_str(&format!(
                "selected: {}\n",
                p.loaded
                    .iter()
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        return s;
    }
    let skills: Vec<Value> = l
        .skills
        .iter()
        .map(|s| {
            json!({"id": s.id, "name": s.name, "description": s.description, "tags": s.tags, "bytes": s.bytes,
                   "lexical_size": s.lexical_size, "digest": s.digest, "origin": s.origin, "version": s.version,
                   "license": s.license, "conflict": s.conflict, "missing_dependencies": s.missing_dependencies})
        })
        .collect();
    let mut v = json!({
        "skills": skills,
        "omitted": l.omitted.iter().map(|o| json!({"name": o.name, "digest": o.digest, "reason": o.reason})).collect::<Vec<_>>(),
        "diagnostics": l.diagnostics.iter().map(|x| x.json()).collect::<Vec<_>>(),
        "model_visible_bytes": l.model_visible_bytes,
        "text": l.text,
    });
    if let Some(p) = p {
        v["prompt"] = json!({
            "text": p.text, "model_visible_bytes": p.model_visible_bytes, "budget": p.budget,
            "loaded": p.loaded.iter().map(|(n, dg)| json!({"name": n, "digest": dg})).collect::<Vec<_>>(),
            "omitted": p.omitted.iter().map(|o| json!({"name": o.name, "digest": o.digest, "reason": o.reason})).collect::<Vec<_>>(),
            "warnings": p.warnings.iter().map(|(n, w)| json!({"skill": n, "code": w.code, "kind": w.kind, "line": w.line})).collect::<Vec<_>>(),
            "unresolved": p.unresolved,
        });
    }
    format!("{}\n", crate::json::canonical(&v))
}
