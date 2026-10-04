//! `context.repository/v1`: source-bound references and coverage.

use super::*;

pub fn validate(op: &str, dir: Direction, v: &Value) -> HarnessResult<()> {
    match dir {
        Direction::Request => request(op, v),
        Direction::Result => result(op, v),
    }
}

fn request(op: &str, v: &Value) -> HarnessResult<()> {
    match op {
        "orient" => {
            let m = shape(v, "context orient request", &["max_items"], &["refresh"])?;
            uint_of(m, "max_items")?;
            refresh_of(m)?;
        }
        "search" => {
            let m = shape(
                v,
                "context search request",
                &["query"],
                &["max_items", "refresh", "in"],
            )?;
            str_of(m, "query", 4096)?;
            opt_uint(m, "max_items")?;
            refresh_of(m)?;
            opt_path(m, "in")?;
        }
        "skeleton" => {
            let m = shape(v, "context skeleton request", &["path"], &["refresh"])?;
            path_of(m, "path")?;
            refresh_of(m)?;
        }
        _ => {
            let m = shape(
                v,
                "context references request",
                &["symbol"],
                &["max_items", "refresh", "exhaustive", "in"],
            )?;
            str_of(m, "symbol", 1024)?;
            opt_uint(m, "max_items")?;
            refresh_of(m)?;
            if m.contains_key("exhaustive") {
                bool_of(m, "exhaustive")?;
            }
            opt_path(m, "in")?;
        }
    }
    Ok(())
}

/// Optional index-refresh policy (additive in v1).
fn refresh_of(m: &Map<String, Value>) -> HarnessResult<()> {
    if m.contains_key("refresh")
        && !matches!(str_of(m, "refresh", 16)?, "auto" | "rebuild" | "never")
    {
        return Err(e(
            "SPX-HPA040",
            "`refresh` must be `auto`, `rebuild` or `never`",
        ));
    }
    Ok(())
}

fn opt_path(m: &Map<String, Value>, key: &str) -> HarnessResult<()> {
    if m.contains_key(key) {
        path_of(m, key)?;
    }
    Ok(())
}

/// Largest serialized `metadata` object a provider may return.
const METADATA_MAX_BYTES: usize = 8192;

/// Provider-local diagnostic data: scalars or one level of scalar-valued
/// objects, bounded in size. Never interpreted as authority or provenance.
fn metadata_of(v: &Value) -> HarnessResult<()> {
    let m = v
        .as_object()
        .ok_or_else(|| e("SPX-HPA040", "`metadata` must be an object"))?;
    if m.len() > 64 || v.to_string().len() > METADATA_MAX_BYTES {
        return Err(e(
            "SPX-HPA040",
            format!("`metadata` exceeds {METADATA_MAX_BYTES} bytes or 64 members"),
        ));
    }
    let scalar = |x: &Value| !(x.is_array() || x.is_object());
    for (k, x) in m {
        if k.len() > 64 {
            return Err(e("SPX-HPA040", "`metadata` key exceeds 64 bytes"));
        }
        let ok = match x.as_object() {
            Some(o) => o.len() <= 32 && o.keys().all(|k| k.len() <= 64) && o.values().all(scalar),
            None => scalar(x),
        };
        if !ok {
            return Err(e(
                "SPX-HPA040",
                format!("`metadata.{k}` must be a scalar or an object of scalars"),
            ));
        }
    }
    Ok(())
}

/// Item `edges`: structural links an external provider found. Never
/// `compiler-verified`: only the compiler can verify.
fn edges_of(v: &Value) -> HarnessResult<()> {
    let a = v.as_array().filter(|a| a.len() <= 64).ok_or_else(|| {
        e(
            "SPX-HPA040",
            "`edges` must be an array of at most 64 entries",
        )
    })?;
    for ed in a {
        let m = shape(
            ed,
            "edge",
            &["target", "relation", "provenance"],
            &["resolution"],
        )?;
        if m.contains_key("resolution")
            && !matches!(
                str_of(m, "resolution", 16)?,
                "resolved" | "ambiguous" | "unsupported"
            )
        {
            return Err(e(
                "SPX-HPA040",
                "edge `resolution` must be `resolved`, `ambiguous` or `unsupported`",
            ));
        }
        str_of(m, "target", 1024)?;
        str_of(m, "relation", 64)?;
        if !matches!(str_of(m, "provenance", 16)?, "structural" | "inferred") {
            return Err(e(
                "SPX-HPA040",
                "edge provenance must be `structural` or `inferred`",
            ));
        }
    }
    Ok(())
}

fn result(op: &str, v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "context result",
        &["items", "coverage"],
        &["no_references", "metadata"],
    )?;
    if let Some(md) = m.get("metadata") {
        metadata_of(md)?;
    }
    for it in array_of(m, "items", 4096)? {
        let i = shape(
            it,
            "context item",
            &["path", "span", "digest", "provenance", "language", "rank"],
            &["text", "edges", "span_kind"],
        )?;
        path_of(i, "path")?;
        let s = shape(&i["span"], "span", &["start_line", "end_line"], &[])?;
        let (a, b) = (uint_of(s, "start_line")?, uint_of(s, "end_line")?);
        if a == 0 || b < a {
            return Err(e(
                "SPX-HPA040",
                "span must satisfy 1 <= start_line <= end_line",
            ));
        }
        digest_of(i, "digest")?;
        if !matches!(
            str_of(i, "provenance", 32)?,
            "compiler-verified" | "structural" | "inferred"
        ) {
            return Err(e("SPX-HPA040", "unknown context provenance"));
        }
        str_of(i, "language", 32)?;
        if !i["rank"].is_number() {
            return Err(e("SPX-HPA040", "`rank` must be a number"));
        }
        if i.contains_key("span_kind")
            && !matches!(str_of(i, "span_kind", 16)?, "definition" | "start-line")
        {
            return Err(e(
                "SPX-HPA040",
                "`span_kind` must be `definition` or `start-line`",
            ));
        }
        if let Some(ed) = i.get("edges") {
            edges_of(ed)?;
        }
        if i.contains_key("text") {
            str_of(i, "text", 1 << 20)?;
        }
    }
    let c = shape(
        &m["coverage"],
        "coverage",
        &["complete", "indexed_files", "skipped", "exhaustive"],
        &["extraction_errors"],
    )?;
    let complete = bool_of(c, "complete")?;
    let exhaustive = bool_of(c, "exhaustive")?;
    uint_of(c, "indexed_files")?;
    for s in array_of(c, "skipped", 4096)? {
        let s = shape(s, "skipped entry", &["path", "reason"], &[])?;
        path_of(s, "path")?;
        str_of(s, "reason", 256)?;
    }
    if c.contains_key("extraction_errors") {
        for x in array_of(c, "extraction_errors", 256)? {
            let x = shape(x, "extraction error", &["path", "reason"], &[])?;
            path_of(x, "path")?;
            str_of(x, "reason", 256)?;
        }
    }
    if m.contains_key("no_references") {
        let none = bool_of(m, "no_references")?;
        if none && (op != "references" || !(complete && exhaustive)) {
            return Err(e(
                "SPX-HPA040",
                "`no_references` requires an exhaustive, complete references result",
            ));
        }
    }
    Ok(())
}
