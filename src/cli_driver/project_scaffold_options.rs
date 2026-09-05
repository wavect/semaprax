//! The closed option grammar of `semaprax project-scaffold`.

use semaprax::project;

pub(super) fn parse(arguments: &[String]) -> Result<(&str, &str, project::ScaffoldLayout), u8> {
    let mut name = None;
    let mut template = None;
    let mut layout = None;
    let mut index = 0usize;
    while index < arguments.len() {
        let option = arguments[index].as_str();
        if !matches!(option, "--name" | "--template" | "--layout") {
            eprintln!("project-scaffold accepts only --name, --template, and --layout");
            return Err(2);
        }
        let value = arguments
            .get(index + 1)
            .filter(|value| !value.starts_with('-'))
            .ok_or_else(|| {
                eprintln!("project-scaffold option requires one value");
                2
            })?;
        match option {
            "--name" if name.is_none() => name = Some(value.as_str()),
            "--template" if template.is_none() => {
                if !project::PROJECT_SCAFFOLD_TEMPLATES.contains(&value.as_str()) {
                    eprintln!("project-scaffold template must be calculator or library");
                    return Err(2);
                }
                template = Some(value.as_str());
            }
            "--layout" if layout.is_none() => {
                layout = Some(match value.as_str() {
                    "frozen" => project::ScaffoldLayout::Frozen,
                    "tables" => project::ScaffoldLayout::Tables,
                    _ => {
                        eprintln!("project-scaffold layout must be frozen or tables");
                        return Err(2);
                    }
                });
            }
            "--name" => {
                eprintln!("duplicate project-scaffold option --name");
                return Err(2);
            }
            "--template" => {
                eprintln!("duplicate project-scaffold option --template");
                return Err(2);
            }
            "--layout" => {
                eprintln!("duplicate project-scaffold option --layout");
                return Err(2);
            }
            _ => unreachable!("closed project-scaffold option grammar"),
        }
        index += 2;
    }
    let name = name.ok_or_else(|| {
        eprintln!("project-scaffold requires --name project-name");
        2
    })?;
    Ok((
        name,
        template.unwrap_or(project::PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR),
        layout.unwrap_or(project::ScaffoldLayout::Frozen),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|token| (*token).to_owned()).collect()
    }

    #[test]
    fn name_is_required_and_the_other_two_options_have_defaults() {
        let arguments = argv(&["--name", "demo"]);
        assert_eq!(
            parse(&arguments).unwrap(),
            (
                "demo",
                project::PROJECT_SCAFFOLD_TEMPLATE_CALCULATOR,
                project::ScaffoldLayout::Frozen
            )
        );
        assert_eq!(parse(&[]).unwrap_err(), 2);
        assert_eq!(parse(&argv(&["--template", "library"])).unwrap_err(), 2);
    }

    #[test]
    fn every_admitted_template_and_layout_value_is_stored() {
        for template in project::PROJECT_SCAFFOLD_TEMPLATES {
            let arguments = argv(&["--name", "demo", "--template", template]);
            assert_eq!(parse(&arguments).unwrap().1, template);
        }
        for (spelling, expected) in [
            ("frozen", project::ScaffoldLayout::Frozen),
            ("tables", project::ScaffoldLayout::Tables),
        ] {
            let arguments = argv(&["--name", "demo", "--layout", spelling]);
            assert_eq!(parse(&arguments).unwrap().2, expected);
        }
    }

    #[test]
    fn option_order_does_not_change_the_parse() {
        let forward = argv(&[
            "--name",
            "demo",
            "--template",
            "library",
            "--layout",
            "tables",
        ]);
        let reverse = argv(&[
            "--layout",
            "tables",
            "--template",
            "library",
            "--name",
            "demo",
        ]);
        assert_eq!(parse(&forward).unwrap(), parse(&reverse).unwrap());
    }

    #[test]
    fn a_repeated_option_is_an_error_rather_than_last_or_first_wins() {
        for option in ["--name", "--template", "--layout"] {
            let value = match option {
                "--template" => "library",
                "--layout" => "tables",
                _ => "demo",
            };
            let arguments = argv(&["--name", "demo", option, value, option, value]);
            assert_eq!(parse(&arguments).unwrap_err(), 2, "{option}");
        }
    }

    #[test]
    fn a_value_taking_option_never_swallows_the_following_flag() {
        // Unlike the option grammars in `options.rs`, this parser filters a
        // candidate value that starts with `-`, so a missing value is a
        // diagnostic instead of a silently adopted flag name.
        assert_eq!(
            parse(&argv(&["--name", "--template", "library"])).unwrap_err(),
            2
        );
        assert_eq!(parse(&argv(&["--name", "-demo"])).unwrap_err(), 2);
        // The same filter also rejects a value-less option at the very end.
        assert_eq!(parse(&argv(&["--name"])).unwrap_err(), 2);
        assert_eq!(
            parse(&argv(&["--name", "demo", "--layout"])).unwrap_err(),
            2
        );
    }

    #[test]
    fn unknown_flags_and_unexpected_positionals_are_refused() {
        assert_eq!(parse(&argv(&["demo"])).unwrap_err(), 2);
        assert_eq!(parse(&argv(&["--name", "demo", "extra"])).unwrap_err(), 2);
        assert_eq!(parse(&argv(&["--verbose", "1"])).unwrap_err(), 2);
        assert_eq!(parse(&argv(&["-n", "demo"])).unwrap_err(), 2);
    }

    #[test]
    fn template_and_layout_vocabularies_are_closed_and_case_sensitive() {
        for rejected in ["Library", "app", "", "calculator,library"] {
            assert_eq!(
                parse(&argv(&["--name", "demo", "--template", rejected])).unwrap_err(),
                2,
                "{rejected}"
            );
        }
        for rejected in ["Frozen", "table", "", "frozen,tables"] {
            assert_eq!(
                parse(&argv(&["--name", "demo", "--layout", rejected])).unwrap_err(),
                2,
                "{rejected}"
            );
        }
    }
}
