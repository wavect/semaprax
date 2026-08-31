//! Rust representations retain integer range and optional/null distinctions.
use super::{Model, Result, Shape};
use std::collections::BTreeSet;
use std::fmt::Write;

const SUPPORT: &str = r#"
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInteger { Signed(i64), Unsigned(u64) }

/// Missing differs from a present value, including a schema-admitted null.
#[derive(Clone, Debug, PartialEq)]
pub enum Presence<T> { Missing, Present(T) }
impl<T> Default for Presence<T> { fn default() -> Self { Self::Missing } }
impl<T> Presence<T> { pub fn is_missing(&self) -> bool { matches!(self, Self::Missing) } }
impl<'de,T:Deserialize<'de>> Deserialize<'de> for Presence<T> {
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
        T::deserialize(deserializer).map(Self::Present)
    }
}
impl<T:Serialize> Serialize for Presence<T> {
    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {
        match self {
            Self::Present(value)=>value.serialize(serializer),
            Self::Missing=>Err(serde::ser::Error::custom("missing field must be omitted")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedResultEnvelope<T> {
    pub schema:String, pub protocol:String, pub image_revision:String,
    pub project_revision:String, pub payload:T,
}
/// Convert only after the ordinary decoder has checked method, id and schema.
/// This conversion is not independent schema validation or source authority.
pub fn typed_result<T:serde::de::DeserializeOwned>(envelope:ResultEnvelope)->Result<TypedResultEnvelope<T>,String> {
    let payload=serde_json::from_value(envelope.payload).map_err(|error|error.to_string())?;
    Ok(TypedResultEnvelope {schema:envelope.schema, protocol:envelope.protocol,
        image_revision:envelope.image_revision,project_revision:envelope.project_revision,payload})
}
"#;

pub(super) fn emit(model: &Model) -> Result<String> {
    let mut source = SUPPORT.to_owned();
    for definition in &model.definitions {
        let name = &definition.name;
        match &definition.shape {
            Shape::Any => writeln!(source, "pub type {name} = Value;").unwrap(),
            Shape::Bool => writeln!(source, "pub type {name} = bool;").unwrap(),
            Shape::Integer => writeln!(source, "pub type {name} = ResponseInteger;").unwrap(),
            Shape::String => writeln!(source, "pub type {name} = String;").unwrap(),
            Shape::Null => writeln!(source, "pub type {name} = ();").unwrap(),
            Shape::Alias(target) => writeln!(source, "pub type {name} = {target};").unwrap(),
            Shape::Array(item) => writeln!(source, "pub type {name} = Vec<{item}>;").unwrap(),
            Shape::Tuple(items) => {
                if items.is_empty() {
                    writeln!(source, "pub type {name} = [Value; 0];").unwrap();
                } else if items.len() <= 12 {
                    writeln!(source, "pub type {name} = ({},);", items.join(",")).unwrap();
                } else {
                    // Standard Debug tuples stop at twelve (serde at sixteen).
                    // Keep every position typed in a fixed sequence instead of
                    // widening a long compiler-owned constant into JSON values.
                    sequence(&mut source, name, items);
                }
            }
            Shape::Literal(value) => {
                let literal = value.to_string();
                writeln!(
                    source,
                    "#[derive(Clone, Debug, PartialEq)]\npub struct {name};"
                )
                .unwrap();
                writeln!(source,"impl<'de> Deserialize<'de> for {name} {{\n    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {{\n        let value=Value::deserialize(deserializer)?;\n        let expected:Value=serde_json::from_str({literal:?}).map_err(serde::de::Error::custom)?;\n        if value==expected {{ Ok(Self) }} else {{ Err(serde::de::Error::custom(\"response literal mismatch\")) }}\n    }}\n}}").unwrap();
                writeln!(source,"impl Serialize for {name} {{\n    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {{\n        let expected:Value=serde_json::from_str({literal:?}).map_err(serde::ser::Error::custom)?;\n        expected.serialize(serializer)\n    }}\n}}").unwrap();
            }
            Shape::Union(alternatives) => {
                writeln!(source,"#[derive(Clone, Debug, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum {name} {{").unwrap();
                for (index, ty) in alternatives.iter().enumerate() {
                    writeln!(source, "    Choice{index}({ty}),").unwrap();
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
                    writeln!(source, "    #[serde(rename = {original:?})]").unwrap();
                    let ty = if field.required {
                        field.ty.clone()
                    } else {
                        writeln!(
                            source,
                            "    #[serde(default, skip_serializing_if = \"Presence::is_missing\")]"
                        )
                        .unwrap();
                        format!("Presence<{}>", field.ty)
                    };
                    writeln!(source, "    pub r#{identifier}: {ty},").unwrap();
                }
                if *open {
                    let mut extra = "__response_extra".to_owned();
                    while used.contains(&extra) {
                        extra.push('_');
                    }
                    writeln!(source,"    #[serde(flatten)]\n    pub {extra}: std::collections::BTreeMap<String,Value>,").unwrap();
                }
                writeln!(source, "}}").unwrap();
            }
        }
        if source.len() > super::MAX_SOURCE_BYTES {
            return Err(super::capacity(
                "typed Rust response source exceeds 900 KiB",
            ));
        }
    }
    Ok(source)
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
        writeln!(source, "    pub item_{index}: {ty},").unwrap();
    }
    writeln!(source,"}}\nimpl<'de> Deserialize<'de> for {name} {{\n    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {{\n        struct SequenceVisitor;\n        impl<'de> serde::de::Visitor<'de> for SequenceVisitor {{\n            type Value={name};\n            fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {{ f.write_str(\"exact typed response sequence\") }}\n            fn visit_seq<A:serde::de::SeqAccess<'de>>(self,mut sequence:A)->Result<Self::Value,A::Error> {{").unwrap();
    for (index, ty) in items.iter().enumerate() {
        writeln!(source,"                let item_{index}=sequence.next_element::<{ty}>()?.ok_or_else(||serde::de::Error::invalid_length({index},&self))?;").unwrap();
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
