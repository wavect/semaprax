#![allow(
    clippy::result_large_err,
    reason = "the CLI preserves structured Diagnostic values across command boundaries"
)]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use semaprax::diagnostic::{Diagnostic, Severity};
use semaprax::{
    abi_report, agent_economics, agent_transport, c_header, capability_manifest, codegen, cxx_shim,
    format, freestanding_object, graph, hygienic, impact, interpreter, openapi, package_report,
    parse, patch, patch_evidence, plugin_manifest, project, properties, protocol_check,
    quality_route, region_report, repair, review, semantic_workspace, semantic_workspace_change,
    semantic_workspace_operations, semantic_workspace_structural_change, simd_report,
    target_evidence, ui_schema, verify, wasm, workspace, workspace_analysis, workspace_graph,
    workspace_patch_evidence,
};

#[path = "cli/mod.rs"]
mod cli;
#[path = "native_scratch.rs"]
mod native_scratch;

#[cfg(test)]
#[path = "cli/native_scratch_tests.rs"]
mod native_scratch_tests;

/// Explicit private-host hooks supplied only by the unpublished toolchain.
pub type DoctorHook = fn(&[String]) -> Result<(String, u8), String>;
pub type NewProjectHook = fn(&[String]) -> Result<PathBuf, (String, u8)>;

pub struct PrivateHost {
    pub doctor: DoctorHook,
    pub new_project: NewProjectHook,
    pub build_rust: fn(&mut project::ProjectSnapshot, &Path) -> Result<(), Vec<Diagnostic>>,
    #[cfg(windows)]
    pub build_owned_npm: fn(&mut project::ProjectSnapshot, &Path) -> Result<(), Vec<Diagnostic>>,
}

// Windows reserves a smaller main-thread stack than the admitted compiler
// depth requires. One explicit worker keeps the CLI's stack budget identical
// across targets without changing the library entry points.
const CLI_STACK_BYTES: usize = 16 * 1024 * 1024;

pub fn main_with_host(host: Option<&'static PrivateHost>) -> ExitCode {
    let args = std::env::args().skip(1).collect();
    let worker = match std::thread::Builder::new()
        .name("semaprax-cli".to_owned())
        .stack_size(CLI_STACK_BYTES)
        .spawn(move || run(args, host))
    {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("fatal: unable to start the bounded compiler worker: {error}");
            return ExitCode::from(2);
        }
    };
    let outcome = match worker.join() {
        Ok(outcome) => outcome,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn require_private_host<'a>(
    host: Option<&'a PrivateHost>,
    operation: &str,
) -> Result<&'a PrivateHost, u8> {
    host.ok_or_else(|| {
        eprintln!("{operation} is unavailable in the standalone crates.io package; use the unpublished semaprax-full toolchain CLI");
        2
    })
}

