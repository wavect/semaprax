//! Runtime and hostile-HIR gates for the tagged source-copy operation.
use super::*;

#[test]
fn tagged_source_native_o0_o2_preserve_success_and_precommit_failure() {
    if !command_available("clang") {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-tagged-source-native-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    for (label, source, successful) in [
        ("one-five", SET1_OR5_LOOP, true),
        ("one-six-forty-eight", SET1_OR6_OR48_LOOP, true),
        ("five-failure", SET1_OR5_COMPUTED_OUT_OF_RANGE, false),
        (
            "forty-eight-failure",
            SET1_OR6_OR48_COMPUTED_OUT_OF_RANGE,
            false,
        ),
    ] {
        let parsed = parse(source, "tagged-source-native.spx").unwrap();
        let c = codegen::emit_c(&parsed).unwrap();
        let c_path = root.join(format!("{label}.c"));
        std::fs::write(&c_path, c).unwrap();
        for optimization in ["-O0", "-O2"] {
            let executable = root.join(format!(
                "{label}{optimization}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let build = Command::new("clang")
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&c_path)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                build.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&build.stderr)
            );
            let run = Command::new(&executable).output().unwrap();
            if successful {
                assert!(
                    run.status.success(),
                    "{label}: {}",
                    String::from_utf8_lossy(&run.stderr)
                );
                assert_eq!(run.stdout, b"7\n", "{label} {optimization}");
            } else {
                assert_eq!(run.status.code(), Some(73), "{label} {optimization}");
                assert!(run.stdout.is_empty());
                assert_eq!(
                    String::from_utf8_lossy(&run.stderr).trim(),
                    "SEMAPRAX operation failure: semaprax.byte-buffer.v1/1"
                );
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn tagged_source_call_commit_is_independently_authenticated() {
    let parsed = parse(SET1_OR6_OR48_LOOP, "tagged-source-commit.spx").unwrap();
    let mut resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let transition = main_function_mut(&mut resolved).cleanup_plan.blocks.iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| matches!(transition, CleanupTransition::CallCommit { arguments, .. } if !arguments.is_empty()))
        .unwrap();
    let CleanupTransition::CallCommit { arguments, .. } = transition else {
        unreachable!()
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments[0].parameter_index, 0);
    arguments[0].parameter_index = 3;
    assert_eq!(hir::validate(&resolved).unwrap_err().code, "SPX-H006");
}
