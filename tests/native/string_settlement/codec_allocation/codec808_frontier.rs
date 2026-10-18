//! Same original project preparation prefix; diagnostic output is after its ledger settles.
//! This witness neither runs nor qualifies the later native/C allocation assertions.
use crate::bounded_output::stage_debits::{take_completed, take_last_completed};
use crate::{format, parse, project};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static MARK: std::cell::Cell<Option<(bool, &'static str)>> = const { std::cell::Cell::new(None) };
}
fn mark(policy: project::JsonCodecProfile, phase: &'static str) {
    MARK.with(|slot| {
        slot.set(Some((
            matches!(policy, project::JsonCodecProfile::NestedRequest { .. }),
            phase,
        )))
    });
}
const SCHEMA: &str = r#"module consumer.schema;
@id("fault.config") record Config {@id("fault.config.label") label:string,@id("fault.config.retry") retry:usize,}
@id("fault.row") record Row {@id("fault.row.sku") sku:string,@id("fault.row.count") count:u8,}
@id("fault.report") record Report {@id("fault.report.config") config:Config,@id("fault.report.rows") rows:Vec<Row>,@id("fault.report.ok") ok:bool,}
@id("fault.anchor") fn anchor()->i64{0}
"#;
fn canonical(source: &str) -> String {
    format::canonical(&parse(source, "codec-allocation.spx").unwrap())
}
fn prepare_v2(response: project::JsonCodecProfile) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "native-codec-allocation-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("semaprax.toml"),
        include_str!("../bulk_utf8/semaprax.toml"),
    )
    .unwrap();
    fs::write(
        root.join("src/schema.spx"),
        canonical(&SCHEMA.replace("Vec<Row>", "Vec<string>")),
    )
    .unwrap();
    for (path, source) in [
        (
            "src/app.spx",
            "module consumer.app;@id(\"consumer.main\") fn main()->i64{0}",
        ),
        (
            "src/tests.spx",
            "module consumer.tests;@id(\"consumer.tests\") fn main()->i64{0}",
        ),
    ] {
        fs::write(root.join(path), canonical(source)).unwrap();
    }
    for policy in [
        project::JsonCodecProfile::NestedRequest {
            max_string_bytes: 64,
            max_array_items: 256,
        },
        response,
    ] {
        mark(policy, "prepare");
        let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            mark(policy, "derive");
            let revision = snapshot.retain_revision();
            let source = project::derive_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "fault.report",
                policy,
            )?;
            mark(policy, "verify");
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "fault.report",
                &source,
                policy,
            )?;
            assert_eq!(source, canonical(&source));
            Ok(source)
        })
        .unwrap();
        fs::write(root.join("src/schema.spx"), source).unwrap();
    }
    root
}
fn exact_original_fixture_prefix() {
    let original = include_str!(
        "../../../../docs/evidence/codec808-frontier-witness/original-string_vectors.rs"
    );
    let parent = include_str!(
        "../../../../docs/evidence/codec808-frontier-witness/original-codec_allocation.rs"
    );
    let current = crate::source_reflows_merge::current344::project(
        "tests/native/string_settlement/codec_allocation/string_vectors.rs",
        include_str!("string_vectors.rs"),
    );
    assert_eq!(current, original);
    assert_eq!(
        crate::source_reflows_merge::current344::project(
            "tests/native/string_settlement/codec_allocation.rs",
            include_str!("../codec_allocation.rs"),
        ),
        parent
    );
    let original = original.split_once("fn generated_v2(").unwrap().1;
    let start = original
        .find("    let root = std::env::temp_dir()")
        .unwrap();
    let stop = original
        .find("    fs::write(root.join(\"src/app.spx\"), canonical(&app_v2(invalid))).unwrap();")
        .unwrap();
    let prefix = &original[start..stop];
    let current = include_str!("codec808_frontier.rs");
    let helper = current.split_once("fn prepare_v2(").unwrap().1;
    let start = helper.find("    let root = std::env::temp_dir()").unwrap();
    let stop = helper.find("\n    root\n}").unwrap();
    let mut current_prefix = helper[start..stop + 1].to_owned();
    for hook in [
        "        mark(policy, \"prepare\");\n",
        "            mark(policy, \"derive\");\n",
        "            mark(policy, \"verify\");\n",
    ] {
        assert_eq!(current_prefix.matches(hook).count(), 1);
        current_prefix = current_prefix.replacen(hook, "", 1);
    }
    assert_eq!(
        current_prefix, prefix,
        "entire original body after only three exact diagnostic hooks"
    );
    let schema = &parent[parent.find("const SCHEMA:").unwrap()
        ..parent.find("// Independent exact oracle.").unwrap()];
    assert!(current.contains(schema));
    let canonical =
        &parent[parent.find("fn canonical(").unwrap()..parent.find("fn app()").unwrap()];
    assert!(current.contains(canonical));
}
#[test]
fn primitive_string_codec_actual_preparation_reports_existing_first_operation_without_replacing_failure(
) {
    let _successor_application = crate::cleanup_plan::SuccessorAppCensus::codec808();
    exact_original_fixture_prefix();
    let _route_observation = crate::bounded_output::stage_debits::route_debits::observe();
    take_completed();
    take_last_completed();
    MARK.with(|slot| slot.set(None));
    let result = std::panic::catch_unwind(|| {
        prepare_v2(project::JsonCodecProfile::NestedResponseV2 {
            max_string_bytes: 64,
            max_array_items: 256,
        })
    });
    // Existing cfg(test) instrumentation is observed only after the complete
    // actual ordinary ledger and its original failure have settled.
    let phase = MARK.with(|slot| slot.take());
    eprintln!("OPT808 actual preparation policy_is_request/phase: {phase:?}");
    let maximum = take_completed();
    let last = take_last_completed();
    eprintln!("OPT808 actual unchanged native preparation maximum core trace: {maximum:?}");
    eprintln!("OPT808 actual unchanged native preparation final core trace: {last:?}");
    crate::bounded_output::stage_debits::route_debits::print_settled();
    match result {
        Ok(root) => fs::remove_dir_all(root).unwrap(),
        Err(original) => {
            eprintln!(
                "OPT808 preserved original panic String: {:?}",
                original.downcast_ref::<String>()
            );
            std::panic::resume_unwind(original)
        }
    }
}

