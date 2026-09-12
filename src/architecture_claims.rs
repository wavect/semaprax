//! Architecture Claims v1: a closed, initial claim language evaluated
//! directly over compiler-derived HIR call facts, with deterministic minimal
//! witnesses and explicit unevaluable status for dynamic or external edges.
//!
//! This is a first slice of issue #205 ("Derive bounded architecture claims
//! from checked facts"), not the full claim language the issue describes.
//! It implements exactly one operator end to end: `forbid_reaches`, over the
//! direct static call graph retained in one [`ProjectRevision`]'s three HIR
//! programs (entry, public API, test). The remaining operators the issue
//! lists (unique/bounded writers, effect/capability attribution, dependency
//! direction, deployment facts, authorization mint/consume) are out of scope
//! here and are not claimed as done; see
//! `docs/ARCHITECTURE-CLAIMS-V1.md` for the exact scope line and what is
//! deferred.
//!
//! A claim's evaluation is a pure function of the exact `ProjectRevision`'s
//! already-checked HIR: this module builds no caller-authored edge list and
//! no separate hand-maintained graph. Every claim result is bound to that
//! revision's `project_revision` digest, so a stale claim cannot silently
//! keep reading true against different source: recompute against the new
//! revision and the bound digest changes with it.
//!
//! Edges recognized by this slice:
//!
//! - `ResolvedExprKind::Call { callee, .. }` is a statically known direct
//!   call and becomes a graph edge that is fully traversed.
//! - `ResolvedExprKind::NativeRustImportCall` names a statically known
//!   target id, but that target is foreign code with no retained HIR body in
//!   this revision's three programs. Reaching it is a known fact (it can be
//!   the witness target `to`), but *what it does* is not, so it is never
//!   treated as a safe dead end when a claim's status is still undecided.
//! - `ResolvedExprKind::Invoke { callable, .. }` calls a *computed* callable
//!   value (dynamic dispatch). Its concrete target is never statically
//!   known here, so any node that performs one taints the whole claim as
//!   `unevaluable` unless a definite violation was already found elsewhere.
//! - `ResolvedExprKind::HostCommandCall` is a closed, enumerable host
//!   primitive with no further call-graph reachability into program code by
//!   construction (see the capability boundary model); it contributes no
//!   edge and no unevaluable signal here. Attributing its effects is a
//!   distinct, unimplemented claim kind (`require_all_effects_attributed`),
//!   not this one.
//!
//! Determinism: adjacency is built into `BTreeMap`/`BTreeSet`, so neighbor
//! expansion order is fixed by declaration id alone. The witness search is a
//! breadth-first search over that fixed order, so the discovered shortest
//! path and the discovered unevaluable frontier are both reproducible for
//! identical input, even when more than one shortest path exists. Rendered
//! JSON is recursively key-sorted and size-capped, matching the convention
//! used by `semaprax.semantic-query.v1`.
//!
//! Fail-closed capacity: the reachability search is bounded by
//! [`MAX_ARCHITECTURE_CLAIM_GRAPH_WALK`] and each function body walk by
//! [`MAX_ARCHITECTURE_CLAIM_HIR_WALK`]. Exceeding either is a hard error
//! (`SPX-AC602`), never a silently reported `held`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer};
use crate::project::ProjectRevision;

/// Schema of one evaluated architecture claim set result.
pub const ARCHITECTURE_CLAIM_SET_RESULT_SCHEMA: &str = "semaprax.architecture-claim-set-result.v1";
/// Maximum encoded byte length of one claim id.
pub const MAX_ARCHITECTURE_CLAIM_ID_BYTES: usize = 256;
/// Maximum encoded byte length of one `from`/`to` declaration reference.
pub const MAX_ARCHITECTURE_CLAIM_TARGET_BYTES: usize = 4096;
/// Maximum number of claims accepted in one [`ArchitectureClaimSet`].
pub const MAX_ARCHITECTURE_CLAIMS_PER_SET: usize = 256;
/// Maximum HIR expression nodes walked per indexed declaration body.
pub const MAX_ARCHITECTURE_CLAIM_HIR_WALK: usize = 65_536;
/// Maximum declaration nodes visited by one claim's reachability search.
pub const MAX_ARCHITECTURE_CLAIM_GRAPH_WALK: usize = 65_536;
/// Maximum rendered byte length of one claim set result.
pub const MAX_ARCHITECTURE_CLAIM_RESULT_BYTES: usize = 8 * 1024 * 1024;

