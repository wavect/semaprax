//! Provider-neutral context item. Compiler facts and external hints share this
//! shape but never share authority: only `CompilerVerified` items can ground an edit.

use crate::json::canonical;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    CompilerVerified,
    Structural,
    Inferred,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CompilerVerified => "compiler-verified",
            Self::Structural => "structural",
            Self::Inferred => "inferred",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [Self::CompilerVerified, Self::Structural, Self::Inferred]
            .into_iter()
            .find(|t| t.as_str() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start_line: u64,
    pub end_line: u64,
}

/// How an item relates to compiler-owned identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// Not a `.spx` reference (or native item itself).
    None,
    /// Span bytes authenticated and mapped to a compiler-confirmed stable id.
    Resolved(String),
    /// Bytes no longer hash to the reported digest.
    Stale(String),
    /// Authentic bytes but no compiler-confirmed identity for the span.
    Unmappable(String),
}

/// Optional relationship reported by a provider; carries its own provenance and
/// is never upgraded.
#[derive(Clone, Debug, PartialEq)]
pub struct Edge {
    pub relation: String,
    pub target_path: String,
    pub provenance: Tier,
    /// The provider's own claim about the target (`resolved|ambiguous|unsupported`).
    pub resolution: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextItem {
    pub project_id: String,
    pub worktree_id: String,
    pub revision: String,
    pub path: String,
    pub span: Span,
    /// Provider-declared span meaning (`definition|start-line`), when reported.
    pub span_kind: Option<String>,
    pub digest: String,
    pub provenance: Tier,
    pub language: String,
    pub provider_id: String,
    /// Provider-local rank; never comparable across providers.
    pub provider_rank: Option<f64>,
    pub complete: bool,
    pub omission_reason: Option<String>,
    pub edges: Vec<Edge>,
    pub text: Option<String>,
    pub stable_id: Option<String>,
    pub link: Link,
    /// Digest of the span matches the current snapshot bytes.
    pub verified: bool,
    pub retrieval_handle: Option<String>,
    /// Other providers that reported the same path+span+digest, each with its own
    /// local rank and provenance (never merged or re-scored).
    pub also_reported: Vec<(String, Option<f64>, Tier)>,
}

impl ContextItem {
    /// Only compiler-verified items can authorize an edit; everything else is a hint.
    pub fn authorizes_edits(&self) -> bool {
        self.provenance == Tier::CompilerVerified && self.verified
    }

    pub fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("path".into(), json!(self.path));
        m.insert(
            "span".into(),
            json!({"start_line": self.span.start_line, "end_line": self.span.end_line}),
        );
        if let Some(k) = &self.span_kind {
            m.insert("span_kind".into(), json!(k));
        }
        m.insert("digest".into(), json!(self.digest));
        m.insert("provenance".into(), json!(self.provenance.as_str()));
        m.insert("language".into(), json!(self.language));
        m.insert("provider_id".into(), json!(self.provider_id));
        if let Some(r) = self.provider_rank {
            m.insert("provider_rank".into(), json!(r));
        }
        m.insert("complete".into(), json!(self.complete));
        if let Some(o) = &self.omission_reason {
            m.insert("omission_reason".into(), json!(o));
        }
        if !self.edges.is_empty() {
            let e: Vec<Value> = self
                .edges
                .iter()
                .map(|e| {
                    let mut x = json!({"relation": e.relation, "target_path": e.target_path, "provenance": e.provenance.as_str()});
                    if let Some(r) = &e.resolution {
                        x["resolution"] = json!(r);
                    }
                    x
                })
                .collect();
            m.insert("edges".into(), Value::Array(e));
        }
        if let Some(t) = &self.text {
            m.insert("text".into(), json!(t));
        }
        if let Some(id) = &self.stable_id {
            m.insert("stable_id".into(), json!(id));
        }
        match &self.link {
            Link::None => {}
            Link::Resolved(id) => {
                m.insert("link".into(), json!({"state": "resolved", "stable_id": id}));
            }
            Link::Stale(why) => {
                m.insert("link".into(), json!({"state": "stale", "reason": why}));
            }
            Link::Unmappable(why) => {
                m.insert("link".into(), json!({"state": "unmappable", "reason": why}));
            }
        }
        m.insert("verified".into(), json!(self.verified));
        m.insert("authorizes_edits".into(), json!(self.authorizes_edits()));
        if !self.also_reported.is_empty() {
            let a: Vec<Value> = self
                .also_reported
                .iter()
                .map(|(p, r, t)| json!({"provider_id": p, "provider_rank": r, "provenance": t.as_str()}))
                .collect();
            m.insert("also_reported".into(), Value::Array(a));
        }
        if let Some(h) = &self.retrieval_handle {
            m.insert("retrieval_handle".into(), json!(h));
        }
        Value::Object(m)
    }

    /// Canonical byte size of this item inside a document.
    pub fn rendered_len(&self) -> usize {
        canonical(&self.to_json()).len()
    }

    /// Dedup key across providers: path + span + digest.
    pub fn dedup_key(&self) -> (String, u64, u64, String) {
        (
            self.path.clone(),
            self.span.start_line,
            self.span.end_line,
            self.digest.clone(),
        )
    }
}
