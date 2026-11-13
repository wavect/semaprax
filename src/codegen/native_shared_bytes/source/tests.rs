use super::super::tests::{compile_and_run, scratch};
use super::Projection;
use crate::hir;
const SOURCE: &str = r#"module partition_native;
@id("partition_native.split") fn split()->Vec<Bytes>{
 let buffer=bytes_zeroed(65536usize);
 bytes_partition(buffer,4096usize,16usize)
}
@id("partition_native.main") fn main()->i64{
 let values=split();
 if vec_len<Bytes>(values)==4096usize {7}else{0}
}
"#;
const SHARED_SOURCE: &str = include_str!("../../../../tests/fixtures/byte-partition-private.spx");
const EMPTY_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-empty.spx");
const CAPACITY_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-capacity.spx");
const REPEATED_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-repeated.spx");
fn source(text: &str) -> String {
    let ast = crate::parse(text, "partition_native.spx").unwrap();
    assert!(crate::check(text, "partition_native.spx").is_err());
    let owner =
        hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX })
            .unwrap();
    assert!(crate::codegen::emit_hir_c(owner.program()).is_err());
    Projection::receive(&owner, &mut { usize::MAX })
        .unwrap()
        .emit()
        .unwrap()
}
#[test]
fn original_4096_source_reaches_shared_native_carriers_and_canonical_settlement_at_o0_o2() {
    let ast = crate::parse(SOURCE, "partition_native.spx").unwrap();
    let owner =
        hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX })
            .unwrap();
    let mut work = usize::MAX;
    let projection = Projection::receive(&owner, &mut work).unwrap();
    let used = usize::MAX - work;
    let mut exact = used;
    Projection::receive(&owner, &mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(Projection::receive(&owner, &mut short).is_err());
    assert_eq!(short, 0);
    let emitted = projection.emit().unwrap();
    assert_eq!(emitted, source(SOURCE));
    assert!(emitted.contains("spx_shb_partition_prepare(spx_shb_held(spx_ctx)"));
    assert!(emitted.contains("spx_shb_partition_commit(spx_shb_held(spx_ctx)"));
    assert!(emitted.contains("#define spx_bytes_drop(value) spx_shb_drop"));
    assert!(emitted.contains("spx_shb_detach(&context);"));
    for (unit, expected) in [
        (emitted, 7),
        (source(SHARED_SOURCE), 4096),
        (source(EMPTY_SOURCE), 0),
        (source(CAPACITY_SOURCE), 8192),
        (source(REPEATED_SOURCE), 8193),
    ] {
        let root = scratch("source-partition");
        compile_and_run(
            &root,
            &format!(
                r#"
#define SPX_NO_ENTRY_WRAPPER
{unit}
int main(void) {{
    for (uint32_t i = 0; i < 3u; ++i) {{
        int64_t result = INT64_C(-999); spx_partition_status_domain domain = "unpublished"; uint32_t code = 0u;
        if (spx_partition_run(&result, &domain, &code) != 0u || result != INT64_C({expected})) return 1;
        if (strcmp(domain, "unpublished") != 0 || code != 0u) return 2;
    }}
    return 0;
}}
"#
            ),
            &[],
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    // Genuine consuming iteration stays closed until its exact IterStep ABI
    // joins; an old two-word owned payload cannot leak into this unit.
    let iterator = SOURCE.replace("if vec_len<Bytes>(values)==4096usize {7}else{0}",
        "let mut seen=0; for own item in vec_into_iter<Bytes>(values) { let view=bytes_as_slice(item); if byte_len(view)==16usize {seen=seen+1;0}else{seen=0;0} } seen");
    let ast = crate::parse(&iterator, "partition_native.spx").unwrap();
    let owner =
        hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX })
            .unwrap();
    let mut work = usize::MAX;
    assert!(Projection::receive(&owner, &mut work).is_err());
    assert_eq!(work, 0);
}
#[test]
fn native_partition_failure_keeps_exact_status_and_poison_result_through_cleanup_and_reentry() {
    use crate::construction_fusion::partition::operation::{STATUS_CODES, STATUS_DOMAIN};
    let mut cases = vec![(
        CAPACITY_SOURCE.replace("8192usize", "8193usize"),
        STATUS_CODES[3],
    )];
    for ((count, width), code) in [
        (("4096", "0"), STATUS_CODES[0]),
        (("18446744073709551615", "2"), STATUS_CODES[1]),
        (("4095", "16"), STATUS_CODES[2]),
        (("65536", "1"), STATUS_CODES[3]),
    ] {
        for (fixture, operands) in [
            (SOURCE, "4096usize,16usize"),
            (SHARED_SOURCE, "4096usize, 16usize"),
        ] {
            let text = fixture.replace(operands, &format!("{count}usize, {width}usize"));
            cases.push((text, code));
        }
    }
    for (text, code) in cases {
        let emitted = source(&text);
        let root = scratch("source-operation-failure");
        let probe = format!(
            r#"
int main(void) {{
    for (uint32_t i = 0; i < 3u; ++i) {{
        int64_t result = INT64_C(-999); spx_partition_status_domain domain = "unpublished"; uint32_t code = 0u;
        if (spx_partition_run(&result, &domain, &code) != 1u) return 1;
        if (result != INT64_C(-999) || strcmp(domain, "{STATUS_DOMAIN}") != 0 || code != {code}u) return 2;
    }}
    return 0;
}}
"#
        );
        compile_and_run(
            &root,
            &format!("#define SPX_NO_ENTRY_WRAPPER\n{emitted}\n{probe}"),
            &[],
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn native_bounded_producer_keeps_its_exact_status_before_partition() {
    for (requested, width, expected) in [(65536, 16, 0), (65537, 0, 2)] {
        let text = SHARED_SOURCE
            .replace(
                "bytes_zeroed(65536usize)",
                &format!("bytes_zeroed_bounded({requested}usize, 65536usize)"),
            )
            .replace("4096usize, 16usize", &format!("4096usize, {width}usize"));
        let unit = source(&text);
        let root = scratch("source-bounded");
        compile_and_run(
            &root,
            &format!(
                r#"
#include <stdlib.h>
static size_t payload_allocations;
static size_t payload_frees;
static void *count_payload(size_t count, size_t size) {{
    ++payload_allocations;
    return calloc(count, size);
}}
static void release_payload(void *payload) {{
    ++payload_frees;
    free(payload);
}}
#define SPX_SHB_PAYLOAD_CALLOC count_payload
#define SPX_SHB_PAYLOAD_FREE release_payload
#define SPX_NO_ENTRY_WRAPPER
{unit}
int main(void) {{
    for (uint32_t i = 0; i < 3u; ++i) {{
        int64_t result = INT64_C(-999); spx_partition_status_domain domain = "unpublished"; uint32_t code = 0u;
        uint32_t failed = spx_partition_run(&result, &domain, &code);
        if ({expected}u == 0u) {{
            if (failed != 0u || result != INT64_C(4096) || strcmp(domain, "unpublished") != 0 || code != 0u) return 1;
        }} else {{
            if (failed != 1u || result != INT64_C(-999) || code != {expected}u) return 2;
            if (strcmp(domain, "semaprax.byte-buffer.v1") != 0) return 3;
        }}
        size_t allocations = {expected}u == 0u ? (size_t)i + 1u : 0u;
        if (payload_allocations != allocations || payload_frees != allocations) return 4;
    }}
    return 0;
}}
"#
            ),
            &[],
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
