//! Bound every valid spelling after whitespace removal, not the raw stream.
use super::descriptor::{Kind, Record};
use super::*;

const NORMALIZED_CAPACITY: usize = 131_072;

pub(super) fn validate_envelope(
    root: &Record<'_>,
    string_bound: usize,
    array_bound: usize,
) -> Result<(), Vec<Diagnostic>> {
    if record_bound(root, string_bound, array_bound).is_none_or(|bound| bound > NORMALIZED_CAPACITY)
    {
        return Err(refusal(
            "stream nested request worst valid normalized JSON spelling exceeds the existing 131072-byte buffer; reduce declared bounds or use the direct-input selector",
        ));
    }
    Ok(())
}

fn string_spelling_bound(decoded_bytes: usize) -> Option<usize> {
    // A decoded ASCII byte can use six bytes (\u00XX). Surrogate pairs use
    // twelve bytes for four decoded UTF-8 bytes, so six per byte is an upper
    // bound for every valid escape spelling, including object keys and NUL.
    decoded_bytes.checked_mul(6)?.checked_add(2)
}
fn record_bound(record: &Record<'_>, text: usize, array: usize) -> Option<usize> {
    let mut total = 2usize.checked_add(record.fields.len().saturating_sub(1))?;
    for field in &record.fields {
        total = total
            .checked_add(string_spelling_bound(field.declaration.name.len())?)?
            .checked_add(1)? // Colon.
            .checked_add(kind_bound(&field.kind, text, array)?)?;
    }
    Some(total)
}
fn kind_bound(kind: &Kind<'_>, text: usize, array: usize) -> Option<usize> {
    match kind {
        Kind::Scalar(Type::I64 | Type::Usize) => Some(20),
        Kind::Scalar(Type::U8) => Some(3),
        Kind::Scalar(Type::Bool) => Some(5),
        Kind::Scalar(_) => unreachable!("validated JSON scalar"),
        Kind::Text => string_spelling_bound(text),
        Kind::Record(record) => record_bound(record, text, array),
        Kind::Vector { kind, .. } => kind_bound(kind, text, array)?
            .checked_mul(array)?
            .checked_add(array.saturating_sub(1))?
            .checked_add(2),
    }
}

pub(super) fn wrapper(root: &TypeDeclaration) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    format!(
        "@id(\"{id}.json.nested.stream-decode\")\nfn json_{name}_nested_stream_decode()->{name}JsonNestedDecode uses {{process.stdin.read}} {{\nlet normalized=json_{name}_stream_normalize();\nmatch own normalized {{\n{name}JsonStreamInput::Error{{code,offset,field}}=>{name}JsonNestedDecode::Error{{code:code,offset:offset,field:field}},\n{name}JsonStreamInput::Ready{{bytes,length}}=>json_{name}_nested_decode(byte_range(bytes_as_slice(bytes),0usize,length),{NORMALIZED_CAPACITY}usize),\n}}\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCHEMA: &str = super::super::tests::SCHEMA;

    #[test]
    fn stream_nested_spelling_envelope_counts_escaped_keys_values_and_punctuation() {
        let program = crate::parse(SCHEMA, "schema.spx").unwrap();
        let shape = descriptor::validate(&program, &program.types[2], 16, 8).unwrap();
        // Independent wire calculation: Configuration187; Line176; [Line;8]1417;
        // escaped outer keys, urgent value and punctuation162, plus those two values =1766.
        assert_eq!(record_bound(&shape.root, 16, 8), Some(1766));
        assert!(validate_envelope(&shape.root, 16, 8).is_ok());
        assert_eq!(string_spelling_bound(0), Some(2));
        assert_eq!(string_spelling_bound(64), Some(386));
        assert_eq!(string_spelling_bound(usize::MAX), None);
        let wide = SCHEMA
            .replace("sku:string", &format!("{}:string", "a".repeat(64)))
            .replace("quantity:u8", &format!("{}:u8", "b".repeat(64)));
        let program = crate::parse(&wide, "wide.spx").unwrap();
        let shape = descriptor::validate(&program, &program.types[2], 64, 256).unwrap();
        assert!(record_bound(&shape.root, 64, 256).unwrap() > NORMALIZED_CAPACITY);
        assert_eq!(
            validate_envelope(&shape.root, 64, 256).unwrap_err()[0].code,
            "SPX-J180"
        );
        assert!(derive(&program, &program.types[2], 64, 256).is_ok());
    }

    #[test]
    fn stream_nested_adapter_requires_authored_permit_and_retains_input_until_owned_result() {
        let mut program = crate::parse(SCHEMA, "schema.spx").unwrap();
        assert_eq!(
            derive_stream(&program, &program.types[2], 16, 8).unwrap_err()[0].code,
            "SPX-J180"
        );
        program.permits.push("process.stdin.read".into());
        let root = &program.types[2];
        let source = derive_stream(&program, root, 16, 8).unwrap();
        let direct = derive(&program, root, 16, 8).unwrap();
        let normalization =
            super::super::super::views::stream_normalizer_source(&program, root).unwrap();
        assert_eq!(source, format!("{direct}{normalization}{}", wrapper(root)));
        assert!(source.contains("fn json_OrderRequest_nested_stream_decode()->OrderRequestJsonNestedDecode uses {process.stdin.read}"));
        assert!(source.contains("byte_range(bytes_as_slice(bytes),0usize,length),131072usize)"));
        assert!(!wrapper(root).contains("vec_with_capacity"));
        assert!(!wrapper(root).contains("string_concat"));
        let parsed = crate::parse(&source, "generated.spx").unwrap();
        let effectful: Vec<_> = parsed
            .functions
            .iter()
            .filter(|f| !f.effects.is_empty())
            .collect();
        assert_eq!(effectful.len(), 2);
        assert!(effectful
            .iter()
            .all(|f| f.effects == ["process.stdin.read"]));
        let canonical = crate::format::canonical(&parsed);
        assert_eq!(
            canonical,
            crate::format::canonical(&crate::parse(&canonical, "roundtrip.spx").unwrap())
        );
        assert!(source.len() <= super::super::super::MAX_GENERATED_BYTES);
    }

    #[test]
    fn stream_nested_template_never_rewrites_authored_marker_text() {
        for name in ["__ID__", "__FINISH_VALUE__", "__R__"] {
            let mut program = crate::parse(SCHEMA, "schema.spx").unwrap();
            program.permits.push("process.stdin.read".into());
            program.types[2].name = name.into();
            program.types[2].stable_id = "orders.__FINISH_VALUE__.__R__".into();
            let root = &program.types[2];
            let source = derive_stream(&program, root, 16, 8).unwrap();
            let parsed = crate::parse(&source, "stream-marker.spx").unwrap();
            let normalize = parsed
                .functions
                .iter()
                .find(|function| function.name == format!("json_{name}_stream_normalize"))
                .unwrap();
            assert_eq!(
                normalize.stable_id,
                format!("{}.json.stream.normalize", root.stable_id)
            );
            let canonical = crate::format::canonical(&parsed);
            assert_eq!(
                canonical,
                crate::format::canonical(&crate::parse(&canonical, "roundtrip.spx").unwrap())
            );
        }
    }
}
