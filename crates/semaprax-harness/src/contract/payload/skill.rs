//! `skill.catalog/v1`: bounded metadata listing and exact-digest loading.

use super::*;
use serde_json::json;

pub fn validate(op: &str, dir: Direction, v: &Value) -> HarnessResult<()> {
    match (op, dir) {
        ("list", Direction::Request) => {
            uint_of(shape(v, "skill list request", &["limit"], &[])?, "limit")?;
        }
        ("list", Direction::Result) => {
            let m = shape(v, "skill list result", &["skills", "truncated"], &[])?;
            bool_of(m, "truncated")?;
            for s in array_of(m, "skills", 1024)? {
                let s = shape(
                    s,
                    "skill entry",
                    &["id", "name", "description", "digest", "bytes"],
                    &[],
                )?;
                str_of(s, "id", 128)?;
                str_of(s, "name", 128)?;
                str_of(s, "description", 512)?;
                digest_of(s, "digest")?;
                uint_of(s, "bytes")?;
            }
        }
        (_, Direction::Request) => {
            let m = shape(v, "skill load request", &["digest"], &["resource"])?;
            digest_of(m, "digest")?;
            // Versioned progressive resource load: exact path and content digest.
            if let Some(r) = m.get("resource") {
                let r = shape(r, "skill resource request", &["path", "digest"], &[])?;
                path_of(r, "path")?;
                digest_of(r, "digest")?;
            }
        }
        (_, Direction::Result) => {
            let m = shape(
                v,
                "skill load result",
                &["digest", "artifact_refs"],
                &["text"],
            )?;
            digest_of(m, "digest")?;
            let refs = array_of(m, "artifact_refs", 256)?;
            for r in refs {
                let r = shape(r, "artifact reference", &["path", "digest"], &[])?;
                path_of(r, "path")?;
                digest_of(r, "digest")?;
            }
            if m.contains_key("text") {
                str_of(m, "text", 1 << 20)?;
            } else if refs.is_empty() {
                return Err(e(
                    "SPX-HPA040",
                    "a skill load result needs `text` or artifact references",
                ));
            }
        }
    }
    Ok(())
}

pub fn check_against_request(request: &Value, result: &Value) -> HarnessResult<()> {
    if let Some(res) = request.get("resource") {
        let want = [json!({"path": res.get("path"), "digest": res.get("digest")})];
        if result
            .get("artifact_refs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            != Some(&want[..])
        {
            return Err(e(
                "SPX-HPA040",
                "loaded resource differs from the requested path and digest",
            ));
        }
        if !result.get("text").is_some_and(Value::is_string) {
            return Err(e("SPX-HPA040", "a resource load result needs `text`"));
        }
    }
    if let Some(d) = request.get("digest") {
        if result.get("digest") != Some(d) {
            return Err(e(
                "SPX-HPA040",
                "loaded skill digest differs from the requested digest",
            ));
        }
    }
    Ok(())
}
