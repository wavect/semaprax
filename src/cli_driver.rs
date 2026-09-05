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
    freestanding_object, graph, hir, hosted_interpreter, hygienic, impact, interpreter, openapi,
    package_report, parse, patch, patch_evidence, plugin_manifest, project, properties,
    protocol_check, quality_route, region_report, repair, review, semantic_workspace,
    semantic_workspace_change, semantic_workspace_operations, semantic_workspace_structural_change,
    simd_report, target_evidence, ui_schema, verify, wasm, workspace, workspace_analysis,
    workspace_graph, workspace_patch_evidence,
};

#[path = "cli/mod.rs"]
mod cli;
#[path = "native_scratch.rs"]
mod native_scratch;
#[path = "cli_driver/project_scaffold_options.rs"]
mod project_scaffold_options;
#[path = "cli_driver/report_options.rs"]
mod report_options;
#[path = "cli_driver/supply_chain.rs"]
mod supply_chain;

use report_options::*;

#[cfg(test)]
#[path = "cli/native_scratch_tests.rs"]
mod native_scratch_tests;

/// Explicit private-host hooks supplied only by the unpublished toolchain.
pub type DoctorHook = fn(&[String]) -> Result<(String, u8), String>;
/// Creates a project and returns the destination as spelled plus the
/// template it published.
pub type NewProjectHook = fn(&[String]) -> Result<(PathBuf, &'static str), (String, u8)>;

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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let recovery_hint = cli::help::usage_recovery_hint(&args, host.is_some());
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
    cli::help::finish(outcome, recovery_hint)
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
        print_help(host.is_some());
        return Err(2);
    };
    if command == "help" && args.len() == 2 {
        if args[1] == "all" {
            print!("{}", cli::help::catalog(host.is_some()));
            return Ok(());
        }
        if args[1] == "language" {
            print!("{}", cli::help::LANGUAGE_REFERENCE);
            return Ok(());
        }
        if args[1] == "library" {
            print!("{}", cli::help::LIBRARY_CATALOG);
            return Ok(());
        }
        return print_scoped_help(&args[1], host.is_some());
    }
    if command == "help" && args.len() > 2 {
        eprintln!(
            "help accepts exactly one operand; unexpected extra operand `{}`",
            args[2]
        );
        return Err(2);
    }
    if args.len() == 2 && matches!(args[1].as_str(), "--help" | "-h") {
        return print_scoped_help(command, host.is_some());
    }
    if args[1..]
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        eprintln!("help flags are admitted only as the sole operand of a command");
        return Err(2);
    }
    let Some(command_id) = cli::help::parse(command, true) else {
        eprint!("{}", cli::help::unknown_diagnostic(command, host.is_some()));
        print_help(host.is_some());
        return Err(2);
    };
    use cli::help::CommandId;
    match command_id {
        CommandId::Check => {
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
                        .map_err(|errors| {
                            let errors =
                                cli::manifest_hint::hint_missing_manifest(errors, &manifest_path);
                            report(&errors, json)
                        })?;
                    if json {
                        println!(
                            "{{\"status\":\"verified\",\"name\":{},\"revision\":{}}}",
                            semaprax::diagnostic::quote_json(&name),
                            semaprax::diagnostic::quote_json(&revision)
                        );
                    } else {
                        println!("verified project {name} ({revision})");
                    }
                    return Ok(());
                }
            };
            let program = load(&path).map_err(|errors| report(&errors, json))?;
            let diagnostics = hir::analyze(&program).diagnostics;
            let failed = diagnostics
                .iter()
                .any(|item| item.severity == Severity::Error);
            report_all(&diagnostics, json);
            if failed {
                Err(1)
            } else {
                if json {
                    println!(
                        "{{\"status\":\"verified\",\"path\":{},\"revision\":{}}}",
                        semaprax::diagnostic::quote_json(&path.display().to_string()),
                        semaprax::diagnostic::quote_json(&graph::revision(&program))
                    );
                } else {
                    println!(
                        "verified {} ({})",
                        path.display(),
                        graph::revision(&program)
                    );
                }
                Ok(())
            }
        }
        CommandId::ProjectCandidateGitPublish => {
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
        CommandId::SemanticCacheInit
        | CommandId::SemanticCachePersist
        | CommandId::SemanticCacheLoad
        | CommandId::SemanticCacheEvict
        | CommandId::SemanticCacheLifecycle => {
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
                "semantic-cache-load" => cli::semantic_cache::load(Path::new(&args[1]), &args[2]),
                "semantic-cache-evict" => cli::semantic_cache::evict(Path::new(&args[1]), &args[2]),
                _ => cli::semantic_cache::lifecycle(Path::new(&args[1]), Path::new(&args[2])),
            }
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::RetentionMetadataInventory => {
            if args.len() != 2 || args[1].is_empty() || args[1].starts_with('-') {
                eprintln!("{command} requires exactly <declarations.json>");
                return Err(2);
            }
            let output = cli::retention_metadata::inventory(Path::new(&args[1]))
                .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::RetentionMetadataPlan => {
            if args.len() != 9
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                eprintln!("{command} requires its exact positional operands; see --help");
                return Err(2);
            }
            let no_previous = args[6] == "none";
            if no_previous != (args[7] == "none") || (no_previous && args[8] != "none") {
                eprintln!("{command} requires a previous checkpoint file and selector together");
                return Err(2);
            }
            let output = cli::retention_metadata::plan(cli::retention_metadata::PlanOptions {
                inventory: Path::new(&args[1]),
                sequence: &args[2],
                max_subjects: &args[3],
                max_bytes: &args[4],
                protected_generations: &args[5],
                previous_checkpoint: (!no_previous).then(|| Path::new(&args[6])),
                expected_previous: (!no_previous).then_some(args[7].as_str()),
                expected_previous_predecessor: (args[8] != "none").then_some(args[8].as_str()),
            })
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::RetentionMetadataPersist | CommandId::RetentionMetadataLoad => {
            let arity = if command == "retention-metadata-persist" {
                7
            } else {
                5
            };
            if args.len() != arity
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                eprintln!("{command} requires its exact positional operands; see --help");
                return Err(2);
            }
            let previous = (args[4] != "none").then_some(args[4].as_str());
            let output = if command == "retention-metadata-persist" {
                cli::retention_metadata::persist(
                    Path::new(&args[1]),
                    Path::new(&args[2]),
                    &args[3],
                    previous,
                    Path::new(&args[5]),
                    &args[6],
                )
            } else {
                let previous = (args[3] != "none").then_some(args[3].as_str());
                cli::retention_metadata::load(Path::new(&args[1]), &args[2], previous, &args[4])
            }
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::ProjectDraftPersist | CommandId::ProjectDraftLoad => {
            if args.len() != 4
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                let operands = if command == "project-draft-persist" {
                    "<manifest> <draft-capsule.json> <store-root>"
                } else {
                    "<store-root> <archive-digest> <draft-digest>"
                };
                eprintln!("{command} requires exactly {operands}");
                return Err(2);
            }
            let output = if command == "project-draft-persist" {
                cli::draft_archive::persist(
                    Path::new(&args[1]),
                    Path::new(&args[2]),
                    Path::new(&args[3]),
                )
            } else {
                cli::draft_archive::load(Path::new(&args[1]), &args[2], &args[3])
            }
            .map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::ProjectCandidatePersist | CommandId::ProjectCandidateLoad => {
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
        CommandId::ProjectImage
        | CommandId::ProjectImageStore
        | CommandId::ProjectImageLoad
        | CommandId::ProjectImageVerify
        | CommandId::ProjectSymbol
        | CommandId::ProjectCandidatePreview
        | CommandId::ProjectCandidateExport
        | CommandId::ProjectCandidateRestore => {
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
        CommandId::Graph => {
            let path = cli::graph::parse(&args[1..])?;
            let program = checked(&path)?;
            let output = graph::to_json(&program).map_err(|errors| report(&errors, false))?;
            println!("{output}");
            Ok(())
        }
        CommandId::Doc => {
            let options = cli::doc::parse(&args[1..])?;
            cli::doc::run(options, |errors| report(errors, false))
        }
        CommandId::Verify => {
            let options = cli::verify::parse(&args[1..])?;
            let receipt = cli::verify::run(&options, cli::project_image::verify)
                .map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        CommandId::Query => {
            let options = cli::query::parse(&args[1..])?;
            cli::query::run(options, |errors| report(errors, false))
        }
        CommandId::Package => {
            let rewritten = cli::package::long_form(&args[1..])?;
            run(rewritten, host)
        }
        CommandId::Agent => {
            let command = cli::agent::parse(&args[1..])?;
            let output = cli::agent::run(&command).map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::Context => {
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
                report(
                    &[Diagnostic::io(
                        "SPX-G404",
                        format!("symbol `{symbol}` was not found"),
                    )
                    .at_path(path.display().to_string())
                    .with_help("inspect available declaration identities with `semaprax graph <file>`")],
                    false,
                )
            })?;
            println!("{context}");
            Ok(())
        }
        CommandId::ServeWorkspace | CommandId::ServeWorkspaceMcp => {
            if args.len() != 3
                || args[1..]
                    .iter()
                    .any(|argument| argument.is_empty() || argument.starts_with('-'))
            {
                eprintln!("{command} requires exactly <manifest> <host-policy.json>");
                return Err(2);
            }
            if command == "serve-workspace-mcp" {
                cli::workspace_session::run_mcp(Path::new(&args[1]), Path::new(&args[2]))
            } else {
                cli::workspace_session::run(Path::new(&args[1]), Path::new(&args[2]))
            }
            .map_err(|errors| report(&errors, false))
        }
        CommandId::ServeImage
        | CommandId::ServeCandidates
        | CommandId::ServeTestCandidates
        | CommandId::ServeDiagnostics
        | CommandId::ServeDiagnosticsTested => {
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
        CommandId::Serve => {
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
        CommandId::ContextBenchmark => {
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
        CommandId::QualityPlan => {
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
        CommandId::Doctor => {
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
        CommandId::New => {
            let (destination, template) = match host {
                // The full toolchain publishes through its held-parent staged
                // authority; the standalone compiler through the bounded
                // create-new route in the compiler library.
                Some(host) => (host.new_project)(&args[1..]).map_err(|(error, code)| {
                    eprintln!("new: {error}");
                    code
                })?,
                None => {
                    let options = cli::new_project::parse(&args[1..]).map_err(|error| {
                        eprintln!("new: {error}");
                        2
                    })?;
                    let destination = project::create_project(
                        &options.destination,
                        &options.name,
                        options.template,
                    )
                    .map_err(|error| {
                        eprintln!("new: {error}");
                        1
                    })?;
                    (destination, options.template)
                }
            };
            println!("created {template} project {}", destination.display());
            Ok(())
        }
        CommandId::ProjectScaffold => {
            let (name, template, layout) = project_scaffold_options::parse(&args[1..])?;
            let artifact = project::derive_project_scaffold_v1_with_layout(name, template, layout)
                .map_err(|errors| report(&errors, false))?;
            let bytes = artifact.canonical_bytes();
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            stdout
                .write_all(&bytes)
                .and_then(|()| stdout.flush())
                .map_err(|error| {
                    eprintln!("project-scaffold: cannot write descriptor: {error}");
                    1
                })?;
            Ok(())
        }
        CommandId::Build => {
            let options = cli::build::parse_with_capabilities(&args[1..], host.is_some())?;
            if options.target == "rust" {
                require_private_host(host, "build --target rust")?;
            }
            match &options.input {
                cli::build::BuildInput::Source(input) => build_source(&options, input)?,
                cli::build::BuildInput::Project(manifest_path) => {
                    let output = project::with_authenticated_project(manifest_path, |snapshot| {
                        snapshot.check()?;
                        snapshot.manifest().admit_build_target(&options.target)?;
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
                            if snapshot.manifest().schema() != project::PROJECT_SCHEMA
                                && !snapshot.manifest().is_v8()
                                && !snapshot.manifest().is_v9()
                                && !snapshot.manifest().is_v10()
                                && !snapshot.manifest().is_v11()
                            {
                                return Err(vec![Diagnostic::io(
                                    "SPX-J114",
                                    "the rust target requires Project v1 scalar exports or the exact Project v8 owned-data-api.v1, Project v9 flat-owned-record-api.v1, Project v10 owned-utf8-api.v1, or Project v11 nested-owned-record-api.v1 profile",
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
                                || snapshot.manifest().is_v10()
                                || snapshot.manifest().is_v11())
                        {
                            let host = host.ok_or_else(|| {
                                vec![Diagnostic::io(
                                    "SPX-W120",
                                    "Project v8-v11 npm publication requires semaprax-full with safe handle-relative Windows authority",
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
                        if options.target == "rust" {
                            output = cli::build::bind_rust_output_parent(&output)
                                .map_err(|error| vec![error])?;
                        }
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
                    .map_err(|errors| {
                        let errors =
                            cli::manifest_hint::hint_missing_manifest(errors, manifest_path);
                        report(&errors, options.json)
                    })?;
                    cli::project_runtime::report_build_success(
                        &options.target,
                        output.1,
                        &output.0,
                        options.json,
                    );
                }
            }
            Ok(())
        }
        CommandId::Run => {
            let options = cli::execution::parse_run(&args[1..])?;
            match &options.input {
                cli::execution::ExecutionInput::Source(path) if options.native => {
                    run_native_source(path)
                }
                cli::execution::ExecutionInput::Source(path) => {
                    run_interpreted_source(path, &options)
                }
                cli::execution::ExecutionInput::Project(manifest_path) => {
                    cli::project_runtime::execute_held("run", manifest_path, &options)
                }
            }
        }
        CommandId::Test => {
            let options = cli::execution::parse_test(&args[1..])?;
            let cli::execution::ExecutionInput::Project(manifest_path) = &options.input else {
                unreachable!("project test parser rejects source inputs")
            };
            cli::project_runtime::execute_held("test", manifest_path, &options)
        }
        CommandId::Fmt => {
            let options = cli::fmt::parse(&args[1..])?;
            cli::fmt::run(options, |errors| report(errors, false))
        }
        CommandId::Patch => {
            if args.len() != 3 {
                eprintln!("patch requires exactly <file> <patch.spatch>");
                return Err(2);
            }
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let revision =
                patch::apply(&source_path, &patch_path).map_err(|errors| report(&errors, false))?;
            println!("applied semantic patch; graph is now {revision}");
            Ok(())
        }
        CommandId::WorkspaceInit => {
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
        CommandId::SemanticWorkspaceInit => {
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
        CommandId::SemanticWorkspaceChangePreview => {
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
        CommandId::SemanticWorkspaceChangeEvidence => {
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
        CommandId::VerifySemanticWorkspaceChangeEvidence => {
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
        CommandId::ApplySemanticWorkspaceChangeEvidence => {
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
        CommandId::SemanticWorkspaceStructuralChangePreview => {
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
        CommandId::SemanticWorkspaceStructuralChangeEvidence => {
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
        CommandId::VerifySemanticWorkspaceStructuralChangeEvidence => {
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
        CommandId::ApplySemanticWorkspaceStructuralChangeEvidence => {
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
        CommandId::SemanticWorkspaceOperationsDerive => {
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
        CommandId::SemanticWorkspaceOperationsChangeProposal => {
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
        CommandId::SemanticWorkspaceOperationsEvidence => {
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
        CommandId::VerifySemanticWorkspaceOperationsEvidence => {
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
        CommandId::ApplySemanticWorkspaceOperationsEvidence => {
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
        CommandId::WorkspaceSnapshot => {
            if args.len() != 2 {
                eprintln!("workspace-snapshot requires exactly <root>");
                return Err(2);
            }
            let root = required_path(&args, 1)?;
            let snapshot = workspace::snapshot(&root).map_err(|errors| report(&errors, false))?;
            println!("{}", snapshot.to_json());
            Ok(())
        }
        CommandId::WorkspaceGraph => {
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
        CommandId::WorkspaceContext => {
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
        CommandId::WorkspaceImpact => {
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
        CommandId::WorkspaceReview => {
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
        CommandId::WorkspacePreview => {
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
        CommandId::WorkspaceApply => {
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
        CommandId::WorkspacePatchEvidence => {
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
        CommandId::VerifyWorkspacePatchEvidence => {
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
        CommandId::WorkspaceApplyWithEvidence => {
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
        CommandId::Impact => {
            let source_path = required_path(&args, 1)?;
            let patch_path = required_path(&args, 2)?;
            let options = impact_options(&args)?;
            let report = impact::preview(&source_path, &patch_path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        CommandId::Review => {
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
        CommandId::Properties => {
            let path = required_path(&args, 1)?;
            let options = property_options(&args)?;
            let report =
                properties::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        CommandId::HygienicGen => {
            let path = required_path(&args, 1)?;
            let options = hygienic_options(&args)?;
            let report =
                hygienic::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        CommandId::Openapi => {
            let path = required_path(&args, 1)?;
            let (functions, options) = openapi_options(&args)?;
            let report = openapi::generate(&path, &functions, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        CommandId::OpenapiCompat => {
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
        CommandId::AbiReport => {
            let path = required_path(&args, 1)?;
            let options = abi_report_options(&args)?;
            let report =
                abi_report::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{report}");
            Ok(())
        }
        CommandId::CHeader => {
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
        CommandId::FreestandingObject => {
            let path = required_path(&args, 1)?;
            let options = freestanding_object_options(&args)?;
            let envelope = freestanding_object::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::CapabilityManifest => {
            let path = required_path(&args, 1)?;
            let options = capability_manifest_options(&args)?;
            let envelope = capability_manifest::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::PackageReport => {
            let path = required_path(&args, 1)?;
            let options = package_report_options(&args)?;
            let envelope = package_report::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::PackageLock => supply_chain::package_lock(&args[1..]),
        CommandId::Lock => supply_chain::project_lock(&args[1..]),
        CommandId::Resolve => supply_chain::resolve(&args[1..]),
        CommandId::PackageResolve => supply_chain::package_resolve(&args[1..]),
        CommandId::RegionReport => {
            let path = required_path(&args, 1)?;
            let options = region_report_options(&args)?;
            let envelope = region_report::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::SimdReport => {
            let path = required_path(&args, 1)?;
            let options = simd_report_options(&args)?;
            let envelope =
                simd_report::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::ProtocolCheck => {
            let path = required_path(&args, 1)?;
            let options = protocol_check_options(&args)?;
            let envelope = protocol_check::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::Interpret | CommandId::InterpretStrings => {
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
        CommandId::PluginManifest => {
            let path = required_path(&args, 1)?;
            let options = plugin_manifest_options(&args)?;
            let envelope = plugin_manifest::generate(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::UiSchema => {
            let path = required_path(&args, 1)?;
            let options = ui_schema_options(&args)?;
            let envelope =
                ui_schema::generate(&path, &options).map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::CxxShim => {
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
        CommandId::CxxPackage => {
            let path = required_path(&args, 1)?;
            let options = cxx_package_options(&args)?;
            let envelope = cxx_shim::generate_package(&path, &options)
                .map_err(|errors| report(&errors, false))?;
            println!("{envelope}");
            Ok(())
        }
        CommandId::TargetEvidence => {
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
        CommandId::PatchEvidence => {
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
        CommandId::PatchEvidenceV2 => {
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
        CommandId::VerifyPatchEvidence => {
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
        CommandId::VerifyPatchEvidenceV2 => {
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
        CommandId::PatchWithEvidence => {
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
        CommandId::PatchWithEvidenceV2 => {
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
        CommandId::Repairs => {
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
        CommandId::Repair => {
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
        CommandId::Version => {
            let output = cli::version::render(cli::version::Invocation::Command, &args[1..])
                .map_err(|error| {
                    eprintln!("{error}");
                    2
                })?;
            print!("{output}");
            Ok(())
        }
        CommandId::VersionFlag => {
            let output = cli::version::render(cli::version::Invocation::Flag, &args[1..]).map_err(
                |error| {
                    eprintln!("{error}");
                    2
                },
            )?;
            print!("{output}");
            Ok(())
        }
        CommandId::Help => {
            if args.len() == 1 {
                print_help(host.is_some());
                Ok(())
            } else {
                eprintln!("unknown command `{command}`\n");
                print_help(host.is_some());
                Err(2)
            }
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
    cxx_selection_options(args, "cxx-shim", true)
}

fn cxx_package_options(args: &[String]) -> Result<cxx_shim::CxxShimOptions, u8> {
    let (options, emit_fragment) = cxx_selection_options(args, "cxx-package", false)?;
    debug_assert!(!emit_fragment);
    Ok(options)
}

fn cxx_selection_options(
    args: &[String],
    command: &str,
    allow_fragment: bool,
) -> Result<(cxx_shim::CxxShimOptions, bool), u8> {
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
                    eprintln!("{command} option `{option}` requires a value");
                    2
                })?;
                for token in value.split(',') {
                    if token.is_empty() {
                        eprintln!("{command} option `{option}` requires nonempty selections");
                        return Err(2);
                    }
                    functions.push(token.to_owned());
                }
                index += 2;
            }
            "--max-bytes" => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate {command} option `{option}`");
                    return Err(2);
                }
                let value = args.get(index + 1).ok_or_else(|| {
                    eprintln!("{command} option `{option}` requires a value");
                    2
                })?;
                max_bytes = property_number(option, value)?;
                index += 2;
            }
            "--emit-fragment" if allow_fragment => {
                if !seen.insert(option.to_owned()) {
                    eprintln!("duplicate {command} option `{option}`");
                    return Err(2);
                }
                emit_fragment = true;
                index += 1;
            }
            other => {
                eprintln!("unknown {command} option `{other}`");
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
    checked_for_output(path, false)
}

fn checked_for_output(path: &Path, json: bool) -> Result<semaprax::ast::Program, u8> {
    let program = load(path).map_err(|errors| report(&errors, json))?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        Err(report(&diagnostics, json))
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
            .map_err(|errors| report(&errors, options.json))?;
        report_source_build_success(options, "internal String web package", output, None);
        return Ok(());
    }
    let program = checked_for_output(input, options.json)?;
    let output = options
        .output
        .as_deref()
        .expect("source build options always have an output");
    match options.target.as_str() {
        "native" => {
            let mut destination = cli::build::SourceNativeOutput::prepare(output)
                .map_err(|error| report(&[error], options.json))?;
            let c_source =
                codegen::emit_c(&program).map_err(|error| report(&[error], options.json))?;
            let leaf = format!("program{}", std::env::consts::EXE_SUFFIX);
            let mut scratch = native_scratch::Scratch::create(&leaf, None).map_err(|error| {
                report(
                    &[Diagnostic::io(
                        "SPX-I301",
                        format!("cannot create native build scratch: {error}"),
                    )],
                    options.json,
                )
            })?;
            codegen::compile_native_executable(&c_source, scratch.path())
                .map_err(|error| report(&[error], options.json))?;
            scratch.seal().map_err(|error| {
                report(
                    &[Diagnostic::io(
                        "SPX-I301",
                        format!("cannot seal native build scratch: {error}"),
                    )],
                    options.json,
                )
            })?;
            destination
                .publish(scratch.path())
                .map_err(|error| report(&[error], options.json))?;
            let _ = scratch.cleanup();
            report_source_build_success(options, "native executable", output, None);
        }
        "web" | "wasm" => {
            if options.exports.is_empty() {
                wasm::build_web(&program, output)
                    .map_err(|error| report(&[error], options.json))?;
            } else {
                wasm::build_web_with_scalar_exports(&program, output, &options.exports)
                    .map_err(|error| report(&[error], options.json))?;
            }
            report_source_build_success(options, "web package", output, None);
        }
        "native-callable" => {
            let function = options
                .function
                .as_deref()
                .expect("validated build options");
            let bundle = codegen::build_native_callable_bundle(&program, function, output)
                .map_err(|error| report(&[error], options.json))?;
            if options.json {
                report_source_build_success(
                    options,
                    "native-callable bundle",
                    bundle.output_directory(),
                    Some(bundle.manifest_sha256()),
                );
            } else {
                println!(
                    "built native-callable bundle {} (manifest sha256:{})",
                    bundle.output_directory().display(),
                    bundle.manifest_sha256()
                );
            }
        }
        _ => unreachable!("validated build target"),
    }
    Ok(())
}

fn report_source_build_success(
    options: &cli::build::BuildOptions,
    product: &str,
    output: &Path,
    manifest_sha256: Option<&str>,
) {
    if options.json {
        let mut value = serde_json::json!({
            "status": "built",
            "target": options.target,
            "product": product,
            "output": output.display().to_string(),
        });
        if let Some(digest) = manifest_sha256 {
            value["manifest_sha256"] = serde_json::Value::String(digest.to_owned());
        }
        println!("{value}");
    } else {
        println!("built {product} {}", output.display());
    }
}

fn run_native_source(path: &Path) -> Result<(), u8> {
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

fn run_interpreted_source(
    path: &Path,
    options: &cli::execution::ExecutionOptions,
) -> Result<(), u8> {
    let defaults = interpreter::InterpreterOptions::default();
    let interpreter_options = interpreter::InterpreterOptions::new(
        options.max_bytes.unwrap_or(defaults.max_bytes),
        options.max_steps.unwrap_or(defaults.max_steps),
    )
    .map_err(|error| report(&[error], options.json))?;

    // The bounded stdout profile is a distinct interpreter seam because the
    // canonical `semaprax.interpret.v1` profile is deliberately effect-free.
    let program = checked(path)?;
    if program.permits == ["process.stdout.write"] {
        let resolved = hir::resolve(&program).map_err(|errors| report(&errors, options.json))?;
        let hosted = hosted_interpreter::execute_stdout_transcript(
            &resolved,
            "app.main",
            interpreter_options.max_steps,
        )
        .map_err(|errors| report(&errors, options.json))?;
        return publish_interpreted_stdout(hosted, &interpreter_options, options.json);
    }

    let interpretation = interpreter::interpret(path, "app.main", &[], &interpreter_options)
        .map_err(|errors| report(&errors, options.json))?;
    if options.json {
        println!("{}", interpretation.envelope);
        return interpretation.returned.then_some(()).ok_or(1);
    }
    publish_interpretation(&interpretation.envelope)
}

fn publish_interpretation(envelope: &str) -> Result<(), u8> {
    let parsed: serde_json::Value = serde_json::from_str(envelope).map_err(|error| {
        report(
            &[Diagnostic::io(
                "SPX-F106",
                format!("interpreter returned an invalid execution envelope: {error}"),
            )],
            false,
        )
    })?;
    let outcome = &parsed["payload"]["outcome"];
    match outcome["kind"].as_str() {
        Some("returned") => {
            println!("{}", outcome["value"].as_str().unwrap_or(""));
            Ok(())
        }
        Some("failed") => {
            let status = &outcome["status"];
            eprintln!(
                "single-file execution failed with language status {}/{}/{}",
                status["schema"].as_str().unwrap_or("semaprax.status.v1"),
                status["domain_id"].as_str().unwrap_or("unknown"),
                status["code"].as_u64().unwrap_or(0)
            );
            Err(1)
        }
        Some("fuel_exhausted") => {
            eprintln!("single-file execution exhausted its step budget");
            Err(1)
        }
        Some("call_depth_exceeded") => {
            eprintln!(
                "single-file execution exceeded the {}-frame call-depth limit",
                interpreter::MAX_CALL_DEPTH
            );
            Err(1)
        }
        _ => Err(report(
            &[Diagnostic::io(
                "SPX-F106",
                "interpreter envelope has an unknown outcome",
            )],
            false,
        )),
    }
}

fn publish_interpreted_stdout(
    hosted: hosted_interpreter::HostedStdoutTranscript,
    options: &interpreter::InterpreterOptions,
    json: bool,
) -> Result<(), u8> {
    use interpreter::ResolvedEvaluationOutcome;

    if json {
        let outcome = match &hosted.evaluation.outcome {
            ResolvedEvaluationOutcome::ReturnedI64(value) => {
                format!("{{\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"{value}\"}}")
            }
            ResolvedEvaluationOutcome::LanguageFailure(status) => {
                format!("{{\"kind\":\"failed\",\"status\":{}}}", status.to_json())
            }
            ResolvedEvaluationOutcome::FuelExhausted => "{\"kind\":\"fuel_exhausted\"}".to_owned(),
            ResolvedEvaluationOutcome::CallDepthExceeded => {
                "{\"kind\":\"call_depth_exceeded\"}".to_owned()
            }
            ResolvedEvaluationOutcome::GuardError(detail) => {
                return Err(report(&[Diagnostic::io("SPX-F105", detail)], true));
            }
        };
        let stdout = serde_json::to_string(&hosted.transcript).expect("bytes serialize");
        let envelope = format!(
            "{{\"schema\":\"semaprax.single-file-run.v1\",\"fuel\":{{\"steps_used\":{},\"max_steps\":{}}},\"outcome\":{outcome},\"stdout\":{stdout}}}",
            hosted.evaluation.steps_used, hosted.evaluation.max_steps
        );
        if envelope.len() > options.max_bytes {
            return Err(report(
                &[Diagnostic::io(
                    "SPX-F104",
                    "single-file run output exceeds the max-bytes budget; refusing to truncate",
                )],
                true,
            ));
        }
        println!("{envelope}");
    }
    match hosted.evaluation.outcome {
        ResolvedEvaluationOutcome::ReturnedI64(value) => {
            if !json {
                std::io::stdout()
                    .write_all(&hosted.transcript)
                    .map_err(|error| {
                        report(
                            &[Diagnostic::io(
                                "SPX-I101",
                                format!("cannot write stdout: {error}"),
                            )],
                            false,
                        )
                    })?;
                println!("{value}");
            }
            Ok(())
        }
        ResolvedEvaluationOutcome::LanguageFailure(status) => {
            if !json {
                eprintln!(
                    "single-file execution failed with language status {}",
                    status.to_json()
                );
            }
            Err(1)
        }
        ResolvedEvaluationOutcome::FuelExhausted => {
            if !json {
                eprintln!("single-file execution exhausted its step budget");
            }
            Err(1)
        }
        ResolvedEvaluationOutcome::CallDepthExceeded => {
            if !json {
                eprintln!(
                    "single-file execution exceeded the {}-frame call-depth limit",
                    interpreter::MAX_CALL_DEPTH
                );
            }
            Err(1)
        }
        ResolvedEvaluationOutcome::GuardError(detail) => {
            Err(report(&[Diagnostic::io("SPX-F105", detail)], json))
        }
    }
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

fn print_help(has_private_host: bool) {
    print!("{}", global_help(has_private_host));
}

fn print_scoped_help(command: &str, has_private_host: bool) -> Result<(), u8> {
    let help = global_help(has_private_host);
    if let Some(scoped) = cli::help::scoped(command, has_private_host) {
        print!("{scoped}");
        Ok(())
    } else {
        eprint!(
            "{}",
            cli::help::unknown_diagnostic(command, has_private_host)
        );
        print!("{help}");
        Err(2)
    }
}

fn global_help(has_private_host: bool) -> String {
    cli::help::global(has_private_host)
}
