//! The deterministic string templates [`super::generate_typescript_calling_consumer`]
//! composes. Split from `typescript_calling.rs` itself only to keep that
//! file's own generator-contract logic short; every function here is a pure
//! byte-in/text-out renderer, exercised by [`super::tests`]. Mirrors
//! `rust_calling::render`'s own fixed-template/generated-per-shape split.

use std::fmt::Write as _;

use crate::public_generic_abi::descriptor::decode as decode_descriptor;
use crate::public_generic_abi::wasm::binding::WasmProviderBindingV1;

use super::{field_name, RecordShape};

/// Line endings are normalized because a checkout may deliver these `.txt`
/// assets with CRLF -- `.gitattributes` pins them to LF, but a generator
/// that is only deterministic because of a checkout setting is not
/// deterministic. Matches `public_generic_consumer::template`'s own
/// convention exactly.
fn template(text: &str) -> String {
    text.replace("\r\n", "\n")
}

pub(super) fn package_json() -> String {
    template(include_str!("render/package.json.txt"))
}

pub(super) fn package_lock_json() -> String {
    template(include_str!("render/package-lock.json.txt"))
}

pub(super) fn tsconfig_json() -> String {
    template(include_str!("render/tsconfig.json.txt"))
}

pub(super) fn errors_ts() -> String {
    template(include_str!("render/errors.ts.txt"))
}

pub(super) fn index_ts() -> String {
    template(include_str!("render/index.ts.txt"))
}

pub(super) fn wasm_provider_ts() -> String {
    template(include_str!("render/wasm-provider.ts.txt"))
}

/// One numeric byte literal per generated file: twelve bytes a line, so a
/// diff of two generated consumers stays readable. Matches
/// `public_generic_consumer::byte_literal`'s and
/// `rust_calling::render::byte_array_literal`'s own convention, rendered as
/// a TypeScript `Uint8Array.from([...])` initializer.
fn uint8_array_literal(bytes: &[u8]) -> String {
    let mut out = String::from("Uint8Array.from([\n");
    for chunk in bytes.chunks(12) {
        out.push_str("  ");
        for byte in chunk {
            let _ = write!(out, "0x{byte:02x}, ");
        }
        out.push('\n');
    }
    out.push_str("])");
    out
}

/// A canonical, escaped TypeScript double-quoted string literal. Every
/// embedded string this generator emits (an export name, a digest) is
/// already-trusted, already-validated UTF-8 from
/// [`WasmProviderBindingV1`]'s own accessors, but this still escapes `\`,
/// `"`, and control characters rather than assuming they cannot occur --
/// the generator's own contract, not a fact about today's fixture values.
fn ts_string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", other as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

const DESCRIPTOR_HEADER: &str = include_str!("render/descriptor_header.ts.txt");

const DESCRIPTOR_VERIFY: &str = include_str!("render/descriptor_verify.ts.txt");

pub(super) fn descriptor_ts(
    descriptor_bytes: &[u8],
    binding_bytes: &[u8],
    binding: &WasmProviderBindingV1,
) -> String {
    let mut out = String::new();
    out.push_str(&template(DESCRIPTOR_HEADER));
    out.push('\n');
    out.push_str(
        "const MODULE_ARTIFACT_DIGEST_DOMAIN = new TextEncoder().encode(\n  \"semaprax.public-generic-typescript-wasm-consumer.v1.module-artifact\\0\",\n);\n\n",
    );
    out.push_str("export const TRUSTED_DESCRIPTOR_BYTES: Uint8Array = ");
    out.push_str(&uint8_array_literal(descriptor_bytes));
    out.push_str(";\n\n");
    out.push_str("export const TRUSTED_BINDING_BYTES: Uint8Array = ");
    out.push_str(&uint8_array_literal(binding_bytes));
    out.push_str(";\n\n");
    out.push_str("export const TRUSTED_PROVIDER_ARTIFACT_DIGEST: string = ");
    out.push_str(&ts_string_literal(binding.provider_artifact_digest()));
    out.push_str(";\nexport const TRUSTED_ENDPOINT_EXPORT_NAME: string = ");
    out.push_str(&ts_string_literal(binding.exported_endpoint_export_name()));
    out.push_str(";\nexport const TRUSTED_WASM_ADAPTER_ABI_VERSION: string = ");
    out.push_str(&ts_string_literal(binding.wasm_adapter_abi_version()));
    out.push_str(";\nexport const TRUSTED_COMPILED_PROVIDER: boolean = ");
    out.push_str(
        if binding.exported_endpoint_export_name() == "spx_pg_v1_call" {
            "true"
        } else {
            "false"
        },
    );
    out.push_str(";\n");
    out.push_str(&template(DESCRIPTOR_VERIFY));
    out
}

