//! Executable stand-in for build/host tools, never a compiler correctness oracle.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const TARGET: &str = "x86_64-pc-windows-msvc";

fn record(message: &str) {
    let path = std::env::var_os("RELEASE_FIXTURE_LOG").unwrap();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{message}").unwrap();
}

fn main() {
    let executable = std::env::current_exe().unwrap();
    let name = executable
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_ascii_lowercase();
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match name.as_str() {
        "rustc.exe" => {
            assert_eq!(arguments, ["-vV"]);
            record("rustc");
            println!("host: {TARGET}");
        }
        "cargo.exe" => {
            record("cargo");
            assert_eq!(&arguments[..3], ["build", "--locked", "--release"]);
            assert!(arguments
                .windows(2)
                .any(|pair| pair == ["--target", TARGET]));
            for package in ["semaprax", "semaprax-toolchain"] {
                assert!(arguments.windows(2).any(|pair| pair == ["-p", package]));
            }
            for binary in ["semaprax-full", "semapraxd"] {
                assert!(arguments.windows(2).any(|pair| pair == ["--bin", binary]));
            }
            // Model Cargo's ambient target override so the old script's
            // hardcoded copy paths would select the stale sentinels instead.
            let target = arguments
                .windows(2)
                .find(|pair| pair[0] == "--target-dir")
                .map(|pair| PathBuf::from(&pair[1]))
                .unwrap_or_else(|| PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap()));
            assert!(target.is_absolute());
            record(&format!("target-dir:{}", target.display()));
            let release = target.join(TARGET).join("release");
            fs::create_dir_all(&release).unwrap();
            fs::copy(&executable, release.join("semaprax-full.exe")).unwrap();
            fs::copy(&executable, release.join("semapraxd.exe")).unwrap();
            fs::write(release.join("current-build-marker"), b"fresh fake build\n").unwrap();
        }
        "semaprax.exe" => {
            let expected =
                PathBuf::from(std::env::var_os("RELEASE_FIXTURE_UNPACKED_BINARY").unwrap());
            assert_eq!(
                executable.canonicalize().unwrap(),
                expected.canonicalize().unwrap()
            );
            let commit = std::env::var("SEMAPRAX_BUILD_COMMIT").unwrap();
            match arguments
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice()
            {
                ["--version"] => {
                    record("smoke:--version");
                    println!("semaprax 0.2.0 ({commit})");
                }
                ["version", "--json"] => {
                    record("smoke:version-json");
                    println!("{{\"schema\":\"semaprax.version.v1\",\"version\":\"0.2.0\",\"commit\":\"{commit}\",\"maturity\":\"pre-alpha\",\"rust_min\":\"1.88\"}}");
                }
                [operation @ ("check" | "run"), path] => {
                    assert_eq!(fs::read_to_string(path).unwrap(), "module release.smoke;\n\n@id(\"release.smoke.main\")\nfn main() -> i64 { 42 }\n");
                    record(&format!("smoke:{operation}"));
                    if *operation == "run" {
                        println!("42");
                    }
                }
                _ => panic!("unexpected unpacked binary arguments: {arguments:?}"),
            }
        }
        _ => panic!("unexpected fixture binary: {name}"),
    }
}
