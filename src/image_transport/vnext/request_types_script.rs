//! Recursive script-language request types, without runtime schema admission.
//! Python defers field/element references and binds aliases after concrete
//! typing objects exist. No generated module evaluates a recursive alias eagerly.
use super::{Model, Shape};
use crate::diagnostic::Diagnostic;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_SOURCE_BYTES: usize = 900 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn typescript(model: &Model) -> Result<String> {
    let mut source = String::from(
        "// Recursive request types describe structure only; compiler admission remains mandatory.\n",
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
            Shape::Alias(name) => source.push_str(name),
            Shape::Array(item) => write!(source, "Array<{item}>").unwrap(),
            Shape::Tuple(items) => write!(source, "[{}]", items.join(", ")).unwrap(),
            Shape::Union(variants) => {
                if variants.is_empty() {
                    return Err(invalid("request union has no alternatives"));
                }
                source.push_str(&variants.join(" | "));
            }
            Shape::Object { fields, open } => {
                source.push_str("{\n");
                if *open {
                    source.push_str("  [key: string]: unknown;\n");
                } else if fields.is_empty() {
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
    let mut source = String::from(concat!(
        "# Recursive request types describe structure only; compiler admission remains mandatory.\n",
        "from typing import Any, Literal, Never, NotRequired, TypeAlias, TypedDict, Union\n",
    ));
    let definitions = model
        .definitions
        .iter()
        .map(|definition| (definition.name.clone(), &definition.shape))
        .collect::<BTreeMap<_, _>>();
    if definitions.len() != model.definitions.len() {
        return Err(invalid("request type names collide"));
    }
    // This first pass never evaluates a named dependency. Functional TypedDict
    // keeps arbitrary wire keys and immediate NotRequired metadata; strings are
    // ForwardRefs, not a module-wide postponed-annotation approximation.
    for definition in &model.definitions {
        match &definition.shape {
            Shape::Alias(_) => continue,
            Shape::Object {
                fields,
                open: false,
            } => {
                writeln!(
                    source,
                    "{} = TypedDict({}, {{",
                    definition.name,
                    quoted(&definition.name)?
                )
                .unwrap();
                for field in fields {
                    let reference = quoted(&field.ty)?;
                    writeln!(
                        source,
                        "    {}: {},",
                        quoted(&field.name)?,
                        if field.required {
                            reference
                        } else {
                            format!("NotRequired[{reference}]")
                        }
                    )
                    .unwrap();
                }
                source.push_str("})\n");
            }
            shape => {
                write!(source, "{}: TypeAlias = ", definition.name).unwrap();
                match shape {
                    Shape::Any => source.push_str("Any"),
                    Shape::Bool => source.push_str("bool"),
                    Shape::Integer => source.push_str("int"),
                    Shape::String => source.push_str("str"),
                    Shape::Null => source.push_str("None"),
                    Shape::Literal(value) => source.push_str(&py_literal(value)?),
                    Shape::Array(item) => write!(source, "list[{}]", quoted(item)?).unwrap(),
                    Shape::Tuple(items) => {
                        // JSON constructor arrays are Python lists. Their exact
                        // constant length/order remains compiler-checked.
                        write!(source, "list[{}]", py_forward_union(items)?).unwrap();
                    }
                    Shape::Union(variants) => {
                        if variants.is_empty() {
                            return Err(invalid("request union has no alternatives"));
                        }
                        // Even one alternative stays inside Union: assigning
                        // a raw quoted name here would produce a string rather
                        // than a typing ForwardRef or a concrete type object.
                        let names = variants
                            .iter()
                            .map(|name| quoted(name))
                            .collect::<Result<Vec<_>>>()?;
                        write!(source, "Union[{}]", names.join(", ")).unwrap();
                    }
                    Shape::Object { open: true, .. } => {
                        // Python 3.11 cannot express TypedDict extra-item types.
                        source.push_str("dict[str, Any]");
                    }
                    Shape::Alias(_) | Shape::Object { open: false, .. } => {
                        unreachable!("handled above")
                    }
                }
                source.push('\n');
            }
        }
        bound(&source)?;
    }
    // Alias chains are semantically transparent. Resolve them to the already
    // emitted non-alias typing object, so public parameter aliases are real
    // TypedDict/type objects even when source documents forward-reference roots.
    for definition in &model.definitions {
        if let Shape::Alias(target) = &definition.shape {
            let terminal = terminal_alias(target, &definitions)?;
            writeln!(source, "{}: TypeAlias = {terminal}", definition.name).unwrap();
            bound(&source)?;
        }
    }
    Ok(source)
}

fn terminal_alias(start: &str, definitions: &BTreeMap<String, &Shape>) -> Result<String> {
    let mut current = start.to_owned();
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current.clone()) {
            return Err(invalid("request alias cycle has no structural constructor"));
        }
        match definitions.get(&current) {
            Some(Shape::Alias(next)) => current = next.clone(),
            Some(_) => return Ok(current),
            None => return Err(invalid("request alias names an absent definition")),
        }
    }
}

fn py_forward_union(items: &[String]) -> Result<String> {
    let names = items
        .iter()
        .map(|name| quoted(name))
        .collect::<Result<Vec<_>>>()?;
    Ok(match names.as_slice() {
        [] => "Never".into(),
        [only] => only.clone(),
        _ => format!("Union[{}]", names.join(", ")),
    })
}

fn ts_literal(value: &Value) -> Result<String> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => encode(value),
        Value::Number(number) => {
            let safe = number
                .as_u64()
                .is_some_and(|value| value <= MAX_SAFE_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|value| value.unsigned_abs() <= MAX_SAFE_INTEGER);
            if safe {
                encode(value)
            } else {
                Ok("number".into())
            }
        }
        _ => Err(invalid(
            "request literal was not lowered to a structural shape",
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
        Value::String(_) => Ok(format!("Literal[{}]", encode(value)?)),
        Value::Number(number) if number.is_i64() || number.is_u64() => {
            Ok(format!("Literal[{number}]"))
        }
        Value::Number(_) => Ok("float".into()),
        _ => Err(invalid(
            "request literal was not lowered to a structural shape",
        )),
    }
}
fn quoted(value: &str) -> Result<String> {
    serde_json::to_string(value).map_err(|_| invalid("request type name encoding failed"))
}
fn encode(value: &Value) -> Result<String> {
    serde_json::to_string(value).map_err(|_| invalid("request literal encoding failed"))
}
fn bound(source: &str) -> Result<()> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diagnostic::io(
            "SPX-G289",
            "script request types exceed 900 KiB",
        )]);
    }
    Ok(())
}
fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G288", message)]
}

