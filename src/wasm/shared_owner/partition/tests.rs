//! Actual checked source -> module -> private host invocation, never witness substitution.
use super::super::tests::{node_available, run_node, stage, ORDINARY};
use super::*;

const SOURCE: &str = r#"module partition_wasm;
@id("partition_wasm.split") fn split()->Vec<Bytes>{
 let buffer=bytes_zeroed(65536usize);
 bytes_partition(buffer,4096usize,16usize)
}
@id("partition_wasm.main") fn main()->i64{
 let values=split();
 if vec_len<Bytes>(values)==4096usize {7}else{0}
}
"#;
const SHARED_SOURCE: &str = include_str!("../../../../tests/fixtures/byte-partition-private.spx");
const EMPTY_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-empty.spx");
const ITERATOR_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-iterator.spx");
const CLONE_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-clone.spx");
const CAPACITY_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-capacity.spx");
const REPEATED_SOURCE: &str =
    include_str!("../../../../tests/fixtures/byte-partition-private-repeated.spx");

fn emitted(source: &str) -> PartitionModule {
    let ast = crate::parse(source, "partition_wasm.spx").unwrap();
    let owner = crate::hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut {
        usize::MAX
    })
    .unwrap();
    assert!(crate::check(source, "partition_wasm.spx").is_err());
    let wire = owner.cache_projection().unwrap();
    let replayed =
        crate::hir::byte_partition::ValidatedPartitionProgram::replay_cache(&ast, &wire, &mut {
            usize::MAX
        })
        .unwrap();
    let module = PartitionModule::emit(&replayed, &mut { usize::MAX }).unwrap();
    assert!(consumer_closure(module.bytes(), Host::PrivateOrdinary).is_err());
    assert!(module
        .closure
        .lease_aware
        .iter()
        .any(|name| name == "spx_bytes_partition"));
    assert_eq!(
        admit_producer(&module.closure),
        Err(Refusal::ProducerClosed)
    );
    module
}

#[test]
fn checked_source_partition_emits_real_4096_vec_and_consuming_iterator_with_settlement() {
    let iterator = SOURCE.replace(
        "if vec_len<Bytes>(values)==4096usize {7}else{0}",
        "let mut seen=0; for own item in vec_into_iter<Bytes>(values) { let view=bytes_as_slice(item); if byte_len(view)==16usize {seen=seen+1;0}else{seen=0;0} } seen",
    );
    for (source, expected, peak_payload, events, cumulative_payload) in [
        (SOURCE, "7", "65536", "1", "65536"),
        (iterator.as_str(), "4096", "65536", "1", "65536"),
        (SHARED_SOURCE, "4096", "65536", "1", "65536"),
        (ITERATOR_SOURCE, "16", "65536", "1", "65536"),
        (CLONE_SOURCE, "1", "8", "2", "8"),
        (EMPTY_SOURCE, "0", "0", "1", "0"),
        (CAPACITY_SOURCE, "8192", "8192", "1", "8192"),
        (REPEATED_SOURCE, "8193", "8192", "2", "8193"),
    ] {
        let module = emitted(source);
        if node_available() {
            let entry = module.entry_source();
            let script = format!("{ORDINARY}\n{SUCCESS_ENTRY_TEST}");
            let directory = stage(&[
                ("ordinary.mjs", script.as_bytes()),
                ("app.wasm", module.bytes()),
                ("entry.mjs", entry.as_bytes()),
            ]);
            let stdout = run_node(
                &directory,
                "ordinary.mjs",
                &[
                    "app.wasm",
                    expected,
                    "plain",
                    peak_payload,
                    events,
                    cumulative_payload,
                ],
            );
            assert!(stdout.contains("shared-owner-ordinary-ok"));
            assert!(stdout.contains("partition-paired-success-ok"));
        }
    }
}

#[test]
fn actual_partition_failure_codes_keep_owner_live_until_canonical_cleanup_and_reentry() {
    use crate::construction_fusion::partition::operation::{STATUS_CODES, STATUS_DOMAIN};
    let clone_contract =
        CLONE_SOURCE.replace("fn main() -> i64", "fn main() -> i64 ensures result == 0");
    let mut cases = vec![
        (
            CLONE_SOURCE.replace("item, 1usize, 77u8", "item, 2usize, 77u8"),
            crate::byte_ops::SET_STATUS_DOMAIN,
            crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
            "8",
        ),
        (
            clone_contract.replace("bytes_set(item, 1usize", "bytes_set(item, 2usize"),
            crate::byte_ops::SET_STATUS_DOMAIN,
            crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE,
            "8",
        ),
        (
            clone_contract,
            crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
            crate::conformance::CONTRACT_ENSURES_FALSE_CODE,
            "8",
        ),
        (
            SHARED_SOURCE.replace("-> i64", "-> i64 ensures result == 4095"),
            crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
            crate::conformance::CONTRACT_ENSURES_FALSE_CODE,
            "65536",
        ),
        (
            REPEATED_SOURCE.replace("-> i64", "-> i64 ensures result == 8192"),
            crate::conformance::CONTRACT_STATUS_DOMAIN_V1,
            crate::conformance::CONTRACT_ENSURES_FALSE_CODE,
            "8192",
        ),
        (
            CAPACITY_SOURCE.replace("8192usize", "8193usize"),
            STATUS_DOMAIN,
            STATUS_CODES[3],
            "8193",
        ),
        (
            REPEATED_SOURCE.replace("8192usize, 1usize", "8192usize, 0usize"),
            STATUS_DOMAIN,
            STATUS_CODES[0],
            "8192",
        ),
        (
            REPEATED_SOURCE.replace("whole, 1usize, 1usize", "whole, 1usize, 0usize"),
            STATUS_DOMAIN,
            STATUS_CODES[0],
            "8192",
        ),
        (
            REPEATED_SOURCE.replace(
                "bytes_zeroed(1usize)",
                "bytes_zeroed_bounded(1usize, 0usize)",
            ),
            crate::byte_ops::SET_STATUS_DOMAIN,
            crate::byte_ops::CAPACITY_EXCEEDED_CODE,
            "8192",
        ),
    ];
    for (fixture, operands) in [
        (SOURCE, "4096usize,16usize"),
        (SHARED_SOURCE, "4096usize, 16usize"),
    ] {
        for (replacement, code) in [
            ("4096usize, 0usize", 1),
            ("18446744073709551615usize, 2usize", 2),
            ("4095usize, 16usize", 3),
            ("65536usize, 1usize", 4),
        ] {
            cases.push((
                fixture.replace(operands, replacement),
                STATUS_DOMAIN,
                code,
                "65536",
            ));
        }
    }
    for (source, domain, code, payload) in cases {
        let module = emitted(&source);
        if node_available() {
            let entry = module.entry_source();
            let directory = stage(&[
                ("entry.mjs", entry.as_bytes()),
                ("entry-test.mjs", ENTRY_TEST.as_bytes()),
            ]);
            assert!(run_node(
                &directory,
                "entry-test.mjs",
                &[domain, &code.to_string(), payload]
            )
            .contains("partition-entry-ok"));
        }
    }
}

