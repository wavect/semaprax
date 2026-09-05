//! Copy record admission for Reference Interpreter v1.
//!
//! A bounded acyclic record or class carrier whose leaves are admitted Copy
//! scalars owns no cleanup leaf, so the interpreter builds it, reads its
//! fields, and stores Field Mutation v1 scalar fields into it exactly as the
//! native C11 and Core-Wasm backends do. These cases fail closed with
//! `SPX-F102 (record_construction)` before the interpreter admitted the
//! Copy profile, and `case.record.mutation` additionally reached the
//! `SPX-F105` evaluator guard while the field store replaced the whole
//! binding instead of one field.

use super::*;

/// Copy-record surface shared by the interpreter, native C11, and Core-Wasm
/// legs: every entry is explicit-ID, monomorphic, effect-free, takes no
/// parameters, and returns a direct `i64`.
const COPY_RECORD_FIXTURE: &str = r#"
module test.interpreter_copy_records;

@id("copy.point")
record Point {
    @id("copy.point.x") x: i64,
    @id("copy.point.y") y: i64,
}

@id("copy.frame")
record Frame {
    @id("copy.frame.origin") origin: Point,
    @id("copy.frame.tag") tag: i64,
}

@id("copy.meter")
class Meter {
    @id("copy.meter.reading") reading: i64,

    @id("copy.meter.doubled")
    fn doubled(self: Meter) -> i64 { self.reading + self.reading }
}

@id("case.record.flat")
fn record_flat() -> i64 {
    let point = Point { x: 40, y: 2, };
    point.x + point.y
}

@id("case.record.nested")
fn record_nested() -> i64 {
    let frame = Frame { origin: Point { x: 10, y: 20, }, tag: 5, };
    frame.origin.x + frame.origin.y + frame.tag
}

@id("case.record.mutation")
fn record_mutation() -> i64 {
    let mut point = Point { x: 1, y: 2, };
    point.x = point.x + 100;
    point.y = point.y * 3;
    point.x + point.y
}

@id("case.record.copy.alias")
fn record_copy_alias() -> i64 {
    let mut left = Point { x: 1, y: 2, };
    let right = left;
    left.x = 100;
    left.x * 1000 + left.y * 10 + right.x
}

@id("case.record.nested.mutation")
fn record_nested_mutation() -> i64 {
    let mut frame = Frame { origin: Point { x: 1, y: 2, }, tag: 3, };
    frame.tag = 40;
    frame.origin.x + frame.origin.y + frame.tag
}

@id("case.class.value")
fn class_value() -> i64 {
    let meter = Meter { reading: 21, };
    meter.doubled()
}

@id("app.main")
fn main() -> i64 { record_flat() }
"#;

/// Record shapes that stay outside the admitted profile. They are kept apart
/// from the parity fixture because a record-typed signature is outside the
/// Core-Wasm scalar-export lane.
const CLOSED_RECORD_FIXTURE: &str = r#"
module test.interpreter_closed_records;

@id("closed.point")
record Point {
    @id("closed.point.x") x: i64,
    @id("closed.point.y") y: i64,
}

@id("closed.total")
fn total(point: Point) -> i64 { point.x + point.y }

@id("case.closed.update")
fn closed_update() -> i64 {
    let point = Point { x: 1, y: 2, };
    let moved = point with { x: 10, };
    moved.x + moved.y
}

@id("case.closed.callee")
fn closed_callee() -> i64 { total(Point { x: 40, y: 2, }) }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Stable id, source name, and the exact `i64` every producer must return.
const COPY_RECORD_CASES: &[(&str, &str, i64)] = &[
    ("case.record.flat", "record_flat", 42),
    ("case.record.nested", "record_nested", 35),
    ("case.record.mutation", "record_mutation", 107),
    ("case.record.copy.alias", "record_copy_alias", 100_021),
    ("case.record.nested.mutation", "record_nested_mutation", 43),
    ("case.class.value", "class_value", 42),
];

/// The fixture's entrypoint body, rewritten per case for the Core-Wasm leg:
/// Public Scalar Export Profile v1 admits no record declaration, so that
/// backend observes one case at a time through `semaprax_main`.
const FIXTURE_ENTRY: &str = "fn main() -> i64 { record_flat() }";

/// One `<stable id>=<decimal i64>` line per case, in fixture order.
fn expected_transcript() -> String {
    COPY_RECORD_CASES
        .iter()
        .map(|(id, _, value)| format!("{id}={value}\n"))
        .collect()
}

fn interpreter_transcript_over_copy_records(source_path: &Path) -> String {
    COPY_RECORD_CASES
        .iter()
        .map(|(id, _, _)| {
            let envelope = interpret_case(source_path, id, &[])
                .unwrap_or_else(|errors| panic!("`{id}` must be admitted: {errors:?}"));
            let payload: serde_json::Value =
                serde_json::from_str(&envelope).expect("envelope JSON");
            let outcome = &payload["payload"]["outcome"];
            assert_eq!(outcome["kind"], "returned", "{id}: {envelope}");
            assert_eq!(outcome["type"], "i64", "{id}: {envelope}");
            format!("{id}={}\n", outcome["value"].as_str().expect("value text"))
        })
        .collect()
}

