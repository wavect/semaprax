//! Shared exact source for the first admitted private ASCII matcher witness.

const ENGINE: &str = include_str!("../../experiments/ascii-pattern-source/ascii.spx");

fn bytes(name: &str, value: &[u8]) -> String {
    format!(
        "let {name}: [u8; {}] = [{}];\nlet {name}_view = array_as_slice({name});\n",
        value.len(),
        value
            .iter()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The first source admitted by `private_ascii_pattern_compile_and_greedy_capture_witnesses`.
pub fn first_witness_source() -> String {
    let expected =
        "if status(matcher) == 1usize && capture_count(matcher) == 2usize && read(matcher, 2703usize) == 0usize && load8(matcher, 2720usize) == 0usize && capture_start(matcher, 0usize) == 0usize && capture_end(matcher, 0usize) == 2usize && capture_start(matcher, 1usize) == 2usize && capture_end(matcher, 1usize) == 2usize { 1 } else { -1 }";
    format!(
        "{ENGINE}\n@id(\"experiment.pattern.witness.main\")\nfn main() -> i64\n{{\n{}{}\nlet storage = bytes_zeroed(3072usize);\nlet initial = matcher_from_bytes(storage);\nlet compiled = compile(initial, pattern_view, 8192usize);\nlet ready = status(compiled) == 0usize;\nlet mut matcher = compiled;\nlet mut iteration = 0usize;\nwhile iteration < 1usize {{\nmatcher = full_match(matcher, input_view, 8192usize);\niteration = iteration + 1usize;\n0\n}}\nif ready {{ {expected} }} else {{ -2 }}\n}}\n",
        bytes("pattern", b"(a*)(a*)"),
        bytes("input", b"aa")
    )
}
