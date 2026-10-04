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
    let wrapper = op == "wrap";
    let form = if wrapper { "wrapper" } else { "post-execution" };
    match dir {
        Direction::Request => {
            let m = if wrapper {
                shape(v, "command wrap request", &["form", "argv"], &[])?
            } else {
                shape(
                    v,
                    "command view request",
                    &["form", "argv", "stdout", "stderr"],
                    &[],
                )?
            };
            check_form(m, form)?;
            argv(m, "argv")?;
            if !wrapper {
                str_of(m, "stdout", 1 << 22)?;
                str_of(m, "stderr", 1 << 22)?;
            }
        }
        Direction::Result => {
            if wrapper {
                let m = shape(v, "command wrap result", &["form", "plan"], &[])?;
                check_form(m, form)?;
                let p = shape(&m["plan"], "plan", &["argv"], &["cwd"])?;
                argv(p, "argv")?;
                if p.contains_key("cwd") {
                    path_of(p, "cwd")?;
                }
            } else {
                let m = shape(v, "command view result", &["form", "view"], &[])?;
                check_form(m, form)?;
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
