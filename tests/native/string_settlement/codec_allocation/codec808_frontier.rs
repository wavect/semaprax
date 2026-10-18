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

// Diagnostic-only first-manifest prefix of the unchanged STD-08 owner.
// The integration gate and every backend assertion remain on main.
#[test]
#[ignore = "explicit hosted diagnostic; not an application qualification"]
fn std08_io_lines_preparation_reports_actual_ledger_and_preserves_failure() {
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("std/io-lines/semaprax.toml");
    let _observation = crate::bounded_output::stage_debits::route_debits::observe();
    take_completed();
    take_last_completed();
    let result = std::panic::catch_unwind(|| {
        // Same first manifest and authentication/check prefix used by
        // run_examples_and_conformance; no replacement program or allowance.
        project::with_authenticated_project(&manifest, |snapshot| snapshot.check()).unwrap()
    });
    // Read and print only after all original ledger ownership has settled.
    let maximum = take_completed();
    let last = take_last_completed();
    eprintln!("STD08_PREFIX diagnostic expected source forecast=4345378 layouts=4; unit observation profile, not an integration/backend pass");
    eprintln!("STD08_PREFIX maximum core trace: {maximum:?}");
    eprintln!("STD08_PREFIX final core trace: {last:?}");
    crate::bounded_output::stage_debits::route_debits::print_settled();
    if let Err(original) = result {
        eprintln!(
            "STD08_PREFIX preserved original panic String: {:?}",
            original.downcast_ref::<String>()
        );
        std::panic::resume_unwind(original);
    }
}
