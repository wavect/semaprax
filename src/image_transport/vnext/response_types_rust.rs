//! Rust representations retain integer range and optional/null distinctions.
use super::{Model, Result, Shape};
use std::collections::{BTreeMap, BTreeSet};
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

const RECURSIVE_SUPPORT: &str = r#"
std::thread_local! {
    static RESPONSE_TYPED_STATE: std::cell::Cell<(usize,usize,bool)> = const { std::cell::Cell::new((0,0,false)) };
}
struct ResponseTypeDecodeGuard;
impl ResponseTypeDecodeGuard {
    fn enter()->Result<Self,String> {
        RESPONSE_TYPED_STATE.with(|state| {
            let (mut remaining,depth,mut failed)=state.get();
            if depth==0 { remaining=65_536; failed=false; }
            if failed || depth>=128 || remaining==0 {
                state.set((remaining,depth,true));
                return Err("recursive typed response conversion capacity exceeded".into());
            }
            state.set((remaining-1,depth+1,false));
            Ok(Self)
        })
    }
    fn charge()->Result<(),String> {
        RESPONSE_TYPED_STATE.with(|state| {
            let (remaining,depth,failed)=state.get();
            if failed || remaining==0 {
                state.set((remaining,depth,true));
                return Err("recursive typed response conversion capacity exceeded".into());
            }
            state.set((remaining-1,depth,false));
            Ok(())
        })
    }
    fn check()->Result<(),String> {
        if RESPONSE_TYPED_STATE.with(|state|state.get().2) {
            Err("recursive typed response conversion capacity exceeded".into())
        } else { Ok(()) }
    }
}
impl Drop for ResponseTypeDecodeGuard {
    fn drop(&mut self) {
        RESPONSE_TYPED_STATE.with(|state| {
            let (remaining,depth,failed)=state.get();
            state.set((remaining,depth.saturating_sub(1),failed));
        });
    }
}
"#;

// Emit literal implementations once in source. Macro expansion preserves the
// same public unit types and exact serde checks without repeating their text
// for every enum/discriminant in the transport's bounded JSON source string.
const LITERAL_SUPPORT: &str = r#"
macro_rules! response_literal {
    ($name:ident, $literal:literal) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name;
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
                let value=Value::deserialize(deserializer)?;
                let expected:Value=serde_json::from_str($literal).map_err(serde::de::Error::custom)?;
                if value==expected { Ok(Self) } else { Err(serde::de::Error::custom("response literal mismatch")) }
            }
        }
        impl Serialize for $name {
            fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {
                let expected:Value=serde_json::from_str($literal).map_err(serde::ser::Error::custom)?;
                expected.serialize(serializer)
            }
        }
    };
}
"#;