const NONCLAIMS: &[&str] = &[
    "static_direct_call_and_native_import_edges_only",
    "host_command_operations_are_closed_primitives_not_call_graph_edges_here",
    "absence_of_a_static_edge_is_not_proof_against_reflection_or_dynamic_behavior_outside_the_admitted_profile",
    "no_source_execution_or_publication_authority",
    "derived_read_only_projection_bound_to_the_exact_project_revision_above",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One closed architecture claim operator. This slice implements exactly one
/// variant; the enum stays non-exhaustive in spirit (private, closed) so a
/// future operator is an additive variant, not a competing type.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ArchitectureClaimOperator {
    ForbidReaches { from: String, to: String },
}

/// One user-declared architecture claim with a caller-chosen stable id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchitectureClaim {
    id: String,
    operator: ArchitectureClaimOperator,
}

impl ArchitectureClaim {
    /// Construct a `forbid_reaches` claim: `from` must never statically
    /// reach `to` through the direct call graph. `from` and `to` must be
    /// distinct stable declaration ids.
    pub fn forbid_reaches(
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Result<Self> {
        let id = id.into();
        let from = from.into();
        let to = to.into();
        validate_claim_id(&id)?;
        validate_target(&from)?;
        validate_target(&to)?;
        if from == to {
            return Err(invalid(
                "architecture claim forbid_reaches requires distinct from and to declarations",
            ));
        }
        Ok(Self {
            id,
            operator: ArchitectureClaimOperator::ForbidReaches { from, to },
        })
    }

    /// The claim's caller-chosen stable id.
    pub fn id(&self) -> &str {
        &self.id
    }

    fn operator_name(&self) -> &'static str {
        match self.operator {
            ArchitectureClaimOperator::ForbidReaches { .. } => "forbid_reaches",
        }
    }

    fn evaluate(&self, graph: &CallGraphFacts, max_walk: usize) -> Result<Value> {
        match &self.operator {
            ArchitectureClaimOperator::ForbidReaches { from, to } => {
                self.evaluate_forbid_reaches(graph, from, to, max_walk)
            }
        }
    }

    fn evaluate_forbid_reaches(
        &self,
        graph: &CallGraphFacts,
        from: &str,
        to: &str,
        max_walk: usize,
    ) -> Result<Value> {
        if !graph.nodes.contains_key(from) {
            return Err(invalid(
                "architecture claim forbid_reaches `from` is not a checked function or function template in this revision",
            ));
        }
        let reachability = bfs(graph, from, max_walk)?;
        let base = json!({
            "claim_id": self.id,
            "operator": self.operator_name(),
            "from": from,
            "to": to,
        });
        let mut object = base.as_object().unwrap().clone();
        if reachability.visited.contains(to) {
            let path = reconstruct_path(&reachability, from, to);
            let path_json = path
                .iter()
                .map(|id| {
                    json!({
                        "kind": graph.nodes.get(id).map(|node| node.kind).unwrap_or("unknown"),
                        "stable_id": id,
                    })
                })
                .collect::<Vec<_>>();
            object.insert("status".to_owned(), json!("violated"));
            object.insert("path".to_owned(), Value::Array(path_json));
            object.insert("frontier".to_owned(), Value::Null);
        } else {
            let frontier = collect_frontier(graph, &reachability.visited);
            if frontier.is_empty() {
                object.insert("status".to_owned(), json!("held"));
                object.insert("path".to_owned(), Value::Null);
                object.insert("frontier".to_owned(), Value::Null);
            } else {
                object.insert("status".to_owned(), json!("unevaluable"));
                object.insert("path".to_owned(), Value::Null);
                object.insert("frontier".to_owned(), Value::Array(frontier));
            }
        }
        Ok(Value::Object(object))
    }
}