#[cfg(test)]
mod tests {
    use super::super::{Definition, Field};
    use super::*;
    use serde_json::json;

    #[test]
    fn recursive_fields_are_forward_references_and_aliases_bind_concrete_objects() {
        let model = Model {
            definitions: vec![
                Definition {
                    name: "RequestType0000".into(),
                    shape: Shape::Alias("RequestType0001".into()),
                },
                Definition {
                    name: "RequestType0001".into(),
                    shape: Shape::Object {
                        open: false,
                        fields: vec![
                            Field {
                                name: "class".into(),
                                ty: "RequestType0002".into(),
                                required: true,
                            },
                            Field {
                                name: "next-value".into(),
                                ty: "RequestType0000".into(),
                                required: false,
                            },
                        ],
                    },
                },
                Definition {
                    name: "RequestType0002".into(),
                    shape: Shape::Literal(json!("node")),
                },
            ],
            params: BTreeMap::new(),
        };
        let source = python(&model).unwrap();
        assert!(source.contains("\"class\": \"RequestType0002\""));
        assert!(source.contains("\"next-value\": NotRequired[\"RequestType0000\"]"));
        assert!(source.contains("RequestType0000: TypeAlias = RequestType0001"));
        assert!(
            source.find("RequestType0001 = TypedDict").unwrap()
                < source.find("RequestType0000: TypeAlias").unwrap()
        );
        let source = typescript(&model).unwrap();
        assert!(source.contains("\"next-value\"?: RequestType0000"));
        assert!(source.contains("export type RequestType0000 = RequestType0001;"));
    }

    #[test]
    fn unproductive_alias_cycles_and_missing_names_fail_closed() {
        let left = Shape::Alias("Right".into());
        let right = Shape::Alias("Left".into());
        let definitions = BTreeMap::from([("Left".into(), &left), ("Right".into(), &right)]);
        assert!(terminal_alias("Left", &definitions).is_err());
        assert!(terminal_alias("Absent", &definitions).is_err());
    }
}
