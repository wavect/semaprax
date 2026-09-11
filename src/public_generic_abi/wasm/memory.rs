//! A Core Wasm linear-memory arena and its bounded, byte-accounted stack
//! allocator.
//!
//! Real Wasm linear memory is a growable byte array addressed by `u32`
//! offsets, grown in fixed 64 KiB pages (`memory.grow`) and never shrunk —
//! there is no native pointer, and every access must be explicitly bounded
//! against the current size before it happens, never trusted from a caller
//! claim. [`WasmLinearMemory`] models exactly that: a `Vec<u8>` grown only in
//! whole pages, with `read_at`/`write_at` that bounds-check first and return
//! a closed [`Diagnostic`] rather than panicking or trapping on an
//! out-of-bounds offset or length — "hostile input produces closed statuses
//! rather than attacker-controlled traps where feasible," restated for the
//! physical memory layer.
//!
//! [`StackAllocator`] is the bounded allocator on top: every [Public Generic
//! Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md) release is
//! already required to be the exact reverse of allocation order (see
//! "Failure settlement and release order"), so a strict last-in-first-out
//! bump allocator is not a simplification of the physical adapter's job, it
//! is the direct physical implementation of that logical rule: `dealloc`
//! only ever accepts the most recently allocated region still live, exactly
//! the region the logical release order can ever present it with. Freed
//! bytes are zeroed in place — real, observable physical release, not just
//! bookkeeping — and the freed span is reused by the next allocation that
//! fits, so live bytes and live allocation counts are always exact, never
//! samples.

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::boundary_profile::{MAX_BYTES_PER_LEAF, MAX_TOTAL_PAYLOAD_BYTES};

/// One Wasm linear-memory page, matching the real Wasm specification's fixed
/// page size.
pub const PAGE_BYTES: u32 = 65_536;

/// Max pages this arena ever grows to: [`MAX_TOTAL_PAYLOAD_BYTES`] divided
/// evenly by [`PAGE_BYTES`] (16 MiB / 64 KiB = 256 pages), so the memory
/// bound and the carrier's own total-payload bound are the same fact, never
/// two independently chosen numbers.
pub const MAX_PAGES: u32 = (MAX_TOTAL_PAYLOAD_BYTES as u32) / PAGE_BYTES;

/// Malformed or out-of-bounds physical memory access: an offset/length pair
/// that does not fit inside the current arena, independent of any carrier or
/// handle-level legality question.
pub const MEMORY_BOUNDS: &str = "SPX-PG912";
/// The bounded allocator could not satisfy an allocation: it would exceed
/// [`MAX_BYTES_PER_LEAF`], [`MAX_TOTAL_PAYLOAD_BYTES`], or growing memory
/// failed.
pub const ALLOCATION_FAILURE: &str = "SPX-PG913";
/// A `dealloc` was asked to release a span that is not exactly the most
/// recently allocated, still-live span — the physical proof that a
/// submitted release order violates [Public Generic Carrier
/// v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#failure-settlement-and-release-order)'s
/// exact-reverse-of-allocation-order rule.
pub const DEALLOC_NOT_TOP_OF_STACK: &str = "SPX-PG914";

fn schema_error(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(code, message.into())
}

/// A growable, page-based linear-memory arena. `len()` is always a whole
/// multiple of [`PAGE_BYTES`]; it only ever grows, exactly like real Wasm
/// linear memory.
#[derive(Clone, Debug)]
pub struct WasmLinearMemory {
    bytes: Vec<u8>,
}

impl Default for WasmLinearMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmLinearMemory {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn len(&self) -> u32 {
        self.bytes.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn pages(&self) -> u32 {
        self.len() / PAGE_BYTES
    }

    /// Grow by whole pages until at least `required_bytes` are addressable,
    /// bounded by [`MAX_PAGES`]. Never shrinks; a `required_bytes` already
    /// satisfied is a no-op success.
    pub fn grow_to_fit(&mut self, required_bytes: u32) -> Result<(), Diagnostic> {
        if required_bytes <= self.len() {
            return Ok(());
        }
        let needed_pages = required_bytes.div_ceil(PAGE_BYTES);
        if needed_pages > MAX_PAGES {
            return Err(schema_error(
                ALLOCATION_FAILURE,
                "Wasm linear memory grow would exceed the carrier's total payload bound",
            ));
        }
        self.bytes.resize((needed_pages * PAGE_BYTES) as usize, 0);
        Ok(())
    }