/// A bounded, validated collection of architecture claims sharing one
/// evaluation pass over a single `ProjectRevision`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchitectureClaimSet {
    claims: Vec<ArchitectureClaim>,
}

impl ArchitectureClaimSet {
    /// Validate a claim collection: bounded count and unique claim ids.
    /// Claim order is not significant; the rendered result always sorts
    /// claims by id.
    pub fn new(claims: Vec<ArchitectureClaim>) -> Result<Self> {
        if claims.len() > MAX_ARCHITECTURE_CLAIMS_PER_SET {
            return Err(capacity(
                "architecture claim set exceeds its maximum claim count",
            ));
        }
        let mut seen = BTreeSet::new();
        for claim in &claims {
            if !seen.insert(claim.id.clone()) {
                return Err(invalid(
                    "architecture claim set has a duplicate claim id",
                ));
            }
        }
        Ok(Self { claims })
    }

    /// Evaluate every claim directly over `revision`'s checked HIR facts and
    /// render one deterministic, size-capped result document.
    pub fn evaluate(&self, revision: &ProjectRevision) -> Result<ArchitectureClaimSetResult> {
        self.evaluate_bounded(revision, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
    }

    fn evaluate_bounded(
        &self,
        revision: &ProjectRevision,
        max_walk: usize,
    ) -> Result<ArchitectureClaimSetResult> {
        let graph = CallGraphFacts::from_revision(revision)?;
        let mut claim_values = Vec::with_capacity(self.claims.len());
        for claim in &self.claims {
            claim_values.push(claim.evaluate(&graph, max_walk)?);
        }
        claim_values.sort_by(|left, right| left["claim_id"].as_str().cmp(&right["claim_id"].as_str()));
        render_result(revision, claim_values, max_walk)
    }
}

/// One rendered, canonical, size-capped architecture claim set evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchitectureClaimSetResult {
    json: String,
}