const TYPES_HEADER: &str = include_str!("render/types_header.ts.txt");

fn record_interface(name: &str, shape: &RecordShape) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "export interface {name} {{");
    for field in &shape.fields {
        let _ = writeln!(out, "  /** Field identity: {:?} */", field.identity);
        let _ = writeln!(out, "  readonly {}: Uint8Array;", field_name(field));
    }
    out.push_str("}\n");
    out
}

pub(super) fn types_ts(input: &RecordShape, output: &RecordShape) -> String {
    let mut out = String::new();
    out.push_str(&template(TYPES_HEADER));
    out.push('\n');
    out.push_str(&record_interface("Input", input));
    out.push('\n');
    out.push_str(&record_interface("Output", output));
    out
}

const CARRIER_HEADER: &str = include_str!("render/carrier_header.ts.txt");

fn input_leaves_fn(input: &RecordShape) -> String {
    let mut out = String::new();
    out.push_str("function inputLeaves(value: Input): readonly Uint8Array[] {\n");
    out.push_str("  return readInputFields(value, [\n");
    for field in &input.fields {
        let _ = writeln!(out, "    {:?},", field_name(field));
    }
    out.push_str("  ]);\n");
    out.push_str("}\n");
    out
}

fn output_from_leaves_fn(output: &RecordShape) -> String {
    let mut out = String::new();
    out.push_str("function outputFromLeaves(leaves: readonly Uint8Array[]): Output {\n");
    out.push_str("  return {\n");
    for (index, field) in output.fields.iter().enumerate() {
        let _ = writeln!(
            out,
            "    {}: leaves[{index}] as Uint8Array,",
            field_name(field)
        );
    }
    out.push_str("  };\n");
    out.push_str("}\n");
    out
}

pub(super) fn carrier_ts(
    descriptor_bytes: &[u8],
    input: &RecordShape,
    output: &RecordShape,
) -> String {
    let count = input.fields.len();
    // The descriptor bytes are independently replayed by the generated
    // package before a provider is opened. Rendering these facts from the
    // same canonical descriptor lets the package form the frozen carrier
    // identities without inventing a second descriptor format. Deliberately
    // leave hostile/malformed fixture bytes renderable: their generated
    // package must refuse during descriptor admission, before this codec is
    // ever asked to construct a frame.
    let descriptor = decode_descriptor(descriptor_bytes).ok();
    let descriptor_digest = descriptor
        .as_ref()
        .map(|value| value.identity_digest())
        .unwrap_or_default();
    let export_id = descriptor
        .as_ref()
        .map(|value| value.export_id().to_owned())
        .unwrap_or_default();
    let input_instance_digest = descriptor
        .as_ref()
        .map(|value| value.input().instance_digest.clone())
        .unwrap_or_default();
    let output_instance_digest = descriptor
        .as_ref()
        .map(|value| value.result().instance_digest.clone())
        .unwrap_or_default();
    let mut out = String::new();
    out.push_str(&template(CARRIER_HEADER));
    out.push('\n');
    let _ = writeln!(out, "export const FIELD_COUNT = {count};");
    out.push('\n');
    out.push_str(&input_leaves_fn(input));
    out.push('\n');
    out.push_str(&output_from_leaves_fn(output));
    out.push('\n');
    out.push_str(
        "export function encodeInput(value: Input): Uint8Array {\n  return encodeLeaves(inputLeaves(value));\n}\n\n",
    );
    out.push_str(
        "export function decodeOutput(bytes: Uint8Array): Output {\n  return outputFromLeaves(decodeLeaves(bytes, FIELD_COUNT));\n}\n",
    );
    out.push_str("\nexport const CARRIER_DESCRIPTOR_DIGEST = ");
    out.push_str(&ts_string_literal(&descriptor_digest));
    out.push_str(";\nexport const CARRIER_EXPORT_ID = ");
    out.push_str(&ts_string_literal(&export_id));
    out.push_str(";\nexport const INPUT_INSTANCE_DIGEST = ");
    out.push_str(&ts_string_literal(&input_instance_digest));
    out.push_str(";\nexport const OUTPUT_INSTANCE_DIGEST = ");
    out.push_str(&ts_string_literal(&output_instance_digest));
    out.push_str(";\nexport const INPUT_LEAF_PATHS = Object.freeze([\n");
    for field in &input.fields {
        let _ = writeln!(out, "  {},", ts_string_literal(&field.identity));
    }
    out.push_str("]);\nexport const OUTPUT_LEAF_PATHS = Object.freeze([\n");
    for field in &output.fields {
        let _ = writeln!(out, "  {},", ts_string_literal(&field.identity));
    }
    out.push_str("]);\n\n");
    out.push_str("export function encodeCanonicalInput(value: Input): Uint8Array {\n  return encodeCanonicalFrame(\"input\", CARRIER_DESCRIPTOR_DIGEST, CARRIER_EXPORT_ID, INPUT_INSTANCE_DIGEST, INPUT_LEAF_PATHS, inputLeaves(value));\n}\n\n");
    out.push_str("export function decodeCanonicalOutput(bytes: Uint8Array): Output {\n  return outputFromLeaves(decodeCanonicalFrame(\"result\", CARRIER_DESCRIPTOR_DIGEST, CARRIER_EXPORT_ID, OUTPUT_INSTANCE_DIGEST, OUTPUT_LEAF_PATHS, bytes));\n}\n");
    out
}

