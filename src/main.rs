use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use semaprax::diagnostic::{Diagnostic, Severity};
use semaprax::{
    agent_economics, codegen, format, graph, impact, parse, patch, patch_evidence, quality_route,
    repair, review, verify, wasm,
};

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
            let output = graph::to_json(&program).map_err(|errors| report(&errors, false))?;
            println!("{output}");
            Ok(())
        }
        "context" => {
            let path = required_path(&args, 1)?;
            let symbol = args.get(2).ok_or_else(|| {
                eprintln!("context requires a symbol name or stable id");
                2
            })?;
            let options = context_options(&args)?;
            let program = checked(&path)?;
            let context = match &options {
                ParsedContextOptions::V1(options) => {
                    graph::agent_context_json(&program, symbol, options)
                }
                ParsedContextOptions::V2(options) => {
                    graph::agent_context_v2_json(&program, symbol, options)
                }
            }
            .map_err(|errors| report(&errors, false))?
            .ok_or_else(|| {
                eprintln!("symbol `{symbol}` was not found");
                1
            })?;
            println!("{context}");
            Ok(())
        }
        "context-benchmark" => {
            if args.len() != 2 {
                eprintln!("context-benchmark requires exactly one manifest path");
                return Err(2);
            }
            let path = required_path(&args, 1)?;
            let output = agent_economics::benchmark_manifest(&path)
                .map_err(|error| report(&[error], false))?;
            println!("{output}");
            Ok(())
        }
        "quality-plan" => {
            let profile = args.get(1).ok_or_else(|| {
                eprintln!("quality-plan requires quick, changed, or full");
                2
            })?;
            let plan =
                quality_route::plan(Path::new("."), profile, &args[2..]).map_err(|error| {
                    eprintln!("quality-plan: {error}");
                    2
                })?;
            print!("{plan}");
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
                "native-callable" => {
                    let function = option_value(&args, "--function").ok_or_else(|| {
                        eprintln!("native-callable target requires --function <stable-id>");
                        2
                    })?;
                    let bundle = codegen::build_native_callable_bundle(&program, function, &output)
                        .map_err(|error| report(&[error], false))?;
                    println!(
                        "built native-callable bundle {} (manifest sha256:{})",
                        bundle.output_directory().display(),
                        bundle.manifest_sha256()
                    );
                }
                target => {
                    eprintln!(
                        "unsupported target `{target}`; available: native, native-callable, web"
                    );
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
        "impact" => {
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let options = impact_options(&args)?;
            let report = impact::preview(&source_path, &patch_path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "review" => {
            if args.len() != 3 {
                eprintln!("review requires exactly <file> <patch.spatch>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let report = review::preview(&source_path, &patch_path)
                .map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "patch-evidence" => {
            if args.len() != 3 {
                eprintln!("patch-evidence requires exactly <file> <patch.spatch>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence = patch_evidence::generate(&source_path, &patch_path)
                .map_err(|errors| report(&errors, false))?;
            print!("{evidence}");
            Ok(())
        }
        "verify-patch-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "verify-patch-evidence requires exactly <file> <patch.spatch> <evidence.json>"
                );
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence_path = required_path(&args, 3)?;
            let receipt = patch_evidence::verify(&source_path, &patch_path, &evidence_path)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "repairs" => {
            if args.len() != 4 || args[2] != "assign-function-id" {
                eprintln!("repairs requires <file> assign-function-id <automatic-function-id>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let query = repair::DiagnosticRepairQuery::assign_function_id(args[3].clone())
                .map_err(|error| {
                    eprintln!("{error}");
                    2
                })?;
            let report =
                repair::query(&source_path, &query).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "repair" => {
            if args.len() != 5 || args[3] != "--persistent-id" {
                eprintln!("repair requires <file> <repair-id> --persistent-id <persistent-id>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let persistent_id =
                repair::PersistentDeclarationId::new(args[4].clone()).map_err(|error| {
                    eprintln!("{error}");
                    2
                })?;
            let preview = repair::instantiate(&source_path, &args[2], &persistent_id)
                .map_err(|errors| report(&errors, false))?;
            println!("{preview}");
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

fn impact_options(args: &[String]) -> Result<impact::SemanticImpactOptions, u8> {
    let mut depth = 1usize;
    let mut max_bytes = 64 * 1024;
    let mut max_nodes = 256usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 3usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--depth" | "--max-bytes" | "--max-nodes") {
            eprintln!("unknown impact option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate impact option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("impact option `{option}` requires a value");
            2
        })?;
        let value = impact_number(option, value)?;
        match option {
            "--depth" => depth = value,
            "--max-bytes" => max_bytes = value,
            "--max-nodes" => max_nodes = value,
            _ => unreachable!("closed impact option table"),
        }
        index += 2;
    }
    impact::SemanticImpactOptions::new(depth, max_bytes, max_nodes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn impact_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("impact option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("impact option `{option}` requires a canonical nonnegative integer");
        2
    })
}

enum ParsedContextOptions {
    V1(graph::AgentContextOptions),
    V2(graph::AgentContextV2Options),
}

fn context_options(args: &[String]) -> Result<ParsedContextOptions, u8> {
    let defaults = graph::AgentContextOptions::default();
    let mut depth = defaults.depth();
    let mut max_bytes = defaults.max_bytes();
    let mut max_nodes = defaults.max_nodes();
    let mut filters = None;
    let mut direction = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 3;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--depth" | "--max-bytes" | "--max-nodes" | "--filters" | "--direction"
        ) {
            eprintln!("unknown context option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate context option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("context option `{option}` requires a value");
            2
        })?;
        match option {
            "--depth" => depth = context_number(option, value)?,
            "--max-bytes" => max_bytes = context_number(option, value)?,
            "--max-nodes" => max_nodes = context_number(option, value)?,
            "--filters" => {
                if value.is_empty() {
                    eprintln!("context --filters requires a comma-separated nonempty list");
                    return Err(2);
                }
                let mut parsed = std::collections::BTreeSet::new();
                for name in value.split(',') {
                    let Some(filter) = graph::AgentContextFilter::from_name(name) else {
                        eprintln!("unknown context filter `{name}`");
                        return Err(2);
                    };
                    if !parsed.insert(filter) {
                        eprintln!("duplicate context filter `{name}`");
                        return Err(2);
                    }
                }
                filters = Some(parsed);
            }
            "--direction" => {
                let Some(parsed) = graph::AgentContextDirection::from_name(value) else {
                    eprintln!("unknown context direction `{value}`");
                    return Err(2);
                };
                direction = Some(parsed);
            }
            _ => unreachable!("closed context option table"),
        }
        index += 2;
    }
    let filters = filters.unwrap_or_else(|| {
        [
            graph::AgentContextFilter::Contracts,
            graph::AgentContextFilter::Ownership,
            graph::AgentContextFilter::Effects,
            graph::AgentContextFilter::Types,
        ]
        .into_iter()
        .collect()
    });
    match direction {
        Some(direction) => {
            graph::AgentContextV2Options::new(depth, max_bytes, max_nodes, filters, direction)
                .map(ParsedContextOptions::V2)
                .map_err(|error| {
                    eprintln!("{error}");
                    2
                })
        }
        None => graph::AgentContextOptions::new(depth, max_bytes, max_nodes, filters)
            .map(ParsedContextOptions::V1)
            .map_err(|error| {
                eprintln!("{error}");
                2
            }),
    }
}

fn context_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("context option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("context option `{option}` requires a canonical nonnegative integer");
        2
    })
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
           semaprax context <file> <symbol|stable-id> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--filters contracts,ownership,effects,types,targets,diagnostics,tests]\n\
           semaprax context-benchmark <manifest>\n\
           semaprax quality-plan <quick|changed|full> [exact-changed-path ...]\n\
           semaprax build <file> [--target native|native-callable|web] [--function stable-id] [-o path]\n\
           semaprax run <file>\n\
           semaprax fmt <file> [--check]\n\
           semaprax patch <file> <patch.spatch>\n\
           semaprax impact <file> <patch.spatch> [--depth N] [--max-bytes N] [--max-nodes N]\n\
           semaprax review <file> <patch.spatch>\n\
           semaprax patch-evidence <file> <patch.spatch>\n\
           semaprax verify-patch-evidence <file> <patch.spatch> <evidence.json>\n\
           semaprax repairs <file> assign-function-id <automatic-function-id>\n\
           semaprax repair <file> <repair-id> --persistent-id <persistent-id>\n\
           semaprax version"
    );
}
