//! Revision-bound mandatory law inventory. Evidence is data and grants no authority.
//! See docs/LAW-SET-V1.md for policy selection and exact nonclaims.
mod evaluate;
pub mod protected;
mod wire;

use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub use evaluate::{derive_report, require_satisfied, verify_report, LawPolicy};
pub const SCHEMA: &str = "semaprax.law-set.v1";
pub const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_LAWS: usize = 1024;
pub const MAX_MODULES: usize = 256;
pub const MAX_REFERENCES: usize = 64;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractKind {
    Precondition,
    Postcondition,
}

/// Closed subjects and selectors: none evaluates arbitrary asserted text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LawSelector {
    Contract {
        declaration_id: String,
        clause: ContractKind,
        proposition: String,
    },
    ForbidReaches {
        claim_id: String,
        from: String,
        to: String,
    },
    ProtocolRealizersBound {
        claim_id: String,
        protocol_id: String,
    },
    /// A property of the named reference model only, never of arbitrary Project code.
    ModelProperty { model: ModelKind, property: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Authorization,
    Handle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRequirement {
    RuntimeGuarded,
    CompilerProved,
    ModelChecked,
    SmtProved,
    TheoremProved,
}
impl EvidenceRequirement {
    fn class(&self) -> super::AssuranceClass {
        use super::AssuranceClass as A;
        match self {
            Self::RuntimeGuarded => A::RuntimeGuarded,
            Self::CompilerProved => A::CompilerProved,
            Self::ModelChecked => A::ModelChecked,
            Self::SmtProved => A::SmtProved,
            Self::TheoremProved => A::TheoremProved,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LawDefinition {
    pub law_id: String,
    pub selector: LawSelector,
    pub assumption_ids: Vec<String>,
    pub requires_laws: Vec<String>,
    pub evidence: EvidenceRequirement,
}

/// Explicit law module supplied by policy selection. `source_path` is its Project
/// source owner; the exact canonical module bytes are retained as provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LawModule {
    pub module_id: String,
    pub source_path: String,
    pub assumptions: Vec<String>,
    pub laws: Vec<LawDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LawRow {
    definition: LawDefinition,
    semantic_digest: String,
    module_id: String,
    source_path: String,
    source_digest: Option<String>,
    module_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    project_revision: String,
    program_root: String,
    proof_profile: String,
    modules: Vec<LawModule>,
    laws: Vec<LawRow>,
}

/// Opaque, canonical inventory derived against one independently held revision.
#[derive(Clone, Debug)]
pub struct LawSet {
    payload: Payload,
    document: String,
    digest: String,
}
impl LawSet {
    pub fn derive(
        revision: &ProjectRevision,
        proof_profile: &str,
        mut modules: Vec<LawModule>,
    ) -> Result<Self> {
        text_id(proof_profile)?;
        if modules.len() > MAX_MODULES {
            return Err(capacity());
        }
        // Bound before normalization or recursion; never truncate a caller inventory.
        wire::canonical(&modules)?;
        modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));
        let mut module_ids = BTreeSet::new();
        let mut assumptions = BTreeSet::new();
        let mut laws = BTreeMap::new();
        for module in &mut modules {
            text_id(&module.module_id)?;
            text_id(&module.source_path)?;
            if !module_ids.insert(module.module_id.clone()) {
                return Err(invalid("duplicate law module ID"));
            }
            if module.assumptions.len() > MAX_REFERENCES {
                return Err(capacity());
            }
            canonical_ids(&mut module.assumptions)?;
            for assumption in &module.assumptions {
                if !assumptions.insert(assumption.clone()) {
                    return Err(invalid("duplicate assumption ID"));
                }
            }
            if module.laws.len() > MAX_LAWS {
                return Err(capacity());
            }
            for law in &mut module.laws {
                normalize(law)?;
                if laws.insert(law.law_id.clone(), law.clone()).is_some() {
                    return Err(invalid("duplicate law ID across modules"));
                }
                if laws.len() > MAX_LAWS {
                    return Err(capacity());
                }
            }
            module.laws.sort_by(|a, b| a.law_id.cmp(&b.law_id));
        }
        for law in laws.values() {
            if law
                .assumption_ids
                .iter()
                .any(|id| !assumptions.contains(id))
                || law.requires_laws.iter().any(|id| !laws.contains_key(id))
            {
                return Err(invalid("dangling law or assumption reference"));
            }
        }
        // Kahn's algorithm is bounded and stack independent.
        let mut pending: BTreeSet<_> = laws.keys().cloned().collect();
        loop {
            let ready: Vec<_> = pending
                .iter()
                .filter(|id| {
                    laws[*id]
                        .requires_laws
                        .iter()
                        .all(|dep| !pending.contains(dep))
                })
                .cloned()
                .collect();
            if ready.is_empty() {
                break;
            }
            for id in ready {
                pending.remove(&id);
            }
        }
        if !pending.is_empty() {
            return Err(invalid("cyclic law definitions"));
        }
        let mut rows = Vec::new();
        for module in &modules {
            let module_digest =
                wire::digest(b"semaprax.law-module.v1\0", &wire::canonical(module)?);
            let source_digest = revision
                .sources()
                .iter()
                .find(|source| source.path() == module.source_path)
                .map(|source| source.source_digest().to_owned());
            for law in &module.laws {
                rows.push(LawRow {
                    definition: law.clone(),
                    semantic_digest: semantic_digest(law)?,
                    module_id: module.module_id.clone(),
                    source_path: module.source_path.clone(),
                    source_digest: source_digest.clone(),
                    module_digest: module_digest.clone(),
                });
            }
        }
        rows.sort_by(|a, b| a.definition.law_id.cmp(&b.definition.law_id));
        let root = revision.canonical_workspace_revision()?.program_root()?;
        let payload = Payload {
            project_revision: revision.project_revision().to_owned(),
            program_root: root.program_root().to_owned(),
            proof_profile: proof_profile.to_owned(),
            modules,
            laws: rows,
        };
        let (document, digest) = wire::encode(&payload)?;
        Ok(Self {
            payload,
            document,
            digest,
        })
    }
    pub fn to_json(&self) -> &str {
        &self.document
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn semantic_digest(&self, law_id: &str) -> Option<&str> {
        self.payload
            .laws
            .iter()
            .find(|row| row.definition.law_id == law_id)
            .map(|row| row.semantic_digest.as_str())
    }
    /// Independently decode, derive and bind exact revision/profile/module bytes.
    pub fn replay(revision: &ProjectRevision, proof_profile: &str, document: &str) -> Result<Self> {
        let payload: Payload = wire::decode(document)?;
        if payload.proof_profile != proof_profile {
            return Err(drift("law proof profile differs"));
        }
        let expected = Self::derive(revision, proof_profile, payload.modules)?;
        if expected.document != document {
            return Err(drift("law inventory, source or ProgramRoot differs"));
        }
        Ok(expected)
    }
}

fn semantic_digest(law: &LawDefinition) -> Result<String> {
    let mut normalized = law.clone();
    normalize(&mut normalized)?;
    // Persistent ID is independent of meaning; provenance is deliberately excluded.
    Ok(wire::digest(
        b"semaprax.law-semantics.v1\0",
        &wire::canonical(&serde_json::json!({
            "selector": normalized.selector, "assumption_ids": normalized.assumption_ids,
            "requires_laws": normalized.requires_laws, "evidence": normalized.evidence,
        }))?,
    ))
}
fn normalize(law: &mut LawDefinition) -> Result<()> {
    text_id(&law.law_id)?;
    canonical_ids(&mut law.assumption_ids)?;
    canonical_ids(&mut law.requires_laws)?;
    match &mut law.selector {
        LawSelector::Contract {
            declaration_id,
            proposition,
            ..
        } => {
            text_id(declaration_id)?;
            if proposition.len() > 4096 {
                return Err(capacity());
            }
            let source = format!("module law.selector;\n@id(\"law.selector\")\nfn selected() -> i64\n requires {proposition}\n{{ 0 }}\n");
            let program = crate::parse(&source, "<law-selector>")
                .map_err(|_| invalid("unsupported contract selector"))?;
            if program.functions.len() != 1
                || program.functions[0].requires.len() != 1
                || !program.functions[0].ensures.is_empty()
                || !program.functions[0].effects.is_empty()
                || program.functions[0].yields.is_some()
                || program.functions[0].follows.is_some()
            {
                return Err(invalid(
                    "selector must contain exactly one contract expression",
                ));
            }
            scalar_selector(&program.functions[0].requires[0])?;
            *proposition = crate::format::expr(&program.functions[0].requires[0], 0);
        }
        LawSelector::ForbidReaches { claim_id, from, to } => {
            crate::architecture_claims::ArchitectureClaim::forbid_reaches(
                claim_id.as_str(),
                from.as_str(),
                to.as_str(),
            )
            .map_err(|_| invalid("invalid architecture selector"))?;
        }
        LawSelector::ProtocolRealizersBound {
            claim_id,
            protocol_id,
        } => {
            crate::architecture_claims::ArchitectureClaim::protocol_realizers_bound(
                claim_id.as_str(),
                protocol_id.as_str(),
            )
            .map_err(|_| invalid("invalid protocol selector"))?;
        }
        LawSelector::ModelProperty { model, property } => {
            use super::model_checking::{authorization_model as a, handle_model as h};
            let properties = match model {
                ModelKind::Authorization => a::INVARIANTS,
                ModelKind::Handle => h::INVARIANTS,
            };
            if !properties.contains(&property.as_str()) {
                return Err(invalid("unsupported model property selector"));
            }
        }
    }
    Ok(())
}
fn canonical_ids(ids: &mut Vec<String>) -> Result<()> {
    if ids.len() > MAX_REFERENCES {
        return Err(capacity());
    }
    for id in ids.iter() {
        text_id(id)?;
    }
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("duplicate dependency reference"));
    }
    Ok(())
}
fn text_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 512 || id.chars().any(char::is_control) {
        return Err(invalid("invalid law identifier"));
    }
    Ok(())
}
fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-LW101", message)]
}
fn capacity() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-LW102",
        "law inventory capacity exceeded",
    )]
}
fn drift(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-LW104", message)]
}

// v1 binds scalar value propositions, not name-resolved calls that could retarget
// under an unchanged display spelling. Other AST shapes are explicitly refused.
fn scalar_selector(expression: &crate::ast::Expr) -> Result<()> {
    use crate::ast::ExprKind;
    let mut pending = vec![expression];
    let mut visited = 0;
    while let Some(expression) = pending.pop() {
        visited += 1;
        if visited > 1024 {
            return Err(capacity());
        }
        match &expression.kind {
            ExprKind::Int(_) | ExprKind::Int32(_) | ExprKind::Uint8(_) |
            ExprKind::Usize(_) | ExprKind::Char(_) | ExprKind::Float32(_) |
            ExprKind::Float64(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {},
            ExprKind::Unary { value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => { pending.push(left); pending.push(right); },
            _ => return Err(invalid("unsupported scalar contract selector; calls and aliases require a future resolved selector profile")),
        }
    }
    Ok(())
}
