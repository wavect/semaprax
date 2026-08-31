//! Compact descriptive navigation over exact existing typed-hole context bytes.
use serde_json::{json, Value};

use super::{wire, ProjectCandidateDraft};
use crate::diagnostic::Diagnostic;

pub const PROJECT_HOLE_SUMMARY_SCHEMA: &str = "semaprax.project-hole-summary.v1";
pub const PROJECT_HOLE_PAGE_SCHEMA: &str = "semaprax.project-hole-page.v1";
pub const MAX_PROJECT_HOLE_NAVIGATION_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_HOLE_NAVIGATION_ITEMS: usize = 16_384;
const MAX_CONTEXT_BYTES: usize = 1024 * 1024;
const CONTEXT_DOMAIN: &[u8] = b"semaprax.project-hole-context.v1\0";
const REFERENCE_DOMAIN: &[u8] = b"semaprax.project-hole-facet.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectCandidateDraft {
    /// Regenerate the ordinary authenticated context and describe four compact
    /// facets. This is neither a validity receipt nor owned-value liveness.
    /// The original complete context remains available through `hole_context`.
    pub fn hole_summary(&self, expected_draft: &str, hole_id: &str) -> Result<String> {
        let navigation = Navigation::derive(self, expected_draft, hole_id)?;
        render(navigation.summary)
    }

    /// Expand a reference bound to this exact draft, hole and context revision.
    /// Return the largest fitting ordered prefix up to `limit` (1..=64), with
    /// no partial item or zero-progress continuation. No cache or authority is
    /// acquired; every call first regenerates the existing full context.
    pub fn hole_page(
        &self,
        expected_draft: &str,
        hole_id: &str,
        reference: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String> {
        let navigation = Navigation::derive(self, expected_draft, hole_id)?;
        page(&navigation, reference, offset, limit)
    }
}

fn page(navigation: &Navigation, reference: &str, offset: usize, limit: usize) -> Result<String> {
    let facet = navigation
        .facets
        .iter()
        .find(|facet| facet.reference == reference)
        .ok_or_else(|| {
            stale("hole facet reference does not match this exact draft, hole and context")
        })?;
    if offset > MAX_PROJECT_HOLE_NAVIGATION_ITEMS
        || offset > facet.items.len()
        || !(1..=64).contains(&limit)
    {
        return Err(grammar(
            "hole page offset or limit is outside its bounded inventory",
        ));
    }
    let mut items = Vec::new();
    let page = |items: &[Value]| {
        json!({
            "schema":PROJECT_HOLE_PAGE_SCHEMA,
            "draft_revision":navigation.summary["draft_revision"],
            "hole_id":navigation.summary["hole_id"],
            "context_revision":navigation.summary["context_revision"],
            "facet":facet.name,"reference":facet.reference,
            "total":facet.items.len(),"offset":offset,
            "next_offset":(offset+items.len()<facet.items.len()).then_some(offset+items.len()),
            "items":items,"source_authority":false,
        })
    };
    let mut encoded = render(page(&items))?;
    for item in facet.items.iter().skip(offset).take(limit) {
        items.push(item.clone());
        match render(page(&items)) {
            Ok(next) => encoded = next,
            Err(_) if items.len() == 1 => {
                return Err(capacity(
                    "the first hole facet item cannot fit one complete page",
                ))
            }
            Err(_) => break,
        }
    }
    Ok(encoded)
}

struct Facet {
    name: &'static str,
    items: Vec<Value>,
    reference: String,
}
struct Navigation {
    summary: Value,
    facets: Vec<Facet>,
}

