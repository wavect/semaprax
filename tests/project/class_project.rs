use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn cli(root: &std::path::Path, command: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg(command)
        .arg(root.join("semaprax.toml"))
        .output()
        .unwrap()
}

#[test]
fn class_declarations_and_methods_replay_through_project_execution() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-class-project-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v1\"\nname = \"probe\"\nentry = \"probe.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"probe.tests.one\"]\ntests = [\"probe.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module probe.app;\n\n@id(\"probe.base\")\nclass Base {\n    @id(\"probe.base.v\")\n    v: i64,\n\n    @id(\"probe.base.get\")\n    fn get(self: Base) -> i64\n{\n        self.v\n    }\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    let b = Base { v: 5 };\n    b.get()\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module probe.tests;\n\n@id(\"probe.tests.one\")\nfn one() -> i64\n{\n    1\n}\n\n@id(\"probe.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();

    let checked = cli(&root, "check");
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let run = cli(&root, "run");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"5\n");
    let test = cli(&root, "test");
    assert!(
        test.status.success(),
        "{}",
        String::from_utf8_lossy(&test.stderr)
    );
    assert_eq!(test.stdout, b"project tests passed\n");
    std::fs::remove_dir_all(root).unwrap();
}
