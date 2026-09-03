//! Artifact projection and independent replay: descriptor and generated
//! views, byte-substitution rejection, manifest work, and frozen vectors.

use super::*;

#[test]
fn source_descriptor_and_generated_views_reconstruct_from_authenticated_facts() {
    let (program, spec_source) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
    let spec = parse_spec(&program, spec_source.as_bytes()).unwrap();
    let status_domains = prepared
        .imports
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let reconstructed = render_descriptor(
        &spec,
        &prepared.hir_digest,
        &status_domains,
        &prepared.exports,
        &prepared.imports,
    )
    .unwrap();
    assert_eq!(reconstructed, prepared.descriptor);
    assert!(reconstructed.contains(
            "\"status_domains\":[{\"ordinal\":0,\"domain_id\":\"success\"},{\"ordinal\":1,\"domain_id\":\"host.math.v1\"},{\"ordinal\":65533,\"domain_id\":\"semaprax.native-rust-semantics.v1\"},{\"ordinal\":65534,\"domain_id\":\"semaprax.native-rust-host.v1\"},{\"ordinal\":65535,\"domain_id\":\"semaprax.native-rust-adapter.v1\"}]"
        ));
    assert!(reconstructed.contains("\"status_domain_ordinals\":[1,65533,65534,65535]"));
    assert_eq!(
        domain_digest(DESCRIPTOR_DIGEST_DOMAIN, reconstructed.as_bytes()),
        prepared.descriptor_digest
    );
    assert_eq!(
        domain_digest(SOURCE_DOMAIN, crate::format::canonical(&program).as_bytes()),
        prepared.source_revision.clone().unwrap()
    );
    replay_descriptor(
        &reconstructed,
        &spec,
        &prepared.hir_digest,
        &prepared.exports,
        &prepared.imports,
    )
    .unwrap();
    replay_generated(
        &prepared.generated_header,
        &prepared.generated_c,
        &prepared.generated_rust,
        &prepared.private_ffi_source,
    )
    .unwrap();

    let changed_source = SOURCE.replacen("host_add(left, right)", "host_add(right, left)", 1);
    let changed = crate::parse(
        &changed_source,
        Path::new("native-rust-interop-changed.spx"),
    )
    .unwrap();
    let stale = match prepare_native_rust_interop(&changed, spec_source.as_bytes()) {
        Ok(_) => panic!("stale source binding was accepted"),
        Err(error) => error,
    };
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].code, "SPX-B107");
    assert_eq!(
        stale[0].message,
        "Native Rust Interop declaration set is unsupported: selected identity missing"
    );

    let mut changed_spec = spec;
    changed_spec.source_revision = Some(domain_digest(
        SOURCE_DOMAIN,
        crate::format::canonical(&changed).as_bytes(),
    ));
    let changed_prepared =
        prepare_native_rust_interop(&changed, render_spec(&changed_spec).as_bytes()).unwrap();
    assert_ne!(changed_prepared.source_revision, prepared.source_revision);
    assert_ne!(changed_prepared.hir_digest, prepared.hir_digest);
    assert_ne!(
        changed_prepared.descriptor_digest,
        prepared.descriptor_digest
    );
    assert_ne!(changed_prepared.generated_c, prepared.generated_c);
}

