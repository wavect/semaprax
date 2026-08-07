use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use semaprax::diagnostic::{Diagnostic, Severity};
use semaprax::{codegen, format, graph, parse, patch, verify, wasm};

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run(args: Vec<String>) -> Result<(), u8> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Err(2);
    };
    match command {
        "check" => {
            let path = required_path(&args, 1)?;
            let json = args.iter().any(|arg| arg == "--json");
            let program = load(&path).map_err(|errors| report(&errors, json))?;
            let diagnostics = verify::verify(&program);
            let failed = diagnostics
                .iter()
                .any(|item| item.severity == Severity::Error);
            report_all(&diagnostics, json);
            if failed {
                Err(1)
            } else {
                if !json {
                    println!(
                        "verified {} ({})",
                        path.display(),
                        graph::revision(&program)
                    );
                }
                Ok(())
            }
        }
        "graph" => {
            let path = required_path(&args, 1)?;
            let program = checked(&path)?;
            println!("{}", graph::to_json(&program));
            Ok(())
        }
        "context" => {
            let path = required_path(&args, 1)?;
            let symbol = args.get(2).ok_or_else(|| {
                eprintln!("context requires a symbol name or stable id");
                2
            })?;
            let depth = args
                .windows(2)
                .find(|pair| pair[0] == "--depth")
                .and_then(|pair| pair[1].parse().ok())
                .unwrap_or(1);
            let program = checked(&path)?;
            let context = graph::context_json(&program, symbol, depth).ok_or_else(|| {
                eprintln!("symbol `{symbol}` was not found");
                1
            })?;
            println!("{context}");
            Ok(())
        }
        "build" => {
            let path = required_path(&args, 1)?;
            let output = output_path(&args, &path);
            let program = checked(&path)?;
            match option_value(&args, "--target").unwrap_or("native") {
                "native" => {
                    codegen::build(&program, &output).map_err(|error| report(&[error], false))?;
                    println!("built native executable {}", output.display());
                }
                "web" | "wasm" => {
                    wasm::build_web(&program, &output).map_err(|error| report(&[error], false))?;
                    println!("built web package {}", output.display());
                }
                target => {
                    eprintln!("unsupported target `{target}`; available: native, web");
                    return Err(2);
                }
            }
            Ok(())
        }
        "run" => {
            let path = required_path(&args, 1)?;
            let output = std::env::temp_dir().join(format!("semaprax-run-{}", std::process::id()));
            let program = checked(&path)?;
            codegen::build(&program, &output).map_err(|error| report(&[error], false))?;
            let status = Command::new(&output).status().map_err(|error| {
                eprintln!("cannot run {}: {error}", output.display());
                1
            })?;
            let _ = std::fs::remove_file(&output);
            if status.success() {
                Ok(())
            } else {
                Err(status.code().unwrap_or(1) as u8)
            }
        }
        "fmt" => {
            let path = required_path(&args, 1)?;
            let check_only = args.iter().any(|arg| arg == "--check");
            let source = std::fs::read_to_string(&path).map_err(|error| {
                eprintln!("cannot read {}: {error}", path.display());
                1
            })?;
            let program = parse(&source, &path).map_err(|error| report(&[error], false))?;
            let canonical = format::canonical(&program);
            if check_only && source != canonical {
                eprintln!("{} is not canonically formatted", path.display());
                Err(1)
            } else if check_only {
                Ok(())
            } else {
                std::fs::write(&path, canonical).map_err(|error| {
                    eprintln!("cannot write {}: {error}", path.display());
                    1
                })
            }
        }
        "patch" => {
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let revision =
                patch::apply(&source_path, &patch_path).map_err(|errors| report(&errors, false))?;
            println!("applied semantic patch; graph is now {revision}");
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("semaprax {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => {
            eprintln!("unknown command `{other}`\n");
            print_help();
            Err(2)
        }
    }
}

fn checked(path: &Path) -> Result<semaprax::ast::Program, u8> {
    let program = load(path).map_err(|errors| report(&errors, false))?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        Err(report(&diagnostics, false))
    } else {
        Ok(program)
    }
}

fn load(path: &Path) -> Result<semaprax::ast::Program, Vec<Diagnostic>> {
    let source = std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    parse(&source, path).map_err(|error| vec![error])
}

fn required_path(args: &[String], index: usize) -> Result<PathBuf, u8> {
    args.get(index).map(PathBuf::from).ok_or_else(|| {
        eprintln!("missing file path; run `semaprax help` for usage");
        2
    })
}

fn output_path(args: &[String], input: &Path) -> PathBuf {
    args.windows(2)
        .find(|pair| pair[0] == "-o" || pair[0] == "--output")
        .map(|pair| PathBuf::from(&pair[1]))
        .unwrap_or_else(|| input.with_extension("out"))
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == option)
        .map(|pair| pair[1].as_str())
}

fn report(errors: &[Diagnostic], json: bool) -> u8 {
    report_all(errors, json);
    1
}

fn report_all(errors: &[Diagnostic], json: bool) {
    for error in errors {
        if json {
            println!("{}", error.json());
        } else {
            eprintln!("{error}");
        }
    }
}

fn print_help() {
    println!(
        "SEMAPRAX — Meaning in. Verified machine code out.\n\n\
         Usage:\n\
           semaprax check <file> [--json]\n\
           semaprax graph <file>\n\
           semaprax context <file> <symbol|stable-id> [--depth N]\n\
           semaprax build <file> [--target native|web] [-o path]\n\
           semaprax run <file>\n\
           semaprax fmt <file> [--check]\n\
           semaprax patch <file> <patch.spatch>\n\
           semaprax version"
    );
}
