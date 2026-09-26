//! Compiler-owned Core Wasm artifact assembly for the public-generic
//! provider target.  This module deliberately owns the physical artifact
//! identity and closed export inventory; it never reuses a C fixture or a
//! host-owned provider registry.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1;
use crate::public_generic_abi::digest;
use crate::public_generic_abi::wasm::binding::WasmProviderBindingV1;

mod byte_runtime;
mod carrier_classify;
mod carrier_codec;
mod component;
pub use component::PublicGenericWasmComponentArtifactV1;
pub(crate) use component::{emit as emit_component, replay as replay_component};

const ARTIFACT_DOMAIN: &[u8] = b"semaprax.public-generic-wasm-provider.v1.artifact\0";
const RUNTIME_DOMAIN: &[u8] = b"semaprax.public-generic-wasm-provider.v1.runtime\0";
const BINDING_PLACEHOLDER: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const BINDING_SLOT_BYTES: usize = BINDING_PLACEHOLDER.len();
const DESCRIPTOR_OFFSET: u32 = 4_096;
const BINDING_OFFSET: u32 = 131_072;
const SCRATCH_BASE: u32 = 393_216;
const MAX_SCRATCH_BYTES: u32 = 16 * 1024 * 1024 + 2_056;
const PRIVATE_BASE: u32 = SCRATCH_BASE + MAX_SCRATCH_BYTES;
const PROVIDER_MEMORY_LIMIT: u32 = (SCRATCH_BASE + MAX_SCRATCH_BYTES * 2).div_ceil(65_536) * 65_536;
const INPUT_LEAF_TABLE: u32 = PRIVATE_BASE + 1_024;
const RESULT_LEAF_TABLE: u32 = PRIVATE_BASE + 1_536;
const INPUT_AGGREGATE: u32 = PRIVATE_BASE + 4_096;
const RESULT_AGGREGATE: u32 = PRIVATE_BASE + 8_192;
const RESULT_CARRIER: u32 = PRIVATE_BASE + 16_384;
const MAX_COMPONENT_INPUT_PAYLOAD_BYTES: u32 = 2 * 65_536;
/// SHA-256 workspaces live in the initial (static) memory between the
/// descriptor's 64 KiB bound and the binding segment, so carrier admission
/// needs no private reservation. The shadow stack grows down from 64 KiB.
const STATIC_INPUT_SHA256_WORKSPACE: u32 = 98_304;
const STATIC_RESULT_SHA256_WORKSPACE: u32 = STATIC_INPUT_SHA256_WORKSPACE + 512;
const _: () = assert!(DESCRIPTOR_OFFSET + 64 * 1024 <= STATIC_INPUT_SHA256_WORKSPACE);
const _: () = assert!(STATIC_INPUT_SHA256_WORKSPACE + 288 <= STATIC_RESULT_SHA256_WORKSPACE);
const _: () = assert!(STATIC_RESULT_SHA256_WORKSPACE + 288 <= BINDING_OFFSET);

#[derive(Clone, Copy)]
pub(super) struct ProviderLayout {
    pub(super) input_sha256_workspace: u32,
    pub(super) result_sha256_workspace: u32,
    pub(super) input_leaf_table: u32,
    pub(super) result_leaf_table: u32,
    pub(super) input_payloads: u32,
    /// Exact bound of the private input payload window. The codec refuses a
    /// carrier whose payload total exceeds it with the capacity status.
    pub(super) input_payload_capacity: u32,
    pub(super) input_aggregate: u32,
    pub(super) result_aggregate: u32,
    pub(super) result_carrier: u32,
    pub(super) result_carrier_capacity: u32,
    /// Invocation-local owned-byte heap: token table, then payload bytes.
    pub(super) heap_table: u32,
    pub(super) heap_data: u32,
    pub(super) heap_end: u32,
    pub(super) workspace_end: u32,
}

const fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

// The owned-byte heap follows every predecessor region, so no earlier offset
// moves; only the memory limit grows to cover it.
// Two admitted leaves of at most 64 KiB each. The window previously began
// 2 KiB below the input aggregate, which silently overwrote payload bytes.
const MAX_INPUT_PAYLOAD_BYTES: u32 = 2 * 65_536;
const STANDALONE_INPUT_PAYLOADS: u32 = PROVIDER_MEMORY_LIMIT;
const STANDALONE_HEAP_TABLE: u32 = STANDALONE_INPUT_PAYLOADS + MAX_INPUT_PAYLOAD_BYTES;
const STANDALONE_HEAP_DATA: u32 = STANDALONE_HEAP_TABLE + 65_536;
const STANDALONE_HEAP_END: u32 = STANDALONE_HEAP_DATA + byte_runtime::HEAP_DATA_BYTES;
const _: () = assert!(byte_runtime::HEAP_TABLE_BYTES <= 65_536);
const _: () = assert!(STANDALONE_INPUT_PAYLOADS >= RESULT_CARRIER + MAX_SCRATCH_BYTES);
const _: () = assert!(MAX_COMPONENT_INPUT_PAYLOAD_BYTES == MAX_INPUT_PAYLOAD_BYTES);

const STANDALONE_PROVIDER_LAYOUT: ProviderLayout = ProviderLayout {
    input_sha256_workspace: STATIC_INPUT_SHA256_WORKSPACE,
    result_sha256_workspace: STATIC_RESULT_SHA256_WORKSPACE,
    input_leaf_table: INPUT_LEAF_TABLE,
    result_leaf_table: RESULT_LEAF_TABLE,
    input_payloads: STANDALONE_INPUT_PAYLOADS,
    input_payload_capacity: MAX_INPUT_PAYLOAD_BYTES,
    input_aggregate: INPUT_AGGREGATE,
    result_aggregate: RESULT_AGGREGATE,
    result_carrier: RESULT_CARRIER,
    result_carrier_capacity: MAX_SCRATCH_BYTES,
    heap_table: STANDALONE_HEAP_TABLE,
    heap_data: STANDALONE_HEAP_DATA,
    heap_end: STANDALONE_HEAP_END,
    workspace_end: align_up(STANDALONE_HEAP_END, 65_536),
};

