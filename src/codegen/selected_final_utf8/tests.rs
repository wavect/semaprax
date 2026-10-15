use super::*;
const ORIGINAL_WRITE_AFTER_COPY_SOURCE: &str = r#"module selected.final;
@id("data.report") record Report { @id("data.raw") raw:string,@id("data.mode") mode:usize, }
@id("data.result") variant Outcome {
 @id("data.encoded") Encoded { @id("data.text") text:string, },
 @id("data.refused") Refused { @id("data.required") required:usize, },
}
@id("app.make") fn make(mode:usize)->Report {Report{raw:"{\"x\":\"é\\u0000\"}",mode:mode}}
@id("app.dispose") fn dispose(value:own Report)->i64 {0}
@id("app.encode") fn encode(value:borrow Report,limit:usize)->Outcome
ensures value.mode!=2usize
{
 let source=str_as_bytes(string_as_str(value.raw));let required=byte_len(source);
 if required>limit {Outcome::Refused{required:required}} else {
  let bytes=bytes_copy(source);
  let finished=if value.mode==1usize {bytes_set(bytes,0usize,255u8)}else{bytes};
  Outcome::Encoded{text:string_from_utf8(bytes_as_slice(finished))}
 }
}
@id("app.unrelated") fn unrelated()->string {"not selected"}
@id("app.main") fn main()->i64 {0}
"#;
const SOURCE: &str = r#"module selected.final;
@id("data.report") record Report { @id("data.raw") raw:string,@id("data.mode") mode:usize, }
@id("data.result") variant Outcome {
 @id("data.encoded") Encoded { @id("data.text") text:string, },
 @id("data.refused") Refused { @id("data.required") required:usize, },
}
@id("app.make") fn make(mode:usize)->Report {Report{raw:"{\"x\":\"é\\u0000\"}",mode:mode}}
@id("app.dispose") fn dispose(value:own Report)->i64 {0}
@id("app.encode") fn encode(value:borrow Report,limit:usize)->Outcome
ensures value.mode!=2usize
{
 let source=str_as_bytes(string_as_str(value.raw));let required=byte_len(source);
 if required>limit {Outcome::Refused{required:required}} else {
  let invalid:[u8;16]=[255u8,34u8,120u8,34u8,58u8,34u8,195u8,169u8,92u8,117u8,48u8,48u8,48u8,48u8,34u8,125u8];
  let finished=if value.mode==1usize {bytes_copy(array_as_slice(invalid))}else{bytes_copy(source)};
  Outcome::Encoded{text:string_from_utf8(bytes_as_slice(finished))}
 }
}
@id("app.unrelated") fn unrelated()->string {"not selected"}
@id("app.main") fn main()->i64 {0}
"#;
#[test]
fn write_after_copy_remains_refused_and_invalid_payload_is_identical() {
    let diagnostics =
        crate::check(ORIGINAL_WRITE_AFTER_COPY_SOURCE, "write-after-copy.spx").unwrap_err();
    assert!(diagnostics.iter().any(|d| d.code == "SPX-T271"));
    let mut original = b"{\"x\":\"\xc3\xa9\\u0000\"}".to_vec();
    assert_eq!(original.len(), 16);
    original[0] = 255;
    assert_eq!(
        original,
        [255, 34, 120, 34, 58, 34, 195, 169, 92, 117, 48, 48, 48, 48, 34, 125]
    );
}
fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "selected-final-utf8.spx").unwrap()).unwrap()
}
fn emit(p: &ResolvedProgram) -> Result<NativeFinalUtf8Artifact, Diagnostic> {
    emit_native_final_utf8(
        p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
    )
}
#[test]
fn certificate_replay_binds_checked_input_constructor_layout_and_preserves_default_bytes() {
    let p = program(SOURCE);
    let before = super::super::emit_hir_c(&p).unwrap();
    let artifact = emit(&p).unwrap();
    assert_eq!(artifact, emit(&p).unwrap());
    assert_eq!(
        artifact,
        replay_native_final_utf8(
            &p,
            &DeclarationId::new("app.encode"),
            &DeclarationId::new("app.make"),
            &DeclarationId::new("app.dispose"),
            artifact.certificate()
        )
        .unwrap()
    );
    assert_eq!(before, super::super::emit_hir_c(&p).unwrap());
    assert!(!artifact.source().contains("int main("));
    assert!(artifact.source().contains("semaprax_final_utf8_prepare_v1"));
    assert!(artifact
        .source()
        .contains("spx_final_utf8_construct(spx_ctx"));
    assert!(!artifact.source().contains("not selected"));
    let mut forged = artifact.certificate().to_vec();
    forged[0] ^= 1;
    assert!(replay_native_final_utf8(
        &p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
        &forged
    )
    .is_err());
    let changed = program(&SOURCE.replace("mode:mode", "mode:0usize"));
    assert!(replay_native_final_utf8(
        &changed,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
        artifact.certificate()
    )
    .is_err());
    let metadata: serde_json::Value = serde_json::from_slice(artifact.certificate()).unwrap();
    assert_eq!(
        metadata["input_authority"],
        "checked-source-report-handle.v1"
    );
    assert_eq!(metadata["handle_capacity"], 8);
    assert_eq!(metadata["payload_limit"], 131072);
    assert_eq!(metadata["input_layout_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(metadata["result_layout_sha256"].as_str().unwrap().len(), 64);
}
#[test]
fn selected_closure_refuses_alias_hidden_allocations_and_wrong_provenance_roles() {
    let alias = SOURCE.replace(
        "Outcome::Encoded{text:string_from_utf8(bytes_as_slice(finished))}",
        "let text=string_from_utf8(bytes_as_slice(finished));Outcome::Encoded{text:text}",
    );
    assert_eq!(emit(&program(&alias)).unwrap_err().code, "SPX-B104");
    let literal = SOURCE.replace(
        "string_from_utf8(bytes_as_slice(finished))",
        "\"forbidden allocation\"",
    );
    assert!(emit(&program(&literal)).is_err());
    let wrong = SOURCE.replace(
        "fn dispose(value:own Report)->i64 {0}",
        "fn dispose(value:own Report)->i64 {1}",
    );
    assert!(emit(&program(&wrong)).is_err());
    let p = program(SOURCE);
    assert!(emit_native_final_utf8(
        &p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.dispose")
    )
    .is_err());
    let mut stale = p.clone();
    stale
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.encode")
        .unwrap()
        .body
        .ty = crate::hir::ResolvedType::Bool;
    assert!(emit(&stale).is_err());
}
#[test]
fn selected_checked_source_adapter_faults_and_lease_controls_at_o0_o2() {
    run_checked_source_adapter_probe(SOURCE);
}
#[test]
fn selected_nested_string_borrow_adapter_retains_all_fault_and_lease_controls() {
    let nested = SOURCE
        .replace("@id(\"data.report\") record Report { @id(\"data.raw\") raw:string,",
            "@id(\"data.payload\") record Payload { @id(\"data.raw\") raw:string, }\n@id(\"data.report\") record Report { @id(\"data.report.payload\") payload:Payload,")
        .replace("Report{raw:", "Report{payload:Payload{raw:")
        .replace(",mode:mode}}", "},mode:mode}}")
        .replace("value.raw", "value.payload.raw");
    assert_ne!(nested, SOURCE);
    run_checked_source_adapter_probe(&nested);
}
fn run_checked_source_adapter_probe(source: &str) {
    use std::process::Command;
    let p = program(source);
    let artifact = emit(&p).unwrap();
    let root = std::env::temp_dir().join(format!(
        "selected-final-utf8-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("probe.c");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n{}",
            include_str!("probe_allocations.c"),
            artifact.source(),
            include_str!("probe.c")
        ),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let result = Command::new(&binary).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"selected-final-utf8-v1: ok\n");
        assert!(result.stderr.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn final_constructor_accepts_authenticated_bytes_projection_and_refuses_array_or_alias_views() {
    let accepted = program(SOURCE);
    emit(&accepted).unwrap();
    let array = program(&SOURCE.replace(
        "string_from_utf8(bytes_as_slice(finished))",
        "string_from_utf8(array_as_slice(invalid))",
    ));
    let error = emit(&array)
        .err()
        .unwrap_or_else(|| panic!("Array view remains outside the profile"));
    assert_eq!(error.code, "SPX-B104");
    assert!(error.message.contains("exact bounded owned-Bytes view"));
    let aliased = program(&SOURCE.replace(
        "Outcome::Encoded{text:string_from_utf8(bytes_as_slice(finished))}",
        "let view=bytes_as_slice(finished);Outcome::Encoded{text:string_from_utf8(view)}",
    ));
    let error = emit(&aliased)
        .err()
        .unwrap_or_else(|| panic!("named alias remains outside the profile"));
    assert_eq!(error.code, "SPX-B104");
    assert!(error.message.contains("exact bounded owned-Bytes view"));
}

#[test]
fn resource_abi_uses_original_namespace_without_emitting_unselected_main_closure() {
    const EXCLUDED: &str = "EXCLUDED_NATIVE_UTF8_AUTHORITY_SENTINEL";
    let source = SOURCE.replace(
        "@id(\"app.main\") fn main()->i64 {0}",
        "@id(\"app.excluded\") fn excluded()->string {\"EXCLUDED_NATIVE_UTF8_AUTHORITY_SENTINEL\"} @id(\"app.main\") fn main()->i64 {let ignored=excluded();0}",
    );
    let p = program(&source);
    let selection = certificate::derive(
        &p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
    )
    .unwrap();
    let view = selection.emission_view(&p);
    assert_eq!(view.types, p.types);
    assert_eq!(view.interfaces, p.interfaces);
    assert!(!view
        .functions
        .iter()
        .any(|f| f.id == p.entrypoint || f.id.as_str() == "app.excluded"));
    let abi = super::super::native_resource::build_resource_abi(&p).unwrap();
    assert!(abi.resources.is_empty());
    assert!(abi.lifecycles.is_empty());
    assert!(abi.declarations.is_empty());
    // Resource ABI construction authenticates original metadata; it must
    // never expand the exact selected function/runtime/literal closure.
    let artifact = emit(&p).unwrap();
    assert!(!artifact.source().contains(EXCLUDED));
    for id in ["app.main", "app.excluded"] {
        let symbol = super::super::native_emit::c_function_symbol(&DeclarationId::new(id));
        assert!(!artifact.source().contains(&symbol));
        assert!(super::super::emit_hir_c(&p).unwrap().contains(&symbol));
    }
    assert!(super::super::emit_hir_c(&p).unwrap().contains(EXCLUDED));
    let resource = program(&source.replace("module selected.final;",
        "module selected.final; @id(\"unselected.resource\") resource Token {@id(\"unselected.resource.drop\") drop trivial;}"));
    assert_eq!(emit(&resource).unwrap_err().code, "SPX-B104");
}
#[test]
fn selected_emission_still_validates_absent_and_forged_original_main() {
    let p = program(SOURCE);
    let selection = certificate::derive(
        &p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
    )
    .unwrap();
    for mutation in 0..3 {
        let mut forged = p.clone();
        if mutation == 0 {
            forged.functions.retain(|f| f.id != p.entrypoint);
        } else {
            let main = forged
                .functions
                .iter_mut()
                .find(|f| f.id == p.entrypoint)
                .unwrap();
            if mutation == 1 {
                main.return_type = crate::hir::ResolvedType::Bool;
            } else {
                main.body.ty = crate::hir::ResolvedType::Bool;
            }
        }
        let ordinary = crate::hir::validate(&forged).unwrap_err();
        assert_eq!(ordinary.code, "SPX-H006");
        let selected =
            super::super::native_emit::emit_selected_final_utf8(&forged, &selection).unwrap_err();
        assert_eq!(selected.code, ordinary.code);
        assert_eq!(selected.message, ordinary.message);
        assert_eq!(emit(&forged).unwrap_err().code, "SPX-H006");
    }
}
