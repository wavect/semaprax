//! Static script-language types for the already audited response schema model.
//! Runtime decoders retain all exact shape, identity and integer checks.
use super::{Model, Shape};
use crate::diagnostic::Diagnostic;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_SCRIPT_TYPES_BYTES: usize = 900 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn typescript(model: &Model) -> Result<String> {
    let mut source = String::from(
        "// Static response types supplement, but never replace, the runtime decoder.\n\
         export type TypedResultEnvelope<T> = Omit<ResultEnvelope, \"payload\"> & { payload: T };\n",
    );
    for definition in &model.definitions {
        write!(source, "export type {} = ", definition.name).unwrap();
        match &definition.shape {
            Shape::Any => source.push_str("unknown"),
            Shape::Bool => source.push_str("boolean"),
            Shape::Integer => source.push_str("number"),
            Shape::String => source.push_str("string"),
            Shape::Null => source.push_str("null"),
            Shape::Literal(value) => source.push_str(&ts_literal(value)?),
            Shape::Array(item) => write!(source, "Array<{item}>").unwrap(),
            Shape::Tuple(items) => write!(source, "[{}]", items.join(", ")).unwrap(),
            Shape::Alias(name) => source.push_str(name),
            Shape::Union(variants) => {
                if variants.is_empty() {
                    return Err(invalid("response union has no alternatives"));
                }
                source.push_str(&variants.join(" | "));
            }
            Shape::Object { fields, open } => {
                source.push_str("{\n");
                if *open {
                    source.push_str("  [key: string]: unknown;\n");
                } else if fields.is_empty() {
                    // `{}` also admits scalar values in TypeScript. An empty
                    // closed JSON object needs an index signature of never.
                    source.push_str("  [key: string]: never;\n");
                }
                for field in fields {
                    writeln!(
                        source,
                        "  {}{}: {};",
                        quoted(&field.name)?,
                        if field.required { "" } else { "?" },
                        field.ty
                    )
                    .unwrap();
                }
                source.push('}');
            }
        }
        source.push_str(";\n");
        bound(&source)?;
    }
    Ok(source)
}

pub(super) fn python(model: &Model) -> Result<String> {
    // Preserve eager references for existing acyclic models. Recursive and
    // forward references remain quoted typing references until every generated
    // definition exists; postponed aliases resolve to real structural objects.
    let mut source = String::from(concat!(
        "# Static response types supplement, but never replace, the runtime decoder.\n",
        "from typing import Any, Generic, Literal, Never, NotRequired, TypeAlias, TypeVar, TypedDict, Union, cast\n",
        "_ResponsePayload = TypeVar('_ResponsePayload')\n",
        "class TypedResultEnvelope(TypedDict, Generic[_ResponsePayload]):\n",
        "    schema: str\n",
        "    protocol: str\n",
        "    image_revision: str\n",
        "    project_revision: str\n",
        "    payload: _ResponsePayload\n\n",
    ));
    let definitions = model
        .definitions
        .iter()
        .map(|definition| (definition.name.as_str(), &definition.shape))
        .collect::<BTreeMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut postponed = Vec::new();
    for definition in &model.definitions {
        if let Shape::Alias(target) = &definition.shape {
            if !emitted.contains(target) {
                postponed.push(definition);
                continue;
            }
        }
        if let Shape::Object {
            fields,
            open: false,
        } = &definition.shape
        {
            writeln!(
                source,
                "{} = TypedDict({}, {{",
                definition.name,
                quoted(&definition.name)?
            )
            .unwrap();
            for field in fields {
                writeln!(
                    source,
                    "    {}: {},",
                    quoted(&field.name)?,
                    if field.required {
                        py_reference(&field.ty, &emitted)?
                    } else {
                        format!("NotRequired[{}]", py_reference(&field.ty, &emitted)?)
                    }
                )
                .unwrap();
            }
            source.push_str("})\n");
        } else {
            write!(source, "{}: TypeAlias = ", definition.name).unwrap();
            match &definition.shape {
                Shape::Any => source.push_str("Any"),
                Shape::Bool => source.push_str("bool"),
                Shape::Integer => source.push_str("int"),
                Shape::String => source.push_str("str"),
                Shape::Null => source.push_str("None"),
                Shape::Literal(value) => source.push_str(&py_literal(value)?),
                Shape::Array(item) => {
                    write!(source, "list[{}]", py_reference(item, &emitted)?).unwrap()
                }
                Shape::Tuple(items) => {
                    // JSON arrays decode as lists, never Python tuples. Exact
                    // length/order of constant arrays stays a runtime check.
                    let items = items
                        .iter()
                        .map(|item| py_reference(item, &emitted))
                        .collect::<Result<Vec<_>>>()?;
                    write!(source, "list[{}]", py_union(&items, "Never")).unwrap();
                }
                Shape::Alias(name) => source.push_str(name),
                Shape::Union(variants) => {
                    if variants.is_empty() {
                        return Err(invalid("response union has no alternatives"));
                    }
                    let variants = variants
                        .iter()
                        .map(|item| py_reference(item, &emitted))
                        .collect::<Result<Vec<_>>>()?;
                    source.push_str(&py_union(&variants, "Never"));
                }
                Shape::Object { open: true, .. } => {
                    // Python 3.11 has no TypedDict extra-items type. A normal
                    // mapping avoids pretending its open keys are closed.
                    source.push_str("dict[str, Any]");
                }
                Shape::Object { open: false, .. } => unreachable!("handled above"),
            }
            source.push('\n');
        }
        emitted.insert(definition.name.clone());
        bound(&source)?;
    }
    let mut terminals = BTreeMap::new();
    let mut work = 0;
    for definition in postponed {
        let terminal = terminal_alias(&definition.name, &definitions, &mut terminals, &mut work)?;
        writeln!(source, "{}: TypeAlias = {terminal}", definition.name).unwrap();
        bound(&source)?;
    }
    Ok(source)
}

