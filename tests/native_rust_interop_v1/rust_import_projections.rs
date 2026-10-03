use super::*;
use semaprax::{impact, review};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CASE: AtomicU64 = AtomicU64::new(0);

struct SourceCase {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
}

impl SourceCase {
    fn new(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-rust-projections-{}-{}",
            std::process::id(),
            NEXT_CASE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let source_path = directory.join("source.spx");
        let patch = directory.join("change.spatch");
        fs::write(&source_path, source).unwrap();
        let revision = graph::revision(&parse(source, &source_path).unwrap());
        fs::write(
            &patch,
            format!("base {revision}\nrename rust.host.helper to renamed_helper\n"),
        )
        .unwrap();
        Self {
            directory,
            source: source_path,
            patch,
        }
    }
}

impl Drop for SourceCase {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[test]
fn native_rust_imports_remain_closed_in_generic_context_impact_and_review() {
    for source in [
        r#"module test.rust_projection;
@id("rust.host") interface RustHost permits {  } {
    @id("rust.host.selected") import rust fn selected(value: i64) -> i64 effects {  } failure infallible;
}
@id("rust.host.main") fn main() -> i64 { 0 }
@id("rust.host.helper") fn helper() -> i64 { 1 }
"#,
        r#"module test.rust_projection;
@id("rust.host") interface RustHost permits {  } {
    @id("rust.host.selected") import rust selected fn selected from "local_api_fixture::cfg_selected" effects {  } failure infallible;
}
@id("rust.host.main") fn main() -> i64 { 0 }
@id("rust.host.helper") fn helper() -> i64 { 1 }
"#,
    ] {
        let case = SourceCase::new(source);
        let program = parse(source, &case.source).unwrap();
        let context = graph::context_json(&program, "rust.host.main", 1).unwrap_err();
        assert!(matches!(context[0].code, "SPX-G218" | "SPX-B147"));
        let before = fs::read(&case.source).unwrap();
        let impact = impact::preview(
            &case.source,
            &case.patch,
            &impact::SemanticImpactOptions::default(),
        )
        .unwrap_err();
        assert!(
            matches!(impact[0].code, "SPX-G218" | "SPX-B147"),
            "{impact:?}"
        );
        let review = review::preview(&case.source, &case.patch).unwrap_err();
        assert!(matches!(review[0].code, "SPX-G218" | "SPX-B147"));
        assert_eq!(fs::read(&case.source).unwrap(), before);
    }
}