#[test]
fn descriptor_and_generated_source_replay_reject_every_bound_family() {
    let (program, spec_source) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
    let spec = parse_spec(&program, spec_source.as_bytes()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();

    let descriptor_mutations = [
        prepared.descriptor.replacen(
            "\"module\":\"interop.fixture\"",
            "\"module\":\"interop.forgery\"",
            1,
        ),
        prepared.descriptor.replacen(
            prepared.source_revision.as_deref().unwrap(),
            "sha256:forged-source",
            1,
        ),
        prepared
            .descriptor
            .replacen(&prepared.hir_digest, "sha256:forged-hir", 1),
        prepared
            .descriptor
            .replacen("\"pointer_width\":64", "\"pointer_width\":32", 1),
        prepared
            .descriptor
            .replacen("\"ordinal\":65533", "\"ordinal\":65532", 1),
        prepared.descriptor.replacen(
            "\"calling_convention\":\"C\"",
            "\"calling_convention\":\"X\"",
            1,
        ),
        prepared
            .descriptor
            .replacen("\"id\":\"interop.add\"", "\"id\":\"interop.bad\"", 1),
        prepared
            .descriptor
            .replacen("\"id\":\"host.add\"", "\"id\":\"host.bad\"", 1),
        prepared
            .descriptor
            .replacen("\"max_exports\":32", "\"max_exports\":31", 1),
        prepared.descriptor.replacen(
            "no_resource_owned_borrow_shared_or_aggregate_abi",
            "xo_resource_owned_borrow_shared_or_aggregate_abi",
            1,
        ),
        prepared.descriptor.trim_end().to_owned(),
    ];
    for (index, mutation) in descriptor_mutations.into_iter().enumerate() {
        let error = replay_descriptor(
            &mutation,
            &spec,
            &prepared.hir_digest,
            &prepared.exports,
            &prepared.imports,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-B108", "mutation {index}");
        assert_eq!(
            error.message, "Native Rust Interop descriptor disagrees with validated source and HIR",
            "mutation {index}"
        );
    }

    let generated_mutations = [
        (
            prepared.generated_header.replacen("#ifndef", "#ifndez", 1),
            prepared.generated_c.clone(),
            prepared.generated_rust.clone(),
            prepared.private_ffi_source.clone(),
        ),
        (
            prepared.generated_header.clone(),
            prepared.generated_c.replacen("#include", "#includx", 1),
            prepared.generated_rust.clone(),
            prepared.private_ffi_source.clone(),
        ),
        (
            prepared.generated_header.clone(),
            prepared.generated_c.clone(),
            prepared.generated_rust.replacen("forbid", "forbia", 1),
            prepared.private_ffi_source.clone(),
        ),
        (
            prepared.generated_header.clone(),
            prepared.generated_c.clone(),
            prepared.generated_rust.clone(),
            prepared.private_ffi_source.replacen("allow", "allox", 1),
        ),
    ];
    for (index, (header, c, rust, ffi)) in generated_mutations.into_iter().enumerate() {
        let error = replay_generated_exact(
            &spec,
            &closure,
            &prepared.exports,
            &prepared.imports,
            &header,
            &c,
            &rust,
            &ffi,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-B111", "generated mutation {index}");
        assert_eq!(
            error.message, "Native Rust Interop generated artifact replay failed",
            "generated mutation {index}"
        );
    }
}

#[test]
fn exact_replayers_reject_every_generated_and_descriptor_byte_substitution() {
    let (program, spec_source) = fixture();
    let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
    let spec = parse_spec(&program, spec_source.as_bytes()).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let (closure, _) = selected_closure(&resolved, &spec.exports).unwrap();

    each_byte_edit("spec", &spec_source, |mutation| {
        !replay_spec_bytes_exact(mutation, &spec)
    });

    each_byte_edit("descriptor", &prepared.descriptor, |mutation| {
        replay_descriptor(
            mutation,
            &spec,
            &prepared.hir_digest,
            &prepared.exports,
            &prepared.imports,
        )
        .is_err()
    });

    let artifacts = [
        (0, prepared.generated_header.as_str()),
        (1, prepared.generated_c.as_str()),
        (2, prepared.generated_rust.as_str()),
        (3, prepared.private_ffi_source.as_str()),
    ];
    for (selected, artifact) in artifacts {
        each_byte_edit("generated", artifact, |mutation| {
            let mut values = [
                prepared.generated_header.as_str(),
                prepared.generated_c.as_str(),
                prepared.generated_rust.as_str(),
                prepared.private_ffi_source.as_str(),
            ];
            values[selected] = mutation;
            replay_generated_exact(
                &spec,
                &closure,
                &prepared.exports,
                &prepared.imports,
                values[0],
                values[1],
                values[2],
                values[3],
            )
            .is_err()
        });
    }

    let files = [("descriptor.json", prepared.descriptor.as_bytes())];
    let rustc = RustcVersion::from_fields([
        "1.0.0",
        "0123456789abcdef",
        &prepared.target.triple,
        "20.0.0",
    ]);
    let manifest = render_manifest(
        &prepared,
        &files,
        "/held/clang",
        "clang version 20.0.0",
        &rustc,
        &prepared.target.triple,
    );
    each_byte_edit("manifest", &manifest, |mutation| {
        !replay_manifest_bytes_exact(
            mutation,
            &prepared,
            &files,
            "/held/clang",
            "clang version 20.0.0",
            &rustc,
            &prepared.target.triple,
        )
    });
}

#[test]
fn manifest_fixed_names_and_streaming_cursor_work_are_exact() {
    assert_eq!(
        canonical_manifest_file_names(),
        [
            "descriptor.json",
            "module.c",
            if cfg!(windows) {
                "module.obj"
            } else {
                "module.o"
            },
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
        ]
    );

    let assert_linear = |encoded: &str, decoded: &str| {
        let mut cursor = ManifestCursor::new(encoded).unwrap();
        cursor.string_eq(decoded).unwrap();
        let work = cursor.finish().unwrap();
        assert_eq!(work, encoded.len());
        assert!(work <= encoded.len().checked_mul(2).unwrap());
    };
    {
        let decoded = "a".repeat(MAX_MANIFEST_BYTES - 2);
        let mut encoded = String::with_capacity(MAX_MANIFEST_BYTES);
        encoded.push('"');
        encoded.push_str(&decoded);
        encoded.push('"');
        assert_linear(&encoded, &decoded);
    }
    {
        let decoded = "é".repeat((MAX_MANIFEST_BYTES - 2) / 2);
        let mut encoded = String::with_capacity(MAX_MANIFEST_BYTES);
        encoded.push('"');
        encoded.push_str(&decoded);
        encoded.push('"');
        assert_linear(&encoded, &decoded);
    }
    {
        let characters = (MAX_MANIFEST_BYTES - 2) / 6;
        let decoded = "a".repeat(characters);
        let mut encoded = String::with_capacity(characters * 6 + 2);
        encoded.push('"');
        for _ in 0..characters {
            encoded.push_str("\\u0061");
        }
        encoded.push('"');
        assert_linear(&encoded, &decoded);
    }

    for malformed in [
        "\"\\ud800\"",
        "\"\\udc00\"",
        "\"\\ud800\\u0000\"",
        "\"\\x\"",
    ] {
        let mut cursor = ManifestCursor::new(malformed).unwrap();
        assert!(cursor.string_eq("x").is_err());
    }
    let mut leading_zero = ManifestCursor::new("01").unwrap();
    assert!(leading_zero.usize_eq(1).is_err());
    let overflow = "9".repeat(usize::BITS as usize + 2);
    let mut overflow = ManifestCursor::new(&overflow).unwrap();
    assert!(overflow.usize_eq(usize::MAX).is_err());
}

#[test]
fn six_output_artifact_known_answer_vectors_are_frozen() {
    fn independent_sha256(bytes: &[u8]) -> String {
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
        )
    }

    fn independent_domain_sha256(domain: &[u8], bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(bytes);
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hasher.finalize())
        )
    }

    fn assert_raw_kat(name: &str, bytes: &[u8], length: usize, digest: &str) {
        assert_eq!(bytes.len(), length, "{name} byte length changed");
        assert_eq!(
            independent_sha256(bytes),
            digest,
            "{name} raw SHA-256 changed"
        );
    }

    with_test_target(
        Target {
            triple: "x86_64-unknown-linux-gnu".to_owned(),
            pointer_width: 64,
            endian: "little".to_owned(),
            panic_strategy: "unwind".to_owned(),
            thread_policy: "same_thread".to_owned(),
        },
        || {
            let (program, spec_source) = fixture();
            let prepared = prepare_native_rust_interop(&program, spec_source.as_bytes()).unwrap();
            let object = b"semaprax-native-rust-interop-kat-object-v1";
            let files = [
                ("descriptor.json", prepared.descriptor.as_bytes()),
                ("module.c", prepared.generated_c.as_bytes()),
                ("module.o", object.as_slice()),
                (
                    "semaprax_native_rust_interop.h",
                    prepared.generated_header.as_bytes(),
                ),
                (
                    "semaprax_native_rust_interop.rs",
                    prepared.generated_rust.as_bytes(),
                ),
                (
                    "semaprax_native_rust_interop_ffi.rs",
                    prepared.private_ffi_source.as_bytes(),
                ),
            ];
            let rustc = RustcVersion::from_fields([
                "1.88.0",
                "0123456789abcdef",
                &prepared.target.triple,
                "20.1.0",
            ]);
            let manifest = render_manifest(
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            );
            assert_raw_kat(
                "descriptor.json",
                prepared.descriptor.as_bytes(),
                4_498,
                "sha256:603c609409a2e35ee524481aa2225c6f0c6557dbff7d9650df8057daebcf173c",
            );
            assert_raw_kat(
                "semaprax.native-rust-interop.json",
                manifest.as_bytes(),
                3_493,
                "sha256:e8652276a9ea4489c5758aaa1e24456c9b3be96538384a125c8f41fb714b73ca",
            );
            assert_raw_kat(
                "semaprax_native_rust_interop.h",
                prepared.generated_header.as_bytes(),
                870,
                "sha256:3ebdf5567d93b9e24ccdea5a0bb76d83b7bdcc44721e2a65846f83f1c92ace3b",
            );
            assert_raw_kat(
                "module.c",
                prepared.generated_c.as_bytes(),
                4_124,
                "sha256:1f1640553fe746b0c2baef87b76ce1013ee2ac9f8ee1bc9209dae6b4ccbb3e61",
            );
            assert_raw_kat(
                "semaprax_native_rust_interop.rs",
                prepared.generated_rust.as_bytes(),
                2_100,
                "sha256:b75eb57f911ea274cd1ae5fb1a4b789f58008613d027c94d904c90d3085e2d62",
            );
            assert_raw_kat(
                "semaprax_native_rust_interop_ffi.rs",
                prepared.private_ffi_source.as_bytes(),
                4_719,
                "sha256:f317bef66a0ac44ba4ba89862ae645383f7d48668277f6a8fa559ada8fc4ff9a",
            );

            let descriptor_domain =
                "sha256:d10e85e8fefed377df137ac22791099a702b460ed31ea3c65a6061b222e0c7ba";
            assert_eq!(prepared.descriptor_digest, descriptor_domain);
            assert_eq!(
                independent_domain_sha256(DESCRIPTOR_DIGEST_DOMAIN, prepared.descriptor.as_bytes()),
                descriptor_domain
            );
            assert_eq!(
                independent_domain_sha256(BUNDLE_DIGEST_DOMAIN, manifest.as_bytes()),
                "sha256:4fbab384e26a272eb02166bc02aeb59f03cabfc92d5d547854b124d7eaf813bf"
            );
            assert!(replay_manifest_bytes_exact(
                &manifest,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            ));
            assert!(replay_manifest_semantic(
                &manifest,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            )
            .is_ok());

            let escaped =
                manifest.replacen("clang version 20.1.0", "clang\\u0020version 20.1.0", 1);
            assert!(replay_manifest_semantic(
                &escaped,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            )
            .is_ok());
            assert!(!replay_manifest_bytes_exact(
                &escaped,
                &prepared,
                &files,
                "/authenticated/clang",
                "clang version 20.1.0",
                &rustc,
                &prepared.target.triple,
            ));

            let malformed = [
                manifest.replacen("{\"schema\":", "{\"schema\":\"duplicate\",\"schema\":", 1),
                manifest.replacen("{\"schema\":", "{\"unknown\":0,\"schema\":", 1),
                manifest.replacen("{\"schema\":", "{\"missing_schema\":", 1),
                manifest.replacen("\"bytes\":4498", "\"bytes\":\"4498\"", 1),
                manifest.replacen("\"descriptor\":{", "\"descriptor\":[{", 1),
                format!("{manifest}trailing"),
            ];
            for hostile in malformed {
                assert!(replay_manifest_semantic(
                    &hostile,
                    &prepared,
                    &files,
                    "/authenticated/clang",
                    "clang version 20.1.0",
                    &rustc,
                    &prepared.target.triple,
                )
                .is_err());
            }
        },
    );
}

