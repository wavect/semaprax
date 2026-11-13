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
const ITERATOR_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-iterator.spx");
const CAPACITY_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-capacity.spx");
const REPEATED_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-repeated.spx");
const PAYLOAD_CENSUS: &str = r#"
#include <stdlib.h>
static size_t payload_allocations;
static size_t payload_frees;
static size_t payload_bytes;
static void *count_payload(size_t count, size_t size) {
    ++payload_allocations;
    payload_bytes += count * size;
    return calloc(count, size);
}
static void release_payload(void *payload) {
    ++payload_frees;
    free(payload);
}
#define SPX_SHB_PAYLOAD_CALLOC count_payload
#define SPX_SHB_PAYLOAD_FREE release_payload
"#;

fn source(text: &str) -> String {
    let ast = crate::parse(text, "partition_native.spx").unwrap();
    assert!(crate::check(text, "partition_native.spx").is_err());
    let owner =
        hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX })
            .unwrap();
    assert!(crate::codegen::emit_hir_c(owner.program()).is_err());
    let wire = owner.cache_projection().unwrap();
    let replayed =
        hir::byte_partition::ValidatedPartitionProgram::replay_cache(&ast, &wire, &mut {
            usize::MAX
        })
        .unwrap();
    Projection::receive(&replayed, &mut { usize::MAX })
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
    for (unit, expected, allocations, bytes) in [
        (emitted, 7, 1, 65536),
        (source(SHARED_SOURCE), 4096, 1, 65536),
        (source(EMPTY_SOURCE), 0, 0, 0),
        (source(CAPACITY_SOURCE), 8192, 1, 8192),
        (source(REPEATED_SOURCE), 8193, 2, 8193),
        (source(ITERATOR_SOURCE), 16, 1, 65536),
    ] {
        let root = scratch("source-partition");
        compile_and_run(
            &root,
            &format!(
                r#"
{PAYLOAD_CENSUS}#define SPX_NO_ENTRY_WRAPPER
{unit}
int main(void) {{
    for (uint32_t i = 0; i < 3u; ++i) {{
        int64_t result = INT64_C(-999); spx_partition_status_domain domain = "unpublished"; uint32_t code = 0u;
        if (spx_partition_run(&result, &domain, &code) != 0u || result != INT64_C({expected})) return 1;
        if (strcmp(domain, "unpublished") != 0 || code != 0u) return 2;
        size_t runs = (size_t)i + 1u;
        if (payload_allocations != runs * {allocations}u || payload_frees != payload_allocations
            || payload_bytes != runs * {bytes}u) return 3;
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
#[test]
fn native_partition_failure_keeps_exact_status_and_poison_result_through_cleanup_and_reentry() {
    use crate::construction_fusion::partition::operation::{STATUS_CODES, STATUS_DOMAIN};
    let mut cases = vec![
        (
            SHARED_SOURCE.replace("-> i64", "-> i64 ensures result == 4095"),
            crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
            crate::conformance::CONTRACT_ENSURES_FALSE_CODE,
            1,
            65536,
        ),
        (
            REPEATED_SOURCE.replace("-> i64", "-> i64 ensures result == 8192"),
            crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
            crate::conformance::CONTRACT_ENSURES_FALSE_CODE,
            2,
            8193,
        ),
        (
            CAPACITY_SOURCE.replace("8192usize", "8193usize"),
            STATUS_DOMAIN,
            STATUS_CODES[3],
            1,
            8193,
        ),
        (
            REPEATED_SOURCE.replace("8192usize, 1usize", "8192usize, 0usize"),
            STATUS_DOMAIN,
            STATUS_CODES[0],
            1,
            8192,
        ),
        (
            REPEATED_SOURCE.replace("whole, 1usize, 1usize", "whole, 1usize, 0usize"),
            STATUS_DOMAIN,
            STATUS_CODES[0],
            2,
            8193,
        ),
        (
            REPEATED_SOURCE.replace(
                "bytes_zeroed(1usize)",
                "bytes_zeroed_bounded(1usize, 0usize)",
            ),
            crate::byte_ops::SET_STATUS_DOMAIN,
            crate::byte_ops::CAPACITY_EXCEEDED_CODE,
            1,
            8192,
        ),
    ];
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
            cases.push((text, STATUS_DOMAIN, code, 1, 65536));
        }
    }
    for (text, domain, code, allocations, bytes) in cases {
        let emitted = source(&text);
        let root = scratch("source-operation-failure");
        let probe = format!(
            r#"
int main(void) {{
    for (uint32_t i = 0; i < 3u; ++i) {{
        int64_t result = INT64_C(-999); spx_partition_status_domain domain = "unpublished"; uint32_t code = 0u;
        if (spx_partition_run(&result, &domain, &code) != 1u) return 1;
        if (result != INT64_C(-999) || strcmp(domain, "{domain}") != 0 || code != {code}u) return 2;
        size_t runs = (size_t)i + 1u;
        if (payload_allocations != runs * {allocations}u || payload_frees != payload_allocations
            || payload_bytes != runs * {bytes}u) return 3;
    }}
    return 0;
}}
"#
        );
        compile_and_run(
            &root,
            &format!("{PAYLOAD_CENSUS}#define SPX_NO_ENTRY_WRAPPER\n{emitted}\n{probe}"),
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
{PAYLOAD_CENSUS}#define SPX_NO_ENTRY_WRAPPER
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
        if (payload_allocations != allocations || payload_frees != allocations
            || payload_bytes != allocations * 65536u) return 4;
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
