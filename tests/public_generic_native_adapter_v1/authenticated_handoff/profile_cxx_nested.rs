//! C++17 generated-caller positive control for the private
//! `semaprax.authenticated-native-moves-nested.v1` profile (issue #292 / #288
//! follow-on). `checked_nested_moves` already executes this exact genuinely
//! two-level `Outer<Leaf>` checked swap body through the generated C11
//! caller, and `profile_rust` through the generated Rust caller; this module
//! is the same body's generated C++17 move-only wrapper, real `-O0`/`-O2`
//! execution evidence, not a mere compile check. The hostile corpus and
//! omission controls for this profile's C++ caller are `profile_hostility`'s
//! own third profile (it drives only refusal cases); this module supplies
//! the positive control `profile_hostility` does not carry.
use super::*;
use semaprax::public_generic_abi::native::authenticated::{
    render_authenticated_nested_moves_provider, AuthenticatedNativeNestedMovesArtifact,
};
use semaprax::public_generic_consumer::cxx_calling::generate_authenticated_nested_moves_calling_consumer_v1;
use semaprax::public_generic_consumer::rust_calling::{OwnedByteField, RecordShape};
use std::{env, process::Command};

fn provider(artifact: &AuthenticatedNativeNestedMovesArtifact) -> String {
    format!(
        "{}\nstatic size_t endpoint_calls;\n#define SPX_PG_OBSERVE_ENDPOINT() (++endpoint_calls)\n{}\n#undef malloc\n#undef free\nsize_t auth_allocations(void) {{ return fixture_allocations; }}\nsize_t auth_live(void) {{ return fixture_live; }}\nsize_t auth_calls(void) {{ return endpoint_calls; }}\n",
        include_str!("../allocations.c"),
        artifact.source()
    )
}

fn field(path: &str) -> String {
    format!(
        "field_{}",
        path.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

/// Patches the shared `same_subject_cxx.cpp` template's two output
/// assertions (which assume an identity-shaped, unswapped result) to expect
/// the checked nested body's actual `a`/`b` swap -- exactly the same
/// technique `checked_moves`'s own C++ driver patch uses for its swapped
/// branch, restated here because this module cannot reach that
/// submodule-private helper.
fn patch_swap(driver: &mut String, output: &RecordShape) {
    let expected = ["[2, 11, 17, 23]", "[1, 7, 13]"];
    let original = ["[1, 7, 13]", "[2, 11, 17, 23]"];
    for index in 0..2 {
        let name = field(&output.fields[index].identity);
        let from = format!(
            "assert(to_owned(last.{name}()) == std::vector<std::uint8_t>({{{}}}));",
            original[index]
                .trim_start_matches('[')
                .trim_end_matches(']')
        );
        assert_eq!(driver.matches(&from).count(), 1, "template drift: {from}");
        *driver = driver.replace(
            &from,
            &format!(
                "if (to_owned(last.{name}()) != std::vector<std::uint8_t>({{{}}})) return 42;",
                expected[index]
                    .trim_start_matches('[')
                    .trim_end_matches(']')
            ),
        );
    }
}

fn compile_and_run(root: &Path, opt: &str) {
    let clang = env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let mut objects = Vec::new();
    for source in ["provider.c", "spx_pg_calling_consumer.c"] {
        let object = root.join(format!("{source}{opt}.o"));
        let compile = Command::new(&clang)
            .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror", "-c"])
            .arg(root.join(source))
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        objects.push(object);
    }
    let executable = root.join(format!("probe{opt}{}", env::consts::EXE_SUFFIX));
    let link = Command::new(env::var_os("CLANGXX").unwrap_or_else(|| "clang++".into()))
        .args(["-std=c++17", opt, "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(root)
        .arg(root.join("driver.cpp"))
        .args(objects)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        link.status.success(),
        "{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let run = Command::new(executable).output().unwrap();
    assert!(
        run.status.success(),
        "{opt}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"cxx-authenticated-caller-settled");
}

/// Real, compiled (`-O0`/`-O2`) execution of the checked nested-record swap
/// body through the generated C++17 move-only wrapper, and of the same
/// body's `requires false` sibling reporting the checked contract failure --
/// against the SAME two-level `Outer<Leaf>` shape `checked_nested_moves` and
/// `profile_rust` already execute.
#[test]
fn generated_cxx_executes_the_checked_nested_movement_body() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r292-nested-moves-cxx-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    for guard in [true, false] {
        let nested_source = super::super::checked_nested_moves::SOURCE;
        let nested_id = super::super::checked_nested_moves::EXPORT_ID;
        let source = if guard {
            nested_source.to_owned()
        } else {
            nested_source.replacen("requires true", "requires false", 1)
        };
        let parsed = semaprax::check(&source, Path::new("nested-moves-cxx.spx")).unwrap();
        let revision = semaprax::format::canonical(&parsed);
        let program = semaprax::hir::resolve(&parsed).unwrap();
        let endpoint =
            derive_admitted_public_generic_endpoint_v1(&program, &revision, nested_id).unwrap();
        let descriptor = endpoint.descriptor();
        let artifact =
            render_authenticated_nested_moves_provider(&program, &revision, descriptor).unwrap();
        let consumer =
            generate_authenticated_nested_moves_calling_consumer_v1(descriptor, &artifact).unwrap();

        let shape = |paths: &[String]| {
            RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
        };
        let input = shape(&descriptor.input_facts().owned_leaves);
        let output = shape(&descriptor.result_facts().owned_leaves);
        assert_eq!(input.fields.len(), 2);
        assert_eq!(output.fields.len(), 2);

        let directory = root.join(format!("guard-{guard}"));
        for (name, contents) in consumer.files() {
            let path = directory.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        fs::write(directory.join("provider.c"), provider(&artifact)).unwrap();
        super::cxx::write_driver(&directory, &input, &output, 1, guard, 14);
        // Patched unconditionally, matching `checked_moves`'s own precedent:
        // the template's `if (guard)` branch is ordinary compiled code, not
        // `if constexpr`, so it type-checks -- and so must patch cleanly --
        // whichever way `guard` runs at runtime.
        let path = directory.join("driver.cpp");
        let mut driver = fs::read_to_string(&path).unwrap();
        patch_swap(&mut driver, &output);
        fs::write(path, driver).unwrap();
        for opt in ["-O0", "-O2"] {
            compile_and_run(&directory, opt);
            eprintln!("R292 generated C++17 moves-nested guard={guard} {opt}: checked swap/contract-failure passed");
        }
    }
    fs::remove_dir_all(root).unwrap();
}
