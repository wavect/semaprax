//! Transformation lineage: one owner per command output. A nested host
//! invocation, an external hook or an argv that is already a wrapper keeps the
//! host from transforming the same output a second time.

use super::intent::basename;
use std::collections::BTreeMap;

/// Host-set marker in the command's granted environment; a nested host
/// invocation that sees it never transforms again.
pub const LINEAGE_VAR: &str = "SEMAPRAX_HARNESS_COMMAND_VIEW_LINEAGE";
/// Input stating an external host already owns command-output rewriting.
pub const OWNER_VAR: &str = "SEMAPRAX_HARNESS_EXTERNAL_VIEW_OWNER";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ownership {
    /// True only when this host is the sole owner and may transform.
    pub host_owns: bool,
    pub chain: Vec<String>,
    /// Why the host is not the owner (empty when it is).
    pub reason: String,
}

pub fn negotiate(
    argv: &[String],
    vars: &BTreeMap<String, String>,
    external_owner: Option<&str>,
    known_wrappers: &[String],
    self_id: &str,
) -> Ownership {
    let mut chain: Vec<String> = vars
        .get(LINEAGE_VAR)
        .map(|s| {
            s.split(',')
                .filter(|x| !x.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let not_owner = |reason: String, chain: Vec<String>| Ownership {
        host_owns: false,
        chain,
        reason,
    };
    if !chain.is_empty() {
        return not_owner(
            format!("nested inside `{}`; the outer host owns the view", chain[0]),
            chain,
        );
    }
    if let Some(o) = external_owner
        .map(String::from)
        .or_else(|| vars.get(OWNER_VAR).filter(|s| !s.is_empty()).cloned())
    {
        return not_owner(
            format!("external owner `{o}` rewrites command output"),
            vec![format!("external:{o}")],
        );
    }
    let name = basename(&argv[0]);
    let hosted = name == "semaprax-harness" && argv.iter().skip(1).take(3).any(|a| a == "exec")
        || name == "semaprax"
            && argv.get(1).map(String::as_str) == Some("harness")
            && argv.get(2).map(String::as_str) == Some("exec");
    if hosted {
        return not_owner(
            "argv is itself a host exec; it owns the view".into(),
            vec![format!("nested:{name}")],
        );
    }
    if known_wrappers.iter().any(|w| w == name) {
        return not_owner(
            format!("argv[0] `{name}` is an output-rewriting wrapper"),
            vec![format!("wrapper:{name}")],
        );
    }
    chain.push(self_id.to_string());
    Ownership {
        host_owns: true,
        chain,
        reason: String::new(),
    }
}
