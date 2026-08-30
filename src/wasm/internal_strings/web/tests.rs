use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

const SOURCE: &str =
    "module web.bounds;\n@id(\"main\")\nfn main() -> i64 { string_len(\"hello\") }\n";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn directory() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-string-web-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn descriptor_and_complete_package_bounds_are_exact_and_checked() {
    assert!(bounded(DESCRIPTOR_LIMIT, DESCRIPTOR_LIMIT, "descriptor").is_ok());
    assert_eq!(
        bounded(DESCRIPTOR_LIMIT + 1, DESCRIPTOR_LIMIT, "descriptor")
            .unwrap_err()
            .code,
        "SPX-W111"
    );
    assert!(package_size([PACKAGE_LIMIT - 1, 1]).is_ok());
    assert_eq!(
        package_size([PACKAGE_LIMIT, 1]).unwrap_err().code,
        "SPX-W111"
    );
    assert_eq!(package_size([usize::MAX, 1]).unwrap_err().code, "SPX-W111");
}

#[test]
fn exact_source_bound_is_read_and_plus_one_fails_before_output() {
    let root = directory();
    let source = root.join("input.spx");
    let mut text = SOURCE.to_owned();
    text.push_str("//");
    text.extend(std::iter::repeat_n('x', SOURCE_LIMIT - text.len()));
    std::fs::write(&source, &text).unwrap();
    let snapshot =
        crate::patch::read_source_snapshot_bounded(&source, SOURCE_LIMIT, "SPX-W111").unwrap();
    assert_eq!(snapshot.source().len(), SOURCE_LIMIT);
    text.push('x');
    std::fs::write(&source, text).unwrap();
    let output = root.join("output");
    let errors = build_web_from_source(&source, &output, &["main".to_owned()]).unwrap_err();
    assert_eq!(errors[0].code, "SPX-W111");
    assert!(!output.exists());
    std::fs::remove_file(source).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn source_drift_and_growth_are_rejected_before_destination_creation() {
    for grow in [false, true] {
        let root = directory();
        let source = root.join("input.spx");
        let output = root.join("output");
        std::fs::write(&source, SOURCE).unwrap();
        let errors = build(&source, &output, &["main".to_owned()], || {
            if grow {
                std::fs::write(&source, vec![b' '; SOURCE_LIMIT + 1]).unwrap();
            } else {
                std::fs::write(&source, SOURCE.replace("hello", "world")).unwrap();
            }
        })
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-I207");
        assert!(!output.exists());
        std::fs::remove_file(source).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}

#[test]
fn explicit_empty_identity_keeps_existing_compiler_admission() {
    let root = directory();
    let source = root.join("input.spx");
    let output = root.join("output");
    std::fs::write(&source, SOURCE.replace("@id(\"main\")", "@id(\"\")")).unwrap();
    build_web_from_source(&source, &output, &[String::new()]).unwrap();
    let declarations = std::fs::read_to_string(output.join("semaprax.d.ts")).unwrap();
    assert!(declarations.contains("call(id: \"\")"));
    let descriptor =
        std::fs::read_to_string(output.join("semaprax.internal-strings.json")).unwrap();
    assert!(descriptor.contains("\"stable_id\":\"\""));
    for path in [
        "app.wasm",
        "semaprax.js",
        "semaprax.d.ts",
        "semaprax.internal-strings.json",
        "semaprax.manifest.json",
        "package.json",
        "index.html",
        "app.js",
    ] {
        std::fs::remove_file(output.join(path)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
    std::fs::remove_file(source).unwrap();
    std::fs::remove_dir(root).unwrap();
}
