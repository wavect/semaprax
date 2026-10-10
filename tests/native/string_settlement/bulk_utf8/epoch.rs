use super::{directory, fs};
use std::process::Command;

pub(super) fn compile_and_run(source: &str) {
    let root = directory("epoch");
    let path = root.join("probe.c");
    fs::write(&path, source).unwrap();
    let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let built = Command::new(&compiler)
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&built.stderr)
        );
        for argument in ["current", "current-empty", "internal-max", "foreign-max"] {
            let output = Command::new(&executable).arg(argument).output().unwrap();
            assert!(
                output.status.success(),
                "{optimization}/{argument}: {output:?}"
            );
            assert_eq!(output.stdout, b"native-ordinary-strings-settled\n");
            assert!(output.stderr.is_empty());
        }
        for (argument, diagnostic) in [
            ("stale", "borrowed byte slice epoch is stale"),
            ("stale-empty", "borrowed byte slice epoch is stale"),
            ("unleased", "unleased byte slice carries an epoch"),
            (
                "foreign-over",
                "borrowed byte slice exceeds the exact length bound",
            ),
        ] {
            let output = Command::new(&executable).arg(argument).output().unwrap();
            assert!(
                !output.status.success(),
                "{optimization}/{argument}: {output:?}"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(diagnostic),
                "{optimization}/{argument}: {stderr}"
            );
            assert!(!stderr.contains("unexpected allocation"), "{stderr}");
            assert!(output.stdout.is_empty());
        }
    }
    super::super::remove_successful_fixture(&root);
}
