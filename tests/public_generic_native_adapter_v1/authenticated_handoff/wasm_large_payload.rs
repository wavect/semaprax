//! Regression for the standalone Core Wasm provider's input-payload window:
//! payloads whose total exceeded 2 KiB used to be overwritten by the input
//! aggregate record while the call still reported success. Each case runs
//! the same checked program through the retained interpreter and the
//! compiler-emitted Core Wasm provider and requires byte-identical leaves.
use super::wasm_comparison::{unhex, wasm_column};
use super::*;
use semaprax::{
    hir::DeclarationId,
    interpreter::retained_call::{
        evaluate_retained_call, prepare_retained_call, RetainedCallOutcome, RetainedField,
        RetainedRecord, RetainedValue,
    },
};

fn pattern(len: usize, leaf: u8) -> Vec<u8> {
    // Position- and leaf-dependent, so an overwrite or swap is visible.
    (0..len)
        .map(|i| {
            (i as u8)
                .wrapping_mul(31)
                .wrapping_add(leaf.wrapping_mul(97))
                ^ (i >> 8) as u8
        })
        .collect()
}

fn interpret(
    program: &semaprax::hir::ResolvedProgram,
    endpoint: &semaprax::public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1,
    payloads: &[Vec<u8>],
) -> Vec<Vec<u8>> {
    let prepared = prepare_retained_call(program, endpoint.export_id()).unwrap();
    let argument = RetainedValue::Record(RetainedRecord {
        record: DeclarationId::new("auth.pair"),
        fields: endpoint
            .descriptor()
            .input_facts()
            .fields
            .iter()
            .zip(payloads)
            .map(|(field, bytes)| RetainedField {
                field: DeclarationId::new(&field.id),
                value: RetainedValue::Bytes(bytes.clone()),
            })
            .collect(),
    });
    let observed = evaluate_retained_call(program, &prepared, &[argument], 10_000).unwrap();
    let RetainedCallOutcome::Returned(RetainedValue::Record(result)) = observed.outcome else {
        panic!("checked subject did not return: {:?}", observed.outcome)
    };
    result
        .fields
        .into_iter()
        .map(|field| match field.value {
            RetainedValue::Bytes(bytes) => bytes,
            other => panic!("non-Bytes result leaf {other:?}"),
        })
        .collect()
}

#[test]
fn core_wasm_large_input_payloads_match_the_interpreter() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r07-wasm-large-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let mut runs = 0;
    for (label, source) in [
        ("identity", SOURCE.to_owned()),
        (
            "moves",
            SOURCE.replace("{ value }", super::checked_moves::BODY),
        ),
    ] {
        let parsed = semaprax::check(&source, Path::new("r07-wasm-large.spx")).unwrap();
        let revision = semaprax::format::canonical(&parsed);
        let program = semaprax::hir::resolve(&parsed).unwrap();
        let endpoint =
            derive_admitted_public_generic_endpoint_v1(&program, &revision, "auth.identity")
                .unwrap();
        let wasm =
            semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint).unwrap();
        wasm.verify().unwrap();
        let input =
            CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), Direction::Input);
        let result =
            CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), Direction::Result);
        // 1024+1024 is the largest total that never overlapped; the others
        // cross the former 2 KiB boundary up to the 64 KiB leaf bound.
        for (left, right) in [(1024, 1024), (1025, 1024), (1, 65_536), (65_536, 65_536)] {
            let payloads = vec![pattern(left, 1), pattern(right, 2)];
            let frame = input
                .frame_with_leaves(
                    input
                        .leaf_paths()
                        .iter()
                        .zip(&payloads)
                        .map(|(path, bytes)| CarrierLeaf::new(path, LeafKind::Bytes, bytes.clone()))
                        .collect(),
                )
                .encode();
            let directory = root.join(format!("{label}-{left}-{right}"));
            fs::create_dir(&directory).unwrap();
            let rows = wasm_column(&directory, &wasm, &frame, &[]);
            let canonical = rows.last().unwrap();
            assert_eq!(canonical["status"], 0, "{label} {left}+{right}");
            let parsed = parse_bounded(&unhex(canonical["bytes"].as_str().unwrap())).unwrap();
            result.validate_frame(&parsed).unwrap();
            let wasm_leaves: Vec<_> = parsed
                .leaves()
                .iter()
                .map(|leaf| leaf.payload().to_vec())
                .collect();
            let expected = interpret(&program, &endpoint, &payloads);
            // Report the first diverging byte rather than two 64 KiB dumps.
            let divergence =
                wasm_leaves
                    .iter()
                    .zip(&expected)
                    .enumerate()
                    .find_map(|(leaf, (got, want))| {
                        (got != want).then(|| {
                            let byte = got.iter().zip(want).position(|(a, b)| a != b);
                            (leaf, got.len(), want.len(), byte)
                        })
                    });
            assert!(
                wasm_leaves.len() == expected.len() && divergence.is_none(),
                "{label} {left}+{right}: Core Wasm diverges from the interpreter at (leaf, got len, want len, byte) {divergence:?}"
            );
            runs += 1;
            eprintln!("R07 Core Wasm {label} {left}+{right}: leaves match the interpreter");
        }
    }
    assert_eq!(runs, 8);
    fs::remove_dir_all(root).unwrap();
}
