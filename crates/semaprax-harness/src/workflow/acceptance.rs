//! Host-owned acceptance evidence (DV-22). A `{stable_id, contains}` item is
//! judged only inside the declaration the compiler binds to that stable id in
//! the checked candidate project: the compiler's `context` answer names the
//! manifest-admitted source file and the project revision, and the declaration
//! span is taken from that one file with comments and string literals unable
//! to supply identity or widen the span. Anything unbound fails closed.

use super::pipeline::Ctx;
use serde_json::Value;
use std::path::{Component, Path};

/// Comments blanked in `text`; comments and string interiors blanked in `loc`.
/// Both keep byte length and newlines so offsets agree.
fn clean(src: &str) -> (Vec<u8>, Vec<u8>) {
    let b = src.as_bytes();
    let (mut text, mut loc) = (b.to_vec(), b.to_vec());
    let blank = |v: &mut Vec<u8>, i: usize| {
        if v[i] != b'\n' {
            v[i] = b' ';
        }
    };
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    blank(&mut text, i);
                    blank(&mut loc, i);
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let end = src
                    .get(i + 2..)
                    .and_then(|s| s.find("*/"))
                    .map_or(b.len(), |e| i + 2 + e + 2);
                for k in i..end {
                    blank(&mut text, k);
                    blank(&mut loc, k);
                }
                i = end;
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' && i + 1 < b.len() {
                        blank(&mut loc, i);
                        i += 1;
                    }
                    blank(&mut loc, i);
                    i += 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    (text, loc)
}

/// The comment-free source of the unique declaration annotated `@id("id")`.
/// `Ok(None)`: no declaration site. `Err`: ambiguous.
pub(super) fn declaration_span(src: &str, id: &str) -> Result<Option<String>, String> {
    let (text, loc) = clean(src);
    // Blanking only rewrites ASCII bytes over whole code points' bytes; a
    // lossy round trip that changes length means the source is not usable.
    let (Ok(text), Ok(loc)) = (String::from_utf8(text), String::from_utf8(loc)) else {
        return Ok(None);
    };
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut o = 0;
    for l in loc.split_inclusive('\n') {
        lines.push((o, l));
        o += l.len();
    }
    let mut found: Vec<usize> = Vec::new();
    for (k, (off, l)) in lines.iter().enumerate() {
        // A declaration annotation starts its (unindented) line; an import
        // (`use function @id(..) from ..`) never does.
        let Some(rest) = l.strip_prefix("@id(\"") else {
            continue;
        };
        let Some(q) = rest.find('"') else { continue };
        let lit = &text[off + 5..off + 5 + q];
        if lit == id && rest[q + 1..].trim_start().starts_with(')') {
            found.push(k);
        }
    }
    match found.as_slice() {
        [] => Ok(None),
        [k] => {
            let mut seen_header = false;
            let mut end = lines.len();
            for (j, (_, l)) in lines.iter().enumerate().skip(*k + 1) {
                let Some(c) = l.chars().next().filter(|c| !c.is_whitespace()) else {
                    continue;
                };
                match c {
                    '}' | ')' => {}
                    '@' if !seen_header => {}
                    _ if !seen_header => seen_header = true,
                    _ => {
                        end = j;
                        break;
                    }
                }
            }
            let start = lines[*k].0;
            let stop = lines.get(end).map_or(text.len(), |l| l.0);
            Ok(Some(text[start..stop].to_string()))
        }
        _ => Err(format!(
            "acceptance unmet: `{id}` has more than one declaration"
        )),
    }
}

fn bound_source(cx: &Ctx, root: &Path, revision: &str, id: &str) -> Result<String, String> {
    let unbound = |why: &str| {
        format!("acceptance unmet: `{id}` is not bound to a checked declaration ({why})")
    };
    let raw = cx
        .compiler
        .context(root, id, 4096)
        .map_err(|e| unbound(&e.message))?;
    let v: Value =
        serde_json::from_str(&raw).map_err(|_| unbound("unreadable compiler context"))?;
    if v["project_revision"].as_str() != Some(revision) {
        return Err(unbound("compiler context is for a different revision"));
    }
    let t = v["target"].as_array().ok_or_else(|| unbound("no target"))?;
    let (tid, rel) = (
        t.first().and_then(Value::as_str),
        t.get(2).and_then(Value::as_str),
    );
    let (Some(tid), Some(rel)) = (tid, rel) else {
        return Err(unbound("no declaring file"));
    };
    let p = Path::new(rel);
    if tid != id
        || !rel.ends_with(".spx")
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(unbound(
            "compiler named a different or invalid declaration site",
        ));
    }
    std::fs::read_to_string(root.join(p)).map_err(|_| unbound("declaring file unreadable"))
}

/// Count of `{stable_id, contains}` items verified in the checked candidate
/// (`root`, compiler revision `revision`). A proposal cannot influence it.
pub(super) fn verify(
    cx: &Ctx,
    root: &Path,
    revision: &str,
    items: &[Value],
) -> Result<usize, String> {
    let mut verified = 0;
    for it in items.iter().filter_map(Value::as_object) {
        let (id, needle) = (
            it["stable_id"].as_str().unwrap_or(""),
            it["contains"].as_str().unwrap_or(""),
        );
        let src = bound_source(cx, root, revision, id)?;
        match declaration_span(&src, id)? {
            None => return Err(format!("acceptance unmet: `{id}` is not declared")),
            Some(d) if d.contains(needle) => verified += 1,
            Some(_) => {
                return Err(format!(
                    "acceptance unmet: `{id}` does not contain `{needle}`"
                ))
            }
        }
    }
    Ok(verified)
}