impl ArchitectureClaimSetResult {
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

fn render_result(
    revision: &ProjectRevision,
    claims: Vec<Value>,
    max_walk: usize,
) -> Result<ArchitectureClaimSetResult> {
    let mut value = json!({
        "claims": claims,
        "limits": {"max_graph_walk": max_walk, "max_hir_walk": MAX_ARCHITECTURE_CLAIM_HIR_WALK},
        "nonclaims": NONCLAIMS,
        "project_revision": revision.project_revision(),
        "schema": ARCHITECTURE_CLAIM_SET_RESULT_SCHEMA,
    });
    value.sort_all_objects();
    let mut json = serde_json::to_string(&value)
        .map_err(|_| invalid("architecture claim set result cannot be rendered"))?;
    json.push('\n');
    if json.len() > MAX_ARCHITECTURE_CLAIM_RESULT_BYTES {
        return Err(capacity(
            "architecture claim set result exceeds its byte limit",
        ));
    }
    Ok(ArchitectureClaimSetResult { json })
}

fn collect_frontier(graph: &CallGraphFacts, visited: &BTreeSet<String>) -> Vec<Value> {
    let mut frontier = Vec::new();
    for id in visited {
        match graph.nodes.get(id) {
            Some(node) if node.dynamic_invoke => {
                frontier.push(json!({
                    "kind": node.kind,
                    "reason": "dynamic_invoke",
                    "stable_id": id,
                }));
            }
            Some(_) => {}
            None => {
                frontier.push(json!({
                    "kind": "unknown",
                    "reason": "unresolved_call_target",
                    "stable_id": id,
                }));
            }
        }
    }
    frontier
}

fn reconstruct_path(reachability: &Reachability, from: &str, to: &str) -> Vec<String> {
    let mut path = vec![to.to_owned()];
    let mut current = to.to_owned();
    while current != from {
        let parent = reachability
            .parent
            .get(&current)
            .expect("every visited non-root node has a recorded parent");
        path.push(parent.clone());
        current = parent.clone();
    }
    path.reverse();
    path
}

struct Reachability {
    visited: BTreeSet<String>,
    parent: BTreeMap<String, String>,
}

/// Deterministic breadth-first search over the fixed, sorted adjacency of
/// `graph`, bounded by `max_walk` visited declaration nodes.
fn bfs(graph: &CallGraphFacts, from: &str, max_walk: usize) -> Result<Reachability> {
    let mut visited = BTreeSet::new();
    let mut parent = BTreeMap::new();
    let mut queue = VecDeque::new();
    visited.insert(from.to_owned());
    queue.push_back(from.to_owned());
    while let Some(current) = queue.pop_front() {
        let Some(node) = graph.nodes.get(&current) else {
            continue;
        };
        for next in &node.edges {
            if visited.contains(next) {
                continue;
            }
            if visited.len() >= max_walk {
                return Err(capacity(
                    "architecture claim reachability search exceeds its graph walk bound",
                ));
            }
            visited.insert(next.clone());
            parent.insert(next.clone(), current.clone());
            queue.push_back(next.clone());
        }
    }
    Ok(Reachability { visited, parent })
}

/// Direct-call adjacency and per-node dynamic-dispatch facts derived once
/// from one `ProjectRevision`'s three retained HIR programs. Only ids with a
/// known function or function-template body become keys; every other
/// visited id (a native import target, or any edge whose target this
/// revision never walked a body for) is a boundary node with no further
/// known reachability.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CallGraphFacts {
    nodes: BTreeMap<String, NodeFacts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NodeFacts {
    kind: &'static str,
    edges: BTreeSet<String>,
    dynamic_invoke: bool,
}

impl CallGraphFacts {
    fn from_revision(revision: &ProjectRevision) -> Result<Self> {
        let mut nodes = BTreeMap::<String, NodeFacts>::new();
        for program in [
            revision.entry_program(),
            revision.public_api_program(),
            revision.test_program(),
        ] {
            for function in &program.functions {
                index_declaration(
                    &mut nodes,
                    function.id.as_str(),
                    "function",
                    function
                        .requires
                        .iter()
                        .chain(std::iter::once(&function.body))
                        .chain(&function.ensures),
                )?;
            }
            for template in &program.function_templates {
                index_declaration(
                    &mut nodes,
                    template.id.as_str(),
                    "function_template",
                    template
                        .requires
                        .iter()
                        .chain(std::iter::once(&template.body))
                        .chain(&template.ensures),
                )?;
            }
        }
        Ok(Self { nodes })
    }
}

fn index_declaration<'a>(
    nodes: &mut BTreeMap<String, NodeFacts>,
    id: &str,
    kind: &'static str,
    roots: impl Iterator<Item = &'a ResolvedExpr>,
) -> Result<()> {
    let node = nodes.entry(id.to_owned()).or_insert_with(|| NodeFacts {
        kind,
        edges: BTreeSet::new(),
        dynamic_invoke: false,
    });
    if node.kind != kind {
        return Err(invalid(
            "architecture claim graph found conflicting retained declaration facts",
        ));
    }
    let mut walked = 0usize;
    for root in roots {
        walk_for_edges(root, &mut walked, node)?;
    }
    Ok(())
}

/// Iterative (never recursive) exhaustive walk of one expression tree,
/// collecting direct-call and native-import edges and the dynamic-invoke
/// flag into `node`. The match is exhaustive over every `ResolvedExprKind`
/// variant on purpose: a future HIR expression kind that can carry a nested
/// call must be judged here explicitly, not silently fall through a
/// wildcard arm as "no edge."
fn walk_for_edges(root: &ResolvedExpr, walked: &mut usize, node: &mut NodeFacts) -> Result<()> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        *walked = walked.saturating_add(1);
        if *walked > MAX_ARCHITECTURE_CLAIM_HIR_WALK {
            return Err(capacity(
                "architecture claim declaration body walk exceeds its bound",
            ));
        }
        match &expression.kind {
            ResolvedExprKind::Call { callee, args, .. } => {
                node.edges.insert(callee.as_str().to_owned());
                pending.extend(args.iter());
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                node.edges.insert(call.import.as_str().to_owned());
                pending.extend(call.args.iter());
            }
            ResolvedExprKind::HostCommandCall(call) => {
                pending.extend(call.args.iter());
            }
            ResolvedExprKind::Invoke { callable, args } => {
                node.dynamic_invoke = true;
                pending.push(callable);
                pending.extend(args.iter());
            }
            ResolvedExprKind::Closure {
                captures, body, ..
            } => {
                pending.push(body);
                pending.extend(captures.iter().map(|capture| &capture.value));
            }
            ResolvedExprKind::FunctionReference { .. } => {}
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push(source);
                pending.push(start);
                pending.push(end);
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(field_values(fields));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    pending.push(&arm.value);
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                pending.push(operand);
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(field_values(fields));
            }
        }
    }
    Ok(())
}

