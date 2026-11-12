//! Public-package acceptance, independent of the pattern compiler/matcher.
use super::super::{compile_c, interpreter_i64, run_and_capture_i64, temporary, wasm_host};
use semaprax::{codegen, project, wasm};
use std::{fs, path::Path, process::Command};

#[derive(Clone, Copy)]
enum Grammar {
    Header,
    Kv,
}

impl Grammar {
    fn pattern(self) -> &'static [u8] {
        match self {
            Self::Header => b"([A-Za-z-]+):[ \\x09]*([^\\x0D\\x0A]*)",
            Self::Kv => b"([A-Za-z_][A-Za-z0-9_]*)=([^\\x0D\\x0A]*)",
        }
    }
    fn separator(self) -> u8 {
        match self {
            Self::Header => b':',
            Self::Kv => b'=',
        }
    }
    // Direct grammar interpretation. No regex library, matcher table, or
    // source-engine parser participates in the expected capture endpoints.
    fn captures(self, input: &[u8]) -> Option<[[usize; 2]; 2]> {
        let split = input.iter().position(|byte| *byte == self.separator())?;
        if split == 0 {
            return None;
        }
        let key = &input[..split];
        let valid = match self {
            Self::Header => key
                .iter()
                .all(|byte| byte.is_ascii_alphabetic() || *byte == b'-'),
            Self::Kv => {
                (key[0].is_ascii_alphabetic() || key[0] == b'_')
                    && key
                        .iter()
                        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            }
        };
        if !valid
            || input[split + 1..]
                .iter()
                .any(|byte| matches!(byte, b'\r' | b'\n'))
        {
            return None;
        }
        let mut start = split + 1;
        if matches!(self, Self::Header) {
            while start < input.len() && matches!(input[start], b' ' | b'\t') {
                start += 1;
            }
        }
        Some([[0, split], [start, input.len()]])
    }
}
fn checksum(spans: [[usize; 2]; 2]) -> i64 {
    (202 + spans[0][0] * 11 + spans[0][1] * 13 + spans[1][0] * 17 + spans[1][1] * 19) as i64
}
fn array(bytes: &[u8]) -> String {
    format!(
        "[{}]",
        bytes
            .iter()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn prefix(pattern: &[u8]) -> String {
    let imports = include_str!("../pattern_examples.spx");
    let imports = imports.split("// Compile once").next().unwrap();
    format!(
        "{imports}\n@id(\"fixture.pattern.examples.main\")\nfn main() -> i64 {{\nlet pattern = {}; let pattern_view = array_as_slice(pattern);\nlet initial = make(); let mut matcher: Matcher = compile(initial, pattern_view, 262144usize);\nlet ready = result_valid(matcher) && status(matcher) == 0usize;\n",
        array(pattern)
    )
}
fn observed() -> &'static str {
    "i64_from_usize(capture_count(matcher) * 101usize + capture_start(matcher, 0usize) * 11usize + capture_end(matcher, 0usize) * 13usize + capture_start(matcher, 1usize) * 17usize + capture_end(matcher, 1usize) * 19usize)"
}
fn packet(expected_match: &str, capture_check: &str) -> String {
    format!(
        "result_valid(matcher) && reason(matcher) == 0usize && detail(matcher) == 0usize && detail_domain(matcher) == 0usize && work_used(matcher) <= 262144usize && if {expected_match} {{ status(matcher) == 1usize && capture_count(matcher) == 2usize && {capture_check} }} else {{ status(matcher) == 2usize && capture_count(matcher) == 0usize }}"
    )
}
fn ascii_source(grammar: Grammar, key: bool) -> (String, i64) {
    let inputs: Vec<_> = (0u8..=127)
        .map(|byte| {
            if key {
                [byte, grammar.separator(), b'x']
            } else {
                [b'A', grammar.separator(), byte]
            }
        })
        .collect();
    assert_eq!(inputs.len(), 128);
    let expectations: Vec<_> = inputs
        .iter()
        .map(|input| u8::from(grammar.captures(input).is_some()))
        .collect();
    assert_eq!(
        expectations
            .iter()
            .map(|value| usize::from(*value))
            .sum::<usize>(),
        if key { 53 } else { 126 }
    );
    let starts: Vec<_> = inputs
        .iter()
        .map(|input| {
            grammar
                .captures(input)
                .map(|spans| spans[1][0] as u8)
                .unwrap_or(0)
        })
        .collect();
    if matches!(grammar, Grammar::Header) && !key {
        assert_eq!(starts[usize::from(b' ')], 3);
        assert_eq!(starts[usize::from(b'\t')], 3);
    }
    let expected = inputs
        .iter()
        .filter_map(|input| grammar.captures(input))
        .map(checksum)
        .sum();
    // Fixed arrays admit exact u8 literals. Keep all 128 three-byte inputs
    // in one static table, then borrow the current case without allocating.
    let cases: Vec<u8> = inputs
        .iter()
        .flat_map(|input| input.iter().copied())
        .collect();
    assert_eq!(cases.len(), 128 * 3);
    let packet = packet(
        "expected",
        "capture_start(matcher, 0usize) == 0usize && capture_end(matcher, 0usize) == 1usize && capture_start(matcher, 1usize) == expected_start && capture_end(matcher, 1usize) == 3usize",
    );
    (
        format!(
            "{}\nlet expected_bytes = {}; let expected_view = array_as_slice(expected_bytes);\nlet expected_starts = {}; let starts_view = array_as_slice(expected_starts);\nlet inputs = {}; let inputs_view = array_as_slice(inputs);\nlet mut index = 0usize; let mut correct = true; let mut total = 0;\nwhile index < 128usize {{\nlet offset = index * 3usize; let view = byte_range(inputs_view, offset, offset + 3usize);\nlet expected = match byte_get(expected_view, index) {{ Option::Some {{ value }} => value == 1u8, Option::None {{}} => false, }};\nlet expected_start = match byte_get(starts_view, index) {{ Option::Some {{ value }} => usize_from_u8(value), Option::None {{}} => 0usize, }};\nmatcher = full_match(matcher, view, 262144usize);\nif {packet} {{ 0 }} else {{ correct = false; 0 }}\nif status(matcher) == 1usize {{ total = total + {}; 0 }} else {{ 0 }}\nindex = index + 1usize; 0\n}}\nif ready && correct {{ total }} else {{ -1 }}\n}}\n",
            prefix(grammar.pattern()),
            array(&expectations),
            array(&starts),
            array(&cases),
            observed()
        ),
        expected,
    )
}
fn long_source(grammar: Grammar, matches: bool) -> (String, i64) {
    let mut input = match grammar {
        Grammar::Header => b"X-Key: \t".to_vec(),
        Grammar::Kv => b"field_1=".to_vec(),
    };
    input.extend(vec![b'v'; 8192]);
    if !matches {
        input.push(b'\r');
    }
    let spans = grammar.captures(&input);
    assert_eq!(spans.is_some(), matches);
    let expected = spans.map(checksum).unwrap_or(0);
    let capture_check = spans.map(|spans| format!("capture_start(matcher, 0usize) == {}usize && capture_end(matcher, 0usize) == {}usize && capture_start(matcher, 1usize) == {}usize && capture_end(matcher, 1usize) == {}usize", spans[0][0], spans[0][1], spans[1][0], spans[1][1])).unwrap_or_else(|| "false".into());
    let literal = String::from_utf8(input)
        .unwrap()
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    let packet = packet(if matches { "true" } else { "false" }, &capture_check);
    // Whole-owner renewal is admitted only inside a loop body; one iteration
    // performs the single 8192-byte match, as the while-loop witness does.
    (
        format!(
            "{}\nlet input = \"{literal}\"; let text = string_as_str(input); let view = str_as_bytes(text);\nlet mut renewal = 0usize;\nwhile renewal < 1usize {{\nmatcher = full_match(matcher, view, 262144usize);\nrenewal = renewal + 1usize;\n0\n}}\nif ready && {packet} {{ if status(matcher) == 1usize {{ {} }} else {{ 0 }} }} else {{ -1 }}\n}}\n",
            prefix(grammar.pattern()),
            observed()
        ),
        expected,
    )
}

fn run(scratch: &Path, label: &str, source: &str, expected: i64, live_bound: usize) {
    let directory = scratch.join(label);
    fs::create_dir_all(directory.join("src")).unwrap();
    let parsed = semaprax::parse(source, "fixture.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&canonical, "fixture.spx").unwrap()),
        canonical
    );
    fs::write(directory.join("src/main.spx"), canonical).unwrap();
    // Package Manifest v1 requires a declared test module and two sources.
    // Reuse the ordinary package tests; no execution allowance is changed.
    let tests = semaprax::parse(include_str!("../pattern_tests.spx"), "tests.spx").unwrap();
    fs::write(
        directory.join("src/tests.spx"),
        semaprax::format::canonical(&tests),
    )
    .unwrap();
    fs::write(
        directory.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "pattern-acceptance"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "fixture.pattern.examples"
sources = ["src/main.spx", "src/tests.spx"]
tests = ["fixture.pattern.tests"]

[exports]
web = []

[dependencies]
std.pattern = "=0.1.0"
"#,
    )
    .unwrap();
    project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(options.max_steps, 1_000_000, "ordinary default, never an enlarged allowance");
        let entry = snapshot.execute_entry(&options)?;
        assert_eq!(entry.max_steps(), 1_000_000);
        let actual = interpreter_i64(entry.outcome(), label);
        assert_eq!(actual, expected, "independent ASCII capture oracle: {label}");
        assert!(entry.steps_used() <= 1_000_000);
        let exact = project::ProjectExecutionOptions::new(options.max_bytes, entry.steps_used())
            .map_err(|error| vec![error])?;
        let at_limit = snapshot.execute_entry(&exact)?;
        assert_eq!(at_limit.outcome(), entry.outcome(), "{label}: exact fuel");
        assert_eq!(at_limit.steps_used(), exact.max_steps);
        let one_short = project::ProjectExecutionOptions::new(options.max_bytes, exact.max_steps - 1)
            .map_err(|error| vec![error])?;
        let refused = snapshot.execute_entry(&one_short)?;
        assert_eq!(
            refused.outcome(),
            &project::ProjectExecutionOutcome::FuelExhausted,
            "{label}: one step short"
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        // Rename only the generated wrapper, retaining the exact HIR provider.
        // Observe four real invocations and physical malloc/calloc/free settlement.
        let id: String = "fixture.pattern.examples.main".bytes().map(|byte| format!("{byte:02x}")).collect();
        let probe = format!("{}\n#define FIXTURE_TRACK_CALLOC\n{}\n#define main fixture_generated_main\n{c}\n#undef main\n#undef malloc\n#undef calloc\n#undef free\nint main(void) {{ REQUIRE(fixture_binary_stdout()); struct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL)); int64_t value=INT64_MIN; for(unsigned i=0;i<4;++i) {{ REQUIRE(spx_decl_{id}(&context,&value)==0); REQUIRE(value=={actual}); REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees); }} printf(\"%lld\\n\",(long long)value); return 0; }}\n", include_str!("../../../support/native_fixture_stdio.c"), include_str!("../../../native_owned_utf8_settlement_v1/allocations.c"));
        for optimization in ["-O0", "-O2"] {
            let binary = directory.join(format!("native{optimization}"));
            compile_c(&probe, &binary, optimization);
            assert_eq!(run_and_capture_i64(&binary), actual);
        }
        let bytes = wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let wasm_path = directory.join("fixture.wasm");
        fs::write(&wasm_path, bytes).unwrap();
        fs::write(directory.join("fixture.mjs"), wasm_host::wasm_conformance_js("fixture.wasm", live_bound, actual, Some(live_bound))).unwrap();
        let output = Command::new("node").arg("fixture.mjs").current_dir(&directory).output().unwrap();
        assert!(output.status.success(), "{label}: {}", String::from_utf8_lossy(&output.stderr));
        super::assert_adjacent_refusal(&directory, &wasm_path, live_bound, actual);
        Ok(())
    }).unwrap();
}

