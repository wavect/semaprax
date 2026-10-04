//! Machine-local trust store. A descriptor's permissions are only requests;
//! `grant_for` is the single place a [`Grant`] is issued, and only for the exact
//! descriptor, entry and upstream digests the user trusted.

use super::installations::{
    obj, opt_str, permissions_to_json, read_doc, str_field, CurrentDigests, LocalState,
};
use crate::cli::Environment;
use crate::contract::PermissionRequest;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::grant::{Grant, GrantedPermissions};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const TRUST_SCHEMA: &str = "semaprax.harness-trust.v1";

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// What the user approved and the digests that approval is bound to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustRecord {
    pub descriptor_digest: String,
    pub entry_digest: Option<String>,
    pub upstream_digest: Option<String>,
    pub granted: GrantedPermissions,
}

impl TrustRecord {
    pub fn to_json(&self) -> Value {
        json!({"descriptor_digest": self.descriptor_digest, "entry_digest": self.entry_digest,
               "upstream_digest": self.upstream_digest, "granted": permissions_to_json(&self.granted)})
    }
}

pub fn requested_as_granted(r: &PermissionRequest) -> GrantedPermissions {
    GrantedPermissions {
        read: r.read.clone(),
        write: r.write.clone(),
        network: r.network.clone(),
        process: r.process.clone(),
        secrets: r.secrets.clone(),
    }
}

/// Class names in `requested` that `granted` does not cover.
fn widened(requested: &PermissionRequest, granted: &GrantedPermissions) -> Vec<String> {
    let mut out = Vec::new();
    let classes: [(&str, &Vec<String>, &Vec<String>); 5] = [
        ("read", &requested.read, &granted.read),
        ("write", &requested.write, &granted.write),
        ("network", &requested.network, &granted.network),
        ("process", &requested.process, &granted.process),
        ("secrets", &requested.secrets, &granted.secrets),
    ];
    for (class, req, got) in classes {
        for r in req {
            if !got.contains(r) {
                out.push(format!("{class}:{r}"));
            }
        }
    }
    out
}

pub(super) fn load_records(home: &Path) -> HarnessResult<BTreeMap<String, TrustRecord>> {
    const F: &str = "trust.json";
    let mut out = BTreeMap::new();
    let Some(doc) = read_doc(home, F, TRUST_SCHEMA)? else {
        return Ok(out);
    };
    for (id, v) in obj(doc.get("trust").unwrap_or(&Value::Null), F, "`trust`")? {
        let m = obj(v, F, "a trust record")?;
        let g = obj(m.get("granted").unwrap_or(&Value::Null), F, "`granted`")?;
        let list = |k: &str| -> HarnessResult<Vec<String>> {
            g.get(k)
                .and_then(Value::as_array)
                .and_then(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect())
                .ok_or_else(|| {
                    bad(
                        "SPX-HPB020",
                        format!("{F}: `granted.{k}` must be a string array"),
                    )
                })
        };
        out.insert(
            id.clone(),
            TrustRecord {
                descriptor_digest: str_field(m, F, "descriptor_digest")?,
                entry_digest: opt_str(m, F, "entry_digest")?,
                upstream_digest: opt_str(m, F, "upstream_digest")?,
                granted: GrantedPermissions {
                    read: list("read")?,
                    write: list("write")?,
                    network: list("network")?,
                    process: list("process")?,
                    secrets: list("secrets")?,
                },
            },
        );
    }
    Ok(out)
}

/// Issue a grant for `provider_id` iff a trust record exists whose digests all
/// equal `current` and whose permissions cover everything now requested.
/// This is the only caller of `Grant::issue`.
pub fn grant_for(
    state: &LocalState,
    provider_id: &str,
    current: &CurrentDigests,
) -> HarnessResult<Grant> {
    let Some(rec) = state.trust.get(provider_id) else {
        return Err(bad("SPX-HPB030", format!("provider `{provider_id}` is not trusted; run `semaprax harness trust {provider_id}`")));
    };
    let changed = |what: &str| {
        bad(
            "SPX-HPB031",
            format!("{what} of `{provider_id}` changed since it was trusted; review it and run `semaprax harness trust {provider_id}` again"),
        )
    };
    if rec.descriptor_digest != current.descriptor_digest {
        return Err(changed("the descriptor"));
    }
    if rec.entry_digest != current.entry_digest {
        return Err(changed("the adapter entry"));
    }
    if current.requires_upstream && current.upstream_digest.is_none() {
        return Err(bad(
            "SPX-HPB033",
            format!("the upstream executable of `{provider_id}` is not installed or not readable"),
        ));
    }
    if rec.upstream_digest != current.upstream_digest {
        return Err(changed("the upstream executable"));
    }
    let extra = widened(&current.requested, &rec.granted);
    if !extra.is_empty() {
        return Err(bad(
            "SPX-HPB032",
            format!("`{provider_id}` now requests permissions that were never granted ({}); review and trust again", extra.join(", ")),
        ));
    }
    Ok(Grant::issue(
        provider_id.to_string(),
        current.descriptor_digest.clone(),
        current.entry_digest.clone(),
        current.upstream_digest.clone(),
        requested_as_granted(&current.requested),
    ))
}

/// Re-verify a grant against the machine-local stores as they are now. HP-03
/// and HP-05 call this before every dispatch, including cache hits that expose
/// restricted data, so revocation and any digest or permission change take
/// effect on the next dispatch.
pub fn check_grant_current(env: &Environment, grant: &Grant) -> HarnessResult<()> {
    let id = grant.provider_id();
    let state = LocalState::load(env)?;
    let inst = state.installations.get(id).ok_or_else(|| {
        bad(
            "SPX-HPB034",
            format!("grant for `{id}` is stale: the provider is no longer adopted"),
        )
    })?;
    let fresh = inst
        .inspect()
        .and_then(|i| grant_for(&state, id, &i.current))
        .map_err(|e| {
            bad(
                "SPX-HPB034",
                format!("grant for `{id}` is no longer valid: {}", e.message),
            )
        })?;
    if &fresh != grant {
        return Err(bad(
            "SPX-HPB034",
            format!("grant for `{id}` no longer matches the current trust record"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::PermissionRequest;

    fn cur(read: &[&str]) -> CurrentDigests {
        CurrentDigests {
            descriptor_digest: "sha256:d".into(),
            entry_digest: Some("sha256:e".into()),
            upstream_digest: None,
            requires_upstream: false,
            requested: PermissionRequest {
                read: read.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn widening_is_refused_even_with_equal_digests() {
        let mut st = LocalState::default();
        let c = cur(&["project"]);
        st.trust.insert(
            "a/b".into(),
            TrustRecord {
                descriptor_digest: "sha256:d".into(),
                entry_digest: Some("sha256:e".into()),
                upstream_digest: None,
                granted: requested_as_granted(&c.requested),
            },
        );
        assert!(grant_for(&st, "a/b", &c).is_ok());
        let wider = cur(&["project", "home"]);
        assert_eq!(
            grant_for(&st, "a/b", &wider).unwrap_err().code,
            "SPX-HPB032"
        );
        assert_eq!(grant_for(&st, "a/c", &c).unwrap_err().code, "SPX-HPB030");
    }
}