pub(super) fn emit(model: &Model) -> Result<String> {
    let recursive = super::recursive_names(model)?;
    let mut source = if recursive.is_empty() {
        SUPPORT.to_owned()
    } else {
        let mut support = SUPPORT.replace(
            "    let payload=serde_json::from_value(envelope.payload).map_err(|error|error.to_string())?;",
            "    let _budget=ResponseTypeDecodeGuard::enter()?;\n    let payload=serde_json::from_value(envelope.payload);\n    ResponseTypeDecodeGuard::check()?;\n    let payload=payload.map_err(|error|error.to_string())?;",
        );
        support.push_str(RECURSIVE_SUPPORT);
        support
    };
    if model
        .definitions
        .iter()
        .any(|definition| matches!(definition.shape, Shape::Literal(_)))
    {
        source.push_str(LITERAL_SUPPORT);
    }
    let shapes = model
        .definitions
        .iter()
        .map(|definition| (definition.name.as_str(), &definition.shape))
        .collect::<BTreeMap<_, _>>();
    let mut guard_work = 0;
    for definition in &model.definitions {
        let name = &definition.name;
        match &definition.shape {
            Shape::Any => writeln!(source, "pub type {name} = Value;").unwrap(),
            Shape::Bool => writeln!(source, "pub type {name} = bool;").unwrap(),
            Shape::Integer => writeln!(source, "pub type {name} = ResponseInteger;").unwrap(),
            Shape::String => writeln!(source, "pub type {name} = String;").unwrap(),
            Shape::Null => writeln!(source, "pub type {name} = ();").unwrap(),
            Shape::Alias(target) => {
                if recursive.contains(name) {
                    transparent(&mut source, name, &format!("Box<{target}>"));
                } else {
                    writeln!(source, "pub type {name} = {target};").unwrap();
                }
            }
            Shape::Array(item) => {
                if recursive.contains(name) {
                    transparent(&mut source, name, &format!("Vec<{item}>"));
                } else {
                    writeln!(source, "pub type {name} = Vec<{item}>;").unwrap();
                }
            }
            Shape::Tuple(items) => {
                if recursive.contains(name) {
                    let boxed = items
                        .iter()
                        .map(|ty| format!("Box<{ty}>"))
                        .collect::<Vec<_>>();
                    if items.len() <= 12 {
                        transparent(&mut source, name, &format!("({},)", boxed.join(",")));
                    } else {
                        sequence(&mut source, name, &boxed);
                    }
                } else if items.is_empty() {
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
                writeln!(source, "response_literal!({name}, {literal:?});").unwrap();
            }
            Shape::Union(alternatives) => {
                let derives = if recursive.contains(name) {
                    "Clone, Debug, Serialize"
                } else {
                    "Clone, Debug, Serialize, Deserialize"
                };
                writeln!(
                    source,
                    "#[derive({derives})]\n#[serde(untagged)]\npub enum {name} {{"
                )
                .unwrap();
                for (index, ty) in alternatives.iter().enumerate() {
                    if recursive.contains(name) && recursive.contains(ty) {
                        writeln!(source, "    Choice{index}(Box<{ty}>),").unwrap();
                    } else {
                        writeln!(source, "    Choice{index}({ty}),").unwrap();
                    }
                }
                writeln!(source, "}}").unwrap();
                if recursive.contains(name) {
                    recursive_union(
                        &mut source,
                        name,
                        alternatives,
                        &recursive,
                        &shapes,
                        &mut guard_work,
                    )?;
                }
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
                    let inner = if recursive.contains(name) && recursive.contains(&field.ty) {
                        format!("Box<{}>", field.ty)
                    } else {
                        field.ty.clone()
                    };
                    let ty = if field.required {
                        inner
                    } else {
                        writeln!(
                            source,
                            "    #[serde(default, skip_serializing_if = \"Presence::is_missing\")]"
                        )
                        .unwrap();
                        format!("Presence<{inner}>")
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

fn transparent(source: &mut String, name: &str, inner: &str) {
    writeln!(source,"#[derive(Clone, Debug, Serialize, Deserialize)]\n#[serde(transparent)]\npub struct {name}(pub {inner});").unwrap();
}

fn recursive_union(
    source: &mut String,
    name: &str,
    alternatives: &[String],
    recursive: &BTreeSet<String>,
    shapes: &BTreeMap<&str, &Shape>,
    work: &mut usize,
) -> Result<()> {
    writeln!(source,"impl<'de> Deserialize<'de> for {name} {{\n    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {{\n        let _budget=ResponseTypeDecodeGuard::enter().map_err(serde::de::Error::custom)?;\n        let value=Value::deserialize(deserializer)?;").unwrap();
    for (index, target) in alternatives.iter().enumerate() {
        let guard = branch_guard(target, shapes, work)?;
        let ty = if recursive.contains(target) {
            format!("Box<{target}>")
        } else {
            target.clone()
        };
        writeln!(source,"        ResponseTypeDecodeGuard::charge().map_err(serde::de::Error::custom)?;\n        if {guard} {{\n            if let Ok(parsed)=serde_json::from_value::<{ty}>(value.clone()) {{\n                ResponseTypeDecodeGuard::check().map_err(serde::de::Error::custom)?;\n                return Ok(Self::Choice{index}(parsed));\n            }}\n        }}").unwrap();
    }
    writeln!(source,"        ResponseTypeDecodeGuard::check().map_err(serde::de::Error::custom)?;\n        Err(serde::de::Error::custom(\"recursive response union has no admitted branch\"))\n    }}\n}}").unwrap();
    Ok(())
}

fn terminal_shape<'a>(
    mut name: &'a str,
    shapes: &BTreeMap<&str, &'a Shape>,
    work: &mut usize,
) -> Result<&'a Shape> {
    for _ in 0..=128 {
        *work += 1;
        if *work > 65_536 {
            return Err(super::capacity(
                "typed Rust response discriminant traversal exceeds its bound",
            ));
        }
        match shapes.get(name).copied() {
            Some(Shape::Alias(next)) => name = next,
            Some(shape) => return Ok(shape),
            None => {
                return Err(super::invalid(
                    "typed response discriminant names an absent definition",
                ))
            }
        }
    }
    Err(super::invalid(
        "typed response discriminant alias chain is cyclic or too deep",
    ))
}

fn scalar_guard(shape: &Shape, value: &str) -> Option<String> {
    match shape {
        Shape::Null => Some(format!("{value}.is_null()")),
        Shape::Literal(serde_json::Value::String(literal)) => {
            Some(format!("{value}.as_str()==Some({literal:?})"))
        }
        Shape::Literal(serde_json::Value::Bool(literal)) => {
            Some(format!("{value}.as_bool()==Some({literal})"))
        }
        Shape::Literal(serde_json::Value::Number(literal)) if literal.is_i64() => Some(format!(
            "{value}.as_i64()==Some({}i64)",
            literal.as_i64().expect("checked i64")
        )),
        Shape::Literal(serde_json::Value::Number(literal)) if literal.is_u64() => Some(format!(
            "{value}.as_u64()==Some({}u64)",
            literal.as_u64().expect("checked u64")
        )),
        _ => None,
    }
}

fn branch_guard(target: &str, shapes: &BTreeMap<&str, &Shape>, work: &mut usize) -> Result<String> {
    let shape = terminal_shape(target, shapes, work)?;
    if let Some(guard) = scalar_guard(shape, "value") {
        return Ok(guard);
    }
    Ok(match shape {
        Shape::Object { fields, .. } => {
            let mut guard = "value.is_object()".to_owned();
            // Inspect only schema-proven scalar constants. In particular kind
            // and target are checked before recursively decoding arguments or
            // bodies, regardless of canonical JSON/property ordering.
            for field in fields {
                if let Some(check) =
                    scalar_guard(terminal_shape(&field.ty, shapes, work)?, "literal")
                {
                    let predicate = if field.required {
                        "is_some_and"
                    } else {
                        "is_none_or"
                    };
                    write!(
                        guard,
                        " && value.get({:?}).{predicate}(|literal|{check})",
                        field.name
                    )
                    .unwrap();
                }
            }
            guard
        }
        Shape::Array(_) | Shape::Tuple(_) => "value.is_array()".into(),
        Shape::Bool => "value.is_boolean()".into(),
        Shape::Integer => "(value.is_i64() || value.is_u64())".into(),
        Shape::String => "value.is_string()".into(),
        _ => "true".into(),
    })
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