const COMPONENT_INPUT_LEAF_TABLE: u32 = PRIVATE_BASE + 1_024;
const COMPONENT_RESULT_LEAF_TABLE: u32 = PRIVATE_BASE + 1_536;
const COMPONENT_INPUT_PAYLOADS: u32 = PRIVATE_BASE + 2_048;
const COMPONENT_INPUT_AGGREGATE: u32 = align_up(
    COMPONENT_INPUT_PAYLOADS + MAX_COMPONENT_INPUT_PAYLOAD_BYTES,
    16,
);
const COMPONENT_RESULT_AGGREGATE: u32 = COMPONENT_INPUT_AGGREGATE + 16;
const COMPONENT_RESULT_CARRIER: u32 = align_up(COMPONENT_RESULT_AGGREGATE + 16, 8);
const COMPONENT_HEAP_TABLE: u32 = align_up(
    COMPONENT_RESULT_CARRIER + carrier_codec::MAX_FRAME_WIRE_BYTES,
    65_536,
);
const COMPONENT_HEAP_DATA: u32 = COMPONENT_HEAP_TABLE + 65_536;
const COMPONENT_HEAP_END: u32 = COMPONENT_HEAP_DATA + byte_runtime::HEAP_DATA_BYTES;
const COMPONENT_PROVIDER_WORKSPACE_END: u32 = align_up(COMPONENT_HEAP_END, 65_536);
pub(super) const COMPONENT_PROVIDER_LAYOUT: ProviderLayout = ProviderLayout {
    input_sha256_workspace: STATIC_INPUT_SHA256_WORKSPACE,
    result_sha256_workspace: STATIC_RESULT_SHA256_WORKSPACE,
    input_leaf_table: COMPONENT_INPUT_LEAF_TABLE,
    result_leaf_table: COMPONENT_RESULT_LEAF_TABLE,
    input_payloads: COMPONENT_INPUT_PAYLOADS,
    input_payload_capacity: MAX_COMPONENT_INPUT_PAYLOAD_BYTES,
    input_aggregate: COMPONENT_INPUT_AGGREGATE,
    result_aggregate: COMPONENT_RESULT_AGGREGATE,
    result_carrier: COMPONENT_RESULT_CARRIER,
    result_carrier_capacity: carrier_codec::MAX_FRAME_WIRE_BYTES,
    heap_table: COMPONENT_HEAP_TABLE,
    heap_data: COMPONENT_HEAP_DATA,
    heap_end: COMPONENT_HEAP_END,
    workspace_end: COMPONENT_PROVIDER_WORKSPACE_END,
};

const _: () = assert!(PRIVATE_BASE + 1_024 <= COMPONENT_INPUT_LEAF_TABLE);
const _: () = assert!(COMPONENT_INPUT_LEAF_TABLE + 16 <= COMPONENT_RESULT_LEAF_TABLE);
const _: () = assert!(COMPONENT_RESULT_LEAF_TABLE + 16 <= COMPONENT_INPUT_PAYLOADS);
const _: () = assert!(COMPONENT_INPUT_PAYLOADS >= PRIVATE_BASE);
const _: () = assert!(
    COMPONENT_INPUT_PAYLOADS + MAX_COMPONENT_INPUT_PAYLOAD_BYTES <= COMPONENT_INPUT_AGGREGATE
);
const _: () = assert!(COMPONENT_INPUT_AGGREGATE + 16 <= COMPONENT_RESULT_AGGREGATE);
const _: () = assert!(COMPONENT_RESULT_AGGREGATE + 16 <= COMPONENT_RESULT_CARRIER);
const _: () = assert!(
    COMPONENT_RESULT_CARRIER + COMPONENT_PROVIDER_LAYOUT.result_carrier_capacity
        <= COMPONENT_PROVIDER_WORKSPACE_END
);
const _: () = assert!(
    COMPONENT_RESULT_CARRIER + COMPONENT_PROVIDER_LAYOUT.result_carrier_capacity
        <= COMPONENT_HEAP_TABLE
);
const _: () = assert!(COMPONENT_PROVIDER_WORKSPACE_END >= STANDALONE_PROVIDER_LAYOUT.workspace_end);
const BINDING_SLOT_CUSTOM_SECTION: &str = "semaprax.public-generic-provider-binding-slot.v1";
const COMPONENT_INPUT_ENCODE_EXPORT_V1: &str = "spx_pg_component_input_encode_v1";
const COMPONENT_RESULT_COPY_EXPORT_V1: &str = "spx_pg_component_result_copy_v1";

const GLOBAL_SCRATCH_RESERVED: u32 = 1;
const GLOBAL_PROVIDER: u32 = 2;
const GLOBAL_INPUT: u32 = 3;
const GLOBAL_RESULT: u32 = 4;
const GLOBAL_NEXT_HANDLE: u32 = 5;
const GLOBAL_INPUT_PTR: u32 = 6;
const GLOBAL_INPUT_LEN: u32 = 7;
const GLOBAL_RESULT_PTR: u32 = 8;
const GLOBAL_RESULT_LEN: u32 = 9;
const GLOBAL_INPUT_AGGREGATE: u32 = 10;
const GLOBAL_RESULT_AGGREGATE: u32 = 11;

/// The only production exports. Test observations intentionally live in a
/// separate harness and are never added to this inventory.
pub const EXPORTS: [&str; 11] = [
    "memory",
    "spx_pg_v1_scratch_ptr",
    "spx_pg_v1_scratch_reserve",
    "spx_pg_v1_scratch_capacity",
    "spx_pg_v1_open",
    "spx_pg_v1_input_prepare",
    "spx_pg_v1_call",
    "spx_pg_v1_result_export",
    "spx_pg_v1_value_release",
    "spx_pg_v1_result_release",
    "spx_pg_v1_provider_close",
];

/// Location of the fixed-width digest field within an emitted artifact. An
/// independent verifier must locate this exact data segment structurally,
/// prove it occurs once, and zero precisely this span before hashing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingDigestSlotV1 {
    pub offset: usize,
    pub len: usize,
}

/// A reproducible provider plus the binding that its `open` operation must
/// replay byte-for-byte. `binding_slot` makes the sole self-reference
/// explicit rather than relying on an unverifiable hash fixed point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicGenericWasmProviderArtifactV1 {
    wasm: Vec<u8>,
    descriptor: Vec<u8>,
    binding: WasmProviderBindingV1,
    binding_slot: BindingDigestSlotV1,
}

impl PublicGenericWasmProviderArtifactV1 {
    pub fn wasm(&self) -> &[u8] {
        &self.wasm
    }

    pub fn descriptor_bytes(&self) -> &[u8] {
        &self.descriptor
    }

    pub fn binding(&self) -> &WasmProviderBindingV1 {
        &self.binding
    }

    pub fn binding_bytes(&self) -> Vec<u8> {
        self.binding.encode()
    }

    pub fn artifact_digest(&self) -> String {
        artifact_digest(&self.wasm, self.binding_slot)
    }

    pub fn binding_digest(&self) -> String {
        self.binding.binding_digest()
    }

    pub fn binding_slot(&self) -> BindingDigestSlotV1 {
        self.binding_slot
    }

    /// Recompute the compiler artifact identity without trusting retained
    /// offsets. This is also the small independent verifier used by callers
    /// before they make any publication claim about the pair.
    pub fn verify(&self) -> Result<(), Diagnostic> {
        let slot = locate_binding_slot(&self.wasm, self.binding.encode().as_slice())?;
        if slot != self.binding_slot {
            return Err(error(
                "provider binding digest slot metadata disagrees with module data",
            ));
        }
        if artifact_digest(&self.wasm, slot) != self.binding.provider_artifact_digest() {
            return Err(error(
                "provider binding does not cover normalized Core Wasm artifact bytes",
            ));
        }
        Ok(())
    }
}

