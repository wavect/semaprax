//! Small deterministic expression search, admitted only by ordinary hole fill.
//! Scope types guide enumeration; they do not prove liveness or user intent.
use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{intent, wire, ProjectCandidateDraft};
use crate::diagnostic::Diagnostic;

pub const PROJECT_HOLE_FILL_SUGGESTIONS_SCHEMA: &str = "semaprax.project-hole-fill-suggestions.v1";
pub const MAX_PROJECT_HOLE_FILL_SUGGESTIONS_BYTES: usize = 64 * 1024;
const MAX_CONTEXT_BYTES: usize = 1024 * 1024;
const MAX_SCOPE: usize = 16_384;
const MAX_CALLS: usize = 1024;
const MAX_PARAMETERS: usize = 64;
const MAX_ATTEMPTS: usize = 32;
const CONTEXT_DOMAIN: &[u8] = b"semaprax.project-hole-context.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectCandidateDraft {
    /// Preview scope places and direct calls against this unchanged draft.
    /// Accepted previews are discarded; their digests are not retained handles.
    pub fn hole_fill_suggestions(&self, expected_draft: &str, hole_id: &str) -> Result<String> {
        let bytes = self.hole_context(expected_draft, hole_id)?;
        if bytes.len() > MAX_CONTEXT_BYTES {
            return Err(capacity("hole suggestion context exceeds its byte bound"));
        }
        let context_revision = wire::digest(CONTEXT_DOMAIN, bytes.as_bytes());
        let context: Value = serde_json::from_str(&bytes)
            .map_err(|_| grammar("compiler hole suggestion context is not JSON"))?;
        let body = match text(&context, "schema")? {
            "semaprax.project-candidate-hole-context.v1" => true,
            "semaprax.project-candidate-expression-hole-context.v1"
            | "semaprax.project-candidate-contract-expression-hole-context.v1" => false,
            _ => {
                return Err(grammar(
                    "hole suggestions do not recognize the context owner",
                ))
            }
        };
        if context["draft_digest"] != expected_draft
            || context["hole_id"] != hole_id
            || context["materializable"] != false
            || context["source_authority"] != false
            || context["validation"] != "pending_fill_full_source_replay"
        {
            return Err(grammar("hole suggestion context bindings are inconsistent"));
        }
        let inventory = Inventory::derive(&context, body)?;
        let mut previews = Previews::default();
        let search_exhausted = inventory.enumerate(&mut |expression| {
            // One bounded lookahead distinguishes exactly 32 proposals from a
            // truncated search. It does not perform a thirty-third fill.
            if previews.considered == MAX_ATTEMPTS {
                return false;
            }
            previews.considered += 1;
            match self.fill_hole(expected_draft, hole_id, &expression) {
                Ok(draft) => previews.suggestions.push(json!({
                    "expression":expression,"preview_draft_revision":draft.draft_digest()
                })),
                // Conservative and capacity rejections are failed previews,
                // never proofs that the expression is semantically impossible.
                Err(_) => previews.rejected += 1,
            }
            true
        });
        wire::render(
            json!({
                "schema":PROJECT_HOLE_FILL_SUGGESTIONS_SCHEMA,
                "draft_revision":expected_draft,"hole_id":hole_id,
                "context_revision":context_revision,
                "last_valid_revision":text(&context,"last_valid_revision")?,
                "expected_type_id":inventory.expected,
                "considered":previews.considered,"rejected":previews.rejected,
                "search_exhausted":search_exhausted,"suggestions":previews.suggestions,
                "validation":"ordinary_fill_source_replay","tests":"not_run",
                "source_authority":false,"draft_retained":false,
                "nonclaims":["not_intent_correctness","not_runtime_contract_proof",
                    "not_complete_expression_search","not_liveness_inference"]
            }),
            MAX_PROJECT_HOLE_FILL_SUGGESTIONS_BYTES,
        )
        .map_err(|_| capacity("hole fill suggestions exceed their report byte bound"))
    }
}

