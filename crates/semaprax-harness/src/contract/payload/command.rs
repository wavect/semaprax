//! `command.view/v1`: post-execution views and validated wrappers. A view never
//! carries an exit status; the host owns the authoritative result.

use super::*;

const STATUS_KEYS: &[&str] = &[
    "exit_status",
    "exit_code",
    "status",
    "returncode",
    "signal",
    "success",
];

pub fn validate(op: &str, dir: Direction, v: &Value) -> HarnessResult<()> {
    if dir == Direction::Result {
        reject_status(v)?;
    }
    match (op, dir) {
        ("plan", Direction::Request) => plan_request(v),
        ("plan", Direction::Result) => plan_result(v),
        ("view", Direction::Request) => view_request(v),
        ("wrap", Direction::Request) => {
            let m = shape(v, "command wrap request", &["form", "argv"], &[])?;
            check_form(m, "wrapper")?;
            argv(m, "argv")
        }
        ("wrap", Direction::Result) => {
            let m = shape(v, "command wrap result", &["form", "plan"], &[])?;
            check_form(m, "wrapper")?;
            let p = shape(&m["plan"], "plan", &["argv"], &["cwd"])?;
            argv(p, "argv")?;
            if p.contains_key("cwd") {
                path_of(p, "cwd")?;
            }
            Ok(())
        }
        (_, Direction::Request) => Err(e("SPX-HPA046", "unknown command.view operation")),
        (_, Direction::Result) => {
            let m = shape(v, "command view result", &["form", "view"], &[])?;
            check_form(m, "post-execution")?;
            let w = shape(
                &m["view"],
                "view",
                &["text", "lossless", "omissions"],
                &["recovery_handle"],
            )?;
            str_of(w, "text", 1 << 22)?;
            bool_of(w, "lossless")?;
            uint_of(w, "omissions")?;
            if w.contains_key("recovery_handle") {
                str_of(w, "recovery_handle", 256)?;
            }
            Ok(())
        }
    }
}

/// `view` request: both streams, each given by exactly one of text, base64 or
/// a path relative to the provider's retention directory.
fn view_request(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "command view request",
        &["form", "argv"],
        &[
            "stdout",
            "stdout_b64",
            "stdout_path",
            "stderr",
            "stderr_b64",
            "stderr_path",
            "max_bytes",
            "min_bytes",
            "recovery_handle",
            "config",
        ],
    )?;
    check_form(m, "post-execution")?;
    argv(m, "argv")?;
    for s in ["stdout", "stderr"] {
        let given = [s.to_string(), format!("{s}_b64"), format!("{s}_path")]
            .iter()
            .filter(|k| m.contains_key(k.as_str()))
            .count();
        if given != 1 {
            return Err(e(
                "SPX-HPA040",
                format!("`{s}` must be given by exactly one of `{s}`, `{s}_b64`, `{s}_path`"),
            ));
        }
        if m.contains_key(s) {
            str_of(m, s, 1 << 22)?;
        }
        let b64 = format!("{s}_b64");
        if m.contains_key(&b64) {
            let t = str_of(m, &b64, 6 << 20)?;
            if !t
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
            {
                return Err(e("SPX-HPA040", format!("`{b64}` is not base64")));
            }
        }
        let path = format!("{s}_path");
        if m.contains_key(&path) {
            path_of(m, &path)?;
        }
    }
    opt_uint(m, "max_bytes")?;
    opt_uint(m, "min_bytes")?;
    if m.contains_key("recovery_handle") {
        str_of(m, "recovery_handle", 256)?;
    }
    config(m)
}

/// Bounded flat configuration object: scalar members only.
fn config(m: &Map<String, Value>) -> HarnessResult<()> {
    let Some(c) = m.get("config") else {
        return Ok(());
    };
    let o = c.as_object().filter(|o| o.len() <= 32).ok_or_else(|| {
        e(
            "SPX-HPA040",
            "`config` must be an object of at most 32 members",
        )
    })?;
    for (k, x) in o {
        let ok = k.len() <= 64
            && match x {
                Value::String(s) => s.len() <= 1024,
                Value::Bool(_) | Value::Number(_) => true,
                _ => false,
            };
        if !ok {
            return Err(e(
                "SPX-HPA040",
                format!("`config.{k}` must be a bounded scalar"),
            ));
        }
    }
    Ok(())
}

fn string_list(m: &Map<String, Value>, key: &str, max: usize, len: usize) -> HarnessResult<()> {
    if m.contains_key(key)
        && array_of(m, key, max)?
            .iter()
            .any(|s| s.as_str().is_none_or(|s| s.len() > len || s.contains('\0')))
    {
        return Err(e(
            "SPX-HPA040",
            format!("`{key}` must be an array of bounded strings"),
        ));
    }
    Ok(())
}

