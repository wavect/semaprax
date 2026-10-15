//! Raw scalar-word and hostile carrier controls at the actual native helper.
use super::corpus;
use std::process::Command;
use std::sync::atomic::Ordering;
#[test]
fn exact_native_float_payloads_unrelated_copy_words_and_hostile_carriers_preserve_precommit_and_drop(
) {
    for (ty, kind) in [("f32", 6), ("f64", 7)] {
        let ast =
            semaprax::check(&corpus::boundary_source(ty), "field-update-boundary.spx").unwrap();
        let generated = semaprax::codegen::emit_c(&ast).unwrap();
        let layouts = generated
            .lines()
            .filter(|line| {
                line.starts_with("static __attribute__((unused)) const spx_leaf_layout_v1 ")
            })
            .collect::<Vec<_>>();
        assert_eq!(layouts.len(), 1);
        let layout = layouts[0]
            .split("spx_leaf_layout_v1 ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap();
        let probe = include_str!("copy_field_update_boundary.c")
            .replace("PROBE_LAYOUT", layout)
            .replace("PROBE_KIND", &kind.to_string());
        let trap = r#"
#include <setjmp.h>
static jmp_buf fixture_trap;
static int fixture_expect_trap;
static __attribute__((noreturn)) void fixture_abort(void) {
 if(fixture_expect_trap)longjmp(fixture_trap,1);
 abort();
}
#define abort fixture_abort
"#;
        let root = std::env::temp_dir().join(format!(
            "field-update-boundary-{}-{}",
            std::process::id(),
            super::NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("case.c");
        std::fs::write(
            &file,
            format!(
                "{}\n{trap}\n{generated}\n{probe}",
                include_str!("allocations.c")
            ),
        )
        .unwrap();
        for opt in ["-O0", "-O2"] {
            let binary = root.join(format!("case{opt}{}", std::env::consts::EXE_SUFFIX));
            let built = Command::new("clang")
                .args([
                    "-std=c11",
                    opt,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&file)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            let ran = Command::new(&binary).output().unwrap();
            assert!(
                ran.status.success(),
                "{ty} {opt}: {:?} {}",
                ran.status.code(),
                String::from_utf8_lossy(&ran.stderr)
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
