//! README browser-package walkthrough.
//!
//! The README block is a newcomer's first generated artifact, and nothing
//! bound its text to real CLI behavior, so it drifted into a package the
//! documented verifier cannot accept: the exported-scalar package has no
//! `app.main` entry for `scripts/verify-web.mjs` to call. This module parses
//! the documented commands out of the README and runs exactly those, so the
//! walkthrough cannot claim a flow the repository does not execute.

use std::path::Path;
use std::process::Command;

const README_WEB_PACKAGE_COMMANDS: &str = "semaprax build examples/calculator.spx --target web \\
  --export calculator.add --export calculator.subtract \\
  --export calculator.multiply --export calculator.divide \\
  --export calculator.is-negative --export calculator.not \\
  -o target/calculator-web

node scripts/verify-wasm-scalar-exports.mjs target/calculator-web";

const DOCUMENTED_OUTPUT: &str = "scalar-exports-v1-ok";

#[test]
fn the_readme_browser_package_commands_run_as_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert!(
        readme.contains(&format!("```sh\n{README_WEB_PACKAGE_COMMANDS}\n```")),
        "README.md no longer contains the pinned browser-package block"
    );
    assert!(
        readme.contains(&format!("`{DOCUMENTED_OUTPUT}`")),
        "README.md must state the output the verifier prints"
    );

    let mut commands = documented_commands();
    assert_eq!(
        commands.len(),
        2,
        "expected one build and one verify command"
    );
    let verify = commands.pop().unwrap();
    let build = commands.pop().unwrap();

    let output = std::env::temp_dir().join(format!(
        "semaprax-readme-web-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&output);

    assert_eq!(build.first().map(String::as_str), Some("semaprax"));
    let build_arguments = redirect_output(&build[1..], &output);
    let build_result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(root)
        .args(&build_arguments)
        .output()
        .unwrap();
    assert!(
        build_result.status.success(),
        "documented build failed: {}",
        String::from_utf8_lossy(&build_result.stderr)
    );

    assert_eq!(verify.first().map(String::as_str), Some("node"));
    assert_eq!(verify.len(), 3, "expected a script and a package argument");
    let script = root.join(&verify[1]);
    assert!(
        script.is_file(),
        "README names a missing verifier: {}",
        script.display()
    );
    assert_eq!(
        verify[2], "target/calculator-web",
        "the verifier must read the package the documented build wrote"
    );

    if Command::new("node").arg("--version").output().is_ok() {
        let verify_result = Command::new("node")
            .arg(&script)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            verify_result.status.success(),
            "documented verifier failed: {}",
            String::from_utf8_lossy(&verify_result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&verify_result.stdout).trim(),
            DOCUMENTED_OUTPUT
        );
    }

    let _ = std::fs::remove_dir_all(&output);
}

/// Splits the documented block into argument vectors, joining the shell line
/// continuations the README uses for readability.
fn documented_commands() -> Vec<Vec<String>> {
    README_WEB_PACKAGE_COMMANDS
        .replace("\\\n", " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split_whitespace().map(str::to_owned).collect())
        .collect()
}

/// Rewrites the documented `-o` destination so the gate never publishes into
/// the shared build directory the README names.
fn redirect_output(arguments: &[String], destination: &Path) -> Vec<String> {
    let mut rewritten = Vec::with_capacity(arguments.len());
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        rewritten.push(argument.clone());
        if argument == "-o" {
            let documented = arguments.next().expect("-o requires a destination");
            assert_eq!(documented, "target/calculator-web");
            rewritten.push(destination.to_string_lossy().into_owned());
        }
    }
    rewritten
}