fn py_reference(name: &str, emitted: &BTreeSet<String>) -> Result<String> {
    if emitted.contains(name) {
        Ok(name.to_owned())
    } else {
        quoted(name)
    }
}

fn terminal_alias(
    name: &str,
    definitions: &BTreeMap<&str, &Shape>,
    terminals: &mut BTreeMap<String, String>,
    work: &mut usize,
) -> Result<String> {
    let mut current = name;
    let mut path = BTreeSet::new();
    let terminal = loop {
        *work += 1;
        if *work > 65_536 || path.len() > 128 {
            return Err(super::capacity(
                "typed Python response alias traversal exceeds its bound",
            ));
        }
        if let Some(terminal) = terminals.get(current) {
            break terminal.clone();
        }
        if !path.insert(current.to_owned()) {
            return Err(invalid(
                "response alias cycle has no structural constructor",
            ));
        }
        match definitions.get(current) {
            Some(Shape::Alias(next)) => current = next,
            Some(_) => break current.to_owned(),
            None => return Err(invalid("response alias names an absent definition")),
        }
    };
    for name in path {
        terminals.insert(name, terminal.clone());
    }
    Ok(terminal)
}

fn py_union(items: &[String], empty: &str) -> String {
    match items {
        [] => empty.to_owned(),
        [only] => only.clone(),
        _ => format!("Union[{}]", items.join(", ")),
    }
}

fn ts_literal(value: &Value) -> Result<String> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => quoted_value(value),
        Value::Number(number) => {
            let safe = number
                .as_u64()
                .is_some_and(|value| value <= MAX_SAFE_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|value| value.unsigned_abs() <= MAX_SAFE_INTEGER);
            if safe {
                quoted_value(value)
            } else {
                // Do not manufacture a rounded numeric literal type. The
                // existing decoder still rejects unsafe integer responses.
                Ok("number".into())
            }
        }
        _ => Err(invalid(
            "response literal was not lowered to a structural shape",
        )),
    }
}

fn py_literal(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("None".into()),
        Value::Bool(value) => Ok(format!(
            "Literal[{}]",
            if *value { "True" } else { "False" }
        )),
        Value::String(_) => Ok(format!("Literal[{}]", quoted_value(value)?)),
        Value::Number(number) if number.is_i64() || number.is_u64() => {
            Ok(format!("Literal[{number}]"))
        }
        Value::Number(_) => Ok("float".into()),
        _ => Err(invalid(
            "response literal was not lowered to a structural shape",
        )),
    }
}

fn quoted(value: &str) -> Result<String> {
    serde_json::to_string(value).map_err(|_| invalid("response type string encoding failed"))
}
fn quoted_value(value: &Value) -> Result<String> {
    serde_json::to_string(value).map_err(|_| invalid("response literal encoding failed"))
}
fn bound(source: &str) -> Result<()> {
    if source.len() > MAX_SCRIPT_TYPES_BYTES {
        return Err(vec![Diagnostic::io(
            "SPX-G289",
            "generated script response types exceed 900 KiB",
        )]);
    }
    Ok(())
}
fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G288", message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn script_literals_preserve_each_language_representation() {
        assert_eq!(ts_literal(&json!("a\"b\n")).unwrap(), "\"a\\\"b\\n\"");
        assert_eq!(ts_literal(&json!(u64::MAX)).unwrap(), "number");
        assert_eq!(ts_literal(&json!(-42)).unwrap(), "-42");
        assert_eq!(py_literal(&json!(true)).unwrap(), "Literal[True]");
        assert_eq!(py_literal(&Value::Null).unwrap(), "None");
        assert_eq!(
            py_literal(&json!(u64::MAX)).unwrap(),
            "Literal[18446744073709551615]"
        );
        assert_eq!(py_literal(&json!(0.5)).unwrap(), "float");
        assert!(py_literal(&json!([])).is_err());
    }
}
