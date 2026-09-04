//! The `new` invocation grammar shared by both executables.
//!
//! `semaprax new <destination> [--name project-name] [--template calculator|library]`.
//! The full toolchain parses the same grammar inside its private publication
//! module and spells every shared rejection identically; it publishes only the
//! calculator template through its held-parent authority, and says so when
//! asked for another one.

use std::path::PathBuf;

use semaprax::project::{PROJECT_SCAFFOLD_TEMPLATES, PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewOptions {
    pub(crate) destination: PathBuf,
    pub(crate) name: String,
    pub(crate) template: &'static str,
}

pub(crate) fn parse(arguments: &[String]) -> Result<NewOptions, String> {
    let mut destination = None::<PathBuf>;
    let mut explicit_name = None::<String>;
    let mut template = None::<&'static str>;
    let mut index = 0usize;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--name" if explicit_name.is_none() => {
                explicit_name = Some(option_value(arguments, index, "--name")?.to_owned());
                index += 2;
            }
            "--name" => return Err("duplicate new option `--name`".to_owned()),
            "--template" if template.is_none() => {
                let value = option_value(arguments, index, "--template")?;
                template = Some(known_template(value)?);
                index += 2;
            }
            "--template" => return Err("duplicate new option `--template`".to_owned()),
            option if option.starts_with('-') => {
                return Err(format!("unknown new option `{option}`"));
            }
            path if destination.is_none() => {
                destination = Some(PathBuf::from(path));
                index += 1;
            }
            _ => return Err("new accepts exactly one destination".to_owned()),
        }
    }
    let destination = destination.ok_or_else(|| "new requires one destination".to_owned())?;
    let name = match explicit_name {
        Some(name) => name,
        None => destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                "new destination requires --name when its final component is not UTF-8".to_owned()
            })?
            .to_owned(),
    };
    validate_name(&name)?;
    Ok(NewOptions {
        destination,
        name,
        template: template.unwrap_or(PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR),
    })
}

fn known_template(value: &str) -> Result<&'static str, String> {
    PROJECT_SCAFFOLD_TEMPLATES
        .into_iter()
        .find(|known| *known == value)
        .ok_or_else(|| {
            format!(
                "unknown new template `{value}`; expected {}",
                PROJECT_SCAFFOLD_TEMPLATES.join(" or ")
            )
        })
}

fn option_value<'a>(
    arguments: &'a [String],
    index: usize,
    option: &str,
) -> Result<&'a str, String> {
    arguments
        .get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .map(String::as_str)
        .ok_or_else(|| format!("new option `{option}` requires a value"))
}

fn validate_name(name: &str) -> Result<(), String> {
    if (1..=64).contains(&name.len())
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Ok(())
    } else {
        Err("project name must match lowercase [a-z][a-z0-9-]* and be at most 64 bytes".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn grammar_is_closed_and_names_default_to_the_destination_leaf() {
        assert_eq!(
            parse(&strings(&["apps/first-semaprax"])).unwrap(),
            NewOptions {
                destination: PathBuf::from("apps/first-semaprax"),
                name: "first-semaprax".to_owned(),
                template: "calculator",
            }
        );
        assert_eq!(
            parse(&strings(&[
                "out",
                "--name",
                "demo",
                "--template",
                "calculator"
            ]))
            .unwrap()
            .name,
            "demo"
        );
        let library = parse(&strings(&["lib", "--template", "library"])).unwrap();
        assert_eq!(library.template, "library");
        assert_eq!(library.name, "lib");
        assert_eq!(
            parse(&strings(&[
                "x",
                "--template",
                "library",
                "--template",
                "library"
            ]))
            .unwrap_err(),
            "duplicate new option `--template`"
        );
        assert_eq!(
            parse(&strings(&[])).unwrap_err(),
            "new requires one destination"
        );
        assert_eq!(
            parse(&strings(&["a", "b"])).unwrap_err(),
            "new accepts exactly one destination"
        );
        assert_eq!(
            parse(&strings(&["Bad_Name"])).unwrap_err(),
            "project name must match lowercase [a-z][a-z0-9-]* and be at most 64 bytes"
        );
        assert_eq!(
            parse(&strings(&["x", "--template", "web"])).unwrap_err(),
            "unknown new template `web`; expected calculator or library"
        );
        assert_eq!(
            parse(&strings(&["x", "--name"])).unwrap_err(),
            "new option `--name` requires a value"
        );
        assert_eq!(
            parse(&strings(&["x", "--name", "a", "--name", "b"])).unwrap_err(),
            "duplicate new option `--name`"
        );
        assert_eq!(
            parse(&strings(&["x", "--bogus"])).unwrap_err(),
            "unknown new option `--bogus`"
        );
    }
}
