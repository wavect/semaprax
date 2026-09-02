use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::hir::{self, DeclarationId, ExpressionId, ResolvedFunction, ResolvedProgram};
use crate::parse;

use super::super::{native_cleanup, native_host_contract, native_resource, native_value};
use super::*;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module test.native_adapter;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("other.type")
resource Other { @id("other.drop") drop trivial; }

@id("token.mixed")
fn mixed(first: own Token, count: i64, enabled: bool, second: own Other) -> i64 { 0 }

@id("token.identity")
fn identity(count: i64, value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParsedParameter {
    Scalar {
        index: u32,
        value: String,
        kind: u32,
    },
    Owned {
        index: u32,
        value: String,
        ordinal: u32,
        resource: String,
        lifecycle: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParsedResult {
    ScalarI64,
    Owned {
        index: u32,
        value: String,
        ordinal: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedDescriptor {
    target: String,
    schema: [u8; 32],
    target_fingerprint: [u8; 32],
    physical_module: [u8; 32],
    function_template: [u8; 32],
    module: String,
    function: String,
    parameters: Vec<ParsedParameter>,
    result: ParsedResult,
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| "descriptor offset overflow".to_owned())?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "truncated descriptor".to_owned())?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| "invalid u32 width".to_owned())?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn text(&mut self) -> Result<String, String> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| "text length does not fit usize".to_owned())?;
        if length == 0 {
            return Err("empty descriptor identity".to_owned());
        }
        let bytes = self.take(length)?;
        if bytes.contains(&0) {
            return Err("descriptor identity contains NUL".to_owned());
        }
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| "descriptor identity is not UTF-8".to_owned())
    }

    fn fingerprint(&mut self) -> Result<[u8; 32], String> {
        self.take(32)?
            .try_into()
            .map_err(|_| "invalid fingerprint width".to_owned())
    }
}

fn parse_descriptor(bytes: &[u8]) -> Result<ParsedDescriptor, String> {
    let mut reader = Reader { bytes, offset: 0 };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err("wrong descriptor magic".to_owned());
    }
    if reader.u32()? != VERSION {
        return Err("unsupported descriptor version".to_owned());
    }
    if reader.u32()? != HEADER_SIZE {
        return Err("unsupported descriptor header size".to_owned());
    }
    let declared = usize::try_from(reader.u32()?)
        .map_err(|_| "descriptor length does not fit usize".to_owned())?;
    if declared != bytes.len() {
        return Err("descriptor total length is not exact".to_owned());
    }
    let target = reader.text()?;
    let schema = reader.fingerprint()?;
    if schema != schema_fingerprint() {
        return Err("descriptor schema fingerprint is unknown".to_owned());
    }
    let target_fingerprint = reader.fingerprint()?;
    if target_fingerprint != super::target_fingerprint(target.as_bytes()) {
        return Err("descriptor target fingerprint is inconsistent".to_owned());
    }
    let physical_module = reader.fingerprint()?;
    let function_template = reader.fingerprint()?;
    let module = reader.text()?;
    let function = reader.text()?;
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| "parameter count does not fit usize".to_owned())?;
    let mut parameters = Vec::with_capacity(count.min(1024));
    let mut next_owner = 0_u32;
    for expected in 0..count {
        let tag = reader.u32()?;
        let index = reader.u32()?;
        if index != u32::try_from(expected).map_err(|_| "too many parameters".to_owned())? {
            return Err("noncanonical parameter index".to_owned());
        }
        let value = reader.text()?;
        match tag {
            PARAMETER_SCALAR => {
                let kind = reader.u32()?;
                if !matches!(kind, SCALAR_I64 | SCALAR_BOOL) {
                    return Err("unknown scalar kind".to_owned());
                }
                parameters.push(ParsedParameter::Scalar { index, value, kind });
            }
            PARAMETER_OWNED_RESOURCE => {
                let ordinal = reader.u32()?;
                if ordinal != next_owner {
                    return Err("noncanonical owner ordinal".to_owned());
                }
                next_owner = next_owner
                    .checked_add(1)
                    .ok_or_else(|| "owner ordinal overflow".to_owned())?;
                parameters.push(ParsedParameter::Owned {
                    index,
                    value,
                    ordinal,
                    resource: reader.text()?,
                    lifecycle: reader.text()?,
                });
            }
            _ => return Err("unknown parameter tag".to_owned()),
        }
    }
    let result = match reader.u32()? {
        RESULT_SCALAR_I64 => ParsedResult::ScalarI64,
        RESULT_OWNED_INPUT => {
            let index = reader.u32()?;
            let value = reader.text()?;
            let ordinal = reader.u32()?;
            match parameters.get(index as usize) {
                Some(ParsedParameter::Owned {
                    index: expected_index,
                    value: expected_value,
                    ordinal: expected_ordinal,
                    ..
                }) if *expected_index == index
                    && *expected_value == value
                    && *expected_ordinal == ordinal => {}
                _ => return Err("owned result mapping is not exact".to_owned()),
            }
            ParsedResult::Owned {
                index,
                value,
                ordinal,
            }
        }
        _ => return Err("unknown result tag".to_owned()),
    };
    if reader.offset != bytes.len() {
        return Err("descriptor contains trailing bytes".to_owned());
    }
    Ok(ParsedDescriptor {
        target,
        schema,
        target_fingerprint,
        physical_module,
        function_template,
        module,
        function,
        parameters,
        result,
    })
}

