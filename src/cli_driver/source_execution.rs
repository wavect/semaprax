//! Single-file source checking, build, execution, and diagnostic publication.

use super::*;

pub(super) fn checked(path: &Path) -> Result<semaprax::ast::Program, u8> {
    checked_for_output(path, false)
}

pub(super) fn checked_for_output(path: &Path, json: bool) -> Result<semaprax::ast::Program, u8> {
    let program = load(path).map_err(|errors| report(&errors, json))?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        Err(report(&diagnostics, json))
    } else {
        Ok(program)
    }
}

pub(super) fn load(path: &Path) -> Result<semaprax::ast::Program, Vec<Diagnostic>> {
    let source = std::fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    parse(&source, path).map_err(|error| vec![error])
}

pub(super) fn required_path(args: &[String], index: usize) -> Result<PathBuf, u8> {
    args.get(index).map(PathBuf::from).ok_or_else(|| {
        eprintln!("missing file path; run `semaprax help` for usage");
        2
    })
}

pub(super) fn build_source(options: &cli::build::BuildOptions, input: &Path) -> Result<(), u8> {
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

pub(super) fn report_source_build_success(
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

pub(super) fn run_native_source(path: &Path) -> Result<(), u8> {
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

pub(super) fn run_interpreted_source(
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
    // Preliminary loading and verification publish through the requested
    // diagnostic mode so that `run --json` never falls back to human text.
    let program = checked_for_output(path, options.json)?;
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

pub(super) fn run_network_project(options: &cli::execution::NetworkRunOptions) -> Result<(), u8> {
    use interpreter::CommandEvaluationOutcome;

    const MAX_COMMAND_INPUT_BYTES: usize = 65_536;
    let fixture = read_bounded_file(
        &options.fixture_path,
        semaprax::network_provider::MAX_NETWORK_FIXTURE_BYTES,
        "network fixture",
    )?;
    let fixture = String::from_utf8(fixture).map_err(|_| {
        report(
            &[Diagnostic::io("SPX-F110", "network fixture is not UTF-8")],
            false,
        )
    })?;
    let mut provider = semaprax::network_provider::FixtureNetworkProvider::from_json(&fixture)
        .map_err(|error| report(&[error], false))?;
    let stdin = match &options.stdin_path {
        Some(path) => read_bounded_file(path, MAX_COMMAND_INPUT_BYTES, "network stdin")?,
        None => Vec::new(),
    };
    let argument_bytes = options
        .arguments
        .iter()
        .try_fold(0usize, |total, argument| {
            total
                .checked_add(argument.len())
                .filter(|sum| *sum <= MAX_COMMAND_INPUT_BYTES)
        });
    if argument_bytes
        .and_then(|total| total.checked_add(stdin.len()))
        .is_none_or(|total| total > MAX_COMMAND_INPUT_BYTES)
    {
        return Err(report(
            &[Diagnostic::io(
                "SPX-F111",
                "network command argv and stdin exceed the 65536-byte invocation limit",
            )],
            false,
        ));
    }
    let input = hosted_interpreter::HostedCommandInput {
        arguments: options.arguments.clone(),
        stdin,
    };
    let max_steps = options
        .max_steps
        .unwrap_or_else(|| interpreter::InterpreterOptions::default().max_steps);
    let result = project::with_authenticated_project(&options.manifest_path, |snapshot| {
        snapshot.execute_network_command(&input, &mut provider, max_steps)
    })
    .map_err(|errors| report(&errors, false))?;

    std::io::stdout()
        .write_all(&result.stdout)
        .map_err(|error| {
            report(
                &[Diagnostic::io(
                    "SPX-I101",
                    format!("cannot write stdout: {error}"),
                )],
                false,
            )
        })?;
    std::io::stderr()
        .write_all(&result.stderr)
        .map_err(|error| {
            report(
                &[Diagnostic::io(
                    "SPX-I101",
                    format!("cannot write stderr: {error}"),
                )],
                false,
            )
        })?;
    match result.evaluation.outcome {
        CommandEvaluationOutcome::ReturnedBool(true) => Ok(()),
        CommandEvaluationOutcome::ReturnedBool(false) => Err(1),
        CommandEvaluationOutcome::LanguageFailure(status) => {
            eprintln!(
                "network command failed with language status {}",
                status.to_json()
            );
            Err(1)
        }
        CommandEvaluationOutcome::FuelExhausted => {
            eprintln!("network command exhausted its step budget");
            Err(1)
        }
        CommandEvaluationOutcome::CallDepthExceeded => {
            eprintln!("network command exceeded the call-depth limit");
            Err(1)
        }
        CommandEvaluationOutcome::GuardError(detail) => {
            Err(report(&[Diagnostic::io("SPX-F105", detail)], false))
        }
    }
}

fn read_bounded_file(path: &Path, max_bytes: usize, label: &str) -> Result<Vec<u8>, u8> {
    let file = std::fs::File::open(path).map_err(|error| {
        report(
            &[Diagnostic::io(
                "SPX-I001",
                format!("cannot read {label} {}: {error}", path.display()),
            )],
            false,
        )
    })?;
    let mut bytes = Vec::with_capacity(max_bytes.min(64 * 1024));
    file.take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            report(
                &[Diagnostic::io(
                    "SPX-I001",
                    format!("cannot read {label} {}: {error}", path.display()),
                )],
                false,
            )
        })?;
    if bytes.len() > max_bytes {
        return Err(report(
            &[Diagnostic::io(
                "SPX-F111",
                format!("{label} exceeds the {max_bytes}-byte limit"),
            )],
            false,
        ));
    }
    Ok(bytes)
}

pub(super) fn publish_interpretation(envelope: &str) -> Result<(), u8> {
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

pub(super) fn publish_interpreted_stdout(
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

pub(super) fn report(errors: &[Diagnostic], json: bool) -> u8 {
    report_all(errors, json);
    1
}

pub(super) fn report_all(errors: &[Diagnostic], json: bool) {
    for error in errors {
        if json {
            println!("{}", error.json());
        } else {
            eprintln!("{error}");
        }
    }
}