#[derive(Default)]
struct Previews {
    considered: usize,
    rejected: usize,
    suggestions: Vec<Value>,
}

struct Place<'a> {
    name: &'a str,
    ty: &'a str,
}
struct Call<'a> {
    target: &'a str,
    result: &'a str,
    parameters: Vec<&'a str>,
    within_budget: bool,
}
struct Inventory<'a> {
    expected: &'a str,
    target: &'a str,
    scope: Vec<Place<'a>>,
    by_type: BTreeMap<&'a str, Vec<&'a str>>,
    calls: Vec<Call<'a>>,
}

impl<'a> Inventory<'a> {
    fn derive(context: &'a Value, body: bool) -> Result<Self> {
        let expected = text(context, "expected_type_id")?;
        let target = bounded_text(context, "target", intent::MAX_ID_BYTES)?;
        let rows = array(context, "scope", MAX_SCOPE)?;
        let mut scope = Vec::with_capacity(rows.len());
        let mut by_type = BTreeMap::<&str, Vec<&str>>::new();
        for row in rows {
            let name = bounded_text(row, "name", intent::MAX_NAME_BYTES)?;
            let ty = text(row, if body { "type_id" } else { "type" })?;
            text(row, if body { "id" } else { "value_id" })?;
            ownership(row)?;
            if !body && !row["mutable"].is_boolean() {
                return Err(grammar("hole suggestion scope lacks its mutability fact"));
            }
            scope.push(Place { name, ty });
            by_type.entry(ty).or_default().push(name);
        }
        let rows = array(context, "accessible_calls", MAX_CALLS)?;
        let mut calls = Vec::with_capacity(rows.len());
        for row in rows {
            let target = bounded_text(row, "id", intent::MAX_ID_BYTES)?;
            let result = text(row, "return_type_id")?;
            let within_budget = row["within_effect_budget"]
                .as_bool()
                .ok_or_else(|| grammar("hole suggestion call lacks its effect-budget fact"))?;
            let parameters = array(row, "parameters", MAX_PARAMETERS)?
                .iter()
                .map(|parameter| {
                    ownership(parameter)?;
                    text(parameter, "type_id")
                })
                .collect::<Result<Vec<_>>>()?;
            calls.push(Call {
                target,
                result,
                parameters,
                within_budget,
            });
        }
        Ok(Self {
            expected,
            target,
            scope,
            by_type,
            calls,
        })
    }

    /// Visit the finite grammar in context order. Returning false stops before
    /// accepting the current proposal; true means the entire grammar was seen.
    fn enumerate(&self, visit: &mut impl FnMut(Value) -> bool) -> bool {
        for place in &self.scope {
            if place.ty == self.expected && !visit(place_expression(place.name)) {
                return false;
            }
        }
        for call in &self.calls {
            if call.target == self.target || call.result != self.expected || !call.within_budget {
                continue;
            }
            // Lists point into the bounded scope index. Neither the Cartesian
            // product nor its potentially overflowing cardinality is allocated.
            let Some(options) = call
                .parameters
                .iter()
                .map(|ty| self.by_type.get(ty))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let mut positions = vec![0; options.len()];
            loop {
                let arguments = options
                    .iter()
                    .zip(&positions)
                    .map(|(names, index)| place_expression(names[*index]))
                    .collect::<Vec<_>>();
                if !visit(json!({"kind":"call","target":call.target,"arguments":arguments})) {
                    return false;
                }
                if !advance(&mut positions, &options) {
                    break;
                }
            }
        }
        true
    }
}

fn advance(positions: &mut [usize], options: &[&Vec<&str>]) -> bool {
    for index in (0..positions.len()).rev() {
        positions[index] += 1;
        if positions[index] < options[index].len() {
            return true;
        }
        positions[index] = 0;
    }
    false
}

