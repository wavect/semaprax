use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn project(app: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-local-record-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v1\"\nname = \"local-record\"\nentry = \"local.record.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"local.record.web\"]\ntests = [\"local.record.tests\"]\n",
    )
    .unwrap();
    let app = semaprax::parse(app, std::path::Path::new("src/app.spx")).unwrap();
    std::fs::write(root.join("src/app.spx"), semaprax::format::canonical(&app)).unwrap();
    let tests = semaprax::parse(
        "module local.record.tests;\n\n@id(\"local.record.web\")\nfn web() -> i64 { 1 }\n\n@id(\"local.record.tests.main\")\nfn main() -> i64 { 0 }\n",
        std::path::Path::new("src/tests.spx"),
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        semaprax::format::canonical(&tests),
    )
    .unwrap();
    root
}

fn cli(root: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command.args(args).arg(root.join("semaprax.toml"));
    command.output().unwrap()
}

#[test]
fn module_local_copy_record_runs_and_builds() {
    let root = project(
        "module local.record.app;\n\n@id(\"local.record.point\")\nrecord Point {\n    @id(\"local.record.point.x\")\n    x: i64,\n    @id(\"local.record.point.y\")\n    y: i64,\n}\n\n@id(\"local.record.sum\")\nfn sum(x: i64, y: i64) -> i64\n{\n    let point = Point { x: x, y: y };\n    point.x + point.y\n}\n\n@id(\"local.record.main\")\nfn main() -> i64 { sum(2, 3) }\n",
    );
    let checked = cli(&root, &["check"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let run = cli(&root, &["run"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"5\n");
    let built = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["build", "--target", "native", "-o"])
        .arg(root.join("out.c"))
        .arg(root.join("semaprax.toml"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(root.join("out.c").is_file());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn aggregate_project_signature_has_a_located_actionable_diagnostic() {
    let root = project(
        "module local.record.app;\n\n@id(\"local.record.point\")\nrecord Point {\n    @id(\"local.record.point.x\")\n    x: i64,\n}\n\n@id(\"local.record.read\")\nfn read(point: Point) -> i64 { point.x }\n\n@id(\"local.record.main\")\nfn main() -> i64 { 0 }\n",
    );
    let checked = cli(&root, &["check", "--json"]);
    assert!(!checked.status.success());
    let diagnostic = String::from_utf8(checked.stdout).unwrap();
    assert!(diagnostic.contains("\"code\":\"SPX-G174\""), "{diagnostic}");
    assert!(
        diagnostic.contains("\"path\":\"src/app.spx\""),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("\"location\":null"), "{diagnostic}");
    assert!(
        diagnostic.contains("Project v1 function boundaries admit only Copy scalar values"),
        "{diagnostic}"
    );
    std::fs::remove_dir_all(root).unwrap();
}
