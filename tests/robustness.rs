use std::path::Path;

use semaprax::{format, graph, parse, verify};

#[test]
fn deterministic_source_mutations_never_escape_parser_or_canonicalizer() {
    let seed = r#"module fuzz.seed;
@id("flow.choose")
fn choose(flag: bool, value: i64) -> i64 {
    let next = value + 1;
    if flag { next } else { 0 }
}
@id("app.main")
fn main() -> i64 { choose(true, 41) }
"#;
    let replacements = *b"{};=@!()";
    let mut cases = Vec::new();
    for index in 0..seed.len() {
        for replacement in replacements {
            let mut bytes = seed.as_bytes().to_vec();
            bytes[index] = replacement;
            cases.push(String::from_utf8(bytes).unwrap());
        }
    }
    for end in (0..seed.len()).step_by(3) {
        cases.push(seed[..end].to_owned());
    }

    for (index, candidate) in cases.into_iter().enumerate() {
        let path = format!("mutation-{index}.spx");
        if let Ok(program) = parse(&candidate, Path::new(&path)) {
            let canonical = format::canonical(&program);
            let reparsed = parse(&canonical, Path::new("canonical-mutation.spx")).unwrap();
            assert_eq!(graph::revision(&program), graph::revision(&reparsed));
            let first_codes = verify::verify(&program)
                .into_iter()
                .map(|item| item.code)
                .collect::<Vec<_>>();
            let second_codes = verify::verify(&reparsed)
                .into_iter()
                .map(|item| item.code)
                .collect::<Vec<_>>();
            assert_eq!(first_codes, second_codes);
        }
    }
}
