//! Finite Rust request representations for the recursive constructor schema IR.
//! Named recursive edges are boxed, including transparent aliases and arrays.
//! Serialization retains the schema's JSON shape; source admission stays server-owned.
use super::{Model, Result, Shape};
use std::collections::BTreeSet;
use std::fmt::Write;

const SUPPORT: &str = r#"
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestInteger { Signed(i64), Unsigned(u64) }

/// An omitted optional field differs from a present schema-admitted null.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestPresence<T> { Missing, Present(T) }
impl<T> Default for RequestPresence<T> { fn default()->Self { Self::Missing } }
impl<T> RequestPresence<T> { pub fn is_missing(&self)->bool { matches!(self,Self::Missing) } }
impl<'de,T:Deserialize<'de>> Deserialize<'de> for RequestPresence<T> {
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
        T::deserialize(deserializer).map(Self::Present)
    }
}
impl<T:Serialize> Serialize for RequestPresence<T> {
    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {
        match self {
            Self::Present(value)=>value.serialize(serializer),
            Self::Missing=>Err(serde::ser::Error::custom("missing request field must be omitted")),
        }
    }
}
"#;

const LITERAL_SUPPORT: &str = r#"
macro_rules! request_literal {
    ($name:ident, $ty:ty, $expected:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name;
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
                let value=<$ty>::deserialize(deserializer)?;
                if value==$expected { Ok(Self) } else { Err(serde::de::Error::custom("request literal mismatch")) }
            }
        }
        impl Serialize for $name {
            fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {
                Serialize::serialize(&$expected,serializer)
            }
        }
    };
}
"#;