fn place_expression(name: &str) -> Value {
    json!({"kind":"place","name":name})
}

fn array<'a>(value: &'a Value, key: &str, maximum: usize) -> Result<&'a [Value]> {
    let items = value[key]
        .as_array()
        .ok_or_else(|| grammar("hole suggestions require a compiler array field"))?;
    if items.len() > maximum {
        return Err(capacity(
            "hole suggestion metadata inventory exceeds its bound",
        ));
    }
    Ok(items)
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| grammar("hole suggestions require a compiler text field"))
}

fn bounded_text<'a>(value: &'a Value, key: &str, maximum: usize) -> Result<&'a str> {
    let text = text(value, key)?;
    if text.len() > maximum {
        return Err(capacity(
            "hole suggestion identifier exceeds its constructor bound",
        ));
    }
    Ok(text)
}

fn ownership(value: &Value) -> Result<()> {
    if !matches!(
        text(value, "ownership")?,
        "value" | "own" | "borrow" | "shared"
    ) {
        return Err(grammar("hole suggestion ownership fact is unsupported"));
    }
    Ok(())
}

fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G230", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G231", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(name: &str, ty: &str) -> Value {
        json!({"id":name,"name":name,"type_id":ty,"ownership":"value"})
    }
    fn call(target: &str, result: &str, parameters: &[&str], permitted: bool) -> Value {
        json!({"id":target,"return_type_id":result,"within_effect_budget":permitted,
            "parameters":parameters.iter().map(|ty|json!({"type_id":ty,"ownership":"value"})).collect::<Vec<_>>()})
    }
    fn context(scope: Vec<Value>, calls: Vec<Value>) -> Value {
        json!({"target":"selected","expected_type_id":"i64","scope":scope,"accessible_calls":calls})
    }

    #[test]
    fn enumeration_preserves_scope_and_cartesian_order_and_filters_effects_and_self() {
        let context = context(
            vec![
                scope("left", "i64"),
                scope("flag", "bool"),
                scope("right", "i64"),
            ],
            vec![
                call("selected", "i64", &[], true),
                call("effectful", "i64", &[], false),
                call("wrong_result", "bool", &[], true),
                call("missing_argument", "i64", &["usize"], true),
                call("zero", "i64", &[], true),
                call("pair", "i64", &["i64", "i64"], true),
            ],
        );
        let inventory = Inventory::derive(&context, true).unwrap();
        let mut rows = Vec::new();
        assert!(inventory.enumerate(&mut |row| {
            rows.push(row);
            true
        }));
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0], place_expression("left"));
        assert_eq!(rows[1], place_expression("right"));
        assert_eq!(
            rows[2],
            json!({"kind":"call","target":"zero","arguments":[]})
        );
        for (row, (left, right)) in rows[3..].iter().zip([
            ("left", "left"),
            ("left", "right"),
            ("right", "left"),
            ("right", "right"),
        ]) {
            assert_eq!(
                row,
                &json!({"kind":"call","target":"pair",
                "arguments":[place_expression(left),place_expression(right)]})
            );
        }
    }

    #[test]
    fn lookahead_distinguishes_exact_limit_and_metadata_overflow_is_not_skipped() {
        for count in [32, 33] {
            let context = context(
                (0..count)
                    .map(|index| scope(&format!("p{index}"), "i64"))
                    .collect(),
                vec![],
            );
            let inventory = Inventory::derive(&context, true).unwrap();
            let mut attempted = 0;
            let exhausted = inventory.enumerate(&mut |_| {
                if attempted == MAX_ATTEMPTS {
                    return false;
                }
                attempted += 1;
                true
            });
            assert_eq!(attempted, 32);
            assert_eq!(exhausted, count == 32);
        }
        let context = context(vec![], vec![call("selected", "bool", &["i64"; 65], false)]);
        let errors = Inventory::derive(&context, true).err().unwrap();
        assert_eq!(errors[0].code, "SPX-G231");
    }
}
