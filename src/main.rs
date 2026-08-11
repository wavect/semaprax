use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use semaprax::diagnostic::{Diagnostic, Severity};
use semaprax::{
    agent_economics, codegen, format, graph, impact, parse, patch, patch_evidence, quality_route,
    repair, review, semantic_workspace, semantic_workspace_change, target_evidence, verify, wasm,
    workspace, workspace_analysis, workspace_graph, workspace_patch_evidence,
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
        "workspace-init" => {
            if args.len() != 3 {
                eprintln!("workspace-init requires exactly <root> <path-set.json>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let path_set = required_path(&args, 2)?;
            let revision =
                workspace::initialize(&root, &path_set).map_err(|errors| report(&errors, false))?;
            println!("initialized semantic workspace; workspace is {revision}");
            Ok(())
        }
        "semantic-workspace-init" => {
            if args.len() != 3 {
                eprintln!("semantic-workspace-init requires exactly <root> <path-set.json>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let path_set = required_path(&args, 2)?;
            let revision = semantic_workspace::initialize(&root, &path_set)
                .map_err(|errors| report(&errors, false))?;
            println!("initialized semantic graph workspace; workspace is {revision}");
            Ok(())
        }
        "semantic-workspace-change-preview" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-change-preview requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_change::preview(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "semantic-workspace-change-evidence" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-change-evidence requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_change::evidence(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "verify-semantic-workspace-change-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "verify-semantic-workspace-change-evidence requires exactly <root> <proposal.json> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let evidence = required_path(&args, 3)?;
            let receipt = semantic_workspace_change::verify(&root, &proposal, &evidence)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "workspace-snapshot" => {
            if args.len() != 2 {
                eprintln!("workspace-snapshot requires exactly <root>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let snapshot = workspace::snapshot(&root).map_err(|errors| report(&errors, false))?;
            println!("{}", snapshot.to_json());
            Ok(())
        }
        "workspace-graph" => {
            if args.len() != 3 {
                eprintln!("workspace-graph requires exactly <root> <entry-module>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let entry_module = &args[2];
            let graph = workspace_graph::snapshot(&root, entry_module)
                .map_err(|errors| report(&errors, false))?;
            println!("{}", graph.to_json());
            Ok(())
        }
        "workspace-context" => {
            if args.len() < 5 {
                eprintln!("workspace-context requires <root> <entry-module> <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N]");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let entry_module = &args[2];
            let target_kind = workspace_analysis_target_kind("workspace-context", &args[3])?;
            let options = workspace_context_options(&args)?;
            let output =
                workspace_analysis::context(&root, entry_module, target_kind, &args[4], options)
                    .map_err(|errors| report(&errors, false))?;
            println!("{output}");
            Ok(())
        }
        "workspace-impact" => {
            if args.len() < 5 {
                eprintln!("workspace-impact requires <root> <entry-module> <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N]");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let entry_module = &args[2];
            let target_kind = workspace_analysis_target_kind("workspace-impact", &args[3])?;
            let options = workspace_impact_options(&args)?;
            let output =
                workspace_analysis::impact(&root, entry_module, target_kind, &args[4], options)
                    .map_err(|errors| report(&errors, false))?;
            println!("{output}");
            Ok(())
        }
        "workspace-review" => {
            if args.len() != 5 {
                eprintln!("workspace-review requires exactly <root> <entry-module> <declaration|capability> <target>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let entry_module = &args[2];
            let target_kind = workspace_analysis_target_kind("workspace-review", &args[3])?;
            let output = workspace_analysis::review(&root, entry_module, target_kind, &args[4])
                .map_err(|errors| report(&errors, false))?;
            println!("{output}");
            Ok(())
        }
        "workspace-preview" => {
            if args.len() != 3 {
                eprintln!("workspace-preview requires exactly <root> <patch.wspatch>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let preview =
                workspace::preview(&root, &patch_path).map_err(|errors| report(&errors, false))?;
            println!("{preview}");
            Ok(())
        }
        "workspace-apply" => {
            if args.len() != 3 {
                eprintln!("workspace-apply requires exactly <root> <patch.wspatch>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let revision =
                workspace::apply(&root, &patch_path).map_err(|errors| report(&errors, false))?;
            println!("applied semantic workspace transaction; workspace is now {revision}");
            Ok(())
        }
        "workspace-patch-evidence" => {
            if args.len() != 3 {
                eprintln!("workspace-patch-evidence requires exactly <root> <patch.wspatch>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence = workspace_patch_evidence::generate(&root, &patch_path)
                .map_err(|errors| report(&errors, false))?;
            print!("{evidence}");
            Ok(())
        }
        "verify-workspace-patch-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "verify-workspace-patch-evidence requires exactly <root> <patch.wspatch> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence_path = required_path(&args, 3)?;
            let receipt = workspace_patch_evidence::verify(&root, &patch_path, &evidence_path)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "workspace-apply-with-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "workspace-apply-with-evidence requires exactly <root> <patch.wspatch> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence_path = required_path(&args, 3)?;
            let revision = workspace_patch_evidence::apply(&root, &patch_path, &evidence_path)
                .map_err(|errors| report(&errors, false))?;
            println!(
                "applied semantic workspace transaction with exact evidence replay; workspace is now {revision}"
            );
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
        "target-evidence" => {
            if args.len() != 3 {
                eprintln!("target-evidence requires exactly <file> <patch.spatch>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let report = target_evidence::preview(&source_path, &patch_path)
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
        "patch-evidence-v2" => {
            if args.len() != 3 {
                eprintln!("patch-evidence-v2 requires exactly <file> <patch.spatch>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence = patch_evidence::generate_v2(&source_path, &patch_path)
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
        "verify-patch-evidence-v2" => {
            if args.len() != 4 {
                eprintln!(
                    "verify-patch-evidence-v2 requires exactly <file> <patch.spatch> <evidence.json>"
                );
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence_path = required_path(&args, 3)?;
            let receipt = patch_evidence::verify_v2(&source_path, &patch_path, &evidence_path)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "patch-with-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "patch-with-evidence requires exactly <file> <patch.spatch> <evidence.json>"
                );
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence_path = required_path(&args, 3)?;
            let revision = patch_evidence::apply(&source_path, &patch_path, &evidence_path)
                .map_err(|errors| report(&errors, false))?;
            println!("applied semantic patch with exact evidence replay; graph is now {revision}");
            Ok(())
        }
        "patch-with-evidence-v2" => {
            if args.len() != 4 {
                eprintln!(
                    "patch-with-evidence-v2 requires exactly <file> <patch.spatch> <evidence.json>"
                );
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let evidence_path = required_path(&args, 3)?;
            let revision = patch_evidence::apply_v2(&source_path, &patch_path, &evidence_path)
                .map_err(|errors| report(&errors, false))?;
            println!("applied semantic patch with exact evidence replay; graph is now {revision}");
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

fn workspace_analysis_target_kind(
    command: &str,
    value: &str,
) -> Result<workspace_analysis::WorkspaceAnalysisTargetKind, u8> {
    match value {
        "declaration" => Ok(workspace_analysis::WorkspaceAnalysisTargetKind::Declaration),
        "capability" => Ok(workspace_analysis::WorkspaceAnalysisTargetKind::Capability),
        _ => {
            eprintln!("{command} target kind must be `declaration` or `capability`");
            Err(2)
        }
    }
}

fn workspace_context_options(
    args: &[String],
) -> Result<workspace_analysis::WorkspaceContextOptions, u8> {
    let mut direction = workspace_analysis::WorkspaceAnalysisDirection::Both;
    let mut depth = 4usize;
    let mut max_bytes = 1024 * 1024usize;
    let mut max_nodes = 1024usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 5usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--direction" | "--depth" | "--max-bytes" | "--max-nodes"
        ) {
            eprintln!("unknown workspace-context option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate workspace-context option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("workspace-context option `{option}` requires a value");
            2
        })?;
        match option {
            "--direction" => {
                direction = match value.as_str() {
                    "forward" => workspace_analysis::WorkspaceAnalysisDirection::Forward,
                    "reverse" => workspace_analysis::WorkspaceAnalysisDirection::Reverse,
                    "both" => workspace_analysis::WorkspaceAnalysisDirection::Both,
                    _ => {
                        eprintln!("unknown workspace-context direction `{value}`");
                        return Err(2);
                    }
                };
            }
            "--depth" => depth = workspace_analysis_number("workspace-context", option, value)?,
            "--max-bytes" => {
                max_bytes = workspace_analysis_number("workspace-context", option, value)?;
            }
            "--max-nodes" => {
                max_nodes = workspace_analysis_number("workspace-context", option, value)?;
            }
            _ => unreachable!("closed workspace-context option table"),
        }
        index += 2;
    }
    workspace_analysis::WorkspaceContextOptions::new(direction, depth, max_bytes, max_nodes)
        .map_err(|error| {
            eprintln!("{error}");
            2
        })
}

fn workspace_impact_options(
    args: &[String],
) -> Result<workspace_analysis::WorkspaceImpactOptions, u8> {
    let mut depth = 16usize;
    let mut max_bytes = 1024 * 1024usize;
    let mut max_nodes = 1024usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 5usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--depth" | "--max-bytes" | "--max-nodes") {
            eprintln!("unknown workspace-impact option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate workspace-impact option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("workspace-impact option `{option}` requires a value");
            2
        })?;
        let value = workspace_analysis_number("workspace-impact", option, value)?;
        match option {
            "--depth" => depth = value,
            "--max-bytes" => max_bytes = value,
            "--max-nodes" => max_nodes = value,
            _ => unreachable!("closed workspace-impact option table"),
        }
        index += 2;
    }
    workspace_analysis::WorkspaceImpactOptions::new(depth, max_bytes, max_nodes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn workspace_analysis_number(command: &str, option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("{command} option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("{command} option `{option}` requires a canonical nonnegative integer");
        2
    })
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
           semaprax workspace-init <root> <path-set.json>\n\
           semaprax semantic-workspace-init <root> <path-set.json>\n\
           semaprax semantic-workspace-change-preview <root> <proposal.json>\n\
           semaprax semantic-workspace-change-evidence <root> <proposal.json>\n\
           semaprax verify-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>\n\
           semaprax workspace-snapshot <root>\n\
           semaprax workspace-graph <root> <entry-module>\n\
           semaprax workspace-context <root> <entry-module> <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N]\n\
           semaprax workspace-impact <root> <entry-module> <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N]\n\
           semaprax workspace-review <root> <entry-module> <declaration|capability> <target>\n\
           semaprax workspace-preview <root> <patch.wspatch>\n\
           semaprax workspace-apply <root> <patch.wspatch>\n\
           semaprax workspace-patch-evidence <root> <patch.wspatch>\n\
           semaprax verify-workspace-patch-evidence <root> <patch.wspatch> <evidence.json>\n\
           semaprax workspace-apply-with-evidence <root> <patch.wspatch> <evidence.json>\n\
           semaprax impact <file> <patch.spatch> [--depth N] [--max-bytes N] [--max-nodes N]\n\
           semaprax review <file> <patch.spatch>\n\
           semaprax target-evidence <file> <patch.spatch>\n\
           semaprax patch-evidence <file> <patch.spatch>\n\
           semaprax patch-evidence-v2 <file> <patch.spatch>\n\
           semaprax verify-patch-evidence <file> <patch.spatch> <evidence.json>\n\
           semaprax verify-patch-evidence-v2 <file> <patch.spatch> <evidence.json>\n\
           semaprax patch-with-evidence <file> <patch.spatch> <evidence.json>\n\
           semaprax patch-with-evidence-v2 <file> <patch.spatch> <evidence.json>\n\
           semaprax repairs <file> assign-function-id <automatic-function-id>\n\
           semaprax repair <file> <repair-id> --persistent-id <persistent-id>\n\
           semaprax version"
    );
}