fn program(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("native-adapter.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|candidate| candidate.id.as_str() == id)
        .unwrap()
}

fn descriptor(source: &str, id: &str) -> NativeAdapterDescriptor {
    let program = program(source);
    let function = function(&program, id);
    let abi = native_resource::build_resource_abi(&program).unwrap();
    let cleanup = native_cleanup::classify(&program, function).unwrap();
    let values = native_value::plan(
        &program,
        function,
        &cleanup,
        &abi,
        &HashMap::<ExpressionId, String>::new(),
    )
    .unwrap();
    let template = native_host_contract::derive_from_admitted(
        &program,
        &DeclarationId::new(id),
        &abi,
        &cleanup,
        &values,
    )
    .unwrap();
    derive(&template).unwrap()
}

#[test]
fn descriptor_round_trips_complete_ordered_signature_and_result_metadata() {
    let mixed = descriptor(SOURCE, "token.mixed");
    let parsed = parse_descriptor(&mixed.bytes).unwrap();
    assert_eq!(parsed.target, physical_target_tag().unwrap());
    assert_eq!(parsed.parameters.len(), 4);
    assert!(matches!(
        &parsed.parameters[..],
        [
            ParsedParameter::Owned {
                index: 0,
                ordinal: 0,
                ..
            },
            ParsedParameter::Scalar {
                index: 1,
                kind: SCALAR_I64,
                ..
            },
            ParsedParameter::Scalar {
                index: 2,
                kind: SCALAR_BOOL,
                ..
            },
            ParsedParameter::Owned {
                index: 3,
                ordinal: 1,
                ..
            }
        ]
    ));
    assert_eq!(parsed.result, ParsedResult::ScalarI64);
    assert_eq!(
        u32::from_le_bytes(mixed.bytes[16..20].try_into().unwrap()) as usize,
        mixed.bytes.len()
    );
    assert_eq!(
        u32::from_le_bytes(mixed.bytes[12..16].try_into().unwrap()),
        HEADER_SIZE
    );

    let identity = descriptor(SOURCE, "token.identity");
    let parsed = parse_descriptor(&identity.bytes).unwrap();
    assert!(matches!(
        parsed.result,
        ParsedResult::Owned {
            index: 1,
            ordinal: 0,
            ..
        }
    ));
}

#[test]
fn display_and_whitespace_do_not_change_bytes_but_physical_abi_changes_do() {
    let baseline = descriptor(SOURCE, "token.identity");
    let renamed = format!(
        "\n{}",
        SOURCE.replace("fn identity(", "fn renamed_identity(")
    );
    assert_eq!(baseline, descriptor(&renamed, "token.identity"));

    let scalar_changed = SOURCE.replace(
        "fn identity(count: i64, value: own Token)",
        "fn identity(count: bool, value: own Token)",
    );
    assert_ne!(baseline, descriptor(&scalar_changed, "token.identity"));

    let lifecycle_changed = SOURCE.replace("token.drop", "token.drop.v2");
    assert_ne!(baseline, descriptor(&lifecycle_changed, "token.identity"));
}

#[test]
fn same_module_functions_have_distinct_deterministic_getters_and_ordered_bytes() {
    let identity = descriptor(SOURCE, "token.identity");
    let mixed = descriptor(SOURCE, "token.mixed");
    assert_ne!(identity.getter_symbol, mixed.getter_symbol);
    assert_ne!(identity.bytes, mixed.bytes);
    assert_eq!(identity, descriptor(SOURCE, "token.identity"));
    assert_eq!(mixed, descriptor(SOURCE, "token.mixed"));

    let parsed = parse_descriptor(&mixed.bytes).unwrap();
    assert_eq!(
        parsed
            .parameters
            .iter()
            .map(|parameter| match parameter {
                ParsedParameter::Scalar { index, .. } | ParsedParameter::Owned { index, .. } =>
                    *index,
            })
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn hostile_wire_inputs_fail_closed_without_repairs() {
    let descriptor = descriptor(SOURCE, "token.mixed");
    for length in 0..descriptor.bytes.len() {
        assert!(parse_descriptor(&descriptor.bytes[..length]).is_err());
    }
    let mut trailing = descriptor.bytes.clone();
    trailing.push(0);
    assert!(parse_descriptor(&trailing).is_err());

    for (offset, replacement) in [(0, 0_u8), (8, 2_u8), (12, 0_u8), (16, 0_u8)] {
        let mut hostile = descriptor.bytes.clone();
        hostile[offset] = replacement;
        assert!(parse_descriptor(&hostile).is_err());
    }

    let mut reader = Reader {
        bytes: &descriptor.bytes,
        offset: HEADER_SIZE as usize,
    };
    let _ = reader.text().unwrap();
    let _ = reader.take(32 * 4).unwrap();
    let _ = reader.text().unwrap();
    let _ = reader.text().unwrap();
    let count = reader.u32().unwrap();
    assert!(count > 0);
    let first_tag = reader.offset;
    let mut unknown_tag = descriptor.bytes.clone();
    unknown_tag[first_tag..first_tag + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(parse_descriptor(&unknown_tag).is_err());

    let mut wrong_index = descriptor.bytes.clone();
    wrong_index[first_tag + 4..first_tag + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(parse_descriptor(&wrong_index).is_err());

    let mut wrong_schema = descriptor.bytes.clone();
    let target_length = u32::from_le_bytes(wrong_schema[20..24].try_into().unwrap()) as usize;
    wrong_schema[24 + target_length] ^= 0x80;
    assert!(parse_descriptor(&wrong_schema).is_err());
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn create() -> Self {
        let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-native-adapter-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.path).is_ok() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

fn compile(command: &mut Command, context: &str) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{context} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn strict_separate_c_and_cpp_translation_units_link_and_inspect_descriptor() {
    if Command::new("clang").arg("--version").output().is_err()
        || Command::new("clang++").arg("--version").output().is_err()
    {
        return;
    }
    let descriptor = descriptor(SOURCE, "token.identity");
    let directory = TestDirectory::create();
    let header = directory.path.join("adapter.h");
    let provider = directory.path.join("provider.c");
    let consumer = directory.path.join("consumer.c");
    let cpp = directory.path.join("consumer.cpp");
    std::fs::write(&header, emit_header(&descriptor)).unwrap();
    let provider_source = emit_source(&descriptor, "adapter.h").unwrap();
    std::fs::write(&provider, &provider_source).unwrap();
    std::fs::write(
            &consumer,
            format!(
                "#include <string.h>\n#pragma pack(push, 1)\n#include \"adapter.h\"\n#pragma pack(pop)\n\
                 extern int spx_cpp_inspect(void);\n\
                 static uint32_t read_u32(const unsigned char *p) {{\n\
                 return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);\n}}\n\
                 int main(void) {{\n\
                 const unsigned char *p = {symbol}();\n\
                 if (p == (const unsigned char *)0 || memcmp(p, \"SPXNABI1\", 8) != 0) return 1;\n\
                 if (read_u32(p + 8) != UINT32_C(1) || read_u32(p + 12) != UINT32_C(20) || read_u32(p + 16) != UINT32_C({length})) return 2;\n\
                 if (p != {symbol}()) return 3;\n\
                 return spx_cpp_inspect();\n}}\n",
                symbol = descriptor.getter_symbol,
                length = descriptor.bytes.len()
            ),
        )
        .unwrap();
    std::fs::write(
            &cpp,
            format!(
                "#pragma pack(push, 16)\n#include \"adapter.h\"\n#pragma pack(pop)\n\
                 extern \"C\" int spx_cpp_inspect(void) {{ return {symbol}() == nullptr ? 4 : 0; }}\n",
                symbol = descriptor.getter_symbol
            ),
        )
        .unwrap();

    let provider_object = directory.path.join("provider.o");
    let consumer_object = directory.path.join("consumer.o");
    let cpp_object = directory.path.join("consumer_cpp.o");
    let executable = directory.path.join(if cfg!(windows) {
        "adapter_test.exe"
    } else {
        "adapter_test"
    });
    compile(
        Command::new("clang")
            .arg("-std=c11")
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
            .arg("-fvisibility=hidden")
            .arg("-I")
            .arg(&directory.path)
            .arg("-c")
            .arg(&provider)
            .arg("-o")
            .arg(&provider_object),
        "provider compile",
    );

    let mut mismatched_lines = provider_source
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let guard_error = mismatched_lines
        .iter()
        .position(|line| line.contains("provider architecture mismatch"))
        .unwrap();
    assert!(guard_error > 0);
    mismatched_lines[guard_error - 1] =
        "#if !(defined(SPX_DELIBERATELY_WRONG_ARCHITECTURE))".to_owned();
    let mismatched_provider = directory.path.join("mismatched_provider.c");
    std::fs::write(&mismatched_provider, mismatched_lines.join("\n")).unwrap();
    let mismatch = Command::new("clang")
        .arg("-std=c11")
        .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-I")
        .arg(&directory.path)
        .arg("-c")
        .arg(&mismatched_provider)
        .arg("-o")
        .arg(directory.path.join("mismatched_provider.o"))
        .output()
        .unwrap();
    assert!(
        !mismatch.status.success(),
        "mismatched target guard compiled"
    );
    assert!(String::from_utf8_lossy(&mismatch.stderr)
        .contains("SEMAPRAX descriptor provider architecture mismatch"));
    compile(
        Command::new("clang")
            .arg("-std=c11")
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
            .arg("-I")
            .arg(&directory.path)
            .arg("-c")
            .arg(&consumer)
            .arg("-o")
            .arg(&consumer_object),
        "C consumer compile",
    );
    compile(
        Command::new("clang++")
            .arg("-std=c++17")
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
            .arg("-I")
            .arg(&directory.path)
            .arg("-c")
            .arg(&cpp)
            .arg("-o")
            .arg(&cpp_object),
        "C++ consumer compile",
    );
    compile(
        Command::new("clang++")
            .arg(&provider_object)
            .arg(&consumer_object)
            .arg(&cpp_object)
            .arg("-o")
            .arg(&executable),
        "descriptor link",
    );
    let executed = Command::new(&executable).output().unwrap();
    assert!(
        executed.status.success(),
        "descriptor consumer failed: {}",
        String::from_utf8_lossy(&executed.stderr)
    );

    if let Ok(symbols) = Command::new("nm").arg("-g").arg(&provider_object).output() {
        if symbols.status.success() {
            let symbols = String::from_utf8_lossy(&symbols.stdout);
            let adapter_symbols = symbols
                .lines()
                .filter(|line| line.contains("spx_"))
                .collect::<Vec<_>>();
            assert_eq!(adapter_symbols.len(), 1, "unexpected exports: {symbols}");
            assert!(adapter_symbols[0].contains(&descriptor.getter_symbol));
        }
    }
}

#[test]
fn shared_library_exports_only_getter_and_dynamic_consumer_runs() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let descriptor = descriptor(SOURCE, "token.identity");
    let directory = TestDirectory::create();
    let header = directory.path.join("adapter.h");
    let provider = directory.path.join("provider.c");
    let consumer = directory.path.join("dynamic_consumer.c");
    std::fs::write(&header, emit_header(&descriptor)).unwrap();
    std::fs::write(&provider, emit_source(&descriptor, "adapter.h").unwrap()).unwrap();
    std::fs::write(
            &consumer,
            format!(
                "#if defined(_WIN32)\n#define SPX_ADAPTER_DESCRIPTOR_DLL 1\n#endif\n\
                 #include <string.h>\n#include \"adapter.h\"\n\
                 static uint32_t read_u32(const unsigned char *p) {{\n\
                 return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);\n}}\n\
                 int main(void) {{\n\
                 const unsigned char *p = {symbol}();\n\
                 if (p == (const unsigned char *)0 || memcmp(p, \"SPXNABI1\", 8) != 0) return 1;\n\
                 if (read_u32(p + 8) != UINT32_C(1) || read_u32(p + 12) != UINT32_C(20) || read_u32(p + 16) != UINT32_C({length})) return 2;\n\
                 return p == {symbol}() ? 0 : 3;\n}}\n",
                symbol = descriptor.getter_symbol,
                length = descriptor.bytes.len()
            ),
        )
        .unwrap();

    let executable = directory.path.join(if cfg!(windows) {
        "dynamic_consumer.exe"
    } else {
        "dynamic_consumer"
    });
    let library = if cfg!(windows) {
        directory.path.join("adapter.dll")
    } else if cfg!(target_os = "macos") {
        directory.path.join("libadapter.dylib")
    } else {
        directory.path.join("libadapter.so")
    };
    let import_library = directory.path.join("adapter.lib");

    let mut shared = Command::new("clang");
    shared
        .arg("-std=c11")
        .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-fvisibility=hidden")
        .arg("-I")
        .arg(&directory.path);
    if cfg!(target_os = "macos") {
        shared.arg("-dynamiclib").arg("-fPIC");
    } else {
        shared.arg("-shared");
        if !cfg!(windows) {
            shared.arg("-fPIC");
        }
    }
    shared.arg(&provider).arg("-o").arg(&library);
    if cfg!(windows) {
        shared.arg(format!("-Wl,/implib:{}", import_library.display()));
    }
    compile(&mut shared, "shared descriptor provider build");

    assert_dynamic_export_allowlist(&library, &descriptor.getter_symbol);

    let mut consumer_compile = Command::new("clang");
    consumer_compile
        .arg("-std=c11")
        .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-I")
        .arg(&directory.path)
        .arg(&consumer);
    if cfg!(windows) {
        consumer_compile.arg(&import_library);
    } else {
        consumer_compile
            .arg("-L")
            .arg(&directory.path)
            .arg("-ladapter")
            .arg(format!("-Wl,-rpath,{}", directory.path.display()));
    }
    consumer_compile.arg("-o").arg(&executable);
    compile(&mut consumer_compile, "dynamic descriptor consumer build");
    let executed = Command::new(&executable)
        .current_dir(&directory.path)
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "dynamic descriptor consumer failed:\n{}\n{}",
        String::from_utf8_lossy(&executed.stdout),
        String::from_utf8_lossy(&executed.stderr)
    );
}

fn assert_dynamic_export_allowlist(library: &Path, getter: &str) {
    if cfg!(windows) {
        if let Ok(output) = Command::new("llvm-readobj")
            .arg("--coff-exports")
            .arg(library)
            .output()
        {
            if output.status.success() {
                let names = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| line.trim().strip_prefix("Name: "))
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                assert_eq!(names, vec![getter.to_owned()]);
                return;
            }
        }
        if let Ok(output) = Command::new("dumpbin")
            .arg("/exports")
            .arg(library)
            .output()
        {
            if output.status.success() {
                let names = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter_map(|line| {
                        let columns = line.split_whitespace().collect::<Vec<_>>();
                        (columns.len() == 4
                            && columns[0].bytes().all(|byte| byte.is_ascii_digit())
                            && columns[1].bytes().all(|byte| byte.is_ascii_hexdigit())
                            && columns[2].bytes().all(|byte| byte.is_ascii_hexdigit()))
                        .then(|| columns[3].to_owned())
                    })
                    .collect::<Vec<_>>();
                assert_eq!(names, vec![getter.to_owned()]);
            }
        }
        return;
    }

    let mut command = Command::new("nm");
    if cfg!(target_os = "macos") {
        command.args(["-gU", "-j"]);
    } else {
        command.args(["-D", "--defined-only", "--format=posix"]);
    }
    let Ok(output) = command.arg(library).output() else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let names = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let name = if cfg!(target_os = "macos") {
                line.trim().trim_start_matches('_')
            } else {
                line.split_whitespace().next().unwrap_or_default()
            };
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect::<Vec<_>>();
    assert_eq!(names, vec![getter.to_owned()]);
}