// Same four invocations per case; one actual paired installer, no witness ticket.
const ENTRY_TEST: &str = r#"import { install } from './entry.mjs';
const domain = process.argv[2];
const code = Number(process.argv[3]);
const metadata = 16384*6 + 8192*22 + 32*11 + 32*8;
const payload = Number(process.argv[4]);
const entry = await install(metadata + payload);
for (let run=0; run<4; run+=1) {
 const outcome = entry.invoke();
 if (JSON.stringify(outcome) !== JSON.stringify({outcome:'language_failure',status:{domain,code}})) throw new Error(JSON.stringify(outcome));
 if ('result' in outcome || 'reason' in outcome) throw new Error('failure published extra result');
 const account = entry.account();
 if (account.reserved !== metadata || account.peak !== metadata+payload) throw new Error(JSON.stringify(account));
}
let rejected=false;
try { await install({module:new Uint8Array(),managedBytes:metadata+payload}); } catch { rejected=true; }
if (!rejected) throw new Error('caller module accepted');
const short = await install(metadata+payload-1);
const refused = short.invoke();
if (refused.outcome !== 'administrative_refusal' || 'status' in refused || 'result' in refused) throw new Error(JSON.stringify(refused));
if (short.account().reserved !== metadata || short.invoke().reason !== 'private entry is poisoned') throw new Error('short account reused');
console.log('partition-entry-ok');
"#;

// The unchanged raw control and paired entry run in one Node process per source.
const SUCCESS_ENTRY_TEST: &str = r#"
import { install } from './entry.mjs';
const pairedMetadata = 16384*6 + 8192*22 + 32*11 + 32*8;
const payload = Number(process.argv[5]);
const pairedEntry = await install(pairedMetadata+payload);
for (let run=0; run<4; run+=1) {
 const outcome = pairedEntry.invoke();
 if (JSON.stringify(outcome) !== JSON.stringify({outcome:'returned',result:expected.toString()})) throw new Error(JSON.stringify(outcome));
 if ('status' in outcome || 'reason' in outcome) throw new Error('success published failure');
 const account = pairedEntry.account();
 if (account.reserved !== pairedMetadata || account.peak !== pairedMetadata+payload) throw new Error(JSON.stringify(account));
}
console.log('partition-paired-success-ok');
"#;

#[test]
fn paired_bounded_producer_keeps_its_status_and_reserves_no_failed_payload() {
    let text = SHARED_SOURCE.replace(
        "bytes_zeroed(65536usize)",
        "bytes_zeroed_bounded(65536usize, 65536usize)",
    );
    let failed = text
        .replace("65536usize, 65536usize", "65537usize, 65536usize")
        .replace("4096usize, 16usize", "4096usize, 0usize");
    let success = emitted(&text).entry_source();
    let failure = emitted(&failed).entry_source();
    let script = r#"
import assert from 'node:assert/strict';
import {install as successful} from './success.mjs';
import {install as failing} from './failure.mjs';
const metadata = 16384*6 + 8192*22 + 32*11 + 32*8;
const success = await successful(metadata + 65536);
const failure = await failing(metadata);
for (let i = 0; i < 4; i++) {
    assert.deepEqual(success.invoke(), {outcome: 'returned', result: '4096'});
    assert.deepEqual(failure.invoke(), {
        outcome: 'language_failure', status: {domain: 'semaprax.byte-buffer.v1', code: 2}
    });
    assert.equal(failure.account().reserved, metadata);
    assert.equal(failure.account().peak, metadata);
}
console.log('partition-bounded-entry-ok');
"#;
    if node_available() {
        let directory = stage(&[
            ("success.mjs", success.as_bytes()),
            ("failure.mjs", failure.as_bytes()),
            ("probe.mjs", script.as_bytes()),
        ]);
        assert!(run_node(&directory, "probe.mjs", &[]).contains("partition-bounded-entry-ok"));
    }
}