impl Navigation {
    fn derive(draft: &ProjectCandidateDraft, expected: &str, hole_id: &str) -> Result<Self> {
        // This owns selector authentication, hole existence, retained-source
        // joins and prior-proof generation. Never reinterpret stored draft JSON.
        let bytes = draft.hole_context(expected, hole_id)?;
        if bytes.len() > MAX_CONTEXT_BYTES {
            return Err(capacity("hole context exceeds its existing byte bound"));
        }
        let context_revision = wire::digest(CONTEXT_DOMAIN, bytes.as_bytes());
        let mut context: Value = serde_json::from_str(&bytes)
            .map_err(|_| grammar("compiler hole context is not bounded JSON"))?;
        let schema = text(&context, "schema")?.to_owned();
        let body = schema == "semaprax.project-candidate-hole-context.v1";
        let contract = schema == "semaprax.project-candidate-contract-expression-hole-context.v1";
        if !body && !contract && schema != "semaprax.project-candidate-expression-hole-context.v1" {
            return Err(grammar(
                "hole navigation does not recognize the context owner",
            ));
        }
        for key in [
            "draft_digest",
            "hole_id",
            "hole_handle",
            "target",
            "last_valid_revision",
            "expected_type_id",
            "intent_kind",
            "validation",
        ] {
            text(&context, key)?;
        }
        if context["draft_digest"] != expected
            || context["hole_id"] != hole_id
            || context["materializable"] != false
            || context["source_authority"] != false
            || context["validation"] != "pending_fill_full_source_replay"
        {
            return Err(grammar("hole navigation context bindings are inconsistent"));
        }
        let expected_ownership = if body {
            Value::Null
        } else {
            ownership(&context["expected_ownership"])?;
            context["expected_ownership"].clone()
        };
        let policy = &context["effect_policy"];
        strings(&policy["allowed"])?;
        strings(&policy["module_permits"])?;
        text(policy, "forbidden")?;
        let enclosing = if contract {
            strings(&policy["enclosing_declared_effects"])?;
            policy["enclosing_declared_effects"].clone()
        } else {
            Value::Null
        };
        let effect_policy = json!({"allowed":policy["allowed"],"forbidden":policy["forbidden"],
            "module_permits":policy["module_permits"],"enclosing_declared_effects":enclosing});

        let mut scope = take_array(&mut context, "scope")?;
        for row in &mut scope {
            let id = text(row, if body { "id" } else { "value_id" })?;
            let name = text(row, "name")?;
            let ty = text(row, if body { "type_id" } else { "type" })?;
            ownership(&row["ownership"])?;
            let mutable = if body {
                Value::Null
            } else {
                if !row["mutable"].is_boolean() {
                    return Err(grammar("expression scope mutability is absent"));
                }
                row["mutable"].clone()
            };
            *row = json!({"id":id,"name":name,"type_id":ty,"ownership":row["ownership"],"mutable":mutable});
        }
        let calls = take_array(&mut context, "accessible_calls")?;
        for call in &calls {
            validate_call(call)?;
        }
        let obligations = take_array(&mut context, "obligations")?;
        let constructors = take_array(&mut context, "constructor_kinds")?;
        if obligations
            .iter()
            .chain(&constructors)
            .any(|value| !value.is_string())
        {
            return Err(grammar(
                "hole obligations and constructor kinds must be strings",
            ));
        }
        let mut facets = Vec::new();
        for (name, items) in [
            ("scope", scope),
            ("calls", calls),
            ("obligations", obligations),
            ("constructors", constructors),
        ] {
            let binding = render(
                json!({"draft_revision":expected,"hole_id":hole_id,"context_revision":context_revision,"facet":name}),
            )?;
            facets.push(Facet {
                name,
                items,
                reference: wire::digest(REFERENCE_DOMAIN, binding.as_bytes()),
            });
        }
        let summary = json!({
            "schema":PROJECT_HOLE_SUMMARY_SCHEMA,"context_schema":schema,
            "context_revision":context_revision,"draft_revision":expected,"hole_id":hole_id,
            "hole_handle":context["hole_handle"],"target":context["target"],
            "last_valid_revision":context["last_valid_revision"],
            "expected_type_id":context["expected_type_id"],"expected_ownership":expected_ownership,
            "intent_kind":context["intent_kind"],"effect_policy":effect_policy,
            "facets":facets.iter().map(|facet|json!({"facet":facet.name,"count":facet.items.len(),"reference":facet.reference})).collect::<Vec<_>>(),
            "full_context_method":"hole/query","materializable":false,"source_authority":false,
            "validation":"pending_fill_full_source_replay","evidence_class":"descriptive_context_not_candidate_validation",
        });
        Ok(Self { summary, facets })
    }
}

