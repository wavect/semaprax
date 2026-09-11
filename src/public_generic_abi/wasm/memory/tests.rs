use super::*;

#[test]
fn grow_to_fit_rounds_up_to_whole_pages_and_never_shrinks() {
    let mut memory = WasmLinearMemory::new();
    assert_eq!(memory.len(), 0);
    memory.grow_to_fit(1).unwrap();
    assert_eq!(memory.len(), PAGE_BYTES);
    memory.grow_to_fit(PAGE_BYTES).unwrap();
    assert_eq!(memory.len(), PAGE_BYTES);
    memory.grow_to_fit(PAGE_BYTES + 1).unwrap();
    assert_eq!(memory.len(), PAGE_BYTES * 2);
    // Never shrinks even when asked for less than the current size.
    memory.grow_to_fit(1).unwrap();
    assert_eq!(memory.len(), PAGE_BYTES * 2);
}

#[test]
fn grow_to_fit_rejects_a_request_over_the_page_bound() {
    let mut memory = WasmLinearMemory::new();
    let error = memory.grow_to_fit(MAX_PAGES * PAGE_BYTES + 1).unwrap_err();
    assert_eq!(error.code, ALLOCATION_FAILURE);
}

#[test]
fn read_write_round_trip_bounds_checked() {
    let mut memory = WasmLinearMemory::new();
    memory.grow_to_fit(PAGE_BYTES).unwrap();
    memory.write_at(10, b"hello").unwrap();
    assert_eq!(memory.read_at(10, 5).unwrap(), b"hello");
}

#[test]
fn read_past_the_end_is_a_closed_status_not_a_panic() {
    let memory = WasmLinearMemory::new();
    let error = memory.read_at(0, 1).unwrap_err();
    assert_eq!(error.code, MEMORY_BOUNDS);
}

#[test]
fn write_past_the_end_is_a_closed_status_not_a_panic() {
    let mut memory = WasmLinearMemory::new();
    memory.grow_to_fit(PAGE_BYTES).unwrap();
    let error = memory.write_at(PAGE_BYTES - 1, b"ab").unwrap_err();
    assert_eq!(error.code, MEMORY_BOUNDS);
}

#[test]
fn overflowing_offset_plus_length_is_a_closed_status() {
    let memory = WasmLinearMemory::new();
    let error = memory.read_at(u32::MAX, 2).unwrap_err();
    assert_eq!(error.code, MEMORY_BOUNDS);
}

#[test]
fn allocator_alloc_dealloc_round_trip_zeroes_and_reuses_the_span() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    let a = allocator.alloc(&mut memory, 4).unwrap();
    memory.write_at(a, b"data").unwrap();
    assert_eq!(allocator.live_bytes(), 4);
    assert_eq!(allocator.live_allocations(), 1);
    allocator.dealloc(&mut memory, a, 4).unwrap();
    assert_eq!(allocator.live_bytes(), 0);
    assert_eq!(allocator.live_allocations(), 0);
    // The freed span is physically zeroed, not merely unaccounted.
    assert_eq!(memory.read_at(a, 4).unwrap(), &[0, 0, 0, 0]);
    // The freed span is reused by the next allocation.
    let b = allocator.alloc(&mut memory, 4).unwrap();
    assert_eq!(a, b);
}

#[test]
fn allocator_rejects_a_leaf_over_the_per_leaf_bound() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    let error = allocator
        .alloc(&mut memory, (MAX_BYTES_PER_LEAF as u32) + 1)
        .unwrap_err();
    assert_eq!(error.code, ALLOCATION_FAILURE);
    assert_eq!(allocator.live_bytes(), 0);
    assert_eq!(allocator.live_allocations(), 0);
}

#[test]
fn allocator_exact_leaf_bound_succeeds() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    allocator
        .alloc(&mut memory, MAX_BYTES_PER_LEAF as u32)
        .unwrap();
    assert_eq!(allocator.live_bytes(), MAX_BYTES_PER_LEAF as u32);
}

#[test]
fn allocator_dealloc_rejects_a_span_that_is_not_top_of_stack() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    let first = allocator.alloc(&mut memory, 4).unwrap();
    let _second = allocator.alloc(&mut memory, 4).unwrap();
    // Releasing the first (bottom) allocation while the second (top) is
    // still live is not the exact reverse of allocation order.
    let error = allocator.dealloc(&mut memory, first, 4).unwrap_err();
    assert_eq!(error.code, DEALLOC_NOT_TOP_OF_STACK);
    // The rejected attempt mutates nothing.
    assert_eq!(allocator.live_bytes(), 8);
    assert_eq!(allocator.live_allocations(), 2);
}

#[test]
fn allocator_dealloc_rejects_double_release() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    let a = allocator.alloc(&mut memory, 4).unwrap();
    allocator.dealloc(&mut memory, a, 4).unwrap();
    let error = allocator.dealloc(&mut memory, a, 4).unwrap_err();
    assert_eq!(error.code, DEALLOC_NOT_TOP_OF_STACK);
}

#[test]
fn allocator_reverse_order_release_of_several_spans_reaches_exactly_zero() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    let spans: Vec<(u32, u32)> = (0..5)
        .map(|index| (allocator.alloc(&mut memory, index + 1).unwrap(), index + 1))
        .collect();
    for (offset, len) in spans.into_iter().rev() {
        allocator.dealloc(&mut memory, offset, len).unwrap();
    }
    assert_eq!(allocator.live_bytes(), 0);
    assert_eq!(allocator.live_allocations(), 0);
}

#[test]
fn zero_length_allocation_is_immediately_releasable() {
    let mut memory = WasmLinearMemory::new();
    let mut allocator = StackAllocator::new();
    let offset = allocator.alloc(&mut memory, 0).unwrap();
    assert_eq!(allocator.live_allocations(), 1);
    allocator.dealloc(&mut memory, offset, 0).unwrap();
    assert_eq!(allocator.live_allocations(), 0);
}