fn field_values(fields: &[ResolvedFieldInitializer]) -> impl Iterator<Item = &ResolvedExpr> {
    fields.iter().map(|field| &field.value)
}

fn validate_claim_id(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_ARCHITECTURE_CLAIM_ID_BYTES || value.contains('\0') {
        return Err(invalid("architecture claim id is invalid"));
    }
    Ok(())
}

fn validate_target(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_ARCHITECTURE_CLAIM_TARGET_BYTES
        || value.contains('\0')
    {
        return Err(invalid("architecture claim declaration reference is invalid"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-AC601", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-AC602", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: &'static str, edges: &[&str], dynamic_invoke: bool) -> NodeFacts {
        NodeFacts {
            kind,
            edges: edges.iter().map(|edge| (*edge).to_owned()).collect(),
            dynamic_invoke,
        }
    }

    fn graph(nodes: &[(&str, NodeFacts)]) -> CallGraphFacts {
        CallGraphFacts {
            nodes: nodes
                .iter()
                .map(|(id, facts)| ((*id).to_owned(), facts.clone()))
                .collect(),
        }
    }

    fn claim(id: &str, from: &str, to: &str) -> ArchitectureClaim {
        ArchitectureClaim::forbid_reaches(id, from, to).unwrap()
    }

    #[test]
    fn forbid_reaches_holds_when_no_static_path_exists() {
        let graph = graph(&[
            ("a", node("function", &["b"], false)),
            ("b", node("function", &[], false)),
            ("c", node("function", &[], false)),
        ]);
        let value = claim("no-a-to-c", "a", "c")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(value["status"], "held");
        assert!(value["path"].is_null());
        assert!(value["frontier"].is_null());
    }

    #[test]
    fn forbid_reaches_violated_reports_the_minimal_witness_path() {
        let graph = graph(&[
            ("a", node("function", &["b"], false)),
            ("b", node("function", &["c"], false)),
            ("c", node("function", &[], false)),
        ]);
        let value = claim("no-a-to-c", "a", "c")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(value["status"], "violated");
        let path = value["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["stable_id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(path, vec!["a", "b", "c"]);
        assert!(value["frontier"].is_null());
    }

    #[test]
    fn forbid_reaches_is_deterministic_across_multiple_equal_length_paths() {
        // Diamond: a -> b -> d, a -> c -> d. Two shortest paths of equal
        // length exist; the witness must be identical across repeated runs.
        let graph = graph(&[
            ("a", node("function", &["b", "c"], false)),
            ("b", node("function", &["d"], false)),
            ("c", node("function", &["d"], false)),
            ("d", node("function", &[], false)),
        ]);
        let first = claim("no-a-to-d", "a", "d")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        let second = claim("no-a-to-d", "a", "d")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first["status"], "violated");
        let path = first["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["stable_id"].as_str().unwrap())
            .collect::<Vec<_>>();
        // "b" < "c" lexicographically, so sorted-adjacency BFS discovers the
        // b-branch first.
        assert_eq!(path, vec!["a", "b", "d"]);
    }

    #[test]
    fn forbid_reaches_is_unevaluable_when_reachable_closure_touches_a_dynamic_invoke() {
        let graph = graph(&[
            ("a", node("function", &["b"], false)),
            ("b", node("function", &[], true)),
            ("c", node("function", &[], false)),
        ]);
        let value = claim("no-a-to-c", "a", "c")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(value["status"], "unevaluable");
        assert!(value["path"].is_null());
        let frontier = value["frontier"].as_array().unwrap();
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier[0]["stable_id"], "b");
        assert_eq!(frontier[0]["reason"], "dynamic_invoke");
    }

    #[test]
    fn forbid_reaches_is_unevaluable_when_reachable_closure_touches_an_external_boundary() {
        // "b" calls "native.import", which has no known body in this
        // revision (a NativeRustImportCall target).
        let graph = graph(&[
            ("a", node("function", &["b"], false)),
            ("b", node("function", &["native.import"], false)),
            ("c", node("function", &[], false)),
        ]);
        let value = claim("no-a-to-c", "a", "c")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(value["status"], "unevaluable");
        let frontier = value["frontier"].as_array().unwrap();
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier[0]["stable_id"], "native.import");
        assert_eq!(frontier[0]["reason"], "unresolved_call_target");
        assert_eq!(frontier[0]["kind"], "unknown");
    }

    #[test]
    fn forbid_reaches_violation_takes_priority_over_an_unrelated_unevaluable_branch() {
        // a -> c (direct violation) and a -> b (dynamic invoke, unrelated).
        let graph = graph(&[
            ("a", node("function", &["b", "c"], false)),
            ("b", node("function", &[], true)),
            ("c", node("function", &[], false)),
        ]);
        let value = claim("no-a-to-c", "a", "c")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(value["status"], "violated");
    }

    #[test]
    fn forbid_reaches_rejects_reflexive_from_and_to() {
        let error = ArchitectureClaim::forbid_reaches("x", "a", "a").unwrap_err();
        assert_eq!(error[0].code, "SPX-AC601");
    }

    #[test]
    fn forbid_reaches_rejects_an_unknown_from_declaration() {
        let graph = graph(&[("c", node("function", &[], false))]);
        let error = claim("x", "a", "c")
            .evaluate(&graph, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap_err();
        assert_eq!(error[0].code, "SPX-AC601");
    }

    #[test]
    fn claim_set_rejects_duplicate_claim_ids() {
        let error = ArchitectureClaimSet::new(vec![
            claim("dup", "a", "b"),
            claim("dup", "c", "d"),
        ])
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-AC601");
    }

    #[test]
    fn claim_set_rejects_more_than_the_maximum_claim_count() {
        let claims = (0..=MAX_ARCHITECTURE_CLAIMS_PER_SET)
            .map(|index| claim(&format!("c{index}"), "a", "b"))
            .collect::<Vec<_>>();
        let error = ArchitectureClaimSet::new(claims).unwrap_err();
        assert_eq!(error[0].code, "SPX-AC602");
    }

    #[test]
    fn reachability_search_fails_closed_on_a_tiny_capacity_bound() {
        let graph = graph(&[
            ("a", node("function", &["b"], false)),
            ("b", node("function", &["c"], false)),
            ("c", node("function", &[], false)),
        ]);
        // Bound the walk so tight that "b" cannot even be discovered:
        // capacity must fail closed rather than silently report `held`.
        let error = claim("x", "a", "c").evaluate(&graph, 1).unwrap_err();
        assert_eq!(error[0].code, "SPX-AC602");
    }

    #[test]
    fn claim_set_render_is_byte_identical_across_repeated_evaluation_of_equivalent_input() {
        let graph_a = graph(&[
            ("a", node("function", &["b"], false)),
            ("b", node("function", &[], false)),
        ]);
        let graph_b = graph(&[
            ("b", node("function", &[], false)),
            ("a", node("function", &["b"], false)),
        ]);
        let single_claim = claim("no-a-to-b", "a", "b");
        let one = single_claim
            .evaluate(&graph_a, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        let two = single_claim
            .evaluate(&graph_b, MAX_ARCHITECTURE_CLAIM_GRAPH_WALK)
            .unwrap();
        assert_eq!(
            serde_json::to_string(&one).unwrap(),
            serde_json::to_string(&two).unwrap()
        );
    }
}
