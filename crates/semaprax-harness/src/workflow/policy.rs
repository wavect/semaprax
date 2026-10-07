//! Host apply policy and protected-fact guards. Providers may suggest a
//! candidate; native compiler facts (requirements, laws, effects, test
//! verdicts) are protected and checked on the compiler's own preview output.

use super::compiler::CandidatePreview;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProtectedSourceFacts {
    declarations: Vec<ProtectedDeclarationFacts>,
}

impl ProtectedSourceFacts {
    pub(super) fn same_laws(&self, other: &Self) -> bool {
        self.declarations
            .iter()
            .map(|facts| (&facts.id, &facts.laws))
            .eq(other
                .declarations
                .iter()
                .map(|facts| (&facts.id, &facts.laws)))
    }

    pub(super) fn same_effects(&self, other: &Self) -> bool {
        self.declarations
            .iter()
            .map(|facts| (&facts.id, &facts.effects))
            .eq(other
                .declarations
                .iter()
                .map(|facts| (&facts.id, &facts.effects)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtectedDeclarationFacts {
    id: String,
    laws: Vec<(String, String)>,
    effects: BTreeSet<String>,
}

/// The compiler's fixed requirement inventory; the host always sends all of it.
pub const REQUIREMENTS: [&str; 9] = [
    "preserve_stable_identity",
    "preserve_public_exports",
    "update_all_callers",
    "no_new_effects",
    "no_new_capabilities",
    "preserve_contracts",
    "revalidate_ownership_and_cleanup",
    "preserve_project_profile_admission",
    "preserve_admitted_core_targets",
];

pub const APPLY_POLICY_SCHEMA: &str = "semaprax.harness-apply-policy.v1";

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Preexisting host policy permitting automatic publication of a fully
/// checked candidate through the compiler's Git publication route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyPolicy {
    pub auto_apply: bool,
    /// Absolute path of a `semaprax.candidate-git-host-policy.v1` file.
    pub publication_policy: PathBuf,
    pub allowed_intents: Option<Vec<String>>,
    pub digest: String,
}

impl ApplyPolicy {
    /// Load `file`; it and the publication policy must be regular files
    /// outside the project (a repository must not grant itself authority).
    pub fn load(file: &Path, project_root: &Path) -> HarnessResult<ApplyPolicy> {
        let bytes = read_regular(file, "apply policy")?;
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| d("SPX-HPD082", format!("apply policy is not JSON: {e}")))?;
        let m = v
            .as_object()
            .ok_or_else(|| d("SPX-HPD082", "apply policy must be an object"))?;
        for k in m.keys() {
            if ![
                "schema",
                "auto_apply",
                "publication_policy",
                "allowed_intents",
            ]
            .contains(&k.as_str())
            {
                return Err(d(
                    "SPX-HPD082",
                    format!("unknown apply policy member `{k}`"),
                ));
            }
        }
        if m.get("schema").and_then(Value::as_str) != Some(APPLY_POLICY_SCHEMA) {
            return Err(d(
                "SPX-HPD082",
                format!("apply policy schema must be `{APPLY_POLICY_SCHEMA}`"),
            ));
        }
        let auto_apply = m
            .get("auto_apply")
            .and_then(Value::as_bool)
            .ok_or_else(|| d("SPX-HPD082", "`auto_apply` must be a boolean"))?;
        let pp = PathBuf::from(
            m.get("publication_policy")
                .and_then(Value::as_str)
                .ok_or_else(|| d("SPX-HPD082", "`publication_policy` must be a string"))?,
        );
        if !pp.is_absolute() {
            return Err(d(
                "SPX-HPD082",
                "`publication_policy` must be an absolute path",
            ));
        }
        read_regular(&pp, "publication policy")?;
        for (what, p) in [("apply policy", file), ("publication policy", pp.as_path())] {
            let canon = p
                .canonicalize()
                .map_err(|e| d("SPX-HPD082", format!("{what}: {e}")))?;
            if canon.starts_with(project_root) {
                return Err(d("SPX-HPD060", format!("{what} lives inside the project; authority must be a preexisting host file outside it")));
            }
        }
        let allowed_intents = match m.get("allowed_intents") {
            None => None,
            Some(a) => Some(
                a.as_array()
                    .and_then(|a| {
                        a.iter()
                            .map(|x| x.as_str().map(str::to_string))
                            .collect::<Option<Vec<_>>>()
                    })
                    .ok_or_else(|| d("SPX-HPD082", "`allowed_intents` must be a string array"))?,
            ),
        };
        Ok(ApplyPolicy {
            auto_apply,
            publication_policy: pp,
            allowed_intents,
            digest: crate::json::sha256_plain(&bytes),
        })
    }

    pub fn permits(&self, intent_kind: &str) -> bool {
        self.auto_apply
            && self
                .allowed_intents
                .as_ref()
                .is_none_or(|a| a.iter().any(|k| k == intent_kind))
    }
}

fn read_regular(path: &Path, what: &str) -> HarnessResult<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| d("SPX-HPD082", format!("{what} {}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > 64 * 1024 {
        return Err(d(
            "SPX-HPD082",
            format!("{what} must be a regular file of at most 64 KiB"),
        ));
    }
    std::fs::read(path).map_err(|e| d("SPX-HPD082", format!("{what}: {e}")))
}

/// The compiler's source digest: SHA-256 over a fixed domain, the LE u64
/// length and the bytes (`semaprax.semantic-review.source-digest.v1`).
pub fn source_digest(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"semaprax.semantic-review.source-digest.v1\0");
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    format!(
        "sha256:{}",
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

pub(super) fn law_lines(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim)
        .filter(|l| {
            ["requires ", "ensures ", "invariant "]
                .iter()
                .any(|p| l.starts_with(p))
        })
        .map(str::to_string)
        .collect()
}

pub(super) fn effect_tokens(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for l in src.lines().map(str::trim) {
        if let Some(rest) = l.strip_prefix("uses") {
            let inner = rest
                .trim()
                .trim_start_matches('{')
                .split('}')
                .next()
                .unwrap_or("");
            out.extend(
                inner
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty()),
            );
        }
    }
    out
}

/// Parse the source with the compiler's grammar and retain complete,
/// declaration-bound contract/effect facts. If only implementation bodies are
/// damaged, replace those bodies in memory and parse the recoverable headers;
/// ambiguity remains a refusal instead of an empty protected inventory.
pub(super) fn protected_source_facts(src: &str, path: &str) -> HarnessResult<ProtectedSourceFacts> {
    let program = match semaprax::parse(src, path) {
        Ok(program) => program,
        Err(_) => {
            let recovered = recover_function_headers(src).ok_or_else(|| {
                d(
                    "SPX-HPD042",
                    format!("protected contracts/effects in `{path}` cannot be recovered"),
                )
            })?;
            semaprax::parse(&recovered, path).map_err(|_| {
                d(
                    "SPX-HPD042",
                    format!("protected contracts/effects in `{path}` are ambiguous"),
                )
            })?
        }
    };

    let mut declarations = Vec::new();
    for function in &program.functions {
        declarations.push(function_facts(function));
    }
    for declaration in &program.types {
        if !declaration.invariants().is_empty() {
            declarations.push(ProtectedDeclarationFacts {
                id: declaration.stable_id.clone(),
                laws: declaration
                    .invariants()
                    .iter()
                    .map(|law| ("invariant".to_string(), semaprax::format::expr(law, 0)))
                    .collect(),
                effects: BTreeSet::new(),
            });
        }
        if let semaprax::ast::TypeDeclarationKind::Class { methods, .. } = &declaration.kind {
            declarations.extend(methods.iter().map(function_facts));
        }
    }
    declarations.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(ProtectedSourceFacts { declarations })
}

fn function_facts(function: &semaprax::ast::Function) -> ProtectedDeclarationFacts {
    let mut laws = function
        .requires
        .iter()
        .map(|law| ("requires".to_string(), semaprax::format::expr(law, 0)))
        .collect::<Vec<_>>();
    laws.extend(
        function
            .ensures
            .iter()
            .map(|law| ("ensures".to_string(), semaprax::format::expr(law, 0))),
    );
    ProtectedDeclarationFacts {
        id: function.stable_id.clone(),
        laws,
        effects: function.effects.iter().cloned().collect(),
    }
}

#[derive(Clone, Copy)]
struct HeaderToken<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    depth: usize,
}

