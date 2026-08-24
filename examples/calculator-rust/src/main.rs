use std::path::{Path, PathBuf};

use semaprax_native_rust_interop::{
    build_native_rust_sdk, build_project_native_rust_sdk, NativeRustSdkOptions,
};

const CALCULATOR: &str = include_str!("../../calculator.spx");
const CALLBACK: &str = include_str!("../callback.spx");

fn usage() -> ! {
    eprintln!(
        "usage:\n  semaprax-calculator-rust-setup <calculator|calculator-renamed|callback> <output-directory>\n  semaprax-calculator-rust-setup project <manifest-path> <output-directory>"
    );
    std::process::exit(2);
}

fn output_path(value: Option<String>) -> PathBuf {
    let path = PathBuf::from(value.unwrap_or_else(|| usage()));
    if !path.is_absolute() {
        eprintln!("output directory must be absolute");
        std::process::exit(2);
    }
    path
}

fn build(source: &str, source_path: &Path, exports: &[&str], imports: &[&str], output: &Path) {
    let options = NativeRustSdkOptions {
        exports: exports.iter().map(|value| (*value).to_owned()).collect(),
        imports: imports.iter().map(|value| (*value).to_owned()).collect(),
        capabilities: Vec::new(),
    };
    match build_native_rust_sdk(source, source_path, options, output) {
        Ok(bundle) => println!(
            "{} {} {}",
            bundle.crate_name(),
            bundle.target_triple(),
            bundle.manifest_digest()
        ),
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}: {}", diagnostic.code, diagnostic.message);
            }
            std::process::exit(1);
        }
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let mode = arguments.next().unwrap_or_else(|| usage());
    if mode == "project" {
        let manifest = PathBuf::from(arguments.next().unwrap_or_else(|| usage()));
        let output = output_path(arguments.next());
        if arguments.next().is_some() {
            usage();
        }
        match build_project_native_rust_sdk(&manifest, &output) {
            Ok(bundle) => println!(
                "{} {} {} {} {} {}",
                bundle.sdk().crate_name(),
                bundle.sdk().target_triple(),
                bundle.sdk().manifest_digest(),
                bundle.project_revision(),
                bundle.workspace_revision(),
                bundle.subject_digest()
            ),
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    eprintln!("{}: {}", diagnostic.code, diagnostic.message);
                }
                std::process::exit(1);
            }
        }
        return;
    }
    let output = output_path(arguments.next());
    if arguments.next().is_some() {
        usage();
    }
    match mode.as_str() {
        "calculator" => build(
            CALCULATOR,
            Path::new("examples/calculator.spx"),
            &[
                "calculator.add",
                "calculator.subtract",
                "calculator.multiply",
                "calculator.divide",
                "calculator.is-negative",
                "calculator.not",
            ],
            &[],
            &output,
        ),
        "calculator-renamed" => {
            let renamed = CALCULATOR.replacen("fn add(", "fn sum(", 1).replacen(
                "    add(19, 23)",
                "    sum(19, 23)",
                1,
            );
            build(
                &renamed,
                Path::new("examples/calculator-renamed.spx"),
                &[
                    "calculator.add",
                    "calculator.subtract",
                    "calculator.multiply",
                    "calculator.divide",
                    "calculator.is-negative",
                    "calculator.not",
                ],
                &[],
                &output,
            )
        }
        "callback" => build(
            CALLBACK,
            Path::new("examples/calculator-rust/callback.spx"),
            &["calculator.callback.apply"],
            &["calculator.callback.adjust"],
            &output,
        ),
        _ => usage(),
    }
}