// Exact original complete fixture generator; namespace/input-location adapter only.
mod std08_cases {
    use super::{format, parse};
    use std::path::{Path, PathBuf};
    pub(super) fn conformance_manifests(scratch: &Path, manifest: &Path) -> Vec<PathBuf> {
        use crate::ast::{ModuleUseKind, Type};
        use std::collections::BTreeSet;
        const SOURCE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/project/standard_library/io_lines_cases.spx"
        ));
        const CASES: &[&str] = &[
            "test_crlf_line",
            "test_lf_line",
            "test_empty_line",
            "test_crlf_only",
            "test_bare_cr",
            "test_bare_cr_eof",
            "test_lone_cr_eof",
            "test_double_cr_eof",
            "test_bare_cr_offset",
            "test_unterminated_tail",
            "test_two_line_walk",
            "test_runtime_line_walk",
            "test_runtime_zero_line_walk",
            "test_runtime_many_line_walk",
            "test_offset_write",
        ];
        let parsed = parse(SOURCE, "io-lines-cases.spx").unwrap();
        assert_eq!(format::canonical(&parsed), SOURCE);
        let actual: Vec<_> = parsed
            .functions
            .iter()
            .filter(|f| f.name.starts_with("test_"))
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(actual, CASES, "line case inventory changed");
        let package = manifest.parent().unwrap();
        let mut manifests = vec![manifest.to_path_buf()];
        let mut covered = BTreeSet::new();
        for case in CASES {
            let mut selected = parsed.clone();
            let main = selected
                .functions
                .iter_mut()
                .find(|f| f.name == "main")
                .unwrap();
            main.body = parse(
                &format!("module fixture; @id(\"fixture.main\") fn main() -> i64 {{ {case}() }}"),
                "io-lines-entry.spx",
            )
            .unwrap()
            .functions[0]
                .body
                .clone();
            let mut calls = BTreeSet::from(["main".to_owned()]);
            loop {
                let previous = calls.len();
                for function in &selected.functions {
                    if calls.contains(&function.name) {
                        assert!(function.type_parameters.is_empty());
                        for expression in function
                            .requires
                            .iter()
                            .chain(&function.ensures)
                            .chain(std::iter::once(&function.body))
                        {
                            expression.visit_calls(&mut |name, _| {
                                calls.insert(name.to_owned());
                            });
                        }
                    }
                }
                if previous == calls.len() {
                    break;
                }
            }
            selected.functions.retain(|f| calls.contains(&f.name));
            let entry = selected.functions.iter().find(|f| f.name == *case).unwrap();
            assert!(entry.params.is_empty());
            assert_eq!(entry.return_type, Type::I64);
            selected
                .module_uses
                .retain(|u| u.kind != ModuleUseKind::Function || calls.contains(&u.alias));
            for import in &selected.module_uses {
                if import.kind == ModuleUseKind::Function && import.target_module == "std.io.lines"
                {
                    covered.insert(import.persistent_id.clone());
                }
            }
            let directory = scratch.join(case);
            std::fs::create_dir_all(directory.join("src")).unwrap();
            std::fs::copy(manifest, directory.join("semaprax.toml")).unwrap();
            for file in std::fs::read_dir(package.join("src")).unwrap() {
                let file = file.unwrap();
                std::fs::copy(file.path(), directory.join("src").join(file.file_name())).unwrap();
            }
            std::fs::write(
                directory.join("src/tests.spx"),
                format::canonical(&selected),
            )
            .unwrap();
            manifests.push(directory.join("semaprax.toml"));
        }
        let library = std::fs::read_to_string(package.join("src/lines.spx")).unwrap();
        let library = parse(&library, "lines.spx").unwrap();
        let public: BTreeSet<_> = library
            .functions
            .iter()
            .map(|f| f.stable_id.clone())
            .collect();
        assert_eq!(
            covered, public,
            "every line function needs direct executed coverage"
        );
        manifests
    }
}
// Diagnostic-only preparation prefixes of ALL unchanged STD-08 manifests.
#[test]
#[ignore = "explicit hosted diagnostic; not an application qualification"]
fn std08_io_lines_preparation_reports_actual_ledger_and_preserves_failure() {
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("std/io-lines/semaprax.toml");
    let scratch = std::env::temp_dir().join(format!(
        "std08-ledger-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&scratch).unwrap();
    let manifests = std08_cases::conformance_manifests(&scratch, &manifest);
    assert_eq!(manifests.len(), 16);
    assert_eq!(manifests[0], manifest);
    let _observation = crate::bounded_output::stage_debits::route_debits::observe();
    for (ordinal, current) in manifests.iter().enumerate() {
        take_completed();
        take_last_completed();
        eprintln!("STD08_PREFIX actual original manifest ordinal={ordinal} path={current:?}");
        let result = std::panic::catch_unwind(|| {
            project::with_authenticated_project(current, |snapshot| snapshot.check()).unwrap()
        });
        let maximum = take_completed();
        let last = take_last_completed();
        eprintln!("STD08_PREFIX ordinal={ordinal} maximum core trace: {maximum:?}");
        eprintln!("STD08_PREFIX ordinal={ordinal} final core trace: {last:?}");
        crate::bounded_output::stage_debits::route_debits::print_settled();
        if let Err(original) = result {
            eprintln!(
                "STD08_PREFIX ordinal={ordinal} preserved original panic String: {:?}",
                original.downcast_ref::<String>()
            );
            let _ = std::fs::remove_dir_all(&scratch);
            std::panic::resume_unwind(original);
        }
    }
    std::fs::remove_dir_all(scratch).unwrap();
}
