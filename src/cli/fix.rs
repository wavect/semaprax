//! `semaprax fix`: authority-free installed and current-source fix planning.

use std::path::PathBuf;

use semaprax::diagnostic::Diagnostic;
use semaprax::installed_fix_plan::{
    current_source_fix_plan, installed_fix_plan_catalog, FixPlanRequest,
};

pub(crate) enum FixCommand {
    Catalog,
    CurrentSource {
        source: PathBuf,
        automatic_function_id: String,
    },
}

pub(crate) fn parse(args: &[String]) -> Result<FixCommand, u8> {
    match args {
        [option] if option == "--plan" => Ok(FixCommand::Catalog),
        [source, operation, target, option]
            if !source.is_empty()
                && !source.starts_with('-')
                && operation == "assign-function-id"
                && !target.is_empty()
                && !target.starts_with('-')
                && option == "--plan" =>
        {
            Ok(FixCommand::CurrentSource {
                source: PathBuf::from(source),
                automatic_function_id: target.to_owned(),
            })
        }
        _ => usage(),
    }
}

pub(crate) fn run(command: FixCommand, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let output = match command {
        FixCommand::Catalog => installed_fix_plan_catalog()
            .map(|catalog| catalog.to_json().to_owned())
            .map_err(|errors| report(&errors))?,
        FixCommand::CurrentSource {
            source,
            automatic_function_id,
        } => {
            let request = FixPlanRequest::assign_function_id(automatic_function_id)
                .map_err(|errors| report(&errors))?;
            current_source_fix_plan(&source, &request)
                .map(|plan| plan.to_json().to_owned())
                .map_err(|errors| report(&errors))?
        }
    };
    print!("{output}");
    Ok(())
}

fn usage<T>() -> Result<T, u8> {
    eprintln!(
        "fix requires exactly --plan or <file> assign-function-id <automatic-function-id> --plan"
    );
    Err(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn grammar_is_closed() {
        assert!(matches!(
            parse(&strings(&["--plan"])).unwrap(),
            FixCommand::Catalog
        ));
        let FixCommand::CurrentSource {
            source,
            automatic_function_id,
        } = parse(&strings(&[
            "module.spx",
            "assign-function-id",
            "automatic:0",
            "--plan",
        ]))
        .unwrap()
        else {
            panic!("current-source grammar selected the catalog");
        };
        assert_eq!(source, PathBuf::from("module.spx"));
        assert_eq!(automatic_function_id, "automatic:0");

        for malformed in [
            &[][..],
            &["module.spx", "--plan"][..],
            &["--plan", "module.spx"][..],
            &["--unknown"][..],
            &["module.spx", "assign-function-id", "automatic:0"][..],
            &[
                "module.spx",
                "assign-function-id",
                "automatic:0",
                "--unknown",
            ][..],
            &["module.spx", "other", "automatic:0", "--plan"][..],
            &["", "assign-function-id", "automatic:0", "--plan"][..],
            &["--source", "assign-function-id", "automatic:0", "--plan"][..],
            &["module.spx", "assign-function-id", "--target", "--plan"][..],
            &[
                "module.spx",
                "assign-function-id",
                "automatic:0",
                "--plan",
                "extra",
            ][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
