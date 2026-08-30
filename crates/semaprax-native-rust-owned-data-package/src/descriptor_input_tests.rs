//! Input admission evidence, not a claim that arbitrary padded JSON is a
//! canonical descriptor. Digest discovery must bound bytes before JSON parsing.
use super::*;

fn padded_json(schema: &str, length: usize) -> Vec<u8> {
    let prefix = format!("{{\"schema\":\"{schema}\",\"padding\":\"");
    let suffix = "\"}\n";
    assert!(length >= prefix.len() + suffix.len());
    let mut bytes = prefix.into_bytes();
    bytes.resize(length - suffix.len(), b'x');
    bytes.extend_from_slice(suffix.as_bytes());
    assert_eq!(bytes.len(), length);
    bytes
}

#[test]
fn digest_discovery_applies_exact_byte_bound_before_parsing() {
    for schema in [PUBLIC_OWNED_DATA_API_SCHEMA, PUBLIC_OWNED_UTF8_API_SCHEMA] {
        let exact = padded_json(schema, MAX_DESCRIPTOR_BYTES);
        assert!(descriptor::validate_input(&exact).is_ok());
        assert_eq!(
            descriptor_digest_for_bytes(&exact),
            descriptor_digest_for_schema(schema, &exact)
        );
        // This is syntactically valid JSON with a supported schema. Previously
        // digest discovery parsed and hashed it before replay rejected its size.
        let oversized = padded_json(schema, MAX_DESCRIPTOR_BYTES + 1);
        assert!(descriptor_digest_for_bytes(&oversized).is_none());
        assert_eq!(
            descriptor::validate_input(&oversized).unwrap_err().kind(),
            PackageErrorKind::Descriptor
        );
        assert_eq!(
            descriptor::replay(&oversized, "invalid", &[])
                .unwrap_err()
                .kind(),
            PackageErrorKind::Descriptor
        );
        // Passing the byte/framing guard does not confer canonical authority.
        let digest = descriptor_digest_for_schema(schema, &exact).unwrap();
        assert!(descriptor::replay(&exact, &digest, &[]).is_err());
    }
}

#[test]
fn discovery_and_replay_share_existing_empty_nul_and_newline_rejections() {
    for bytes in [
        Vec::new(),
        b"{\"schema\":\"semaprax.public-owned-data-api.v1\"}".to_vec(),
        b"{\"schema\":\"semaprax.public-owned-utf8-api.v1\",\"value\":\"\0\"}\n".to_vec(),
    ] {
        assert!(descriptor_digest_for_bytes(&bytes).is_none());
        assert_eq!(
            descriptor::replay(&bytes, "invalid", &[])
                .unwrap_err()
                .kind(),
            PackageErrorKind::Descriptor
        );
    }
}

fn canonical(schema: &str, project: &str, result: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":\"{schema}\",\"project_schema\":\"{project}\",\"project_revision\":\"sha256:{0}\",\"workspace_revision\":\"sha256:{0}\",\"project_graph_digest\":\"sha256:{0}\",\"exports\":[{{\"stable_id\":\"fixture.value\",\"typescript_name\":\"fixture.value\",\"rust_method_name\":\"spx_fixture_dot_value\",\"parameters\":[],\"result\":\"{result}\"}}],\"limits\":{{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576}}}}\n",
        "1".repeat(64)
    ).into_bytes()
}

#[test]
fn valid_v8_v10_replay_and_domain_separation_are_unchanged() {
    for (schema, project, result) in [
        (
            PUBLIC_OWNED_DATA_API_SCHEMA,
            PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            "owned-bytes",
        ),
        (
            PUBLIC_OWNED_UTF8_API_SCHEMA,
            PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
            "owned-utf8",
        ),
    ] {
        let bytes = canonical(schema, project, result);
        let digest = descriptor_digest_for_schema(schema, &bytes).unwrap();
        assert_eq!(descriptor_digest_for_bytes(&bytes), Some(digest.clone()));
        assert_eq!(
            descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()])
                .unwrap()
                .exports_len(),
            1
        );
        let wrong_domain = if schema == PUBLIC_OWNED_DATA_API_SCHEMA {
            utf8_descriptor_digest(&bytes)
        } else {
            descriptor_digest(&bytes)
        };
        assert!(descriptor::replay(&bytes, &wrong_domain, &["fixture.value".to_owned()]).is_err());
    }
}

#[test]
fn public_builder_preserves_provider_error_precedence_and_rejects_before_publication() {
    let output = Path::new(""); // No valid publication destination or tools are needed.
    let oversized = padded_json(PUBLIC_OWNED_DATA_API_SCHEMA, MAX_DESCRIPTOR_BYTES + 1);
    for valid_provider in [false, true] {
        let provider = if valid_provider {
            b"fixture provider".to_vec()
        } else {
            Vec::new()
        };
        let plan = PackagePlan::new(
            oversized.clone(),
            descriptor_digest(&oversized),
            vec!["fixture.value".to_owned()],
            provider.clone(),
            provider_sha256(&provider),
            PackageMode::StandaloneEvidence,
        );
        let expected = if HostTarget::current().is_none() {
            PackageErrorKind::ToolConfiguration
        } else if valid_provider {
            PackageErrorKind::Descriptor
        } else {
            PackageErrorKind::Provider
        };
        assert_eq!(
            build_and_publish(plan, output).unwrap_err().kind(),
            expected
        );
    }
}

#[test]
fn provider_binding_borrowed_comparison_preserves_legacy_predicate() {
    fn legacy(provider: &[u8], digest: &str) -> bool {
        let Ok(provider) = std::str::from_utf8(provider) else {
            return false;
        };
        let expected = format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{digest}\"");
        provider
            .lines()
            .filter(|line| line.starts_with("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 "))
            .eq([expected.as_str()])
    }
    // Do not add digest syntax validation here: this earlier provider check
    // deliberately preserves even malformed-but-matching binding behavior.
    for digest in [
        "",
        "short",
        "sha256:not-hex",
        "quoted\"value",
        "line\nbreak",
        "line\rbreak",
        "é",
    ] {
        let exact = format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{digest}\"");
        for provider in [
            exact.clone(),
            format!("{exact}\n"),
            format!("{exact}\r\n"),
            format!("{exact}\n{exact}\n"),
            format!("/* unrelated */\n{exact}\n"),
            format!(" {exact}\n"),
            format!("{exact} \n"),
            format!("{exact}\n#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 malformed\n"),
            format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 {digest}\n"),
            format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{digest}\n"),
        ] {
            assert_eq!(
                provider_binds_descriptor(provider.as_bytes(), digest),
                legacy(provider.as_bytes(), digest),
                "provider={provider:?}, digest={digest:?}"
            );
        }
    }
    assert!(!provider_binds_descriptor(b"\xff", "short"));
    assert!(!provider_binds_descriptor(b"unrelated\n", "short"));
}

#[test]
fn provider_binding_mismatches_long_digest_without_building_comparison_string() {
    let digest = "x".repeat(MAX_DESCRIPTOR_BYTES + 1);
    assert!(!provider_binds_descriptor(
        b"#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"short\"\n",
        &digest,
    ));
    assert!(!provider_binds_descriptor(b"", &digest));
}