#[test]
fn public_capture_work_cuts_and_hostile_repeats_keep_distinct_packets() {
    let scratch = temporary("pattern-work-cuts");
    // The short greedy path charges 128 header, 18 atom validation, 10
    // capture validation, 34 dispatch/frame, 4 scan and 9 terminal units.
    // Publishing two captures adds 90. A refusal publishes only the
    // 32-unit packet instead, with no capture spans.
    let greedy = b"(a*)(a*)".as_slice();
    for (label, pattern, input, limit, check) in [
        (
            "exact",
            greedy,
            b"aa".as_slice(),
            293,
            "status(observed) == 1usize && reason(observed) == 0usize
             && work_used(observed) == 293usize && capture_count(observed) == 2usize
             && capture_start(observed, 0usize) == 0usize && capture_end(observed, 0usize) == 2usize
             && capture_start(observed, 1usize) == 2usize && capture_end(observed, 1usize) == 2usize",
        ),
        (
            "one-short",
            greedy,
            b"aa".as_slice(),
            292,
            "status(observed) == 4usize && reason(observed) == 7usize
             && work_used(observed) == 235usize && capture_count(observed) == 0usize",
        ),
        (
            "hostile-repeats",
            b"a*a*a*a*a*a*a*a*b".as_slice(),
            b"aaaaaaaa".as_slice(),
            4096,
            "status(observed) == 4usize && reason(observed) == 7usize
             && work_used(observed) <= 4096usize && capture_count(observed) == 0usize",
        ),
    ] {
        let source = format!(
            r#"{}
let input = {};
let view = array_as_slice(input);
if ready {{
    let observed = full_match(matcher, view, {limit}usize);
    if result_valid(observed) && detail(observed) == 0usize
        && detail_domain(observed) == 0usize && {check} {{ 1 }} else {{ -1 }}
}} else {{ -2 }}
}}
"#,
            prefix(pattern),
            array(input),
        );
        run(&scratch, label, &source, 1, 1);
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn public_header_and_kv_capture_oracle_exhausts_ascii_at_both_positions_under_default_fuel() {
    let scratch = temporary("pattern-ascii-exhaustive");
    // Four compiled fixtures; each loops over all 128 bytes, including NUL,
    // literal +/-, CR/LF and DEL. No compilation per candidate input.
    for (grammar, label) in [(Grammar::Header, "header"), (Grammar::Kv, "kv")] {
        for key in [true, false] {
            let (source, expected) = ascii_source(grammar, key);
            run(
                &scratch,
                &format!("{label}-{}", if key { "key" } else { "value" }),
                &source,
                expected,
                1,
            );
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn public_header_and_kv_8192_byte_valid_and_nonmatching_records_keep_default_fuel_and_settle() {
    let scratch = temporary("pattern-long-default-fuel");
    for (grammar, label) in [(Grammar::Header, "header"), (Grammar::Kv, "kv")] {
        for matches in [true, false] {
            let (source, expected) = long_source(grammar, matches);
            run(
                &scratch,
                &format!("{label}-{matches}"),
                &source,
                expected,
                2,
            );
        }
    }
    fs::remove_dir_all(scratch).unwrap();
}