fn plan_request(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "command plan request",
        &["argv", "cwd_rel"],
        &[
            "estimated_output_bytes",
            "external_hooks",
            "lineage",
            "form",
            "config",
        ],
    )?;
    argv(m, "argv")?;
    if str_of(m, "cwd_rel", 1024)? != "." {
        path_of(m, "cwd_rel")?;
    }
    opt_uint(m, "estimated_output_bytes")?;
    string_list(m, "external_hooks", 32, 128)?;
    string_list(m, "lineage", 64, 256)?;
    if m.contains_key("form") {
        let f = str_of(m, "form", 32)?;
        if f != "post-execution" && f != "wrapper" {
            return Err(e(
                "SPX-HPA040",
                "`form` must be `post-execution` or `wrapper`",
            ));
        }
    }
    config(m)
}

/// `plan` result: route `post-execution` | `wrapped` | `bypass`. A wrapped
/// route must carry its validated argv and a bounded recovery object.
fn plan_result(v: &Value) -> HarnessResult<()> {
    let m = shape(
        v,
        "command plan result",
        &["route"],
        &[
            "form",
            "reason",
            "family",
            "filter",
            "operation",
            "raw_recovery",
            "argv",
            "env",
            "resolves_via",
            "recovery",
        ],
    )?;
    let route = str_of(m, "route", 32)?;
    if !["post-execution", "wrapped", "bypass"].contains(&route) {
        return Err(e(
            "SPX-HPA040",
            "`route` must be `post-execution`, `wrapped` or `bypass`",
        ));
    }
    for (k, max) in [
        ("form", 32),
        ("reason", 128),
        ("family", 64),
        ("filter", 64),
        ("operation", 32),
        ("raw_recovery", 64),
        ("resolves_via", 16),
    ] {
        if m.contains_key(k) {
            str_of(m, k, max)?;
        }
    }
    if route == "bypass" && !m.contains_key("reason") {
        return Err(e("SPX-HPA040", "a `bypass` route must carry a `reason`"));
    }
    if m.contains_key("argv") {
        argv(m, "argv")?;
    } else if route == "wrapped" {
        return Err(e("SPX-HPA040", "a `wrapped` route must carry `argv`"));
    }
    if m.contains_key("env") {
        let env = m["env"]
            .as_object()
            .filter(|o| o.len() <= 32)
            .ok_or_else(|| {
                e(
                    "SPX-HPA040",
                    "`env` must be an object of at most 32 members",
                )
            })?;
        if env
            .iter()
            .any(|(k, x)| k.len() > 64 || x.as_str().is_none_or(|s| s.len() > 4096))
        {
            return Err(e("SPX-HPA040", "`env` members must be bounded strings"));
        }
    }
    if m.contains_key("recovery") {
        let r = m["recovery"]
            .as_object()
            .filter(|o| o.len() <= 16)
            .ok_or_else(|| e("SPX-HPA040", "`recovery` must be a bounded object"))?;
        for (k, x) in r {
            let ok = k.len() <= 64
                && match x {
                    Value::String(s) => s.len() <= 4096,
                    Value::Array(a) => {
                        a.len() <= 32
                            && a.iter()
                                .all(|s| s.as_str().is_some_and(|s| s.len() <= 4096))
                    }
                    _ => false,
                };
            if !ok {
                return Err(e(
                    "SPX-HPA040",
                    format!("`recovery.{k}` must be a bounded string or string array"),
                ));
            }
        }
    }
    Ok(())
}

fn check_form(m: &Map<String, Value>, want: &str) -> HarnessResult<()> {
    if str_of(m, "form", 32)? == want {
        Ok(())
    } else {
        Err(e(
            "SPX-HPA040",
            format!("`form` must be `{want}` for this operation"),
        ))
    }
}

fn argv(m: &Map<String, Value>, key: &str) -> HarnessResult<()> {
    let a = array_of(m, key, 1024)?;
    if a.is_empty()
        || a.iter()
            .any(|s| s.as_str().is_none_or(|s| s.contains('\0')))
        || a[0].as_str() == Some("")
    {
        return Err(e(
            "SPX-HPA040",
            "`argv` must be a non-empty array of strings",
        ));
    }
    Ok(())
}

fn reject_status(v: &Value) -> HarnessResult<()> {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if STATUS_KEYS.contains(&k.to_ascii_lowercase().as_str()) {
                    return Err(e(
                        "SPX-HPA042",
                        format!("command view carries `{k}`; the host owns the exit status"),
                    ));
                }
                reject_status(x)?;
            }
            Ok(())
        }
        Value::Array(a) => a.iter().try_for_each(reject_status),
        _ => Ok(()),
    }
}
