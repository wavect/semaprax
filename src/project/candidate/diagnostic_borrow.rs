//! One structural repair proposal, authenticated by ordinary candidate replay.
//! A rejected diagnostic span is never treated as a checked source reference.
use super::*;
use crate::project::candidate::intent::{MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES};

const CLASS: &str = "borrow_owned_byte_field_without_staging";

pub(super) fn repair(
    attempt: &ProjectCandidateAttempt,
    target: &str,
) -> Result<(Option<Proposal>, &'static str), Vec<Diagnostic>> {
    let mut repaired = attempt.change.intent.clone();
    let mut replacements = Vec::new();
    let mut nodes = 0;
    if !expression(&mut repaired["body"], &mut replacements, &mut nodes, 0)?
        || replacements.is_empty()
    {
        return Ok((None, "no_supported_direct_owned_byte_field_projection"));
    }
    let change = SemanticChange::new(attempt.base.revision().project_revision(), &repaired)?;
    let candidate = match attempt.base.apply(attempt.base.candidate_digest(), &change) {
        Ok(candidate) => Arc::new(candidate),
        Err(_) => return Ok((None, "derived_change_failed_full_candidate_admission")),
    };
    let change_value: Value = serde_json::from_str(change.to_json())
        .map_err(|_| grammar("derived repair change is invalid"))?;
    let identity = render(json!({
        "attempt_revision":attempt.digest,"class":CLASS,"change":change_value
    }))?;
    let id = wire::digest(REPAIR_DOMAIN, identity.as_bytes());
    let semantic_change_intent = json!({"kind":"repair_diagnostic","target":target,
        "rejected_intent":attempt.change.intent,"repair_id":id});
    // Nested rejected constructors must still fit the ordinary wire owner.
    // Do not advertise an unusable history-preserving selection.
    if SemanticChange::new(
        attempt.base.revision().project_revision(),
        &semantic_change_intent,
    )
    .is_err()
    {
        return Ok((None, "repair_intention_exceeds_semantic_change_bounds"));
    }
    let description = json!({
        "repair_id":id,"class":CLASS,"target":target,"diagnostic_code":"SPX-T266",
        "replacement_count":replacements.len(),"replacements":replacements,
        "change":change_value,
        "semantic_change_intent":semantic_change_intent,
        "validated_candidate_revision":candidate.candidate_digest(),
        "validation":"normal_full_candidate_apply",
        "evidence_owner":"closed_builtin_projection_pattern_and_full_candidate_admission",
        "tests":"not_run","source_authority":false
    });
    Ok((
        Some(Proposal {
            id,
            description,
            candidate,
            change,
        }),
        "one_compiler_admitted_typed_repair",
    ))
}

