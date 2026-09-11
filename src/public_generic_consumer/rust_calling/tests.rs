use super::*;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};

const DESCRIPTOR_BYTES: &[u8] = b"fixture-public-generic-descriptor-bytes-issue-156";

fn binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            TargetProfile::NativeC11,
            "runtime:native-c11-fixture-issue-156",
        ),
        "sha256:4444444444444444444444444444444444444444444444444444444444444444",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
}

fn shapes() -> (RecordShape, RecordShape) {
    let input = RecordShape::new(vec![
        OwnedByteField::new("rust_calling.head"),
        OwnedByteField::new("rust_calling.tail"),
    ]);
    let output = RecordShape::new(vec![
        OwnedByteField::new("rust_calling.head"),
        OwnedByteField::new("rust_calling.tail"),
    ]);
    (input, output)
}

fn generate() -> CallingConsumer {
    let (input, output) = shapes();
    generate_rust_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output)
        .expect("a well-formed shape must generate")
}

#[test]
fn regeneration_is_byte_identical() {
    let first = generate();
    let second = generate();
    assert_eq!(first.files(), second.files());
}

#[test]
fn emits_the_expected_file_set_in_a_stable_order() {
    let consumer = generate();
    let names: Vec<&str> = consumer
        .files()
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "Cargo.toml",
            "build.rs",
            "src/lib.rs",
            "src/error.rs",
            "src/descriptor.rs",
            "src/types.rs",
            "src/carrier.rs",
            "src/provider.rs",
            "tests/round_trip.rs",
        ]
    );
}

#[test]
fn every_file_is_lf_only_and_ends_with_a_trailing_newline() {
    for (name, contents) in generate().files() {
        assert!(!contents.contains('\r'), "{name} contains a CR byte");
        assert!(
            contents.ends_with('\n'),
            "{name} does not end with a newline"
        );
    }
}

/// Whether `source` uses the bare `unsafe` keyword anywhere outside a `//`
/// comment line. Deliberately not a plain substring search: `unsafe_code`
/// (the lint name every generated file's `Cargo.toml`/`lib.rs` legitimately
/// mentions) and prose inside doc comments (e.g. "All `unsafe` in this crate
/// is confined to...") must not themselves trip this check.
fn contains_unsafe_keyword(source: &str) -> bool {
    fn is_ident_char(byte: u8) -> bool {
        byte == b'_' || (byte as char).is_ascii_alphanumeric()
    }
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let bytes = line.as_bytes();
        let mut search_from = 0usize;
        while let Some(relative) = line[search_from..].find("unsafe") {
            let start = search_from + relative;
            let end = start + "unsafe".len();
            let before_is_boundary = start == 0 || !is_ident_char(bytes[start - 1]);
            let after_is_boundary = end == bytes.len() || !is_ident_char(bytes[end]);
            if before_is_boundary && after_is_boundary {
                return true;
            }
            search_from = end;
        }
    }
    false
}

/// "Add a generator test that rejects unexpected `unsafe` outside the
/// approved module(s)": every generated file except `src/provider.rs` must
/// use no `unsafe` keyword at all. `src/provider.rs` itself is exercised
/// separately below to confirm every `unsafe` block there carries the
/// required safety documentation immediately above it.
#[test]
fn unsafe_is_confined_to_the_provider_module() {
    for (name, contents) in generate().files() {
        if name == "src/provider.rs" {
            assert!(
                contains_unsafe_keyword(contents),
                "src/provider.rs is expected to use unsafe at least once"
            );
            continue;
        }
        assert!(
            !contains_unsafe_keyword(contents),
            "{name} must use no `unsafe` keyword; only src/provider.rs may"
        );
    }
}

#[test]
fn every_unsafe_block_in_the_provider_module_is_preceded_by_a_safety_comment() {
    let consumer = generate();
    let (_, provider_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/provider.rs")
        .expect("src/provider.rs must be generated");
    let lines: Vec<&str> = provider_source.lines().collect();
    let mut saw_unsafe_block = false;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("unsafe {") || trimmed.contains("= unsafe {") {
            saw_unsafe_block = true;
            // Walk upward through the contiguous `//` comment block directly
            // above this line (skipping nothing else), and require that
            // whole block to mention "SAFETY" somewhere in it. This does not
            // depend on the comment's exact length, only on it being an
            // unbroken run of comment lines immediately preceding the
            // `unsafe` block.
            let mut cursor = index;
            let mut block_mentions_safety = false;
            while cursor > 0 && lines[cursor - 1].trim_start().starts_with("//") {
                cursor -= 1;
                if lines[cursor].contains("SAFETY") {
                    block_mentions_safety = true;
                }
            }
            assert!(
                block_mentions_safety,
                "unsafe block at provider.rs line {} has no immediately preceding SAFETY comment block",
                index + 1
            );
        }
    }
    assert!(
        saw_unsafe_block,
        "expected at least one unsafe block in src/provider.rs"
    );
}