/// Replace each function body with `0` while leaving declaration headers,
/// contracts and effect sets byte-for-byte available to the real parser.
fn recover_function_headers(source: &str) -> Option<String> {
    let tokens = header_tokens(source)?;
    let mut bodies = Vec::new();
    for (index, token) in tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.text == "fn")
    {
        let base = token.depth;
        let mut candidates = Vec::new();
        let mut cursor = index + 1;
        while cursor < tokens.len() {
            let current = tokens[cursor];
            if current.depth < base
                || (current.depth == base
                    && (current.text == "fn"
                        || current.text == "@" && !candidates.is_empty()
                        || current.text == "}"))
            {
                break;
            }
            if current.text == "{" && current.depth == base {
                let mut close = cursor + 1;
                while close < tokens.len()
                    && !(tokens[close].text == "}" && tokens[close].depth == base + 1)
                {
                    close += 1;
                }
                if close == tokens.len() {
                    return None;
                }
                candidates.push((current.end, tokens[close].start));
                cursor = close;
            }
            cursor += 1;
        }
        bodies.push(*candidates.last()?);
    }
    if bodies.is_empty() {
        return None;
    }
    bodies.sort_unstable();
    bodies.dedup();
    let mut recovered = source.to_string();
    for (start, end) in bodies.into_iter().rev() {
        recovered.replace_range(start..end, " 0 ");
    }
    Some(recovered)
}