fn run(args: Vec<String>, host: Option<&PrivateHost>) -> Result<(), u8> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Err(2);
    };
    match command {
        "check" => {
            let options = cli::project::parse_check_options(&args[1..])?;
            let json = options.json;
            let path = match options.input {
                cli::project::CheckInput::Source(path) => path,
                cli::project::CheckInput::Project(manifest_path) => {
                    let (name, revision) =
                        project::with_authenticated_project(&manifest_path, |snapshot| {
                            snapshot.check()?;
                            Ok((
                                snapshot.manifest().name().to_owned(),
                                snapshot.project_revision().to_owned(),
                            ))
                        })
                        .map_err(|errors| report(&errors, json))?;
                    if !json {
                        println!("verified project {name} ({revision})");
                    }
                    return Ok(());
                }
            };
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
        "project-candidate-git-publish" => {
            if args.len() != 5
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                eprintln!("project-candidate-git-publish requires exactly <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>");
                return Err(2);
            }
            let output = cli::candidate_git::publish(
                Path::new(&args[1]),
                Path::new(&args[2]),
                &args[3],
                Path::new(&args[4]),
            )
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "semantic-cache-init" | "semantic-cache-persist" | "semantic-cache-load" => {
            let arity = if command == "semantic-cache-init" {
                2
            } else {
                3
            };
            if args.len() != arity
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                eprintln!("{command} requires its exact positional operands; see --help");
                return Err(2);
            }
            let output = match command {
                "semantic-cache-init" => cli::semantic_cache::initialize(Path::new(&args[1])),
                "semantic-cache-persist" => {
                    cli::semantic_cache::persist(Path::new(&args[1]), Path::new(&args[2]))
                }
                _ => cli::semantic_cache::load(Path::new(&args[1]), &args[2]),
            }
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "project-candidate-persist" | "project-candidate-load" => {
            if args.len() != 4
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                let operands = if command == "project-candidate-persist" {
                    "<manifest> <capsule.json> <store-root>"
                } else {
                    "<store-root> <archive-digest> <candidate-digest>"
                };
                eprintln!("{command} requires exactly {operands}");
                return Err(2);
            }
            let output = if command == "project-candidate-persist" {
                cli::candidate_archive::persist(
                    Path::new(&args[1]),
                    Path::new(&args[2]),
                    Path::new(&args[3]),
                )
            } else {
                cli::candidate_archive::load(Path::new(&args[1]), &args[2], &args[3])
            }
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "project-image"
        | "project-image-store"
        | "project-image-load"
        | "project-image-verify"
        | "project-symbol"
        | "project-candidate-preview"
        | "project-candidate-export"
        | "project-candidate-restore" => {
            let arity = match command {
                "project-image" => 2,
                "project-image-load" => 4,
                _ => 3,
            };
            if args.len() != arity
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                let operands = match command {
                    "project-image" => "<manifest>",
                    "project-image-store" => "<manifest> <store-root>",
                    "project-image-load" => "<store-root> <receipt.json> <expected-image-digest>",
                    "project-image-verify" => "<manifest> <image.json>",
                    "project-candidate-preview" | "project-candidate-export" => {
                        "<manifest> <change.json>"
                    }
                    "project-candidate-restore" => "<manifest> <capsule.json>",
                    _ => "<manifest> <stable-id>",
                };
                eprintln!("{command} requires exactly {operands}");
                return Err(2);
            }
            let manifest = Path::new(&args[1]);
            let output = match command {
                "project-image" => cli::project_image::derive(manifest),
                "project-image-store" => cli::project_image::persist(manifest, Path::new(&args[2])),
                "project-image-load" => {
                    cli::project_image::load(manifest, Path::new(&args[2]), &args[3])
                }
                "project-image-verify" => cli::project_image::verify(manifest, Path::new(&args[2])),
                "project-candidate-preview" => {
                    cli::project_candidate::preview(manifest, Path::new(&args[2]))
                }
                "project-candidate-export" => {
                    cli::project_candidate::export(manifest, Path::new(&args[2]))
                }
                "project-candidate-restore" => {
                    cli::project_candidate::restore(manifest, Path::new(&args[2]))
                }
                _ => cli::project_image::symbol(manifest, &args[2]),
            }
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
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
        "serve-workspace" => {
            if args.len() != 3
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                eprintln!("serve-workspace requires exactly <manifest> <host-policy.json>");
                return Err(2);
            }
            cli::workspace_session::run(Path::new(&args[1]), Path::new(&args[2]))
                .map_err(|errors| report(&errors, false))
        }
        "serve-image"
        | "serve-candidates"
        | "serve-test-candidates"
        | "serve-diagnostics"
        | "serve-diagnostics-tested" => {
            if args.len() != 2 || args[1].is_empty() || args[1].starts_with('-') {
                eprintln!("{command} requires exactly <manifest>");
                return Err(2);
            }
            semaprax::image_transport::serve(
                std::io::stdin().lock(),
                std::io::stdout().lock(),
                Path::new(&args[1]),
                if command == "serve-diagnostics-tested" {
                    semaprax::image_transport::ImageHostCapability::DiagnosticTests
                } else if command == "serve-diagnostics" {
                    semaprax::image_transport::ImageHostCapability::CandidateDiagnostics
                } else if command == "serve-test-candidates" {
                    semaprax::image_transport::ImageHostCapability::TestEnabled
                } else if command == "serve-candidates" {
                    semaprax::image_transport::ImageHostCapability::CandidateOnly
                } else {
                    semaprax::image_transport::ImageHostCapability::ReadOnly
                },
            )
            .map_err(|error| {
                eprintln!("{error}");
                1
            })
        }
        "serve" => {
            let path = required_path(&args, 1)?;
            let limits = serve_options(&args)?;
            let outcome = agent_transport::serve(
                &mut std::io::stdin().lock(),
                &mut std::io::stdout().lock(),
                &path,
                limits,
            )
            .map_err(|errors| report(&errors, false))?;
            if outcome.stopped_by_shutdown {
                println!("agent transport session stopped by shutdown");
            }
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
        "doctor" => {
            let (output, exit_code) = (require_private_host(host, "doctor")?.doctor)(&args[1..])
                .map_err(|error| {
                    eprintln!("doctor: {error}");
                    2
                })?;
            print!("{output}");
            if exit_code == 0 {
                Ok(())
            } else {
                Err(exit_code)
            }
        }
        "new" => {
            let destination = (require_private_host(host, "new")?.new_project)(&args[1..])
                .map_err(|(error, code)| {
                    eprintln!("new: {error}");
                    code
                })?;
            println!("created calculator project {}", destination.display());
            Ok(())
        }
        "build" => {
            let options = cli::build::parse(&args[1..])?;
            if options.target == "rust" {
                require_private_host(host, "build --target rust")?;
            }
            match &options.input {
                cli::build::BuildInput::Source(input) => build_source(&options, input)?,
                cli::build::BuildInput::Project(manifest_path) => {
                    let output = project::with_authenticated_project(manifest_path, |snapshot| {
                        snapshot.check()?;
                        let mut output = options.output.clone().unwrap_or_else(|| {
                            let suffix = match options.target.as_str() {
                                "web" | "wasm" => "web".to_owned(),
                                "npm" => "npm".to_owned(),
                                "rust" => "rust".to_owned(),
                                _ => format!("out{}", std::env::consts::EXE_SUFFIX),
                            };
                            snapshot
                                .root()
                                .join(format!("{}-{suffix}", snapshot.manifest().name()))
                        });
                        if options.target == "native" {
                            output = with_native_executable_suffix(output);
                        }
                        if options.target == "rust" {
                            output = cli::build::absolute_rust_output(&output)
                                .map_err(|error| vec![error])?;
                            if !snapshot.manifest().is_v8()
                                && !snapshot.manifest().is_v9()
                                && !snapshot.manifest().is_v10()
                            {
                                return Err(vec![Diagnostic::io(
                                    "SPX-J114",
                                    "the rust target requires the exact Project v8 owned-data-api.v1, Project v9 flat-owned-record-api.v1, or Project v10 owned-utf8-api.v1 profile",
                                )]);
                            }
                        }
                        // The private Windows publisher owns every filesystem effect,
                        // including parent admission. Do not pass through the legacy
                        // pathname-based parent creation/cleanup helper on this route.
                        #[cfg(windows)]
                        if matches!(options.target.as_str(), "web" | "wasm" | "npm")
                            && (snapshot.manifest().is_v8()
                                || snapshot.manifest().is_v9()
                                || snapshot.manifest().is_v10())
                        {
                            let host = host.ok_or_else(|| {
                                vec![Diagnostic::io(
                                    "SPX-W120",
                                    "Project v8-v10 npm publication requires semaprax-full with safe handle-relative Windows authority",
                                )]
                            })?;
                            (host.build_owned_npm)(snapshot, &output)?;
                            return Ok((output, snapshot.manifest().project_profile()));
                        }
                        let mut output_parent = options
                            .output
                            .as_ref()
                            .map(|_| cli::build::ProjectOutputParent::prepare(&output))
                            .transpose()
                            .map_err(|error| vec![error])?;
                        match options.target.as_str() {
                            "web" | "wasm" => snapshot.build_web(&output)?,
                            "npm" => snapshot.build_npm(&output)?,
                            "native" => snapshot.build_native(&output)?,
                            "rust" => (host.expect("private target admitted above").build_rust)(snapshot, &output)?,
                            _ => unreachable!("validated project target"),
                        }
                        if let Some(parent) = &mut output_parent {
                            parent.retain().map_err(|error| vec![error])?;
                        }
                        Ok((output, snapshot.manifest().project_profile()))
                    })
                    .map_err(|errors| report(&errors, false))?;
                    println!(
                        "{}",
                        project_build_success(&options.target, output.1, &output.0)
                    );
                }
            }
            Ok(())
        }
        "run" => {
            let options = cli::execution::parse_run(&args[1..])?;
            match &options.input {
                cli::execution::ExecutionInput::Source(path) => run_legacy_source(path),
                cli::execution::ExecutionInput::Project(manifest_path) => {
                    project_execution_held("run", manifest_path, &options)
                }
            }
        }
        "test" => {
            let options = cli::execution::parse_test(&args[1..])?;
            let cli::execution::ExecutionInput::Project(manifest_path) = &options.input else {
                unreachable!("project test parser rejects source inputs")
            };
            project_execution_held("test", manifest_path, &options)
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
        "apply-semantic-workspace-change-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "apply-semantic-workspace-change-evidence requires exactly <root> <proposal.json> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let evidence = required_path(&args, 3)?;
            let receipt = semantic_workspace_change::apply(&root, &proposal, &evidence)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "semantic-workspace-structural-change-preview" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-structural-change-preview requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_structural_change::preview(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "semantic-workspace-structural-change-evidence" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-structural-change-evidence requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_structural_change::evidence(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "verify-semantic-workspace-structural-change-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "verify-semantic-workspace-structural-change-evidence requires exactly <root> <proposal.json> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let evidence = required_path(&args, 3)?;
            let receipt = semantic_workspace_structural_change::verify(&root, &proposal, &evidence)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "apply-semantic-workspace-structural-change-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "apply-semantic-workspace-structural-change-evidence requires exactly <root> <proposal.json> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let evidence = required_path(&args, 3)?;
            let receipt = semantic_workspace_structural_change::apply(&root, &proposal, &evidence)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        "semantic-workspace-operations-derive" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-operations-derive requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_operations::derivation(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "semantic-workspace-operations-change-proposal" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-operations-change-proposal requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_operations::derived_change_proposal(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "semantic-workspace-operations-evidence" => {
            if args.len() != 3 {
                eprintln!(
                    "semantic-workspace-operations-evidence requires exactly <root> <proposal.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let output = semantic_workspace_operations::evidence(&root, &proposal)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "verify-semantic-workspace-operations-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "verify-semantic-workspace-operations-evidence requires exactly <root> <proposal.json> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let evidence = required_path(&args, 3)?;
            let output = semantic_workspace_operations::verify(&root, &proposal, &evidence)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        "apply-semantic-workspace-operations-evidence" => {
            if args.len() != 4 {
                eprintln!(
                    "apply-semantic-workspace-operations-evidence requires exactly <root> <proposal.json> <evidence.json>"
                );
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let proposal = required_path(&args, 2)?;
            let evidence = required_path(&args, 3)?;
            let output = semantic_workspace_operations::apply(&root, &proposal, &evidence)
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
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
        "properties" => {
            let path = required_path(&args, 1)?;
            let options = property_options(&args)?;
            let report =
                properties::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "hygienic-gen" => {
            let path = required_path(&args, 1)?;
            let options = hygienic_options(&args)?;
            let report =
                hygienic::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "openapi" => {
            let path = required_path(&args, 1)?;
            let (functions, options) = openapi_options(&args)?;
            let report = openapi::generate(&path, &functions, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "openapi-compat" => {
            if args.len() < 3 {
                eprintln!(
                    "openapi-compat requires exactly <base.json> <candidate.json> [--max-bytes N]"
                );
                return Err(2);
            }
            let base = required_path(&args, 1)?;
            let candidate = required_path(&args, 2)?;
            let options = openapi_compat_options(&args)?;
            let report = openapi::compatibility(&base, &candidate, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "abi-report" => {
            let path = required_path(&args, 1)?;
            let options = abi_report_options(&args)?;
            let report =
                abi_report::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        "c-header" => {
            let path = required_path(&args, 1)?;
            let (options, emit_header) = c_header_options(&args)?;
            if emit_header {
                let header = c_header::header_text(&path, &options)
                    .map_err(|errors| report(&errors, false))?;
                print!("{header}");
            } else {
                let envelope =
                    c_header::generate(&path, &options).map_err(|errors| report(&errors, false))?;
                println!("{envelope}");
            }
            Ok(())
        }
        "freestanding-object" => {
            let path = required_path(&args, 1)?;
            let options = freestanding_object_options(&args)?;
            let envelope = freestanding_object::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "capability-manifest" => {
            let path = required_path(&args, 1)?;
            let options = capability_manifest_options(&args)?;
            let envelope = capability_manifest::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "package-report" => {
            let path = required_path(&args, 1)?;
            let options = package_report_options(&args)?;
            let envelope = package_report::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "package-lock" => match cli::package_lock::run(&args[1..]) {
            Ok(lock) => {
                println!("{lock}");
                Ok(())
            }
            Err(cli::package_lock::PackageLockCliError::Usage(message)) => {
                eprintln!("{message}");
                Err(2)
            }
            Err(cli::package_lock::PackageLockCliError::Domain(errors)) => {
                Err(report(&errors, false))
            }
        },
        "package-resolve" => match cli::package_resolver::run(&args[1..]) {
            Ok(evidence) => {
                write_package_resolver_stdout(&evidence).map_err(|error| report(&[error], false))
            }
            Err(cli::package_resolver::PackageResolverCliError::Usage(message)) => {
                eprintln!("{message}");
                Err(2)
            }
            Err(cli::package_resolver::PackageResolverCliError::Domain(errors)) => {
                Err(report(&errors, false))
            }
        },
        "region-report" => {
            let path = required_path(&args, 1)?;
            let options = region_report_options(&args)?;
            let envelope = region_report::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "simd-report" => {
            let path = required_path(&args, 1)?;
            let options = simd_report_options(&args)?;
            let envelope =
                simd_report::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "protocol-check" => {
            let path = required_path(&args, 1)?;
            let options = protocol_check_options(&args)?;
            let envelope = protocol_check::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "interpret" | "interpret-strings" => {
            let path = required_path(&args, 1)?;
            let (function, arguments, options) = interpret_options(&args)?;
            let interpret = if args[0] == "interpret-strings" {
                interpreter::internal_strings::interpret
            } else {
                interpreter::interpret
            };
            let interpretation = interpret(&path, &function, &arguments, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{}", interpretation.envelope);
            if interpretation.returned {
                Ok(())
            } else {
                Err(1)
            }
        }
        "plugin-manifest" => {
            let path = required_path(&args, 1)?;
            let options = plugin_manifest_options(&args)?;
            let envelope = plugin_manifest::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "ui-schema" => {
            let path = required_path(&args, 1)?;
            let options = ui_schema_options(&args)?;
            let envelope =
                ui_schema::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        "cxx-shim" => {
            let path = required_path(&args, 1)?;
            let (options, emit_fragment) = cxx_shim_options(&args)?;
            if emit_fragment {
                let fragment = cxx_shim::fragment_text(&path, &options)
                    .map_err(|errors| report(&errors, false))?;
                print!("{fragment}");
            } else {
                let envelope =
                    cxx_shim::generate(&path, &options).map_err(|errors| report(&errors, false))?;
                println!("{envelope}");
            }
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
        "version" => {
            let output = cli::version::render(cli::version::Invocation::Command, &args[1..])
                .map_err(|error| {
                    eprintln!("{error}");
                    2
                })?;
            print!("{output}");
            Ok(())
        }
        "--version" | "-V" => {
            let output = cli::version::render(cli::version::Invocation::Flag, &args[1..]).map_err(
                |error| {
                    eprintln!("{error}");
                    2
                },
            )?;
            print!("{output}");
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

fn write_package_resolver_stdout(evidence: &str) -> Result<(), Diagnostic> {
    #[cfg(unix)]
    let mut stdout = std::fs::File::from(
        rustix::io::dup(rustix::stdio::stdout()).map_err(|_| package_resolver_stdout_error())?,
    );
    #[cfg(not(unix))]
    let stdout = std::io::stdout();
    #[cfg(not(unix))]
    let mut stdout = stdout.lock();
    stdout
        .write_all(evidence.as_bytes())
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .map_err(|_| package_resolver_stdout_error())
}

fn package_resolver_stdout_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-I215",
        "cannot write package-resolve evidence to standard output",
    )
}

fn serve_options(args: &[String]) -> Result<agent_transport::TransportLimits, u8> {
    let mut max_request_bytes = agent_transport::DEFAULT_MAX_REQUEST_BYTES;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-request-bytes") {
            eprintln!("unknown serve option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate serve option `{option}`");
            return Err(2);
        }
        let Some(value) = args.get(index + 1) else {
            eprintln!("serve option `{option}` requires a value");
            return Err(2);
        };
        let parsed = context_number(option, value)?;
        max_request_bytes = parsed;
        index += 2;
    }
    agent_transport::TransportLimits::new(max_request_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn with_native_executable_suffix(path: PathBuf) -> PathBuf {
    let extension = std::env::consts::EXE_EXTENSION;
    if extension.is_empty() || path.extension().is_some() {
        return path;
    }
    path.with_extension(extension)
}

#[cfg(test)]
#[path = "cli/native_output_tests.rs"]
mod native_output_tests;

/// Exit status of a child that was terminated by a signal. Shell convention
/// reports `128 + signal`; platforms without signal exit statuses fall back
/// to the generic failure code.
fn child_exit_code(status: &std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map_or(1, |signal| 128 + signal)
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        1
    }
}

/// The `run` command reports child failures as its own `u8` exit code. Raw
/// platform codes can exceed that range (Windows NTSTATUS crash codes such
/// as `0xC0000005`), so out-of-range values fall back to the generic failure
/// code after printing the exact code for diagnosis instead of silently
/// truncating a hard crash into an ordinary small failure.
fn child_result_code(status: &std::process::ExitStatus) -> u8 {
    let raw = status.code().unwrap_or_else(|| child_exit_code(status));
    u8::try_from(raw).unwrap_or_else(|_| {
        eprintln!("child process exited with code {raw}");
        1
    })
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

fn openapi_options(args: &[String]) -> Result<(Vec<String>, openapi::OpenApiOptions), u8> {
    let mut functions = Vec::new();
    let mut max_bytes = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--function" | "--max-bytes") {
            eprintln!("unknown openapi option `{option}`");
            return Err(2);
        }
        if option == "--max-bytes" && !seen.insert(option.to_owned()) {
            eprintln!("duplicate openapi option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("openapi option `{option}` requires a value");
            2
        })?;
        match option {
            "--function" => {
                if value.is_empty() {
                    eprintln!("openapi option `--function` requires a function name or stable id");
                    return Err(2);
                }
                functions.push(value.clone());
            }
            _ => max_bytes = Some(openapi_number(option, value)?),
        }
        index += 2;
    }
    if functions.is_empty() {
        eprintln!("openapi requires at least one --function <name|stable-id> selection");
        return Err(2);
    }
    let options = openapi::OpenApiOptions::new(
        max_bytes.unwrap_or_else(|| openapi::OpenApiOptions::default().max_bytes),
    )
    .map_err(|error| {
        eprintln!("{error}");
        2
    })?;
    Ok((functions, options))
}

fn openapi_compat_options(args: &[String]) -> Result<openapi::OpenApiOptions, u8> {
    let mut max_bytes = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 3usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown openapi-compat option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate openapi-compat option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("openapi-compat option `{option}` requires a value");
            2
        })?;
        max_bytes = Some(openapi_number(option, value)?);
        index += 2;
    }
    openapi::OpenApiOptions::new(
        max_bytes.unwrap_or_else(|| openapi::OpenApiOptions::default().max_bytes),
    )
    .map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn openapi_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("openapi option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("openapi option `{option}` requires a canonical nonnegative integer");
        2
    })
}

fn property_options(args: &[String]) -> Result<properties::PropertyTestOptions, u8> {
    let mut max_cases = properties::PropertyTestOptions::default().max_cases;
    let mut max_functions = properties::PropertyTestOptions::default().max_functions;
    let mut max_bytes = properties::PropertyTestOptions::default().max_bytes;
    let mut seed = properties::PropertyTestOptions::default().seed;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(
            option,
            "--max-cases" | "--max-functions" | "--max-bytes" | "--seed"
        ) {
            eprintln!("unknown properties option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate properties option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("properties option `{option}` requires a value");
            2
        })?;
        match option {
            "--seed" => seed = property_seed(option, value)?,
            _ => {
                let number = property_number(option, value)?;
                match option {
                    "--max-cases" => max_cases = number,
                    "--max-functions" => max_functions = number,
                    "--max-bytes" => max_bytes = number,
                    _ => unreachable!("closed properties option table"),
                }
            }
        }
        index += 2;
    }
    properties::PropertyTestOptions::new(max_cases, max_functions, max_bytes, seed).map_err(
        |error| {
            eprintln!("{error}");
            2
        },
    )
}

fn hygienic_options(args: &[String]) -> Result<hygienic::HygienicGenOptions, u8> {
    let mut templates: Vec<hygienic::Template> = Vec::new();
    let mut max_bytes = hygienic::HygienicGenOptions::default().max_bytes();
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--templates" | "--max-bytes") {
            eprintln!("unknown hygienic-gen option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate hygienic-gen option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("hygienic-gen option `{option}` requires a value");
            2
        })?;
        match option {
            "--templates" => templates = hygienic_templates(option, value)?,
            "--max-bytes" => max_bytes = property_number(option, value)?,
            _ => unreachable!("closed hygienic-gen option table"),
        }
        index += 2;
    }
    let selection = if templates.is_empty() {
        hygienic::Template::REGISTRY.to_vec()
    } else {
        templates
    };
    hygienic::HygienicGenOptions::new(&selection, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn hygienic_templates(option: &str, value: &str) -> Result<Vec<hygienic::Template>, u8> {
    let mut templates = Vec::new();
    for token in value.split(',') {
        let Some(template) = hygienic::Template::from_id(token) else {
            eprintln!(
                "hygienic-gen option `{option}` only accepts registry template ids; \
                 unknown `{token}`"
            );
            return Err(2);
        };
        if templates.contains(&template) {
            eprintln!("hygienic-gen option `{option}` repeats template `{token}`");
            return Err(2);
        }
        templates.push(template);
    }
    Ok(templates)
}

fn abi_report_options(args: &[String]) -> Result<abi_report::AbiReportOptions, u8> {
    let mut functions: Vec<String> = Vec::new();
    let mut max_bytes = abi_report::AbiReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("abi-report option `{option}` requires a value");
                    2
                })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("abi-report option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate abi-report option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("abi-report option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            other => {
                eprintln!("unknown abi-report option `{other}`");
                return Err(2);
            }
        }
    }
    abi_report::AbiReportOptions::new(functions, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn c_header_options(args: &[String]) -> Result<(c_header::CHeaderOptions, bool), u8> {
    let mut functions: Vec<String> = Vec::new();
    let mut max_bytes = c_header::CHeaderOptions::default().max_bytes;
    let mut emit_header = false;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("c-header option `{option}` requires a value");
                    2
                })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("c-header option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate c-header option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("c-header option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            "--emit-header" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate c-header option `{option}`");
                    return Err(2);
                }
                emit_header = true;
                index += 1;
            }
            other => {
                eprintln!("unknown c-header option `{other}`");
                return Err(2);
            }
        }
    }
    let options = c_header::CHeaderOptions::new(functions, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })?;
    Ok((options, emit_header))
}

fn freestanding_object_options(
    args: &[String],
) -> Result<freestanding_object::FreestandingObjectOptions, u8> {
    let mut max_bytes = freestanding_object::FreestandingObjectOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate freestanding-object option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("freestanding-object option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            other => {
                eprintln!("unknown freestanding-object option `{other}`");
                return Err(2);
            }
        }
    }
    freestanding_object::FreestandingObjectOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn capability_manifest_options(
    args: &[String],
) -> Result<capability_manifest::CapabilityManifestOptions, u8> {
    let mut max_bytes = capability_manifest::CapabilityManifestOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown capability-manifest option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate capability-manifest option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("capability-manifest option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    capability_manifest::CapabilityManifestOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn package_report_options(args: &[String]) -> Result<package_report::PackageReportOptions, u8> {
    let mut max_bytes = package_report::PackageReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown package-report option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate package-report option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("package-report option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    package_report::PackageReportOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn region_report_options(args: &[String]) -> Result<region_report::RegionReportOptions, u8> {
    let mut max_bytes = region_report::RegionReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown region-report option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate region-report option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("region-report option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    region_report::RegionReportOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn simd_report_options(args: &[String]) -> Result<simd_report::SimdReportOptions, u8> {
    let mut max_bytes = simd_report::SimdReportOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown simd-report option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate simd-report option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("simd-report option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    simd_report::SimdReportOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn protocol_check_options(args: &[String]) -> Result<protocol_check::ProtocolCheckOptions, u8> {
    let mut max_bytes = protocol_check::ProtocolCheckOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown protocol-check option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate protocol-check option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("protocol-check option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    protocol_check::ProtocolCheckOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn interpret_options(
    args: &[String],
) -> Result<(String, Vec<String>, interpreter::InterpreterOptions), u8> {
    let mut function = None;
    let mut arguments = Vec::new();
    let mut options = interpreter::InterpreterOptions::default();
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate interpret option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("interpret option `{option}` requires a value");
                    2
                })?;
                if value.is_empty() {
                    eprintln!("interpret option `{option}` requires a function name or stable id");
                    return Err(2);
                }
                function = Some(value.clone());
            }
            "--arg" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("interpret option `{option}` requires a value");
                    2
                })?;
                if value.is_empty() {
                    eprintln!("interpret option `{option}` requires a scalar literal");
                    return Err(2);
                }
                arguments.push(value.clone());
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate interpret option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("interpret option `{option}` requires a value");
                    2
                })?;
                options.max_bytes = property_number(option, value)?;
            }
            other => {
                eprintln!("unknown interpret option `{other}`");
                return Err(2);
            }
        }
        index += 2;
    }
    let Some(function) = function else {
        eprintln!("interpret requires --function <name|stable-id>");
        return Err(2);
    };
    let options = interpreter::InterpreterOptions::new(options.max_bytes, options.max_steps)
        .map_err(|error| {
            eprintln!("{error}");
            2
        })?;
    Ok((function, arguments, options))
}

fn plugin_manifest_options(args: &[String]) -> Result<plugin_manifest::PluginManifestOptions, u8> {
    let mut max_bytes = plugin_manifest::PluginManifestOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown plugin-manifest option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate plugin-manifest option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("plugin-manifest option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    plugin_manifest::PluginManifestOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn ui_schema_options(args: &[String]) -> Result<ui_schema::UiSchemaOptions, u8> {
    let mut max_bytes = ui_schema::UiSchemaOptions::default().max_bytes;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        if !matches!(option, "--max-bytes") {
            eprintln!("unknown ui-schema option `{option}`");
            return Err(2);
        }
        if !seen.insert(option.to_owned()) {
            eprintln!("duplicate ui-schema option `{option}`");
            return Err(2);
        }
        let value = args.get(index + 1).ok_or_else(|| {
            eprintln!("ui-schema option `{option}` requires a value");
            2
        })?;
        max_bytes = property_number(option, value)?;
        index += 2;
    }
    ui_schema::UiSchemaOptions::new(max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })
}

fn cxx_shim_options(args: &[String]) -> Result<(cxx_shim::CxxShimOptions, bool), u8> {
    let mut functions: Vec<String> = Vec::new();
    let mut max_bytes = cxx_shim::CxxShimOptions::default().max_bytes;
    let mut emit_fragment = false;
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 2usize;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--function" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("cxx-shim option `{option}` requires a value");
                    2
                })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("cxx-shim option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate cxx-shim option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("cxx-shim option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            "--emit-fragment" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate cxx-shim option `{option}`");
                    return Err(2);
                }
                emit_fragment = true;
                index += 1;
            }
            other => {
                eprintln!("unknown cxx-shim option `{other}`");
                return Err(2);
            }
        }
    }
    let options = cxx_shim::CxxShimOptions::new(functions, max_bytes).map_err(|error| {
        eprintln!("{error}");
        2
    })?;
    Ok((options, emit_fragment))
}

fn property_number(option: &str, value: &str) -> Result<usize, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<usize>().map_err(|_| {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        2
    })
}

fn property_seed(option: &str, value: &str) -> Result<u64, u8> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
        return Err(2);
    }
    value.parse::<u64>().map_err(|_| {
        eprintln!("properties option `{option}` requires a canonical nonnegative integer");
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

fn build_source(options: &cli::build::BuildOptions, input: &Path) -> Result<(), u8> {
    if options.profile.as_deref() == Some("internal-strings-v1") {
        let output = options.output.as_deref().expect("source output");
        wasm::internal_strings::build_web_from_source(input, output, &options.exports)
            .map_err(|errors| report(&errors, false))?;
        println!("built internal String web package {}", output.display());
        return Ok(());
    }
    let program = checked(input)?;
    let output = options
        .output
        .as_deref()
        .expect("source build options always have an output");
    match options.target.as_str() {
        "native" => {
            codegen::build(&program, output).map_err(|error| report(&[error], false))?;
            println!("built native executable {}", output.display());
        }
        "web" | "wasm" => {
            if options.exports.is_empty() {
                wasm::build_web(&program, output).map_err(|error| report(&[error], false))?;
            } else {
                wasm::build_web_with_scalar_exports(&program, output, &options.exports)
                    .map_err(|error| report(&[error], false))?;
            }
            println!("built web package {}", output.display());
        }
        "native-callable" => {
            let function = options
                .function
                .as_deref()
                .expect("validated build options");
            let bundle = codegen::build_native_callable_bundle(&program, function, output)
                .map_err(|error| report(&[error], false))?;
            println!(
                "built native-callable bundle {} (manifest sha256:{})",
                bundle.output_directory().display(),
                bundle.manifest_sha256()
            );
        }
        _ => unreachable!("validated build target"),
    }
    Ok(())
}

fn run_legacy_source(path: &Path) -> Result<(), u8> {
    // Source rejection cannot acquire scratch or cleanup authority.
    let program = checked(path)?;
    let c_source = codegen::emit_c(&program).map_err(|error| report(&[error], false))?;
    let leaf = format!("program{}", std::env::consts::EXE_SUFFIX);
    let mut scratch = native_scratch::Scratch::create(&leaf, None).map_err(|error| {
        report(
            &[Diagnostic::io(
                "SPX-I101",
                format!("cannot create native run scratch: {error}"),
            )],
            false,
        )
    })?;
    codegen::compile_native_executable(&c_source, scratch.path())
        .map_err(|error| report(&[error], false))?;
    scratch.seal().map_err(|error| {
        report(
            &[Diagnostic::io(
                "SPX-I101",
                format!("cannot seal native run scratch: {error}"),
            )],
            false,
        )
    })?;
    let status = Command::new(scratch.path()).status().map_err(|error| {
        eprintln!("cannot run {}: {error}", scratch.path().display());
        1
    })?;
    if !status.success() {
        return Err(child_result_code(&status));
    }
    // Failures retain their exact scratch for inspection. Even successful
    // cleanup cannot replace the child status with a secondary cleanup error.
    let _ = scratch.cleanup();
    Ok(())
}

fn project_execution_held(
    command: &str,
    manifest_path: &Path,
    options: &cli::execution::ExecutionOptions,
) -> Result<(), u8> {
    let defaults = project::ProjectExecutionOptions::default();
    let execution_options = project::ProjectExecutionOptions::new(
        options.max_bytes.unwrap_or(defaults.max_bytes),
        options.max_steps.unwrap_or(defaults.max_steps),
    )
    .map_err(|error| report(&[error], options.json))?;
    let execution = project::with_authenticated_project(manifest_path, |snapshot| match command {
        "run" => snapshot.execute_entry(&execution_options),
        "test" => snapshot.execute_test(&execution_options),
        _ => unreachable!("validated project execution command"),
    })
    .map_err(|errors| report(&errors, options.json))?;

    if options.json {
        println!("{}", execution.envelope());
    }

    match (command, execution.outcome()) {
        ("run", project::ProjectExecutionOutcome::Returned(value)) => {
            if !options.json {
                println!("{value}");
            }
            Ok(())
        }
        ("test", project::ProjectExecutionOutcome::Returned(0)) => {
            if !options.json {
                println!("project tests passed");
            }
            Ok(())
        }
        ("test", project::ProjectExecutionOutcome::Returned(value)) => {
            if !options.json {
                eprintln!("project tests failed with result {value}");
            }
            Err(1)
        }
        (_, project::ProjectExecutionOutcome::LanguageFailure(status)) => {
            if !options.json {
                eprintln!(
                    "project execution failed with language status {}",
                    status.to_json()
                );
            }
            Err(1)
        }
        (_, project::ProjectExecutionOutcome::FuelExhausted) => {
            if !options.json {
                eprintln!("project execution exhausted its step budget");
            }
            Err(1)
        }
        (_, project::ProjectExecutionOutcome::CallDepthExceeded) => {
            if !options.json {
                eprintln!("project execution exceeded its call-depth bound");
            }
            Err(1)
        }
        _ => unreachable!("validated project execution command"),
    }
}

fn project_build_success(
    target: &str,
    profile: project::ProjectProfile,
    output: &std::path::Path,
) -> String {
    let product = match (target, profile) {
        ("native", _) => "project native executable",
        ("rust", project::ProjectProfile::FlatOwnedRecordApiV1) => {
            "Project v9 Native Rust flat owned-record package"
        }
        ("rust", project::ProjectProfile::OwnedUtf8ApiV1) => {
            "Project v10 Native Rust owned-data package"
        }
        ("rust", _) => "Project v8 Native Rust owned-data package",
        ("npm", project::ProjectProfile::FlatOwnedRecordApiV1) => "Project v9 npm package",
        ("npm", project::ProjectProfile::OwnedUtf8ApiV1) => "Project v10 npm package",
        ("npm", _) => "Project v2 npm package",
        _ => "project web package",
    };
    format!("built {product} {}", output.display())
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
           semaprax check [<file>|semaprax.toml|--manifest-path path] [--json]\n\
           semaprax graph <file>\n\
           semaprax project-image <manifest>\n\
           semaprax project-image-store <manifest> <store-root>\n\
           semaprax project-image-load <store-root> <receipt.json> <expected-image-digest>\n\
           semaprax project-image-verify <manifest> <image.json>\n\
           semaprax project-symbol <manifest> <stable-id>\n\
           semaprax project-candidate-preview <manifest> <change.json>\n\
           semaprax project-candidate-export <manifest> <change.json>\n\
           semaprax project-candidate-restore <manifest> <capsule.json>\n\
           semaprax semantic-cache-init <store-root>\n\
           semaprax semantic-cache-persist <manifest> <store-root>\n\
           semaprax semantic-cache-load <store-root> <entry-digest>\n\
           semaprax project-candidate-persist <manifest> <capsule.json> <store-root>\n\
           semaprax project-candidate-load <store-root> <archive-digest> <candidate-digest>\n\
           semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>\n\
           semaprax serve-workspace <manifest> <host-policy.json>\n\
           semaprax serve-image <manifest>\n\
           semaprax serve-candidates <manifest>\n\
           semaprax serve-test-candidates <manifest>\n\
           semaprax serve-diagnostics <manifest>\n\
           semaprax serve-diagnostics-tested <manifest>\n\
           semaprax context <file> <symbol|stable-id> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N] [--filters contracts,ownership,effects,types,targets,diagnostics,tests]\n\
            semaprax context-benchmark <manifest>\n\
            semaprax serve <file> [--max-request-bytes N]\n\
            semaprax quality-plan <quick|changed|full> [exact-changed-path ...]\n\
            semaprax doctor [--profile <id>] [--target native|web|all] [--json]\n\
            semaprax new <destination> [--name project-name] [--template calculator]\n\
           semaprax build [<file>|semaprax.toml|--manifest-path path] [--target native|native-callable|web|wasm|npm] [--profile internal-strings-v1] [--function stable-id] [--export stable-id ...] [-o path]\n\
           semaprax run <file>\n\
           semaprax run [semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]\n\
           semaprax test [semaprax.toml|--manifest-path path] [--json] [--max-steps N] [--max-bytes N]\n\
           semaprax fmt <file> [--check]\n\
           semaprax patch <file> <patch.spatch>\n\
           semaprax workspace-init <root> <path-set.json>\n\
           semaprax semantic-workspace-init <root> <path-set.json>\n\
           semaprax semantic-workspace-change-preview <root> <proposal.json>\n\
           semaprax semantic-workspace-change-evidence <root> <proposal.json>\n\
           semaprax verify-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>\n\
           semaprax apply-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>\n\
           semaprax semantic-workspace-structural-change-preview <root> <proposal.json>\n\
           semaprax semantic-workspace-structural-change-evidence <root> <proposal.json>\n\
           semaprax verify-semantic-workspace-structural-change-evidence <root> <proposal.json> <evidence.json>\n\
           semaprax apply-semantic-workspace-structural-change-evidence <root> <proposal.json> <evidence.json>\n\
           semaprax semantic-workspace-operations-derive <root> <proposal.json>\n\
           semaprax semantic-workspace-operations-change-proposal <root> <proposal.json>\n\
           semaprax semantic-workspace-operations-evidence <root> <proposal.json>\n\
           semaprax verify-semantic-workspace-operations-evidence <root> <proposal.json> <evidence.json>\n\
           semaprax apply-semantic-workspace-operations-evidence <root> <proposal.json> <evidence.json>\n\
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
           semaprax properties <file> [--max-cases N] [--max-functions N] [--max-bytes N] [--seed N]\n\
           semaprax hygienic-gen <file> [--templates default-constructor,field-accessors] [--max-bytes N]\n\
           semaprax openapi <file> --function <name|stable-id> ... [--max-bytes N]\n\
           semaprax openapi-compat <base.json> <candidate.json> [--max-bytes N]\n\
            semaprax c-header <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-header]\n\
            semaprax freestanding-object <file> [--max-bytes N]\n\
            semaprax abi-report <file> --function name|stable-id[,...] [--function ...] [--max-bytes N]\n\
             semaprax capability-manifest <file> [--max-bytes N]\n\
              semaprax package-report <file> [--max-bytes N]\n\
              semaprax package-lock <subject.json>... [--max-bytes N]\n\
              semaprax package-resolve <subject.json>... --require <package>:<range> [--require ...] --target <native64|wasm32> [--allow-capability <capability>]... [--max-bytes N]\n\
             semaprax region-report <file> [--max-bytes N]\n\
             semaprax simd-report <file> [--max-bytes N]\n\
            semaprax protocol-check <file> [--max-bytes N]\n\
            semaprax interpret <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]\n\
            semaprax interpret-strings <file> --function <name|stable-id> [--arg <scalar literal>]... [--max-bytes N]\n\
             semaprax ui-schema <file> [--max-bytes N]\n\
           semaprax plugin-manifest <file> [--max-bytes N]\n\
            semaprax cxx-shim <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-fragment]\n\
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
           semaprax version [--json]\n\
           semaprax --version"
    );
}

#[cfg(test)]
mod project_build_success_tests {
    use super::*;

    #[test]
    fn profile_selected_success_labels_are_exact() {
        let output = std::path::Path::new("dist");
        assert_eq!(
            project_build_success(
                "rust",
                project::ProjectProfile::FlatOwnedRecordApiV1,
                output,
            ),
            "built Project v9 Native Rust flat owned-record package dist"
        );
        assert_eq!(
            project_build_success("npm", project::ProjectProfile::FlatOwnedRecordApiV1, output,),
            "built Project v9 npm package dist"
        );
        assert_eq!(
            project_build_success("rust", project::ProjectProfile::OwnedDataApiV1, output),
            "built Project v8 Native Rust owned-data package dist"
        );
        assert_eq!(
            project_build_success("npm", project::ProjectProfile::OwnedUtf8ApiV1, output),
            "built Project v10 npm package dist"
        );
    }
}