#[test]
fn capacity_module_has_no_physical_or_platform_authority() {
    let source = CAPACITY_SOURCE;
    for forbidden in [
        "platform::",
        "std::fs",
        "std::process",
        "create_directory_new_prepared",
        "write_file_new_prepared",
        "discard_owned_stage_prepared",
        "compile_c_prepared",
        "compile_rust_prepared",
        "link_or_copy_prepared",
        "run_prepared",
        "archive_tool_prepared",
        "publish_directory_new_prepared",
    ] {
        assert!(
            !source.contains(forbidden),
            "capacity-only implementation admitted `{forbidden}`"
        );
    }
}

#[test]
fn artifact_projection_module_has_no_physical_authority_or_replay_generator_shortcut() {
    let artifacts = ARTIFACTS_SOURCE;
    let cursor = include_str!("../exact_replay.rs");
    for forbidden in [
        "platform::",
        "std::fs",
        "std::process",
        "create_directory_new_prepared",
        "write_file_new_prepared",
        "archive_tool_prepared",
        "archive_prepared",
        "compile_c_tool_prepared",
        "compile_rust_tool_prepared",
        "link_tool_prepared",
        "execute_tool_prepared",
        "publish_directory_new_prepared",
        "discard_owned_stage_prepared",
        "settle_for_publish",
        "settle_regular_file_for_publish",
    ] {
        assert!(
            !artifacts.contains(forbidden) && !cursor.contains(forbidden),
            "pure artifact boundary admitted `{forbidden}`"
        );
    }
    let start = artifacts
        .find("fn replay_c_expression_linear_independent(")
        .unwrap();
    let end = artifacts[start..]
        .find("\nfn replay_c_expression(")
        .map(|offset| start + offset)
        .unwrap();
    let independent = &artifacts[start..end];
    for generator in [
        "c_expression_linear(",
        "c_expr_iterative(",
        "generate_c_into(",
        "c_expression_hash(",
        "c_expression_scalar(",
        "c_expression_resolved_scalar(",
    ] {
        assert!(
            !independent.contains(generator),
            "independent C replay called generator helper `{generator}`"
        );
    }
    assert!(artifacts.contains("enum CExpressionFrame<'a>"));
    assert!(artifacts.contains("enum ReplayCExpressionFrame<'a>"));
    assert!(cursor.contains("pub(super) struct ExactReplay<'a>"));
}
