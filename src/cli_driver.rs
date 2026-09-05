#![allow(
    clippy::result_large_err,
    reason = "the CLI preserves structured Diagnostic values across command boundaries"
)]

use std::io::{Read as _, Write as _};
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
#[path = "cli_driver/options.rs"]
mod options;
#[path = "cli_driver/project_scaffold_options.rs"]
mod project_scaffold_options;
#[path = "cli_driver/report_options.rs"]
mod report_options;
#[path = "cli_driver/source_execution.rs"]
mod source_execution;
#[path = "cli_driver/supply_chain.rs"]
mod supply_chain;

#[cfg(test)]
#[path = "cli/native_output_tests.rs"]
mod native_output_tests;

use options::*;
use report_options::*;
use source_execution::*;

#[cfg(test)]
#[path = "cli/native_scratch_tests.rs"]
mod native_scratch_tests;

/// Explicit private-host hooks supplied only by the unpublished toolchain.
/// Creates a project and returns the destination as spelled plus the
/// template it published.
pub type NewProjectHook = fn(&[String]) -> Result<(PathBuf, &'static str), (String, u8)>;

pub struct PrivateHost {
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
    if let Some(outcome) = cli::help::dispatch(&args, host.is_some()) {
        return outcome;
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
            let command = cli::query::parse_command(&args[1..])?;
            cli::query::run_command(command, |errors| report(errors, false))
        }
        CommandId::Change => {
            let preview = cli::change::parse(&args[1..])?;
            cli::change::run(preview, |errors| report(errors, false))
        }
        CommandId::Package => {
            let rewritten = cli::package::long_form(&args[1..])?;
            run(rewritten, host)
        }
        CommandId::Add => {
            let options = cli::add::parse(&args[1..])?;
            cli::add::run(&options, |errors| report(errors, false))
        }
        CommandId::Fetch => {
            let options = cli::fetch::parse(&args[1..])?;
            let receipt = cli::fetch::run(&options).map_err(|errors| report(&errors, false))?;
            print!("{receipt}");
            Ok(())
        }
        CommandId::Agent => {
            let command = cli::agent::parse(&args[1..])?;
            let output = cli::agent::run(&command).map_err(|errors| report(&errors, false))?;
            print!("{output}");
            Ok(())
        }
        CommandId::Context => {
            let path = cli::project::resolve_positional(required_path(&args, 1)?);
            let symbol = args.get(2).ok_or_else(|| {
                eprintln!("context requires a symbol name or stable id");
                2
            })?;
            let options = context_options(&args)?;
            if let Some(context) =
                cli::context::project(&path, symbol, &args[3..], &options, |errors| {
                    report(errors, false)
                })?
            {
                println!("{context}");
                return Ok(());
            }
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
            let outcome = semaprax::doctor::run(&args[1..]).map_err(|error| {
                eprintln!("doctor: {error}");
                2
            })?;
            let (output, exit_code) = (outcome.output, outcome.exit_code);
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
        CommandId::NetworkRun => {
            let options = cli::execution::parse_network_run(&args[1..])?;
            run_network_project(&options)
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

fn print_help(has_private_host: bool) {
    print!("{}", cli::help::global(has_private_host));
}

fn print_scoped_help(command: &str, has_private_host: bool) -> Result<(), u8> {
    let help = cli::help::global(has_private_host);
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
