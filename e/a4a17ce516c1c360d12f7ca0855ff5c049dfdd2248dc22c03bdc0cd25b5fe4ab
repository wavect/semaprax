pub(super) fn emit_provider(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
    project_v8: bool,
) -> Result<NativeOwnedDataProviderArtifact, Diagnostic> {
    crate::hir::validate(program)?;
    let descriptor = replay_public_api_descriptor(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )?;
    let owned_utf8 = descriptor.project_schema() == PUBLIC_OWNED_UTF8_PROJECT_SCHEMA;
    if !project_v8
        && !owned_utf8
        && descriptor.exports().iter().any(|export| {
            !matches!(
                export.result(),
                PublicApiResultType::OwnedBytes
                    | PublicApiResultType::OptionOwnedBytes
                    | PublicApiResultType::ResultOwnedBytesI64
            )
        })
    {
        return Err(provider_error(
            "native owned-data provider requires only owned Bytes result exports",
        ));
    }
    let mut source = String::from("#define SPX_NO_ENTRY_WRAPPER 1\n");
    if project_v8 {
        writeln!(
            source,
            "#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{descriptor_digest}\""
        )
        .expect("writing provider descriptor binding cannot fail");
    }
    let semantic_source = if owned_utf8 {
        super::emit_hir_c_for_owned_utf8_provider(program)?
    } else {
        super::emit_hir_c_for_owned_data_provider(program)?
    };
    source.push_str(&semantic_source);
    source.push('\n');
    let mut runtime = String::new();
    emit_provider_runtime_profile(&mut runtime, owned_utf8);
    if project_v8 {
        runtime = runtime.replace(
            "static bool spx_owned_data_overlap_v1(",
            "static __attribute__((unused)) bool spx_owned_data_overlap_v1(",
        );
    }
    source.push_str(&runtime);
    for export in descriptor.exports() {
        let function = program
            .functions
            .iter()
            .find(|function| function.id == *export.stable_id())
            .ok_or_else(|| provider_error("replayed native owned-data export is absent"))?;
        emit_export(
            &mut source,
            program,
            export,
            &function.return_type,
            project_v8,
        )?;
    }
    Ok(NativeOwnedDataProviderArtifact {
        source,
        descriptor: descriptor_bytes.to_vec(),
        descriptor_digest: descriptor_digest.to_owned(),
    })
}