/// Traverse only expression-bearing positions in the existing typed grammar.
/// Binder/type/pattern metadata is preserved verbatim. Whole-candidate
/// construction remains the authority for the complete closed node grammar.
fn expression(
    value: &mut Value,
    replacements: &mut Vec<Value>,
    nodes: &mut usize,
    depth: usize,
) -> Result<bool, Vec<Diagnostic>> {
    *nodes += 1;
    if *nodes > MAX_EXPRESSION_NODES || depth > MAX_EXPRESSION_DEPTH {
        return Err(capacity(
            "borrow repair exceeds the expression traversal bound",
        ));
    }
    if value["kind"] == "builtin_call" && value["target"] == crate::byte_ops::BYTES_AS_SLICE_ID {
        if !closed(value, &["kind", "target", "arguments"]) {
            return Ok(false);
        }
        let Some(arguments) = value["arguments"].as_array_mut() else {
            return Ok(false);
        };
        if arguments.len() != 1 {
            return Ok(false);
        }
        let projected = &arguments[0];
        if projected["kind"] == "project" {
            let projection_closed = closed(projected, &["kind", "target", "base"])
                || (closed(projected, &["kind", "target", "base", "type_arguments"])
                    && projected["type_arguments"]
                        .as_array()
                        .is_some_and(Vec::is_empty));
            if !projection_closed
                || !closed(&projected["base"], &["kind", "name"])
                || projected["base"]["kind"] != "place"
            {
                return Ok(false);
            }
            let (Some(field), Some(root)) = (
                projected["target"].as_str(),
                projected["base"]["name"].as_str(),
            ) else {
                return Ok(false);
            };
            // This copies no source expression: the unchanged stable field
            // selector and lexical name are checked again by field_place and
            // the ordinary source verifier before a proposal can be offered.
            let replacement = json!({"kind":"field_place","target":field,"root":root});
            replacements.push(json!({"field":field,"root":root}));
            arguments[0] = replacement;
        }
    }

    let Some(kind) = value["kind"].as_str() else {
        return Ok(false);
    };
    match kind {
        "i64" | "i32" | "u8" | "usize" | "bool" | "string" | "array_u8" | "place"
        | "field_place" => Ok(true),
        "let" => children(value, &["value", "body"], replacements, nodes, depth),
        "binary" => children(value, &["left", "right"], replacements, nodes, depth),
        "unary" => children(value, &["value"], replacements, nodes, depth),
        "if" => children(
            value,
            &["condition", "then", "else"],
            replacements,
            nodes,
            depth,
        ),
        "project" => children(value, &["base"], replacements, nodes, depth),
        "call" | "builtin_call" => {
            let Some(arguments) = value["arguments"].as_array_mut() else {
                return Ok(false);
            };
            for argument in arguments {
                if !expression(argument, replacements, nodes, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "record" | "variant" | "update" => {
            if kind == "update" && !children(value, &["base"], replacements, nodes, depth)? {
                return Ok(false);
            }
            let Some(fields) = value["fields"].as_array_mut() else {
                return Ok(false);
            };
            for field in fields {
                if !children(field, &["value"], replacements, nodes, depth)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "match" => {
            if !children(value, &["value"], replacements, nodes, depth)? {
                return Ok(false);
            }
            let Some(arms) = value["arms"].as_array_mut() else {
                return Ok(false);
            };
            for arm in arms {
                if !children(arm, &["body"], replacements, nodes, depth)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn children(
    value: &mut Value,
    keys: &[&str],
    replacements: &mut Vec<Value>,
    nodes: &mut usize,
    depth: usize,
) -> Result<bool, Vec<Diagnostic>> {
    for key in keys {
        let Some(child) = value.get_mut(*key) else {
            return Ok(false);
        };
        if !expression(child, replacements, nodes, depth + 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn closed(value: &Value, keys: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_literal_siblings_remain_opaque_during_projection_repair() {
        let contents = "\0\n\\\"🦀{\"kind\":\"project\",\"target\":\"foreign.field\",\"base\":{\"kind\":\"place\",\"name\":\"foreign\"}}";
        let original = json!({"kind":"let","name":"text",
            "value":{"kind":"string","value":contents},
            "body":{"kind":"let","name":"bytes",
                "value":{"kind":"array_u8","values":[255,0,17,1]},
                "body":{"kind":"builtin_call","target":crate::byte_ops::BYTES_AS_SLICE_ID,
                    "arguments":[{"kind":"project","target":"packet.payload",
                        "base":{"kind":"place","name":"packet"}}]}}});
        let mut expected = original.clone();
        expected["body"]["body"]["arguments"][0] =
            json!({"kind":"field_place","target":"packet.payload","root":"packet"});
        let mut repaired = original.clone();
        let mut replacements = Vec::new();
        let mut nodes = 0;
        assert!(expression(&mut repaired, &mut replacements, &mut nodes, 0).unwrap());
        // This proves traversal preservation only; repair() must still admit
        // the complete derived candidate before exposing any proposal.
        assert_eq!(repaired, expected);
        assert_eq!(
            replacements,
            vec![json!({"field":"packet.payload","root":"packet"})]
        );

        let mut unsupported = original;
        unsupported["value"]["kind"] = json!("unknown_literal");
        let mut replacements = Vec::new();
        let mut nodes = 0;
        assert!(!expression(&mut unsupported, &mut replacements, &mut nodes, 0).unwrap());
        assert!(replacements.is_empty());
    }
}