/// Emit a deterministic, import-free Core Wasm provider shell from the one
/// checked endpoint admission product. The semantic endpoint lowering is
/// deliberately accepted as an input only after the aggregate backend has
/// authenticated its closure; this module owns the physical ABI and binding
/// identity around that lowering.
pub fn emit(
    program: &ResolvedProgram,
    endpoint: &AdmittedPublicGenericEndpointV1,
) -> Result<PublicGenericWasmProviderArtifactV1, Diagnostic> {
    let core = emit_bound_core(program, endpoint, false)?;
    let binding_slot = locate_binding_slot(&core.wasm, &core.binding.encode())?;
    let artifact = PublicGenericWasmProviderArtifactV1 {
        wasm: core.wasm,
        descriptor: core.descriptor,
        binding: core.binding,
        binding_slot,
    };
    artifact.verify()?;
    Ok(artifact)
}

pub(crate) struct ComponentProviderCoreV1 {
    pub(crate) wasm: Vec<u8>,
    pub(crate) descriptor: Vec<u8>,
    pub(crate) binding: WasmProviderBindingV1,
}

pub(crate) fn emit_component_core(
    program: &ResolvedProgram,
    endpoint: &AdmittedPublicGenericEndpointV1,
) -> Result<ComponentProviderCoreV1, Diagnostic> {
    emit_bound_core(program, endpoint, true)
}

fn emit_bound_core(
    program: &ResolvedProgram,
    endpoint: &AdmittedPublicGenericEndpointV1,
    component_helpers: bool,
) -> Result<ComponentProviderCoreV1, Diagnostic> {
    let layout = if component_helpers {
        COMPONENT_PROVIDER_LAYOUT
    } else {
        STANDALONE_PROVIDER_LAYOUT
    };
    let runtime_identity = digest(RUNTIME_DOMAIN, b"closed-core-wasm-runtime-v1");
    let carrier = CarrierBindingV1::new(
        endpoint.descriptor().descriptor_digest(),
        TargetProfile::CoreWasm,
        runtime_identity,
    );
    let provisional = WasmProviderBindingV1::new_v2(
        carrier.clone(),
        BINDING_PLACEHOLDER,
        "spx_pg_v1_call",
        env!("CARGO_PKG_VERSION"),
    );
    let input_codec = carrier_codec::CarrierCodecEmission::new(
        carrier_codec::CarrierCodecFacts::from_verified_descriptor(
            endpoint.descriptor(),
            crate::public_generic_abi::carrier::trace::Direction::Input,
        )
        .map_err(error)?,
        carrier_codec::CarrierCodecLayout {
            trusted_data_offset: 196_608,
            sha256_workspace_offset: layout.input_sha256_workspace,
        },
    )
    .map_err(error)?;
    let result_codec = carrier_codec::CarrierCodecEmission::new(
        carrier_codec::CarrierCodecFacts::from_verified_descriptor(
            endpoint.descriptor(),
            crate::public_generic_abi::carrier::trace::Direction::Result,
        )
        .map_err(error)?,
        carrier_codec::CarrierCodecLayout {
            trusted_data_offset: 262_144,
            sha256_workspace_offset: layout.result_sha256_workspace,
        },
    )
    .map_err(error)?;
    if input_codec.leaf_count() != 2 || result_codec.leaf_count() != 2 {
        return Err(error(
            "Phase-B provider currently admits exactly two ordered owned Bytes leaves per endpoint direction",
        ));
    }
    let lowering = crate::wasm::aggregate::lower_public_generic_provider_closure(
        program,
        &crate::hir::DeclarationId::new(endpoint.export_id()),
        23,
    )?;
    let empty_input = {
        use crate::public_generic_abi::carrier::frame::{
            CarrierFrameBinding, CarrierLeaf, LeafKind,
        };
        let plan = CarrierFrameBinding::from_verified_descriptor(
            endpoint.descriptor(),
            crate::public_generic_abi::carrier::trace::Direction::Input,
        );
        plan.frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .map(|path| CarrierLeaf::new(path, LeafKind::Bytes, Vec::new()))
                .collect(),
        )
        .encode()
    };
    let classifier = carrier_classify::Classifier::new(&empty_input).map_err(error)?;
    let provisional_bytes = provisional.encode();
    let provisional_wasm = assemble(
        endpoint.descriptor_bytes(),
        &provisional_bytes,
        &lowering,
        &input_codec,
        &result_codec,
        &classifier,
        component_helpers,
        layout,
    )?;
    let provisional_slot = locate_binding_slot(&provisional_wasm, &provisional_bytes)?;
    let artifact_digest = artifact_digest(&provisional_wasm, provisional_slot);
    let binding = WasmProviderBindingV1::new_v2(
        carrier,
        artifact_digest,
        "spx_pg_v1_call",
        env!("CARGO_PKG_VERSION"),
    );
    let wasm = assemble(
        endpoint.descriptor_bytes(),
        &binding.encode(),
        &lowering,
        &input_codec,
        &result_codec,
        &classifier,
        component_helpers,
        layout,
    )?;
    Ok(ComponentProviderCoreV1 {
        wasm,
        descriptor: endpoint.descriptor_bytes().to_vec(),
        binding,
    })
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W121", message.into())
}

fn artifact_digest(wasm: &[u8], slot: BindingDigestSlotV1) -> String {
    let mut normalized = wasm.to_vec();
    normalized[slot.offset..slot.offset + slot.len].fill(0);
    digest(ARTIFACT_DOMAIN, &normalized)
}

fn locate_binding_slot(wasm: &[u8], binding: &[u8]) -> Result<BindingDigestSlotV1, Diagnostic> {
    let mut offsets = wasm
        .windows(binding.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == binding).then_some(offset));
    let binding_offset = offsets
        .next()
        .ok_or_else(|| error("provider binding data segment is absent"))?;
    if offsets.next().is_some() {
        return Err(error("provider binding data segment is ambiguous"));
    }
    let digest_offset = binding_artifact_digest_offset(binding)?;
    Ok(BindingDigestSlotV1 {
        offset: binding_offset + digest_offset,
        len: BINDING_SLOT_BYTES,
    })
}

fn binding_artifact_digest_offset(binding: &[u8]) -> Result<usize, Diagnostic> {
    // WasmProviderBindingV1 is seven exact length-framed fields. Its fourth
    // field is provider_artifact_digest; parsing positions rather than
    // searching for `sha256:` avoids confusing the descriptor/runtime digest
    // fields with the one intentionally normalized slot.
    let mut offset = 0_usize;
    for field in 0..4 {
        let header = binding
            .get(offset..offset + 8)
            .ok_or_else(|| error("provider binding digest field is truncated"))?;
        let len = u64::from_le_bytes(
            header
                .try_into()
                .map_err(|_| error("provider binding digest length is malformed"))?,
        );
        let len =
            usize::try_from(len).map_err(|_| error("provider binding digest length overflows"))?;
        let start = offset
            .checked_add(8)
            .ok_or_else(|| error("provider binding digest offset overflows"))?;
        let end = start
            .checked_add(len)
            .ok_or_else(|| error("provider binding digest field overflows"))?;
        let bytes = binding
            .get(start..end)
            .ok_or_else(|| error("provider binding digest field is truncated"))?;
        if field == 3 {
            if bytes.len() != BINDING_SLOT_BYTES || !bytes.starts_with(b"sha256:") {
                return Err(error(
                    "provider artifact digest is not the fixed-width sha256 slot",
                ));
            }
            return Ok(start);
        }
        offset = end;
    }
    Err(error("provider binding has no artifact digest field"))
}