fn validate_call(call: &Value) -> Result<()> {
    let object = call
        .as_object()
        .ok_or_else(|| grammar("hole call row must be an object"))?;
    if object.len() != 8 {
        return Err(grammar("hole call row has an unsupported shape"));
    }
    for key in ["id", "binding", "return_type_id", "basis", "admission"] {
        text(call, key)?;
    }
    if !call["within_effect_budget"].is_boolean() {
        return Err(grammar("hole call budget fact is absent"));
    }
    strings(&call["effects"])?;
    let parameters = call["parameters"]
        .as_array()
        .ok_or_else(|| grammar("hole call parameter inventory is absent"))?;
    for parameter in parameters {
        if parameter.as_object().is_none_or(|object| object.len() != 3) {
            return Err(grammar("hole call parameter has an unsupported shape"));
        }
        text(parameter, "name")?;
        text(parameter, "type_id")?;
        ownership(&parameter["ownership"])?;
    }
    Ok(())
}
fn ownership(value: &Value) -> Result<()> {
    if !value
        .as_str()
        .is_some_and(|value| matches!(value, "value" | "own" | "borrow" | "shared"))
    {
        return Err(grammar("hole ownership fact has an unsupported shape"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| grammar("hole navigation requires a compiler text field"))
}
fn strings(value: &Value) -> Result<()> {
    if !value
        .as_array()
        .is_some_and(|values| values.iter().all(Value::is_string))
    {
        return Err(grammar("hole effect policy requires a string array"));
    }
    Ok(())
}
fn take_array(context: &mut Value, key: &str) -> Result<Vec<Value>> {
    let values = context
        .as_object_mut()
        .and_then(|object| object.remove(key))
        .ok_or_else(|| grammar("hole navigation facet is absent"))?;
    let Value::Array(values) = values else {
        return Err(grammar("hole navigation facet must be an array"));
    };
    if values.len() > MAX_PROJECT_HOLE_NAVIGATION_ITEMS {
        return Err(capacity("hole facet exceeds its navigation item bound"));
    }
    Ok(values)
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_HOLE_NAVIGATION_BYTES)
        .map_err(|_| capacity("hole navigation report exceeds 64 KiB"))
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G230", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G231", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G232", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory(items: Vec<Value>) -> Navigation {
        Navigation {
            summary: json!({"draft_revision":format!("sha256:{}","1".repeat(64)),"hole_id":"H",
                "context_revision":format!("sha256:{}","2".repeat(64))}),
            facets: vec![Facet {
                name: "obligations",
                items,
                reference: format!("sha256:{}", "3".repeat(64)),
            }],
        }
    }

    #[test]
    fn byte_bound_returns_complete_ordered_prefix_and_exact_continuation() {
        let navigation = inventory(vec![
            json!("a".repeat(40_000)),
            json!("b".repeat(40_000)),
            json!("last"),
        ]);
        let reference = &navigation.facets[0].reference;
        let first = page(&navigation, reference, 0, 64).unwrap();
        assert!(first.len() <= MAX_PROJECT_HOLE_NAVIGATION_BYTES);
        let first: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(first["items"], json!(["a".repeat(40_000)]));
        assert_eq!(first["next_offset"], 1);
        let second = page(&navigation, reference, 1, 64).unwrap();
        assert!(second.len() <= MAX_PROJECT_HOLE_NAVIGATION_BYTES);
        let second: Value = serde_json::from_str(&second).unwrap();
        assert_eq!(second["items"], json!(["b".repeat(40_000), "last"]));
        assert!(second["next_offset"].is_null());
        let limited: Value =
            serde_json::from_str(&page(&navigation, reference, 1, 1).unwrap()).unwrap();
        assert_eq!(limited["items"].as_array().unwrap().len(), 1);
        assert_eq!(limited["next_offset"], 2);
    }

    #[test]
    fn empty_end_pages_and_oversized_first_items_never_loop() {
        let empty = inventory(Vec::new());
        let result: Value =
            serde_json::from_str(&page(&empty, &empty.facets[0].reference, 0, 16).unwrap())
                .unwrap();
        assert_eq!(result["items"], json!([]));
        assert!(result["next_offset"].is_null());
        let oversized = inventory(vec![json!("\u{0001}".repeat(20_000))]);
        assert_eq!(
            page(&oversized, &oversized.facets[0].reference, 0, 64)
                .err()
                .unwrap()[0]
                .code,
            "SPX-G231"
        );
        assert_eq!(
            page(&empty, "foreign", 0, 16).err().unwrap()[0].code,
            "SPX-G232"
        );
        for (offset, limit) in [(1, 16), (0, 0), (0, 65), (16_385, 16)] {
            assert_eq!(
                page(&empty, &empty.facets[0].reference, offset, limit)
                    .err()
                    .unwrap()[0]
                    .code,
                "SPX-G230"
            );
        }
    }
}
