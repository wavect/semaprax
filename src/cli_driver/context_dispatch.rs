//! Bounded source, Project, and prepared Rust API context dispatch.

use super::*;

pub(super) fn run(args: &[String]) -> Result<(), u8> {
    let path = cli::project::resolve_positional(required_path(args, 1)?);
    let symbol = args.get(2).ok_or_else(|| {
        eprintln!("context requires a symbol name or stable id");
        2
    })?;
    let (context_args, rust_index) = cli::context::split_rust_index_option(args)?;
    let options = context_options(&context_args)?;
    if let Some(context) = cli::context::project(
        &path,
        symbol,
        &context_args[3..],
        &options,
        rust_index.as_deref().map(Path::new),
        |errors| report(errors, false),
    )? {
        println!("{context}");
        return Ok(());
    }
    let program = checked(&path)?;
    let context = match &options {
        ParsedContextOptions::V1(options) => graph::agent_context_json(&program, symbol, options),
        ParsedContextOptions::V2(options) => {
            graph::agent_context_v2_json(&program, symbol, options)
        }
    }
    .map_err(|errors| report(&errors, false))?
    .ok_or_else(|| {
        report(
            &[
                Diagnostic::io("SPX-G404", format!("symbol `{symbol}` was not found"))
                    .at_path(path.display().to_string())
                    .with_help(
                        "inspect available declaration identities with `semaprax graph <file>`",
                    ),
            ],
            false,
        )
    })?;
    println!("{context}");
    Ok(())
}