#[allow(clippy::too_many_arguments)] // One deterministic module; every part is explicit.
fn assemble(
    descriptor: &[u8],
    binding: &[u8],
    lowering: &crate::wasm::aggregate::SelectedAggregateLowering,
    input_codec: &carrier_codec::CarrierCodecEmission,
    result_codec: &carrier_codec::CarrierCodecEmission,
    classifier: &carrier_classify::Classifier,
    component_helpers: bool,
    layout: ProviderLayout,
) -> Result<Vec<u8>, Diagnostic> {
    if descriptor.len() > 64 * 1024 || binding.len() > 256 * 1024 {
        return Err(error(
            "provider descriptor or binding exceeds its bounded data segment",
        ));
    }
    if lowering.bodies.len() != lowering.function_type_indexes.len() {
        return Err(error(
            "provider aggregate lowering returned different source body and signature counts",
        ));
    }
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut types = Vec::new();
    // Internal slots 0..=12 reproduce aggregate lowering's canonical
    // helper reservation without imports; ABI helpers occupy 13..=22 and
    // checked source bodies start at 23. Keep this order synchronized with
    // aggregate::provider_lowering.
    let signatures: &[(&[u8], &[u8])] = &[
        (&[0x7e, 0x7e], &[0x7e]),       // 0 arithmetic binary
        (&[0x7e], &[0x7e]),             // 1 arithmetic unary
        (&[0x7f], &[]),                 // 2 contract failure
        (&[0x7e], &[0x7e]),             // 3 bytes copy/as-slice/zeroed
        (&[0x7e, 0x7e], &[0x7f]),       // 4 bytes get
        (&[0x7e], &[]),                 // 5 bytes drop
        (&[0x7e, 0x7e, 0x7f], &[0x7e]), // 6 bytes set
        (&[], &[0x7f]),
        (&[0x7f], &[0x7e]),
        (&[0x7f, 0x7f, 0x7f, 0x7f], &[0x7e]),
        (&[0x7f, 0x7f, 0x7f], &[0x7e]),
        (&[0x7f, 0x7f], &[0x7e]),
        (&[0x7f], &[0x7f]),
    ];
    u32_leb(
        &mut types,
        (signatures.len()
            + lowering.types.len()
            + input_codec.type_count() as usize
            + result_codec.type_count() as usize
            + byte_runtime::TYPE_COUNT as usize
            + carrier_classify::TYPE_COUNT as usize) as u32,
    );
    for (params, results) in signatures {
        types.push(0x60);
        bytes(&mut types, params);
        bytes(&mut types, results);
    }
    for signature in &lowering.types {
        types.push(0x60);
        bytes(&mut types, &signature.params);
        bytes(&mut types, &signature.results);
    }
    let input_codec_type_base = signatures.len() as u32 + lowering.types.len() as u32;
    input_codec.append_type_entries(&mut types);
    let result_codec_type_base = input_codec_type_base + input_codec.type_count();
    result_codec.append_type_entries(&mut types);
    let byte_runtime_type_base = result_codec_type_base + result_codec.type_count();
    byte_runtime::append_type_entries(&mut types);
    let classify_type_base = byte_runtime_type_base + byte_runtime::TYPE_COUNT;
    carrier_classify::append_type_entries(&mut types);
    section(&mut module, 1, &types);

    let mut function_types = vec![0_u32, 0, 0, 0, 0, 1, 2, 3, 4, 5, 3, 3, 6];
    function_types.extend([7, 8, 7, 9, 10, 11, 10, 12, 12, 12]);
    let source_type_base = signatures.len() as u32;
    function_types.extend(
        lowering
            .function_type_indexes
            .iter()
            .map(|index| source_type_base + *index),
    );
    input_codec.append_function_type_indexes(&mut function_types, input_codec_type_base);
    result_codec.append_function_type_indexes(&mut function_types, result_codec_type_base);
    function_types.extend([byte_runtime_type_base, byte_runtime_type_base + 1]);
    function_types.extend([
        classify_type_base,
        classify_type_base + 1,
        classify_type_base + 2,
    ]);
    let mut functions = Vec::new();
    u32_leb(&mut functions, function_types.len() as u32);
    for index in &function_types {
        u32_leb(&mut functions, *index);
    }
    section(&mut module, 3, &functions);

    // Active data must fit at instantiation (before scratch reserve runs),
    // so the cold start reaches the exact binding segment but nothing from
    // the large public/private arenas. Core Wasm has no ambient authority
    // or imports.
    let static_end = input_codec
        .data_segments()
        .into_iter()
        .chain(result_codec.data_segments())
        .fold(BINDING_OFFSET + binding.len() as u32, |end, segment| {
            end.max(segment.offset + segment.bytes.len() as u32)
        });
    let min_pages = static_end.div_ceil(65_536).max(1);
    let mut memory = vec![1, 1];
    u32_leb(&mut memory, min_pages);
    let max_pages = if component_helpers {
        component::COMPONENT_MEMORY_PAGES
    } else {
        (layout.workspace_end / 65_536).max(1)
    };
    u32_leb(&mut memory, max_pages);
    section(&mut module, 5, &memory);

    // Global 0 is aggregate lowering's private shadow-stack pointer. Every
    // subsequent global is an unexported provider-owned state cell: nothing
    // in the host can mint, inspect, or repair a handle through it.
    let mut globals = Vec::new();
    u32_leb(&mut globals, 14);
    global_i32(&mut globals, 65_536); // aggregate shadow stack
    global_i32(&mut globals, 0); // scratch reserved
    global_i32(&mut globals, 0); // live provider id
    global_i32(&mut globals, 0); // live input id
    global_i32(&mut globals, 0); // live result id
    global_i32(&mut globals, 1); // next opaque id (zero is never issued)
    global_i32(&mut globals, 0); // input carrier pointer
    global_i32(&mut globals, 0); // input carrier length
    global_i32(&mut globals, 0); // result carrier pointer
    global_i32(&mut globals, 0); // result carrier length
    global_i32(&mut globals, layout.input_aggregate as i32); // aggregate input pointer
    global_i32(&mut globals, layout.result_aggregate as i32); // aggregate output pointer
    global_i32(&mut globals, layout.heap_data as i32); // owned-byte heap cursor
    global_i32(&mut globals, 1); // next owned-byte token (zero is never issued)
    section(&mut module, 6, &globals);

    let mut exports = Vec::new();
    u32_leb(
        &mut exports,
        (EXPORTS.len() + if component_helpers { 2 } else { 0 }) as u32,
    );
    name(&mut exports, EXPORTS[0]);
    exports.extend([0x02, 0x00]);
    for (index, export) in EXPORTS[1..].iter().enumerate() {
        name(&mut exports, export);
        exports.push(0x00);
        u32_leb(&mut exports, 13 + index as u32);
    }
    if component_helpers {
        let codec_function_base = 23 + lowering.function_type_indexes.len() as u32;
        name(&mut exports, COMPONENT_INPUT_ENCODE_EXPORT_V1);
        exports.push(0x00);
        u32_leb(&mut exports, codec_function_base + 3);
        name(&mut exports, COMPONENT_RESULT_COPY_EXPORT_V1);
        exports.push(0x00);
        u32_leb(
            &mut exports,
            codec_function_base + input_codec.function_count() + 2,
        );
    }
    section(&mut module, 7, &exports);

    let codec_function_base = 23 + lowering.function_type_indexes.len() as u32;
    let heap = byte_runtime::Heap {
        table: layout.heap_table,
        data: layout.heap_data,
        end: layout.heap_end,
        resolve: codec_function_base + input_codec.function_count() + result_codec.function_count(),
    };
    let mut code = Vec::new();
    u32_leb(&mut code, function_types.len() as u32);
    // Slots 0..=12: no-import aggregate helpers. Slots 7..=12 are the
    // provider-owned byte runtime (`byte_runtime`), not host imports.
    body_i64_binary(&mut code, 0x7c);
    body_i64_binary(&mut code, 0x7d);
    body_i64_binary(&mut code, 0x7e);
    body_i64_binary(&mut code, 0x7f);
    body_i64_binary(&mut code, 0x81);
    body_i64_neg(&mut code);
    body_void(&mut code);
    byte_runtime::runtime_bodies(&mut code, heap);
    // Slots 13..=22: scratch ptr, reserve, capacity and public lifecycle.
    body_i32_const(&mut code, SCRATCH_BASE as i32);
    body_scratch_reserve(&mut code, component_helpers);
    body_i32_const(&mut code, MAX_SCRATCH_BYTES as i32);
    body_open(&mut code, descriptor.len() as u32, binding.len() as u32);
    let input_indexes = input_codec.function_indexes(codec_function_base);
    let result_indexes =
        result_codec.function_indexes(codec_function_base + input_codec.function_count());
    let classify = carrier_classify::Indexes {
        utf8: heap.resolve + byte_runtime::FUNCTION_COUNT,
        sha256: input_indexes.sha256,
        workspace: layout.input_sha256_workspace,
    };
    body_input_prepare(
        &mut code,
        classify.classify(),
        input_indexes.copy,
        component_helpers,
        layout,
    );
    body_call(
        &mut code,
        lowering.selected_index,
        result_indexes.encode,
        layout,
        heap,
    );
    body_result_export(&mut code);
    body_value_release(&mut code);
    body_result_release(&mut code);
    body_provider_close(&mut code);
    for body in &lowering.bodies {
        u32_leb(&mut code, body.len() as u32);
        code.extend_from_slice(body);
    }
    for body in input_codec.bodies(codec_function_base) {
        u32_leb(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
    }
    for body in result_codec.bodies(codec_function_base + input_codec.function_count()) {
        u32_leb(&mut code, body.len() as u32);
        code.extend_from_slice(&body);
    }
    byte_runtime::helper_bodies(&mut code, heap);
    carrier_classify::bodies(&mut code, classifier, classify);
    section(&mut module, 10, &code);

    let mut data = Vec::new();
    u32_leb(&mut data, 5);
    active_data(&mut data, DESCRIPTOR_OFFSET, descriptor);
    active_data(&mut data, carrier_classify::DATA_OFFSET, classifier.data());
    active_data(&mut data, BINDING_OFFSET, binding);
    for segment in input_codec.data_segments() {
        active_data(&mut data, segment.offset, &segment.bytes);
    }
    for segment in result_codec.data_segments() {
        active_data(&mut data, segment.offset, &segment.bytes);
    }
    section(&mut module, 11, &data);

    // Structural locator for independent artifact verifiers. The payload is
    // deliberately data-segment-relative, not a raw module offset: the
    // latter would change when section-length LEB encodings change and would
    // reintroduce the very self-reference this fixed-width slot removes.
    let mut binding_slot = Vec::new();
    name(&mut binding_slot, BINDING_SLOT_CUSTOM_SECTION);
    binding_slot.extend([1, 1, 3]); // schema v1; binding segment; field four.
    u32_leb(&mut binding_slot, BINDING_SLOT_BYTES as u32);
    section(&mut module, 0, &binding_slot);
    Ok(module)
}

fn body_i32_const(code: &mut Vec<u8>, value: i32) {
    let mut body = vec![0, 0x41];
    i32_leb(&mut body, value);
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_void(code: &mut Vec<u8>) {
    let body = [0, 0x0b];
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_i64_binary(code: &mut Vec<u8>, opcode: u8) {
    let body = [0, 0x20, 0, 0x20, 1, opcode, 0x0b];
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_i64_neg(code: &mut Vec<u8>) {
    let body = [0, 0x42, 0, 0x20, 0, 0x7d, 0x0b];
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_call_then_contract_failure(code: &mut Vec<u8>, selected_index: u32) {
    // The actual lifecycle lowerer replaces the fixed scratch pointers with
    // descriptor-bound aggregate marshalling. Keeping the selected call in
    // this earliest artifact establishes that no fixture endpoint is wired
    // into the Core Wasm module: the only executable target is the checked
    // HIR closure returned by aggregate lowering.
    let mut body = vec![0, 0x41];
    i32_leb(&mut body, SCRATCH_BASE as i32);
    body.push(0x41);
    i32_leb(&mut body, (SCRATCH_BASE + 64) as i32);
    body.push(0x10);
    u32_leb(&mut body, selected_index);
    body.push(0x1a); // drop source status after its side effects/cleanup.
    body.push(0x42);
    i64_leb(&mut body, 11);
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_i64_lane(code: &mut Vec<u8>, status: u32, value: u32) {
    let lane = (u64::from(value) << 32) | u64::from(status);
    let mut body = vec![0, 0x42];
    i64_leb(&mut body, lane as i64);
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_scratch_reserve(code: &mut Vec<u8>, component_helpers: bool) {
    // Reserve is idempotent and only exposes the fixed public scratch
    // range. It never accepts a caller-selected pointer or allocates from a
    // host-owned arena.
    let required_pages = (SCRATCH_BASE + MAX_SCRATCH_BYTES).div_ceil(65_536);
    let mut body = vec![0];
    body.extend(local_get(0));
    body.extend(i32_const(MAX_SCRATCH_BYTES as i32));
    body.extend([0x4b, 0x04, 0x40]);
    lane(&mut body, 6, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(global_get(GLOBAL_SCRATCH_RESERVED));
    body.extend([0x45, 0x04, 0x40]); // if not reserved
    if component_helpers {
        body.extend([0x3f, 0x00]); // current memory pages
        body.extend(i32_const(required_pages as i32));
        body.extend([0x4f, 0x04, 0x40]); // if already large enough, skip growth
        body.push(0x05);
        body.extend([0x41]); // required - current pages
        i32_leb(&mut body, required_pages as i32);
        body.extend([0x3f, 0x00]);
        body.push(0x6b);
        body.extend([0x40, 0x00, 0x41]); // grow; failure is -1
        i32_leb(&mut body, -1);
        body.extend([0x46, 0x04, 0x40]);
        lane(&mut body, 10, 0);
        body.push(0x0f);
        body.push(0x0b);
        body.push(0x0b); // close the insufficient-pages branch
    } else {
        // Preserve the standalone provider v1 instruction stream exactly.
        body.extend([0x41]);
        i32_leb(&mut body, required_pages as i32);
        body.extend([0x3f, 0x00]);
        body.push(0x6b);
        body.extend([0x40, 0x00, 0x41]); // grow; failure is -1
        i32_leb(&mut body, -1);
        body.extend([0x46, 0x04, 0x40]);
        lane(&mut body, 10, 0);
        body.push(0x0f);
        body.push(0x0b);
    }
    body.extend(i32_const(1));
    body.extend(global_set(GLOBAL_SCRATCH_RESERVED));
    body.push(0x0b);
    lane(&mut body, 0, SCRATCH_BASE);
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_open(code: &mut Vec<u8>, descriptor_len: u32, binding_len: u32) {
    // (descriptor ptr,len,binding ptr,len) -> lane(status, provider id).
    // Both trusted blobs are compared byte-for-byte against module data;
    // equal lengths alone never constitute replay.
    let mut body = locals_i32(1);
    body.extend(global_get(GLOBAL_SCRATCH_RESERVED));
    body.extend([0x45, 0x04, 0x40]);
    lane(&mut body, 7, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(global_get(GLOBAL_PROVIDER));
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 7, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(1));
    body.extend(i32_const(descriptor_len as i32));
    body.extend([0x47, 0x04, 0x40]);
    lane(&mut body, 2, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(3));
    body.extend(i32_const(binding_len as i32));
    body.extend([0x47, 0x04, 0x40]);
    lane(&mut body, 4, 0);
    body.push(0x0f);
    body.push(0x0b);
    emit_scratch_bound(&mut body, 0, descriptor_len, 1);
    emit_scratch_bound(&mut body, 2, binding_len, 3);
    emit_exact_compare(&mut body, 0, descriptor_len, DESCRIPTOR_OFFSET, 2);
    emit_exact_compare(&mut body, 2, binding_len, BINDING_OFFSET, 4);
    body.extend(global_get(GLOBAL_NEXT_HANDLE));
    body.extend(global_set(GLOBAL_PROVIDER));
    body.extend(global_get(GLOBAL_NEXT_HANDLE));
    body.extend(i32_const(1));
    body.push(0x6a);
    body.extend(global_set(GLOBAL_NEXT_HANDLE));
    body.extend(global_get(GLOBAL_PROVIDER));
    body.push(0xad);
    body.extend(i64_const_imm(32));
    body.push(0x86);
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_input_prepare(
    code: &mut Vec<u8>,
    classify_index: u32,
    copy_index: u32,
    component_helpers: bool,
    layout: ProviderLayout,
) {
    // The carrier codec hooks into this fixed state transition. Until its
    // exact self-digest check has succeeded, no input id is stored or
    // exposed; this preserves a clean failure boundary for malformed bytes.
    let mut body = locals_i32_i64(1, 1);
    emit_live_handle_match(&mut body, 0, GLOBAL_PROVIDER);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 8, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(global_get(GLOBAL_INPUT));
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 7, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(global_get(GLOBAL_RESULT));
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 7, 0);
    body.push(0x0f);
    body.push(0x0b);
    // Admission reads the frame in place, so the scratch range must already
    // be backed by memory. Before scratch reserve (or an equivalent host
    // growth) this is a lifecycle-order refusal, never a trap.
    body.extend([0x3f, 0x00]);
    body.extend(i32_const(
        (SCRATCH_BASE + MAX_SCRATCH_BYTES).div_ceil(65_536) as i32,
    ));
    body.extend([0x49, 0x04, 0x40]);
    lane(&mut body, 7, 0);
    body.push(0x0f);
    body.push(0x0b);
    // The public carrier must be wholly inside the fixed scratch range.
    body.extend(local_get(1));
    body.extend(i32_const(SCRATCH_BASE as i32));
    body.extend([0x49, 0x04, 0x40]);
    lane(&mut body, 6, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(2));
    body.extend(i32_const(MAX_SCRATCH_BYTES as i32));
    body.extend([0x4b, 0x04, 0x40]);
    lane(&mut body, 6, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(1));
    body.extend(i32_const((SCRATCH_BASE + MAX_SCRATCH_BYTES) as i32));
    body.extend(local_get(2));
    body.push(0x6b);
    body.extend([0x4b, 0x04, 0x40]);
    lane(&mut body, 6, 0);
    body.push(0x0f);
    body.push(0x0b);
    // Admission precedes every physical operation: the ABI v2 classifier,
    // a port of the native authenticated frame check, decodes the complete
    // carrier, its self-digest and descriptor binding in static memory before
    // the private reservation grows linear memory. It returns 5, 6 or 14
    // exactly where native does; a refused carrier performs no memory.grow.
    body.extend(local_get(1));
    body.extend(local_get(2));
    body.push(0x10);
    u32_leb(&mut body, classify_index);
    body.extend(local_set(4));
    body.extend(local_get(4));
    body.push(0xa7);
    body.extend(local_set(3));
    // The classifier's low lane is already the physical status.
    body.extend(local_get(3));
    body.extend([0x04, 0x40]);
    body.extend(local_get(3));
    body.extend([0xad, 0x0f, 0x0b]);
    // The validated payload total (high lane) must fit the private window,
    // so a capacity refusal also precedes any memory growth.
    body.extend(local_get(4));
    body.extend(i64_const_imm(32));
    body.push(0x88);
    body.extend(i64_const_imm(i64::from(layout.input_payload_capacity)));
    body.extend([0x56, 0x04, 0x40]);
    lane(&mut body, 6, 0);
    body.push(0x0f);
    body.push(0x0b);
    emit_private_reserve(&mut body, component_helpers, layout.workspace_end);
    // Copy re-validates every carrier field plus its self-digest before it
    // writes private payloads and descriptor-ordered slice rows.
    body.extend(local_get(1));
    body.extend(local_get(2));
    body.extend(i32_const(layout.input_payloads as i32));
    body.extend(i32_const(layout.input_payload_capacity as i32));
    body.extend(i32_const(layout.input_leaf_table as i32));
    body.push(0x10);
    u32_leb(&mut body, copy_index);
    body.extend(local_set(4));
    body.extend(local_get(4));
    body.push(0xa7);
    body.extend(local_set(3));
    emit_codec_refusal(&mut body, 3);
    emit_slice_table_to_aggregate(&mut body, layout.input_leaf_table, layout.input_aggregate);
    body.extend(local_get(1));
    body.extend(global_set(GLOBAL_INPUT_PTR));
    body.extend(local_get(2));
    body.extend(global_set(GLOBAL_INPUT_LEN));
    body.extend(global_get(GLOBAL_NEXT_HANDLE));
    body.extend(global_set(GLOBAL_INPUT));
    body.extend(global_get(GLOBAL_NEXT_HANDLE));
    body.extend(i32_const(1));
    body.push(0x6a);
    body.extend(global_set(GLOBAL_NEXT_HANDLE));
    body.extend(global_get(GLOBAL_INPUT));
    body.push(0xad); // i64.extend_i32_u
    body.extend(i64_const_imm(32));
    body.push(0x86);
    body.extend(i64_lane(0, 0));
    body.push(0x84); // i64.or
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_call(
    code: &mut Vec<u8>,
    selected_index: u32,
    encode_index: u32,
    layout: ProviderLayout,
    heap: byte_runtime::Heap,
) {
    let mut body = locals_i32_i64(1, 1);
    emit_live_handle_match(&mut body, 0, GLOBAL_PROVIDER);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 8, 0);
    body.push(0x0f);
    body.push(0x0b);
    emit_live_handle_match(&mut body, 1, GLOBAL_INPUT);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 8, 0);
    body.push(0x0f);
    body.push(0x0b);
    // Codec marshalling stores a concrete aggregate at global 10 and a
    // clean output record at global 11. The selected checked HIR closure is
    // the only endpoint target in this module. Each invocation starts with a
    // fresh owned-byte heap holding exactly the two prepared input leaves.
    byte_runtime::emit_register_inputs(
        &mut body,
        heap,
        layout.input_leaf_table,
        layout.input_aggregate,
    );
    body.extend(global_get(GLOBAL_INPUT_AGGREGATE));
    body.extend(global_get(GLOBAL_RESULT_AGGREGATE));
    body.push(0x10);
    u32_leb(&mut body, selected_index);
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 11, 0);
    body.push(0x0f);
    body.push(0x0b);
    byte_runtime::emit_resolve_results(
        &mut body,
        heap,
        layout.result_aggregate,
        layout.result_leaf_table,
    );
    body.extend(i32_const(layout.result_leaf_table as i32));
    body.extend(i32_const(2));
    body.extend(i32_const(layout.result_carrier as i32));
    body.extend(i32_const(layout.result_carrier_capacity as i32));
    body.push(0x10);
    u32_leb(&mut body, encode_index);
    body.extend(local_set(3));
    body.extend(local_get(3));
    body.push(0xa7);
    body.extend(local_set(2));
    body.extend(local_get(2));
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x05);
    body.extend(local_get(2));
    body.extend(i32_const(carrier_codec::STATUS_CAPACITY as i32));
    body.extend([0x46, 0x04, 0x7e]);
    lane(&mut body, 6, 0);
    body.push(0x05);
    lane(&mut body, 5, 0);
    body.push(0x0b);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(i32_const(layout.result_carrier as i32));
    body.extend(global_set(GLOBAL_RESULT_PTR));
    body.extend(local_get(3));
    body.extend(i64_const_imm(32));
    body.push(0x88);
    body.push(0xa7);
    body.extend(global_set(GLOBAL_RESULT_LEN));
    body.extend(i32_const(0));
    body.extend(global_set(GLOBAL_INPUT));
    body.extend(global_get(GLOBAL_NEXT_HANDLE));
    body.extend(global_set(GLOBAL_RESULT));
    body.extend(global_get(GLOBAL_NEXT_HANDLE));
    body.extend(i32_const(1));
    body.push(0x6a);
    body.extend(global_set(GLOBAL_NEXT_HANDLE));
    body.extend(global_get(GLOBAL_RESULT));
    body.push(0xad);
    body.extend(i64_const_imm(32));
    body.push(0x86); // i64.shl: success result handle occupies the high value lane.
    body.extend(i64_lane(0, 0));
    body.push(0x84); // i64.or: the low status lane is success (zero).
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_result_export(code: &mut Vec<u8>) {
    let mut body = vec![0];
    emit_live_handle_match(&mut body, 0, GLOBAL_RESULT);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    lane(&mut body, 8, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(2));
    body.extend(global_get(GLOBAL_RESULT_LEN));
    body.extend([0x49, 0x04, 0x40]);
    body.extend(global_get(GLOBAL_RESULT_LEN));
    body.push(0xad);
    body.extend(i64_const_imm(32));
    body.push(0x86);
    body.extend(i64_lane(12, 0));
    body.push(0x84);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(1));
    body.extend(global_get(GLOBAL_RESULT_PTR));
    body.extend(global_get(GLOBAL_RESULT_LEN));
    body.extend([0xfc, 0x0a, 0, 0]); // memory.copy dst src len
    body.extend(global_get(GLOBAL_RESULT_LEN));
    body.push(0xad);
    body.extend(i64_const_imm(32));
    body.push(0x86);
    body.extend(i64_lane(0, 0));
    body.push(0x84);
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_value_release(code: &mut Vec<u8>) {
    let mut body = vec![0];
    emit_live_handle_match(&mut body, 0, GLOBAL_INPUT);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    body.extend(i32_const(8));
    body.push(0x0f);
    body.push(0x0b);
    body.extend(i32_const(0));
    body.extend(global_set(GLOBAL_INPUT));
    body.extend(i32_const(0));
    body.extend(global_set(GLOBAL_INPUT_LEN));
    body.extend(i32_const(0));
    body.extend(global_set(GLOBAL_INPUT_PTR));
    body.extend(i32_const(0));
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_result_release(code: &mut Vec<u8>) {
    let mut body = vec![0];
    emit_live_handle_match(&mut body, 0, GLOBAL_RESULT);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    body.extend(i32_const(8));
    body.push(0x0f);
    body.push(0x0b);
    for global in [GLOBAL_RESULT, GLOBAL_RESULT_PTR, GLOBAL_RESULT_LEN] {
        body.extend(i32_const(0));
        body.extend(global_set(global));
    }
    body.extend(i32_const(0));
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

fn body_provider_close(code: &mut Vec<u8>) {
    let mut body = vec![0];
    emit_live_handle_match(&mut body, 0, GLOBAL_PROVIDER);
    body.extend([0x04, 0x40]);
    body.push(0x05);
    body.extend(i32_const(8));
    body.push(0x0f);
    body.push(0x0b);
    body.extend(global_get(GLOBAL_INPUT));
    body.push(0x45);
    body.extend(global_get(GLOBAL_RESULT));
    body.extend([0x45, 0x71, 0x04, 0x40]);
    body.push(0x05);
    body.extend(i32_const(7));
    body.push(0x0f);
    body.push(0x0b);
    body.extend(i32_const(0));
    body.extend(global_set(GLOBAL_PROVIDER));
    body.extend(i32_const(0));
    body.push(0x0b);
    u32_leb(code, body.len() as u32);
    code.extend(body);
}

// Zero is the empty slot sentinel, never admitted as a live provider/value/result.
// Equality alone would authenticate an absent handle after initialization or
// release, including a zero-input call before any owned value was prepared.
fn emit_live_handle_match(body: &mut Vec<u8>, local: u32, global: u32) {
    body.extend(local_get(local));
    body.extend(global_get(global));
    body.push(0x46); // i32.eq
    body.extend(local_get(local));
    body.extend([0x45, 0x45, 0x71]); // nonzero && equal
}

fn emit_exact_compare(
    body: &mut Vec<u8>,
    supplied_pointer: u32,
    len: u32,
    expected: u32,
    status: u32,
) {
    // local 4 is the byte index in every `open` body.
    body.extend(i32_const(0));
    body.extend(local_set(4));
    body.extend([0x02, 0x40, 0x03, 0x40]);
    body.extend(local_get(4));
    body.extend(i32_const(len as i32));
    body.extend([0x4f, 0x0d, 0x01]);
    body.extend(local_get(supplied_pointer));
    body.extend(local_get(4));
    body.push(0x6a);
    body.extend([0x2d, 0, 0]);
    body.extend(i32_const(expected as i32));
    body.extend(local_get(4));
    body.push(0x6a);
    body.extend([0x2d, 0, 0, 0x47, 0x04, 0x40]);
    lane(body, status, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(4));
    body.extend(i32_const(1));
    body.push(0x6a);
    body.extend(local_set(4));
    body.extend([0x0c, 0x00, 0x0b, 0x0b]);
}

fn emit_scratch_bound(body: &mut Vec<u8>, pointer: u32, len: u32, status: u32) {
    // `pointer <= end - len` proves the later byte loads cannot wrap or
    // trap. The ABI surface stays total for hostile numeric arguments.
    body.extend(local_get(pointer));
    body.extend(i32_const(SCRATCH_BASE as i32));
    body.extend([0x49, 0x04, 0x40]);
    lane(body, status, 0);
    body.push(0x0f);
    body.push(0x0b);
    body.extend(local_get(pointer));
    body.extend(i32_const((SCRATCH_BASE + MAX_SCRATCH_BYTES - len) as i32));
    body.extend([0x4b, 0x04, 0x40]);
    lane(body, status, 0);
    body.push(0x0f);
    body.push(0x0b);
}

fn emit_private_reserve(body: &mut Vec<u8>, component_helpers: bool, workspace_end: u32) {
    let pages = workspace_end.div_ceil(65_536);
    if component_helpers {
        // Component constructors can grow memory beyond the provider's own
        // private floor. Skip the subtraction/grow path when that floor is
        // already met; unsigned subtraction would otherwise wrap.
        body.extend([0x3f, 0x00]);
        body.extend(i32_const(pages as i32));
        body.extend([0x4b, 0x04, 0x40, 0x05]);
    }
    body.extend(i32_const(pages as i32));
    body.extend([0x3f, 0x00, 0x6b, 0x40, 0x00]);
    body.extend(i32_const(-1));
    body.extend([0x46, 0x04, 0x40]);
    lane(body, 10, 0);
    body.push(0x0f);
    body.push(0x0b);
    if component_helpers {
        body.push(0x0b);
    }
}

/// Return the Wasm adapter ABI v2 physical refusal for a nonzero codec
/// status in `local`: capacity maps to 6, a decoded carrier whose semantic
/// binding does not replay (`SPX-PG803`) to 14, every other refusal to 5.
fn emit_codec_refusal(body: &mut Vec<u8>, local: u32) {
    body.extend(local_get(local));
    body.extend([0x45, 0x04, 0x40]);
    body.push(0x05);
    body.extend(local_get(local));
    body.extend(i32_const(carrier_codec::STATUS_CAPACITY as i32));
    body.extend([0x46, 0x04, 0x7e]);
    lane(body, 6, 0);
    body.push(0x05);
    body.extend(local_get(local));
    body.extend(i32_const(carrier_codec::STATUS_REPLAY_MISMATCH as i32));
    body.extend([0x46, 0x04, 0x7e]);
    lane(
        body,
        crate::public_generic_abi::wasm::binding::WASM_ADAPTER_V2_STATUS_CARRIER_REPLAY_MISMATCH,
        0,
    );
    body.push(0x05);
    lane(body, 5, 0);
    body.push(0x0b);
    body.push(0x0b);
    body.push(0x0f);
    body.push(0x0b);
}

fn emit_slice_table_to_aggregate(body: &mut Vec<u8>, table: u32, aggregate: u32) {
    for leaf in 0..2_u32 {
        body.extend(i32_const((aggregate + leaf * 8) as i32));
        body.extend(i32_const((table + leaf * 8) as i32));
        body.extend([0x28, 2, 0, 0xad]);
        body.extend(i64_const_imm(32));
        body.push(0x86);
        body.extend(i32_const((table + leaf * 8 + 4) as i32));
        body.extend([0x28, 2, 0, 0xad, 0x84, 0x37, 3, 0]);
    }
}

fn global_i32(out: &mut Vec<u8>, value: i32) {
    out.extend([0x7f, 1, 0x41]);
    i32_leb(out, value);
    out.push(0x0b);
}

fn locals_i32(count: u32) -> Vec<u8> {
    vec![1, count as u8, 0x7f]
}

fn locals_i32_i64(i32_count: u32, i64_count: u32) -> Vec<u8> {
    vec![2, i32_count as u8, 0x7f, i64_count as u8, 0x7e]
}

fn local_get(index: u32) -> Vec<u8> {
    let mut out = vec![0x20];
    u32_leb(&mut out, index);
    out
}

fn local_set(index: u32) -> Vec<u8> {
    let mut out = vec![0x21];
    u32_leb(&mut out, index);
    out
}

fn global_get(index: u32) -> Vec<u8> {
    let mut out = vec![0x23];
    u32_leb(&mut out, index);
    out
}

fn global_set(index: u32) -> Vec<u8> {
    let mut out = vec![0x24];
    u32_leb(&mut out, index);
    out
}

fn i32_const(value: i32) -> Vec<u8> {
    let mut out = vec![0x41];
    i32_leb(&mut out, value);
    out
}

fn i64_lane(status: u32, value: u32) -> Vec<u8> {
    let mut out = vec![0x42];
    i64_leb(
        &mut out,
        ((u64::from(value) << 32) | u64::from(status)) as i64,
    );
    out
}

fn i64_const_imm(value: i64) -> Vec<u8> {
    let mut out = vec![0x42];
    i64_leb(&mut out, value);
    out
}

fn lane(out: &mut Vec<u8>, status: u32, value: u32) {
    out.extend(i64_lane(status, value));
}

fn active_data(out: &mut Vec<u8>, offset: u32, data: &[u8]) {
    out.push(0);
    out.push(0x41);
    i32_leb(out, offset as i32);
    out.push(0x0b);
    bytes(out, data);
}

fn section(module: &mut Vec<u8>, id: u8, contents: &[u8]) {
    module.push(id);
    u32_leb(module, contents.len() as u32);
    module.extend_from_slice(contents);
}

fn name(out: &mut Vec<u8>, value: &str) {
    bytes(out, value.as_bytes());
}

fn bytes(out: &mut Vec<u8>, value: &[u8]) {
    u32_leb(out, value.len() as u32);
    out.extend_from_slice(value);
}

fn u32_leb(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn i32_leb(out: &mut Vec<u8>, mut value: i32) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        out.push(if done { byte } else { byte | 0x80 });
        if done {
            return;
        }
    }
}

fn i64_leb(out: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        out.push(if done { byte } else { byte | 0x80 });
        if done {
            return;
        }
    }
}
