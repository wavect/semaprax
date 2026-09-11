use super::*;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::wasm::provider::FIXTURE_ENDPOINT_EXPORT_NAME;

const DESCRIPTOR_BYTES: &[u8] = b"fixture-public-generic-descriptor-bytes-issue-157";

fn binding() -> WasmProviderBindingV1 {
    WasmProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:7777777777777777777777777777777777777777777777777777777777777777",
            TargetProfile::CoreWasm,
            "runtime:core-wasm-fixture-issue-157",
        ),
        "sha256:8888888888888888888888888888888888888888888888888888888888888888",
        FIXTURE_ENDPOINT_EXPORT_NAME,
        "semaprax-0.4.1",
    )
}

fn shapes() -> (RecordShape, RecordShape) {
    let input = RecordShape::new(vec![
        OwnedByteField::new("typescript_calling.head"),
        OwnedByteField::new("typescript_calling.tail"),
    ]);
    let output = RecordShape::new(vec![
        OwnedByteField::new("typescript_calling.head"),
        OwnedByteField::new("typescript_calling.tail"),
    ]);
    (input, output)
}

fn generate() -> CallingConsumer {
    let (input, output) = shapes();
    generate_typescript_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output)
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
            "package.json",
            "package-lock.json",
            "tsconfig.json",
            "src/errors.ts",
            "src/descriptor.ts",
            "src/types.ts",
            "src/carrier.ts",
            "src/wasm-provider.ts",
            "src/index.ts",
            "test/round-trip.mjs",
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

#[test]
fn no_any_appears_in_the_generated_typescript_public_surface() {
    // "no `any` in generated public surface except tightly justified
    // WebAssembly host types" -- this generator never spells the literal
    // type `any` anywhere; the one place a WebAssembly host value is
    // narrowed (`instance.exports[...]`) uses `instanceof`/`typeof`
    // narrowing and an `as EndpointFn` cast, never `any`.
    for (name, contents) in generate().files() {
        if !name.ends_with(".ts") {
            continue;
        }
        assert!(
            !contents.contains(": any")
                && !contents.contains("<any>")
                && !contents.contains("as any"),
            "{name} must not spell the type `any`"
        );
    }
}

#[test]
fn duplicate_field_identity_in_one_record_is_rejected() {
    let input = RecordShape::new(vec![
        OwnedByteField::new("same.identity"),
        OwnedByteField::new("same.identity"),
    ]);
    let output = RecordShape::new(vec![OwnedByteField::new("x"), OwnedByteField::new("y")]);
    let error = generate_typescript_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output)
        .unwrap_err();
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
    let error = generate_typescript_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output)
        .unwrap_err();
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
        .find(|(name, _)| name == "src/descriptor.ts")
        .expect("src/descriptor.ts must be generated");
    for byte in DESCRIPTOR_BYTES {
        let needle = format!("0x{byte:02x}");
        assert!(
            descriptor_source.contains(&needle),
            "descriptor.ts must embed byte {needle} of the trusted descriptor"
        );
    }
    let binding_bytes = binding().encode();
    for byte in &binding_bytes {
        let needle = format!("0x{byte:02x}");
        assert!(
            descriptor_source.contains(&needle),
            "descriptor.ts must embed byte {needle} of the trusted binding"
        );
    }
    assert!(descriptor_source.contains(FIXTURE_ENDPOINT_EXPORT_NAME));
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
        generate_typescript_calling_consumer(DESCRIPTOR_BYTES, &binding(), &input, &output)
            .unwrap();
    let (_, carrier_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/carrier.ts")
        .unwrap();
    assert!(carrier_source.contains("export const FIELD_COUNT = 3;"));
    let (_, types_source) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "src/types.ts")
        .unwrap();
    for field in &input.fields {
        assert!(types_source.contains(&field_name(field)));
    }
}

#[test]
fn field_names_are_derived_from_identity_bytes_not_display_text() {
    let field = OwnedByteField::new("some.declaration.identity");
    assert_eq!(
        field_name(&field),
        format!("field_{}", identifier("some.declaration.identity"))
    );
    let other = OwnedByteField::new("some.other.identity");
    assert_ne!(field_name(&field), field_name(&other));
}

#[test]
fn package_json_and_lockfile_pin_the_same_typescript_version() {
    let consumer = generate();
    let (_, package_json) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "package.json")
        .unwrap();
    let (_, lockfile) = consumer
        .files()
        .iter()
        .find(|(name, _)| name == "package-lock.json")
        .unwrap();
    assert!(package_json.contains("\"typescript\": \"5.8.3\""));
    assert!(lockfile.contains("\"version\": \"5.8.3\""));
}

#[test]
fn generated_package_declares_no_runtime_dependency() {
    let (_, package_json) = generate()
        .files()
        .iter()
        .find(|(name, _)| name == "package.json")
        .cloned()
        .unwrap();
    assert!(
        !package_json.contains("\"dependencies\""),
        "the generated package must declare no runtime dependency beyond the platform"
    );
}
