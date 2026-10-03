//! Instrument the generated C boundary; Rust dependencies are not instrumented.
use super::*;

pub(super) fn run_native_sanitizers(
    cargo: &str,
    clang: &str,
    sdk: &Path,
    target: &Path,
    object: &Path,
) {
    let name = if cfg!(target_os = "macos") {
        "libclang_rt.asan_osx_dynamic.dylib".to_owned()
    } else {
        format!("libclang_rt.asan-{}.so", std::env::consts::ARCH)
    };
    let found = Command::new(clang)
        .arg(format!("--print-file-name={name}"))
        .output()
        .unwrap();
    assert!(
        found.status.success(),
        "configured Clang runtime discovery failed"
    );
    let runtime = std::path::PathBuf::from(String::from_utf8(found.stdout).unwrap().trim());
    assert!(
        runtime.is_absolute() && runtime.is_file(),
        "configured sanitizer runtime is unavailable: {}",
        runtime.display()
    );
    let source_path = sdk.join("src/url_project.c");
    let authentic = std::fs::read_to_string(&source_path).unwrap();
    let sanitized = sdk.join("url_project_sanitized.o");
    let compile = || {
        let result = Command::new(clang)
            .args([
                "-std=c11",
                "-O1",
                "-g",
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-c",
            ])
            .arg(&source_path)
            .arg("-o")
            .arg(&sanitized)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    // Only the final consumer links the instrumented C and runtime. Applying
    // these flags to proc-macro dylibs loads ASan too late inside rustc.
    let run = || {
        let built = Command::new(cargo)
            .args([
                "rustc",
                "--bin",
                "ri06-url-owner",
                "--offline",
                "--locked",
                "--quiet",
                "--",
                "-C",
            ])
            .arg(format!("linker={clang}"))
            .arg("-C")
            .arg(format!("link-arg={}", sanitized.display()))
            .args(["-C", "link-arg=-fsanitize=address,undefined", "-C"])
            .arg(format!("link-arg={}", runtime.display()))
            .arg("-C")
            .arg(format!(
                "link-arg=-Wl,-rpath,{}",
                runtime.parent().unwrap().display()
            ))
            .current_dir(sdk)
            .env("CARGO_TARGET_DIR", target)
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "sanitized consumer link: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        let mut command = Command::new(target.join("debug/ri06-url-owner"));
        if cfg!(target_os = "macos") {
            command.env("DYLD_INSERT_LIBRARIES", &runtime);
        }
        command
            .env("ASAN_OPTIONS", "halt_on_error=1:detect_leaks=0")
            .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1")
            .output()
            .unwrap()
    };
    compile();
    let success = run();
    assert!(
        success.status.success(),
        "sanitized checked body/callback/corpus: {}",
        String::from_utf8_lossy(&success.stderr)
    );
    // Deliberately write the terminator one byte beyond its allocation. This
    // must fail under ASan, proving instrumentation reaches the generated C.
    assert!(authentic.contains("[v.length]=0"));
    let mutant = authentic.replacen("[v.length]=0", "[v.length+1]=0", 1);
    std::fs::write(&source_path, mutant).unwrap();
    compile();
    // Cargo cannot see arbitrary linker-input content changes; force a tiny
    // caller rebuild without changing its assertions or dependency identity.
    let main = sdk.join("src/main.rs");
    let caller = std::fs::read_to_string(&main).unwrap();
    std::fs::write(&main, format!("{caller}\n// sanitizer mutation control\n")).unwrap();
    let failure = run();
    let errors = String::from_utf8_lossy(&failure.stderr);
    assert!(
        !failure.status.success()
            && errors.contains("AddressSanitizer")
            && errors.contains("heap-buffer-overflow"),
        "sanitizer mutation was not detected: {errors}"
    );
    std::fs::write(main, caller).unwrap();
    std::fs::write(source_path, authentic).unwrap();
    assert!(object.is_file(), "ordinary O0 object remains untouched");
}
