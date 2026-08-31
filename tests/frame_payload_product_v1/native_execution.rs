//! Explicit native compiler policy; corpus construction stays in backend_equivalence.
use super::*;

pub(super) enum Execution {
    Ordinary,
    Sanitized(PathBuf),
}

impl Execution {
    fn sanitized() -> Self {
        let compiler = PathBuf::from(
            std::env::var_os("SEMAPRAX_FRAME_SANITIZER_CLANG")
                .expect("selected frame sanitizer gate requires SEMAPRAX_FRAME_SANITIZER_CLANG"),
        );
        assert!(compiler.is_absolute() && compiler.is_file());
        Self::Sanitized(compiler)
    }

    pub(super) fn write_source(&self, path: &Path, source: &str) {
        match self {
            Self::Ordinary => fs::write(path, source.as_bytes()).unwrap(),
            Self::Sanitized(_) => {
                // A compiler driver that silently drops instrumentation flags
                // must not turn this explicitly selected gate into plain C.
                let required = "#ifndef __has_feature\n#error Clang sanitizer support is required\n#endif\n#if !__has_feature(address_sanitizer)\n#error AddressSanitizer instrumentation is required\n#endif\n#if !__has_feature(undefined_behavior_sanitizer)\n#error UndefinedBehaviorSanitizer instrumentation is required\n#endif\n";
                fs::write(path, format!("{required}{source}")).unwrap();
            }
        }
    }

    pub(super) fn compile_and_run(
        &self,
        source: &Path,
        executable: &Path,
        optimization: &str,
        settlement: bool,
    ) {
        let lane = if settlement { "settlement " } else { "" };
        self.compile(source, executable, optimization, lane);
        let execute = self.command(executable).output().unwrap();
        assert!(
            execute.status.success(),
            "{optimization} {lane}status={:?} stdout={} stderr={}",
            execute.status.code(),
            String::from_utf8_lossy(&execute.stdout),
            String::from_utf8_lossy(&execute.stderr)
        );
        if settlement || matches!(self, Self::Sanitized(_)) {
            assert!(execute.stdout.is_empty());
            assert!(execute.stderr.is_empty());
        }
    }

    fn compile(&self, source: &Path, executable: &Path, optimization: &str, lane: &str) {
        let compiler = match self {
            Self::Ordinary => Path::new("clang"),
            Self::Sanitized(compiler) => compiler,
        };
        let mut compile = Command::new(compiler);
        compile.args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"]);
        if matches!(self, Self::Sanitized(_)) {
            compile.args([
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-fno-omit-frame-pointer",
            ]);
        }
        let compile = compile
            .arg(source)
            .arg("-o")
            .arg(executable)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{optimization} {lane}compile stderr={}",
            String::from_utf8_lossy(&compile.stderr)
        );
    }

    fn command(&self, executable: &Path) -> Command {
        let mut execute = Command::new(executable);
        if matches!(self, Self::Sanitized(_)) {
            execute.env("ASAN_OPTIONS", "halt_on_error=1");
            execute.env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        execute
    }

    fn calibrate(&self) {
        assert!(matches!(self, Self::Sanitized(_)));
        let root = temporary("sanitizer-calibration");
        fs::create_dir_all(&root).unwrap();
        // These deliberately invalid, independent runtime controls are not
        // product cases. Volatile reads retain the faults at both optimizations.
        for (name, source, diagnostic) in [
            (
                "address",
                "#include <stdlib.h>\nint main(void){unsigned char *allocation=malloc(1);if(allocation==NULL)return 2;volatile unsigned char *escaped=allocation;free(allocation);return *escaped;}\n",
                "ERROR: AddressSanitizer: heap-use-after-free",
            ),
            (
                "undefined",
                "#include <limits.h>\nint main(void){volatile int value=INT_MAX;volatile int sum=value+1;return sum==0;}\n",
                "runtime error: signed integer overflow",
            ),
        ] {
            for optimization in ["-O0", "-O2"] {
                let path = root.join(format!("{name}-{optimization}.c"));
                let executable = root.join(format!(
                    "{name}-{optimization}{}",
                    std::env::consts::EXE_SUFFIX
                ));
                self.write_source(&path, source);
                self.compile(&path, &executable, optimization, "calibration ");
                let output = self.command(&executable).output().unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(!output.status.success(), "{name} {optimization} did not reject");
                assert!(output.stdout.is_empty());
                assert!(
                    stderr.contains(diagnostic),
                    "{name} {optimization} lacked its sanitizer diagnostic: {stderr}"
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
#[ignore = "requires an absolute SEMAPRAX_FRAME_SANITIZER_CLANG with ASan and UBSan runtimes"]
fn isolated_and_retained_project_corpora_pass_asan_and_ubsan_at_o0_and_o2() {
    // Validate the explicit prerequisite before creating Project fixtures.
    let execution = Execution::sanitized();
    execution.calibrate();
    let selected = SELECTED.map(str::to_owned);
    let program = resolve(FRAME_SOURCE);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let isolated = semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    backend_equivalence::assert_native_corpus_with(
        isolated.source(),
        "sanitized-isolated",
        &execution,
    );

    // These are actual retained baseline/renamed Projects, not fabricated
    // revision facts. No npm publication, native SDK or toolchain CLI is used.
    let root = temporary("sanitized-projects");
    let before = root.join("before");
    let after = root.join("after");
    copy_project(&before, false);
    copy_project(&after, true);
    let before = subject_binding::retain(&before);
    let after = subject_binding::retain(&after);
    subject_binding::verify_display_rename(&before, &after);
    for (bound, label) in [
        (&before, "sanitized-baseline"),
        (&after, "sanitized-display-rename"),
    ] {
        let provider = subject_binding::native_provider(bound);
        backend_equivalence::assert_native_corpus_with(provider.source(), label, &execution);
    }
    fs::remove_dir_all(root).unwrap();
}