#[test]
fn owned_types_never_derive_copy_or_clone() {
    let consumer = generate();
    let (_, types_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/types.rs")
        .expect("src/types.rs must be generated");
    // A `#[derive(...)]` attribute line naming either trait, not merely the
    // word appearing in prose (this module's own header doc explains, in
    // English, why neither is derived -- that sentence must not itself trip
    // this check).
    for line in types_source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[derive(") {
            assert!(
                !trimmed.contains("Copy"),
                "a derive attribute names Copy: {trimmed}"
            );
            assert!(
                !trimmed.contains("Clone"),
                "a derive attribute names Clone: {trimmed}"
            );
        }
    }
}

#[test]
fn no_unsafe_function_is_exposed_publicly() {
    let consumer = generate();
    for (name, contents) in consumer.files() {
        assert!(
            !contents.contains("pub unsafe fn") && !contents.contains("pub(crate) unsafe fn"),
            "{name} exposes an unsafe fn; every safe-API entry point must be a safe fn"
        );
    }
}

#[test]
fn field_names_are_derived_from_identity_bytes_not_display_text() {
    let field = OwnedByteField::new("some.declaration.identity");
    assert_eq!(
        field.field_name(),
        format!("field_{}", identifier("some.declaration.identity"))
    );
    // Injective: two distinct identities never collide.
    let other = OwnedByteField::new("some.other.identity");
    assert_ne!(field.field_name(), other.field_name());
}

#[test]
fn duplicate_field_identity_in_one_record_is_rejected() {
    let input = RecordShape::new(vec![
        OwnedByteField::new("same.identity"),
        OwnedByteField::new("same.identity"),
    ]);
    let output = RecordShape::new(vec![OwnedByteField::new("x"), OwnedByteField::new("y")]);
    let error =
        generate_rust_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output).unwrap_err();
    assert_eq!(
        error,
        ShapeError::DuplicateFieldIdentity {
            record: "Input",
            identity: "same.identity".to_owned(),
        }
    );
}

#[test]
fn mismatched_leaf_counts_are_rejected() {
    let input = RecordShape::new(vec![OwnedByteField::new("only-one")]);
    let output = RecordShape::new(vec![OwnedByteField::new("a"), OwnedByteField::new("b")]);
    let error =
        generate_rust_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output).unwrap_err();
    assert_eq!(
        error,
        ShapeError::LeafCountMismatch {
            input: 1,
            output: 2,
        }
    );
}

#[test]
fn embeds_the_exact_trusted_descriptor_and_binding_bytes() {
    let consumer = generate();
    let (_, descriptor_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/descriptor.rs")
        .expect("src/descriptor.rs must be generated");
    for byte in DESCRIPTOR_BYTES {
        let needle = format!("0x{byte:02x}");
        assert!(
            descriptor_source.contains(&needle),
            "descriptor.rs must embed byte {needle} of the trusted descriptor"
        );
    }
    let binding_bytes = binding().encode();
    for byte in &binding_bytes {
        let needle = format!("0x{byte:02x}");
        assert!(
            descriptor_source.contains(&needle),
            "descriptor.rs must embed byte {needle} of the trusted binding"
        );
    }
}

#[test]
fn no_host_path_or_checkout_specific_text_survives_generation() {
    for (name, contents) in generate().files() {
        assert!(
            !contents.contains("/Users/") && !contents.contains("C:\\"),
            "{name} must not embed a checkout-specific host path"
        );
        assert!(
            !contents.contains("{{") && !contents.contains("}}"),
            "{name} must not leave a surviving template placeholder"
        );
    }
}

#[test]
fn field_count_and_field_lists_scale_with_the_shape() {
    let input = RecordShape::new(vec![
        OwnedByteField::new("one"),
        OwnedByteField::new("two"),
        OwnedByteField::new("three"),
    ]);
    let output = input.clone();
    let consumer =
        generate_rust_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output).unwrap();
    let (_, carrier_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/carrier.rs")
        .unwrap();
    assert!(carrier_source.contains("const FIELD_COUNT: usize = 3;"));
    let (_, types_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/types.rs")
        .unwrap();
    for field in &input.fields {
        assert!(types_source.contains(&field.field_name()));
    }
}
