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
            let m = shape(v, "context orient request", &["max_items"], &[])?;
            uint_of(m, "max_items")?;
        }
        "search" => {
            let m = shape(v, "context search request", &["query"], &["max_items"])?;
            str_of(m, "query", 4096)?;
            opt_uint(m, "max_items")?;
        }
        "skeleton" => {
            let m = shape(v, "context skeleton request", &["path"], &[])?;
            path_of(m, "path")?;
        }
        _ => {
            let m = shape(v, "context references request", &["symbol"], &["max_items"])?;
            str_of(m, "symbol", 1024)?;
            opt_uint(m, "max_items")?;
        }
    }
    Ok(())
}

fn result(op: &str, v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "context result",
        &["items", "coverage"],
        &["no_references"],
    )?;
    for it in array_of(m, "items", 4096)? {
        let i = shape(
            it,
            "context item",
            &["path", "span", "digest", "provenance", "language", "rank"],
            &["text"],
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
        if i.contains_key("text") {
            str_of(i, "text", 1 << 20)?;
        }
    }
    let c = shape(
        &m["coverage"],
        "coverage",
        &["complete", "indexed_files", "skipped", "exhaustive"],
        &[],
    )?;
    let complete = bool_of(c, "complete")?;
    let exhaustive = bool_of(c, "exhaustive")?;
    uint_of(c, "indexed_files")?;
    for s in array_of(c, "skipped", 4096)? {
        let s = shape(s, "skipped entry", &["path", "reason"], &[])?;
        path_of(s, "path")?;
        str_of(s, "reason", 256)?;
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