pub(super) fn emit(model: &Model) -> Result<String> {
    let mut source = SUPPORT.to_owned();
    if model
        .definitions
        .iter()
        .any(|definition| matches!(definition.shape, Shape::Literal(_)))
    {
        source.push_str(LITERAL_SUPPORT);
    }
    for definition in &model.definitions {
        let name = &definition.name;
        match &definition.shape {
            // The shared builder emits Any only for an explicitly unconstrained
            // schema. Missing references and unrecognized constructors reject.
            Shape::Any => writeln!(source, "pub type {name} = Value;").unwrap(),
            Shape::Bool => writeln!(source, "pub type {name} = bool;").unwrap(),
            Shape::Integer => writeln!(source, "pub type {name} = RequestInteger;").unwrap(),
            Shape::String => writeln!(source, "pub type {name} = String;").unwrap(),
            Shape::Null => writeln!(source, "pub type {name} = ();").unwrap(),
            Shape::Alias(target) => transparent(&mut source, name, &format!("Box<{target}>")),
            Shape::Array(item) => transparent(&mut source, name, &format!("Vec<Box<{item}>>")),
            Shape::Tuple(items) => {
                if items.is_empty() {
                    // Unit serializes as null; the empty array must stay [].
                    transparent(&mut source, name, "[(); 0]");
                } else if items.len() <= 12 {
                    let fields = items
                        .iter()
                        .map(|item| format!("Box<{item}>"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    transparent(&mut source, name, &format!("({fields},)"));
                } else {
                    // std::fmt::Debug tuples stop at twelve. Preserve every
                    // position and exact sequence length beyond that boundary.
                    sequence(&mut source, name, items);
                }
            }
            Shape::Literal(value) => literal(&mut source, name, value)?,
            Shape::Union(alternatives) => {
                if alternatives.is_empty() {
                    return Err(super::invalid("typed Rust request union has no branches"));
                }
                writeln!(source,"#[derive(Clone, Debug, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum {name} {{").unwrap();
                for (index, alternative) in alternatives.iter().enumerate() {
                    writeln!(source, "    Choice{index}(Box<{alternative}>),").unwrap();
                }
                writeln!(source, "}}").unwrap();
            }
            Shape::Object { fields, open } => {
                writeln!(source, "#[derive(Clone, Debug, Serialize, Deserialize)]").unwrap();
                if !open {
                    writeln!(source, "#[serde(deny_unknown_fields)]").unwrap();
                }
                writeln!(source, "pub struct {name} {{").unwrap();
                let mut used = BTreeSet::new();
                for (index, field) in fields.iter().enumerate() {
                    let original = &field.name;
                    let identifier = if rust_field(original) && used.insert(original.clone()) {
                        original.clone()
                    } else {
                        let mut fallback = format!("field_{index}");
                        while used.contains(&fallback)
                            || fields.iter().any(|field| field.name == fallback)
                        {
                            fallback.push('_');
                        }
                        used.insert(fallback.clone());
                        fallback
                    };
                    if identifier != *original {
                        writeln!(source, "    #[serde(rename = {original:?})]").unwrap();
                    }
                    let ty = if field.required {
                        format!("Box<{}>", field.ty)
                    } else {
                        writeln!(source,"    #[serde(default, skip_serializing_if = \"RequestPresence::is_missing\")]").unwrap();
                        format!("RequestPresence<Box<{}>>", field.ty)
                    };
                    writeln!(source, "    pub r#{identifier}: {ty},").unwrap();
                }
                if *open {
                    // This map represents only the schema's explicitly open
                    // extra keys. Described fields retain their concrete types.
                    let mut extra = "__request_extra".to_owned();
                    while used.contains(&extra) {
                        extra.push('_');
                    }
                    writeln!(source,"    #[serde(flatten)]\n    pub {extra}: std::collections::BTreeMap<String, Value>,").unwrap();
                }
                writeln!(source, "}}").unwrap();
            }
        }
        if source.len() > super::MAX_SOURCE_BYTES {
            return Err(super::capacity("typed Rust request source exceeds 900 KiB"));
        }
    }
    Ok(source)
}

fn transparent(source: &mut String, name: &str, inner: &str) {
    writeln!(source,"#[derive(Clone, Debug, Serialize, Deserialize)]\n#[serde(transparent)]\npub struct {name}(pub {inner});").unwrap();
}

fn literal(source: &mut String, name: &str, value: &serde_json::Value) -> Result<()> {
    let (ty, expected) = match value {
        serde_json::Value::Null => ("()", "()".to_owned()),
        serde_json::Value::Bool(value) => ("bool", value.to_string()),
        serde_json::Value::String(value) => ("String", format!("{value:?}")),
        serde_json::Value::Number(value) if value.is_i64() => (
            "i64",
            format!("{}i64", value.as_i64().expect("checked i64")),
        ),
        serde_json::Value::Number(value) if value.is_u64() => (
            "u64",
            format!("{}u64", value.as_u64().expect("checked u64")),
        ),
        _ => {
            return Err(super::invalid(
                "typed Rust request literal is not a supported scalar",
            ))
        }
    };
    // String's expected expression is a &str, which serializes identically
    // without an allocation. Other literal expressions have explicit widths.
    writeln!(source, "request_literal!({name}, {ty}, {expected});").unwrap();
    Ok(())
}

fn rust_field(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !matches!(name, "_" | "self" | "Self" | "super" | "crate")
}

fn sequence(source: &mut String, name: &str, items: &[String]) {
    writeln!(source, "#[derive(Clone, Debug)]\npub struct {name} {{").unwrap();
    for (index, ty) in items.iter().enumerate() {
        writeln!(source, "    pub item_{index}: Box<{ty}>,").unwrap();
    }
    writeln!(source,"}}\nimpl<'de> Deserialize<'de> for {name} {{\n    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {{\n        struct SequenceVisitor;\n        impl<'de> serde::de::Visitor<'de> for SequenceVisitor {{\n            type Value={name};\n            fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {{ f.write_str(\"exact typed request sequence\") }}\n            fn visit_seq<A:serde::de::SeqAccess<'de>>(self,mut sequence:A)->Result<Self::Value,A::Error> {{").unwrap();
    for (index, ty) in items.iter().enumerate() {
        writeln!(source,"                let item_{index}=sequence.next_element::<Box<{ty}>>()?.ok_or_else(||serde::de::Error::invalid_length({index},&self))?;").unwrap();
    }
    writeln!(source,"                if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {{ return Err(serde::de::Error::invalid_length({},&self)); }}\n                Ok({name} {{",items.len()+1).unwrap();
    for index in 0..items.len() {
        writeln!(source, "                    item_{index},").unwrap();
    }
    writeln!(source,"                }})\n            }}\n        }}\n        deserializer.deserialize_tuple({},SequenceVisitor)\n    }}\n}}",items.len()).unwrap();
    writeln!(source,"impl Serialize for {name} {{\n    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {{\n        let mut sequence=serializer.serialize_tuple({})?;",items.len()).unwrap();
    for index in 0..items.len() {
        writeln!(source,"        serde::ser::SerializeTuple::serialize_element(&mut sequence,&self.item_{index})?;").unwrap();
    }
    writeln!(
        source,
        "        serde::ser::SerializeTuple::end(sequence)\n    }}\n}}"
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::super::{Definition, Field};
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn recursive_edges_are_structural_newtypes_and_boxed_without_json_fallback() {
        let model = Model {
            definitions: vec![
                Definition {
                    name: "RequestType0000".into(),
                    shape: Shape::Union(vec!["RequestType0001".into(), "RequestType0002".into()]),
                },
                Definition {
                    name: "RequestType0001".into(),
                    shape: Shape::Object {
                        fields: vec![Field {
                            name: "body".into(),
                            ty: "RequestType0000".into(),
                            required: true,
                        }],
                        open: false,
                    },
                },
                Definition {
                    name: "RequestType0002".into(),
                    shape: Shape::Array("RequestType0003".into()),
                },
                Definition {
                    name: "RequestType0003".into(),
                    shape: Shape::Alias("RequestType0000".into()),
                },
            ],
            params: BTreeMap::new(),
        };
        let source = emit(&model).unwrap();
        assert!(source.contains("Choice0(Box<RequestType0001>)"));
        assert!(source.contains("pub r#body: Box<RequestType0000>"));
        assert!(source.contains("pub struct RequestType0002(pub Vec<Box<RequestType0003>>);"));
        assert!(source.contains("pub struct RequestType0003(pub Box<RequestType0000>);"));
        assert!(source.contains("#[serde(deny_unknown_fields)]"));
        assert!(!source.contains("= Value"));
        assert!(!source.contains("pub type RequestType0003"));
    }

    #[test]
    fn nullable_required_fields_and_optional_fields_keep_different_wire_presence() {
        let model = Model {
            definitions: vec![
                Definition {
                    name: "RequestType0000".into(),
                    shape: Shape::Object {
                        fields: vec![
                            Field {
                                name: "required_nullable".into(),
                                ty: "RequestType0001".into(),
                                required: true,
                            },
                            Field {
                                name: "optional_nullable".into(),
                                ty: "RequestType0001".into(),
                                required: false,
                            },
                            Field {
                                name: "self".into(),
                                ty: "RequestType0001".into(),
                                required: true,
                            },
                            Field {
                                name: "field_2".into(),
                                ty: "RequestType0001".into(),
                                required: true,
                            },
                        ],
                        open: false,
                    },
                },
                Definition {
                    name: "RequestType0001".into(),
                    shape: Shape::Union(vec!["RequestType0002".into(), "RequestType0003".into()]),
                },
                Definition {
                    name: "RequestType0002".into(),
                    shape: Shape::Integer,
                },
                Definition {
                    name: "RequestType0003".into(),
                    shape: Shape::Null,
                },
            ],
            params: BTreeMap::new(),
        };
        let source = emit(&model).unwrap();
        assert!(source.contains("pub r#required_nullable: Box<RequestType0001>"));
        assert!(source.contains("pub r#optional_nullable: RequestPresence<Box<RequestType0001>>"));
        assert_eq!(source.matches("#[serde(default,").count(), 1);
        assert!(source.contains("pub r#field_2_: Box<RequestType0001>"));
        assert!(source.contains("#[serde(rename = \"self\")]"));
        assert!(!source.contains("pub r#self:"));
    }

    #[test]
    fn literal_markers_preserve_negative_and_full_unsigned_json_integer_ranges() {
        for (value, expected) in [
            (json!(i64::MIN), "-9223372036854775808i64"),
            (json!(u64::MAX), "18446744073709551615u64"),
            (json!(false), "false"),
            (json!("kind\"\\\n"), "\"kind\\\"\\\\\\n\""),
        ] {
            let mut source = String::new();
            literal(&mut source, "RequestType0000", &value).unwrap();
            assert!(source.contains(expected), "{source}");
            assert!(!source.contains("Value::deserialize"));
        }
        assert!(literal(&mut String::new(), "RequestType0000", &json!(1.5)).is_err());
        assert!(literal(&mut String::new(), "RequestType0000", &json!({"kind":"x"})).is_err());
    }

    #[test]
    fn long_sequences_retain_positions_and_reject_extra_items_without_tuple_trait_limits() {
        let mut source = String::new();
        sequence(
            &mut source,
            "RequestType0000",
            &vec!["RequestType0001".into(); 13],
        );
        assert!(source.contains("pub item_12: Box<RequestType0001>"));
        assert!(source.contains("deserialize_tuple(13,SequenceVisitor)"));
        assert!(source.contains("next_element::<serde::de::IgnoredAny>()"));
        assert!(source.contains("invalid_length(14,&self)"));
        assert!(!source.contains("Vec<Value>"));
    }
}
