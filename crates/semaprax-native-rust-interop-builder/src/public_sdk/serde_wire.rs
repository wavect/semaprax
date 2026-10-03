//! Field-wise admission from an owned, deliberately unvalidated transport value.
//! Booleans and UTF-8 are checked before a complete record is published. Rust
//! local ownership drops every moved or untouched buffer on a late refusal.
use super::*;
use semaprax::hir::ResolvedFieldDeclaration;

pub(super) fn render(
    record: &str,
    mirror: &str,
    fields: &[ResolvedFieldDeclaration],
) -> Result<String, Diagnostic> {
    let wire = format!("{mirror}Wire");
    let error = format!("{mirror}ConversionError");
    let mut out=format!("#[derive(Debug,PartialEq,Eq)]pub enum {error}{{InvalidBool(&'static str),InvalidUtf8(&'static str)}}\n#[derive(Debug)]pub struct {wire}{{");
    for field in fields {
        let ty = match &field.ty {
            ResolvedType::I64 => "i64",
            ResolvedType::Bool => "u8",
            ResolvedType::String | ResolvedType::Bytes => "Vec<u8>",
            _ => return Err(sdk_error("Serde wire field is unsupported")),
        };
        write!(out, "pub {}:{ty},", field.name).unwrap();
    }
    write!(out,"}}\nimpl core::convert::TryFrom<{wire}> for {record}{{type Error={error};fn try_from(value:{wire})->Result<Self,Self::Error>{{").unwrap();
    for field in fields {
        let name = &field.name;
        let conversion=match &field.ty {
            ResolvedType::I64|ResolvedType::Bytes=>format!("value.{name}"),
            ResolvedType::Bool=>format!("match value.{name}{{0=>false,1=>true,_=>return Err({error}::InvalidBool({name:?}))}}"),
            ResolvedType::String=>format!("String::from_utf8(value.{name}).map_err(|_|{error}::InvalidUtf8({name:?}))?"),
            _=>return Err(sdk_error("Serde wire field is unsupported")),
        };
        write!(out, "let {name}={conversion};").unwrap();
    }
    out.push_str("Ok(Self{");
    for field in fields {
        write!(out, "{},", field.name).unwrap();
    }
    out.push_str("})}}\n");
    // Copy counts describe explicit owned payload copies only, not total
    // allocator activity, serde parsing, scalar stores, or JSON output.
    write!(out,"impl {record}{{pub fn to_owned_wire(&self)->({wire},usize){{let mut copied_payload_bytes=0usize;let value={wire}{{").unwrap();
    for field in fields {
        let name = &field.name;
        let value = match &field.ty {
            ResolvedType::I64 => format!("self.{name}"),
            ResolvedType::Bool => format!("u8::from(self.{name})"),
            ResolvedType::String => format!(
                "{{copied_payload_bytes+=self.{name}.len();self.{name}.as_bytes().to_vec()}}"
            ),
            ResolvedType::Bytes => {
                format!("{{copied_payload_bytes+=self.{name}.len();self.{name}.clone()}}")
            }
            _ => unreachable!("validated above"),
        };
        write!(out, "{name}:{value},").unwrap();
    }
    out.push_str("};(value,copied_payload_bytes)}}\n");
    Ok(out)
}
