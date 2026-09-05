//! `semaprax skills`: installed, authority-free agent guidance.

use semaprax::diagnostic::Diagnostic;
use semaprax::installed_guidance::{installed_skill, InstalledSkill};

pub(crate) struct SkillsGet {
    skill: InstalledSkill,
}

pub(crate) fn parse(args: &[String]) -> Result<SkillsGet, u8> {
    let [command, skill] = args else {
        return usage();
    };
    if command != "get" {
        return usage();
    }
    let skill = InstalledSkill::parse(skill).ok_or_else(|| {
        eprintln!(
            "unknown installed skill `{skill}`; expected agent, language, graph, stdlib, packages, or effects"
        );
        2
    })?;
    Ok(SkillsGet { skill })
}

pub(crate) fn run(request: SkillsGet, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let guidance = installed_skill(request.skill).map_err(|errors| report(&errors))?;
    print!("{}", guidance.to_json());
    Ok(())
}

fn usage<T>() -> Result<T, u8> {
    eprintln!("skills requires exactly get <agent|language|graph|stdlib|packages|effects>");
    Err(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn installed_skill_grammar_is_closed() {
        for skill in InstalledSkill::ALL {
            assert!(parse(&strings(&["get", skill.as_str()])).is_ok());
        }
        for malformed in [
            &[][..],
            &["get"][..],
            &["list", "agent"][..],
            &["get", "unknown"][..],
            &["get", "agent", "extra"][..],
        ] {
            assert!(parse(&strings(malformed)).is_err(), "{malformed:?}");
        }
    }
}