const ROUND_TRIP_HEADER: &str = include_str!("render/round_trip_header.mjs.txt");

fn sample_input_fn(input: &RecordShape) -> String {
    let mut out = String::new();
    out.push_str("function sampleInput() {\n");
    out.push_str("  return {\n");
    for (index, field) in input.fields.iter().enumerate() {
        let _ = writeln!(
            out,
            "    {}: new TextEncoder().encode(\"sample-{index}\"),",
            field_name(field)
        );
    }
    out.push_str("  };\n");
    out.push_str("}\n");
    out
}

fn input_with_first_field_fn(input: &RecordShape) -> String {
    let mut out = String::from("function inputWithFirstField(bytes) {\n  return {\n");
    for (index, field) in input.fields.iter().enumerate() {
        let value = if index == 0 {
            "bytes"
        } else {
            "new Uint8Array(0)"
        };
        let _ = writeln!(out, "    {}: {value},", field_name(field));
    }
    out.push_str("  };\n}\n");
    out
}

fn assert_reversed_fn(input: &RecordShape, output: &RecordShape) -> String {
    let mut out = String::new();
    out.push_str("function assertReversed(output, original) {\n");
    for (input_field, output_field) in input.fields.iter().zip(&output.fields) {
        let input_name = field_name(input_field);
        let output_name = field_name(output_field);
        let _ = writeln!(
            out,
            "  assert.deepEqual(output.{output_name}, reversed(original.{input_name}));"
        );
    }
    out.push_str("}\n");
    let _ = writeln!(
        out,
        "\nfunction FIRST_OUTPUT_FIELD(output) {{\n  return output.{};\n}}",
        field_name(&output.fields[0])
    );
    out
}

const ROUND_TRIP_BODY: &str = include_str!("render/round_trip_body.mjs.txt");

pub(super) fn round_trip_mjs(input: &RecordShape, output: &RecordShape) -> String {
    let mut out = String::new();
    out.push_str(&template(ROUND_TRIP_HEADER));
    out.push('\n');
    out.push_str(&sample_input_fn(input));
    out.push('\n');
    out.push_str(&input_with_first_field_fn(input));
    out.push('\n');
    out.push_str(&assert_reversed_fn(input, output));
    out.push_str(&template(ROUND_TRIP_BODY));
    out
}

#[cfg(test)]
mod template_tests {
    use super::*;
    use crate::public_generic_consumer::rust_calling::OwnedByteField;

    #[test]
    fn round_trip_assertions_pair_differently_named_leaves_by_position() {
        let input = RecordShape::new(vec![
            OwnedByteField::new("input-a"),
            OwnedByteField::new("input-b"),
        ]);
        let output = RecordShape::new(vec![
            OwnedByteField::new("output-x"),
            OwnedByteField::new("output-y"),
        ]);
        let rendered = assert_reversed_fn(&input, &output);
        for (input_field, output_field) in input.fields.iter().zip(&output.fields) {
            assert!(rendered.contains(&format!(
                "output.{}, reversed(original.{})",
                field_name(output_field),
                field_name(input_field)
            )));
            assert!(!rendered.contains(&format!("original.{}", field_name(output_field))));
        }
    }

    #[test]
    fn every_fixed_asset_has_lf_crlf_equivalent_rendering() {
        for text in [
            DESCRIPTOR_HEADER,
            DESCRIPTOR_VERIFY,
            TYPES_HEADER,
            CARRIER_HEADER,
            ROUND_TRIP_HEADER,
            ROUND_TRIP_BODY,
            include_str!("render/package.json.txt"),
            include_str!("render/package-lock.json.txt"),
            include_str!("render/tsconfig.json.txt"),
            include_str!("render/errors.ts.txt"),
            include_str!("render/index.ts.txt"),
            include_str!("render/wasm-provider.ts.txt"),
        ] {
            let lf = template(text);
            assert_eq!(template(&lf.replace('\n', "\r\n")), lf);
        }
    }
}
