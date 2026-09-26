use super::*;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::descriptor::{DescriptorV1, InstanceBinding};
use crate::public_generic_abi::wasm::provider::FIXTURE_ENDPOINT_EXPORT_NAME;

fn descriptor_bytes() -> Vec<u8> {
    DescriptorV1::new("sample.transform", "transform", "sha256:1111111111111111111111111111111111111111111111111111111111111111", "sha256:2222222222222222222222222222222222222222222222222222222222222222", "sha256:3333333333333333333333333333333333333333333333333333333333333333", InstanceBinding { term: "@11:sample.pair<bytes,bool>".to_owned(), instance_digest: "sha256:4444444444444444444444444444444444444444444444444444444444444444".to_owned() }, InstanceBinding { term: "@11:sample.pair<bytes,i64>".to_owned(), instance_digest: "sha256:5555555555555555555555555555555555555555555555555555555555555555".to_owned() }).encode()
}

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
    generate_typescript_calling_consumer(&descriptor_bytes(), &binding(), &input, &output)
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
    let error =
        generate_typescript_calling_consumer(&descriptor_bytes(), &binding(), &input, &output)
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
    let error =
        generate_typescript_calling_consumer(&descriptor_bytes(), &binding(), &input, &output)
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
    for byte in &descriptor_bytes() {
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
        generate_typescript_calling_consumer(&descriptor_bytes(), &binding(), &input, &output)
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

#[test]
fn reference_shape_has_exact_zero_max_and_first_over_leaf_bounds() {
    for count in [0, 257] {
        let shape = RecordShape::new(
            (0..count)
                .map(|index| OwnedByteField::new(format!("bound.field{index}")))
                .collect(),
        );
        assert_eq!(
            generate_typescript_calling_consumer(&descriptor_bytes(), &binding(), &shape, &shape)
                .unwrap_err(),
            ShapeError::LeafCountOutOfBounds { count }
        );
    }
    let shape = RecordShape::new(
        (0..256)
            .map(|index| OwnedByteField::new(format!("bound.field{index}")))
            .collect(),
    );
    assert!(
        generate_typescript_calling_consumer(&descriptor_bytes(), &binding(), &shape, &shape)
            .is_ok()
    );
}

#[test]
fn generated_reference_runtime_preserves_its_no_ambient_authority_contract() {
    let consumer = generate();
    for (name, contents) in consumer.files() {
        if !name.starts_with("src/") {
            continue;
        }
        for forbidden in [
            "from \"node:",
            "from 'node:",
            "import(\"node:",
            "import('node:",
            "fetch(",
            "XMLHttpRequest",
            "WebSocket",
            "child_process",
            "FinalizationRegistry",
            "setTimeout(",
            "AbortController",
        ] {
            assert!(!contents.contains(forbidden), "{name}: {forbidden}");
        }
    }
}

#[test]
fn authentication_and_cleanup_guards_are_emitted_from_fixed_assets() {
    let consumer = generate();
    let source = |name: &str| {
        consumer
            .files()
            .iter()
            .find(|(path, _)| path == name)
            .unwrap()
            .1
            .as_str()
    };
    let provider = source("src/wasm-provider.ts");
    assert!(provider.contains("throw mismatch(\"module-bytes-required\")"));
    assert!(
        provider
            .find("const bytes = snapshotModuleBytes(moduleOrBytes);")
            .unwrap()
            < provider
                .find("await verifyModuleArtifactDigest(bytes)")
                .unwrap()
    );
    assert!(source("src/descriptor.ts")
        .contains("EXPECTED_DESCRIPTOR_BYTES = TRUSTED_DESCRIPTOR_BYTES.slice()"));
    assert!(source("src/descriptor.ts")
        .contains("EXPECTED_BINDING_BYTES = TRUSTED_BINDING_BYTES.slice()"));
    assert!(provider.contains("this.#releaseIfOwned(result, \"result\")"));
    assert!(provider.contains("throw carrier(\"provider-busy\")"));
    assert!(source("src/errors.ts").contains("secondaryCleanupStatuses"));
    assert!(source("src/carrier.ts").contains("const spans = locateLeaves(checked, expectedCount)"));
}

#[test]
fn wasm_adapter_abi_version_gates_the_v2_carrier_replay_mapping() {
    let source = |consumer: &CallingConsumer, name: &str| {
        consumer
            .files()
            .iter()
            .find(|(path, _)| path == name)
            .unwrap()
            .1
            .clone()
    };
    // The v1 fixture binding keeps its v1 marker, so the generated
    // consumer never maps a status to the v2-only carrier-replay reason.
    let consumer = generate();
    assert!(source(&consumer, "src/descriptor.ts")
        .contains("export const TRUSTED_WASM_ADAPTER_ABI_VERSION: string = \"v1\";"));
    let provider = source(&consumer, "src/wasm-provider.ts");
    assert!(provider.contains("TRUSTED_WASM_ADAPTER_ABI_VERSION"));
    assert!(provider.contains("=== \"v2\""));
    assert!(provider.contains("carrier(\"carrier-replay\")"));
    assert!(source(&consumer, "src/errors.ts").contains("| \"carrier-replay\";"));
    // The same facts under v2 emit the v2 marker the status-14 branch gates on.
    let (input, output) = shapes();
    let v2 = WasmProviderBindingV1::new_v2(
        CarrierBindingV1::new(
            "sha256:7777777777777777777777777777777777777777777777777777777777777777",
            TargetProfile::CoreWasm,
            "runtime:core-wasm-fixture-issue-157",
        ),
        "sha256:8888888888888888888888888888888888888888888888888888888888888888",
        FIXTURE_ENDPOINT_EXPORT_NAME,
        "semaprax-0.4.1",
    );
    let consumer = generate_typescript_calling_consumer(&descriptor_bytes(), &v2, &input, &output)
        .expect("a well-formed v2 shape must generate");
    assert!(source(&consumer, "src/descriptor.ts")
        .contains("export const TRUSTED_WASM_ADAPTER_ABI_VERSION: string = \"v2\";"));
}