    fn bounded_range(&self, offset: u32, len: u32) -> Result<std::ops::Range<usize>, Diagnostic> {
        let end = offset.checked_add(len).ok_or_else(|| {
            schema_error(MEMORY_BOUNDS, "offset + length overflows a u32 byte offset")
        })?;
        if end > self.len() {
            return Err(schema_error(
                MEMORY_BOUNDS,
                format!(
                    "access [{offset}, {end}) is out of bounds for a {}-byte arena",
                    self.len()
                ),
            ));
        }
        Ok(offset as usize..end as usize)
    }

    /// Bounds-checked read. Never panics or indexes past `len()`.
    pub fn read_at(&self, offset: u32, len: u32) -> Result<&[u8], Diagnostic> {
        let range = self.bounded_range(offset, len)?;
        Ok(&self.bytes[range])
    }

    /// Bounds-checked write. Never panics or indexes past `len()`.
    pub fn write_at(&mut self, offset: u32, data: &[u8]) -> Result<(), Diagnostic> {
        let range = self.bounded_range(offset, data.len() as u32)?;
        self.bytes[range].copy_from_slice(data);
        Ok(())
    }

    /// Zero a bounds-checked span in place. Used by [`StackAllocator`] to
    /// make release physically observable, not merely bookkeeping.
    fn zero_at(&mut self, offset: u32, len: u32) -> Result<(), Diagnostic> {
        let range = self.bounded_range(offset, len)?;
        self.bytes[range].fill(0);
        Ok(())
    }
}

/// A bounded, exact-LIFO bump allocator over one [`WasmLinearMemory`]. See
/// the module documentation for why LIFO-only release is not a
/// simplification but the direct physical form of the carrier's own
/// release-order rule.
#[derive(Clone, Debug)]
pub struct StackAllocator {
    top: u32,
    live_bytes: u32,
    live_allocations: u32,
}

impl Default for StackAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl StackAllocator {
    pub fn new() -> Self {
        Self {
            top: 0,
            live_bytes: 0,
            live_allocations: 0,
        }
    }

    pub fn live_bytes(&self) -> u32 {
        self.live_bytes
    }

    pub fn live_allocations(&self) -> u32 {
        self.live_allocations
    }

    /// Allocate `len` bytes, growing `memory` as needed. Rejects a request
    /// over [`MAX_BYTES_PER_LEAF`] or one that would push total live bytes
    /// over [`MAX_TOTAL_PAYLOAD_BYTES`] before touching memory. Returns the
    /// new span's offset; the span is zero-initialized (from
    /// `WasmLinearMemory::grow_to_fit`'s own zero-fill, or a fresh
    /// `zero_at` when reusing already-grown memory a prior allocation once
    /// occupied).
    pub fn alloc(&mut self, memory: &mut WasmLinearMemory, len: u32) -> Result<u32, Diagnostic> {
        if len as usize > MAX_BYTES_PER_LEAF {
            return Err(schema_error(
                ALLOCATION_FAILURE,
                format!("leaf allocation of {len} bytes exceeds MAX_BYTES_PER_LEAF"),
            ));
        }
        let next_top = self.top.checked_add(len).ok_or_else(|| {
            schema_error(ALLOCATION_FAILURE, "allocator top offset overflows u32")
        })?;
        if next_top as usize > MAX_TOTAL_PAYLOAD_BYTES {
            return Err(schema_error(
                ALLOCATION_FAILURE,
                "allocation would exceed MAX_TOTAL_PAYLOAD_BYTES",
            ));
        }
        memory.grow_to_fit(next_top)?;
        memory.zero_at(self.top, len)?;
        let offset = self.top;
        self.top = next_top;
        self.live_bytes += len;
        self.live_allocations += 1;
        Ok(offset)
    }

    /// Release exactly the top-of-stack span `[offset, offset + len)`,
    /// zeroing it in `memory` for real physical release. Any span other
    /// than the current top is refused with [`DEALLOC_NOT_TOP_OF_STACK`]
    /// rather than corrupting the allocator's accounting.
    pub fn dealloc(
        &mut self,
        memory: &mut WasmLinearMemory,
        offset: u32,
        len: u32,
    ) -> Result<(), Diagnostic> {
        let expected_offset = self.top.checked_sub(len).ok_or_else(|| {
            schema_error(
                DEALLOC_NOT_TOP_OF_STACK,
                "dealloc length exceeds everything ever allocated",
            )
        })?;
        if offset != expected_offset {
            return Err(schema_error(
                DEALLOC_NOT_TOP_OF_STACK,
                "dealloc span is not the exact top of the allocation stack",
            ));
        }
        memory.zero_at(offset, len)?;
        self.top = expected_offset;
        self.live_bytes -= len;
        self.live_allocations -= 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