#[test]
fn interpreter_admits_copy_record_construction_projection_and_field_mutation() {
    let path = write_temp(COPY_RECORD_FIXTURE);
    let transcript = interpreter_transcript_over_copy_records(&path);
    cleanup(&path);
    assert_eq!(transcript, expected_transcript());
}

#[test]
fn record_shapes_outside_the_copy_profile_keep_their_closed_admission_reason() {
    let path = write_temp(CLOSED_RECORD_FIXTURE);
    for (token, reason) in [
        ("case.closed.update", "record_update"),
        ("case.closed.callee", "unsupported_callee"),
    ] {
        let errors = interpret_case(&path, token, &[])
            .expect_err("the shape is outside the admitted interpreter profile");
        assert!(
            errors
                .iter()
                .any(|item| item.code == "SPX-F102" && item.message.contains(reason)),
            "{token}: {errors:?}"
        );
        // Admission, never an evaluator guard: a closed reason names the
        // shape, an `SPX-F105` guard would be a backend accident.
        assert!(
            errors.iter().all(|item| item.code != "SPX-F105"),
            "{token}: {errors:?}"
        );
    }
    cleanup(&path);
}

fn native_copy_record_probe() -> String {
    let mut probe = String::from(
        r#"
typedef spx_status_token (*spx_copy_case)(struct spx_context *, int64_t *);

static int spx_emit_copy_case(const char *id, spx_copy_case entry) {
    struct spx_status_entry records[UINT32_C(8)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(911), records, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int64_t value = INT64_C(0);
    if (entry(&context, &value) != SPX_STATUS_SUCCESS) return 11;
    printf("%s=%lld\n", id, (long long)value);
    return 0;
}

int main(void) {
    int failure = 0;
"#,
    );
    for (id, _, _) in COPY_RECORD_CASES {
        probe.push_str(&format!(
            "    failure = spx_emit_copy_case(\"{id}\", {});\n    if (failure != 0) return failure;\n",
            c_symbol(id)
        ));
    }
    probe.push_str("    return 0;\n}\n");
    probe
}

#[test]
fn native_c11_and_core_wasm_agree_with_the_interpreter_on_copy_records() {
    if !require_tools_or_skip() {
        return;
    }
    let program = parse(
        COPY_RECORD_FIXTURE,
        Path::new("interpreter-copy-records.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );

    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-interpreter-copy-records-{}-{ordinal}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();

    let generated = codegen::emit_c(&program).unwrap();
    let mut native = Vec::new();
    for optimization in ["-O0", "-O2"] {
        let source = root.join(format!("copy-records{optimization}.c"));
        let executable = root.join(format!(
            "copy-records{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(
            &source,
            format!("{generated}\n{}", native_copy_record_probe()),
        )
        .unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "native {optimization} compilation failed: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        native.push(normalized_stdout(
            Command::new(&executable).output().unwrap(),
            &format!("native {optimization}"),
        ));
    }

    // Public Scalar Export Profile v1 admits no authored record declaration,
    // so the web leg observes one case at a time through `semaprax_main`.
    let script = root.join("observe-copy-records.mjs");
    std::fs::write(
        &script,
        r#"import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const packageDirectory = resolve(process.argv[2]);
const web = await import(pathToFileURL(resolve(packageDirectory, "semaprax.js")));
const { instance } = await web.instantiateBytes(
  await readFile(resolve(packageDirectory, "app.wasm")),
);
process.stdout.write(`${process.argv[3]}=${instance.exports.semaprax_main().toString()}\n`);
"#,
    )
    .unwrap();
    assert_eq!(
        COPY_RECORD_FIXTURE.matches(FIXTURE_ENTRY).count(),
        1,
        "the entrypoint rewrite must select exactly one body"
    );
    let mut core_wasm = String::new();
    for (index, (id, name, _)) in COPY_RECORD_CASES.iter().enumerate() {
        let case_source =
            COPY_RECORD_FIXTURE.replace(FIXTURE_ENTRY, &format!("fn main() -> i64 {{ {name}() }}"));
        let case_program = parse(&case_source, Path::new("interpreter-copy-records-web.spx"))
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        let package = root.join(format!("web-{index}"));
        wasm::build_web(&case_program, &package).unwrap();
        core_wasm.push_str(
            &String::from_utf8(normalized_stdout(
                Command::new("node")
                    .arg(&script)
                    .arg(&package)
                    .arg(id)
                    .output()
                    .unwrap(),
                "Core-Wasm Node observer",
            ))
            .unwrap(),
        );
    }

    let _ = std::fs::remove_dir_all(&root);

    let expected = expected_transcript();
    assert_eq!(
        String::from_utf8(native[0].clone()).unwrap(),
        expected,
        "native -O0 diverges on the Copy record profile"
    );
    assert_eq!(native[0], native[1], "native optimization changed results");
    assert_eq!(
        core_wasm, expected,
        "Core Wasm diverges on the Copy record profile"
    );

    let path = write_temp(COPY_RECORD_FIXTURE);
    let interpreted = interpreter_transcript_over_copy_records(&path);
    cleanup(&path);
    assert_eq!(
        interpreted, expected,
        "interpreter diverges from both backends on the Copy record profile"
    );
}
