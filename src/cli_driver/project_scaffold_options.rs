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