fn header_tokens(source: &str) -> Option<Vec<HeaderToken<'_>>> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let (mut offset, mut depth) = (0usize, 0usize);
    while offset < bytes.len() {
        match bytes[offset] {
            byte if byte.is_ascii_whitespace() => offset += 1,
            b'/' if bytes.get(offset + 1) == Some(&b'/') => {
                offset += 2;
                while offset < bytes.len() && bytes[offset] != b'\n' {
                    offset += 1;
                }
            }
            b'"' | b'\'' => {
                let quote = bytes[offset];
                let start = offset;
                offset += 1;
                while offset < bytes.len() {
                    if bytes[offset] == b'\\' {
                        offset = offset.checked_add(2)?;
                    } else if bytes[offset] == quote {
                        offset += 1;
                        break;
                    } else {
                        offset += 1;
                    }
                }
                if offset > bytes.len() || bytes.get(offset.wrapping_sub(1)) != Some(&quote) {
                    return None;
                }
                out.push(HeaderToken {
                    text: &source[start..offset],
                    start,
                    end: offset,
                    depth,
                });
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = offset;
                offset += 1;
                while matches!(
                    bytes.get(offset),
                    Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
                ) {
                    offset += 1;
                }
                out.push(HeaderToken {
                    text: &source[start..offset],
                    start,
                    end: offset,
                    depth,
                });
            }
            b'{' => {
                out.push(HeaderToken {
                    text: &source[offset..offset + 1],
                    start: offset,
                    end: offset + 1,
                    depth,
                });
                depth = depth.checked_add(1)?;
                offset += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                out.push(HeaderToken {
                    text: &source[offset..offset + 1],
                    start: offset,
                    end: offset + 1,
                    depth: depth + 1,
                });
                offset += 1;
            }
            _ => {
                out.push(HeaderToken {
                    text: &source[offset..offset + 1],
                    start: offset,
                    end: offset + 1,
                    depth,
                });
                offset += 1;
            }
        }
    }
    Some(out)
}

/// Verify the compiler's preview against the authenticated base. Refusals:
/// HPD041 revision binding, HPD044 weakened requirements, HPD042 deleted law,
/// HPD043 widened effect, HPD005 stale base file.
pub fn check_protected_facts(
    root: &Path,
    compiler_revision: &str,
    intent_kind: &str,
    preview: &CandidatePreview,
) -> HarnessResult<()> {
    if preview.base_revision != compiler_revision {
        return Err(d(
            "SPX-HPD041",
            format!(
                "candidate base `{}` differs from the checked revision `{compiler_revision}`",
                preview.base_revision
            ),
        ));
    }
    let full: Vec<String> = REQUIREMENTS.iter().map(|s| s.to_string()).collect();
    if preview.requirements != full || preview.change_requirements.iter().any(|r| *r != full) {
        return Err(d(
            "SPX-HPD044",
            "candidate requirements differ from the fixed protected inventory",
        ));
    }
    if preview.unresolved_holes != 0 {
        return Err(d("SPX-HPD040", "candidate has unresolved holes"));
    }
    let (mut base_effects, mut cand_effects) = (BTreeSet::new(), BTreeSet::new());
    for c in &preview.source_changes {
        let rel = Path::new(&c.path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|x| matches!(x, std::path::Component::ParentDir))
        {
            return Err(d(
                "SPX-HPD040",
                format!("candidate path `{}` escapes the project", c.path),
            ));
        }
        let base = std::fs::read(root.join(rel)).map_err(|e| {
            d(
                "SPX-HPD005",
                format!("stale revision: cannot read `{}`: {e}", c.path),
            )
        })?;
        if source_digest(&base) != c.base_digest {
            return Err(d(
                "SPX-HPD005",
                format!(
                    "stale revision: `{}` differs from the candidate's base",
                    c.path
                ),
            ));
        }
        let base_src = String::from_utf8_lossy(&base).into_owned();
        let (bl, cl) = (law_lines(&base_src), law_lines(&c.replacement_source));
        let renames = matches!(intent_kind, "rename_declaration" | "move_declaration");
        let deleted = if renames {
            cl.len() < bl.len()
        } else {
            bl.iter().any(|l| !cl.contains(l))
        };
        if deleted {
            return Err(d(
                "SPX-HPD042",
                format!(
                    "candidate deletes or weakens a law (requires/ensures) in `{}`",
                    c.path
                ),
            ));
        }
        base_effects.extend(effect_tokens(&base_src));
        cand_effects.extend(effect_tokens(&c.replacement_source));
    }
    if let Some(extra) = cand_effects.difference(&base_effects).next() {
        return Err(d(
            "SPX-HPD043",
            format!("candidate widens declared effects (`{extra}`)"),
        ));
    }
    Ok(())
}

pub fn policy_json(p: &Option<ApplyPolicy>) -> Value {
    match p {
        None => json!(null),
        Some(p) => json!({"auto_apply": p.auto_apply, "digest": p.digest}),
    }
}
