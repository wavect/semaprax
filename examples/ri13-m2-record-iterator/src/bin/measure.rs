//! Local RI-13 M2 record and callback comparison; each sample is raw CSV.
include!("../generated.rs");

use serde::{Deserialize, Serialize};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::time::Instant;

const WARMUP: usize = 1;
const SAMPLES: usize = 5;
const OPERATIONS: usize = 32;
const JSON: [&str; 2] = [
    r#"{"value":1,"label":"one"}"#,
    r#"{"value":2,"label":"two"}"#,
];

#[derive(Clone, Copy, Default)]
struct Allocations {
    calls: u64,
    bytes: u64,
}

thread_local! {
    static ACTIVE: Cell<Option<Allocations>> = const { Cell::new(None) };
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn note(bytes: usize) {
    let _ = ACTIVE.try_with(|slot| {
        if let Some(mut current) = slot.get() {
            current.calls += 1;
            current.bytes = current.bytes.saturating_add(bytes as u64);
            slot.set(Some(current));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        note(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
}

fn measured<T>(operation: impl FnOnce() -> T) -> (T, Allocations) {
    ACTIVE.with(|slot| assert!(slot.replace(Some(Allocations::default())).is_none()));
    let result = operation();
    let allocations = ACTIVE.with(|slot| slot.replace(None).unwrap());
    (result, allocations)
}

#[derive(Deserialize, Serialize)]
struct DirectEvent {
    value: i64,
    label: String,
}

struct HandwrittenAdapter {
    state: i64,
}

struct HandwrittenRecordAdapter;

impl HandwrittenRecordAdapter {
    fn decode(&self, source: &str) -> DirectEvent {
        serde_json::from_str(source).unwrap()
    }

    fn encode(&self, event: &DirectEvent) -> String {
        serde_json::to_string(event).unwrap()
    }
}

impl HandwrittenAdapter {
    fn call(&mut self, value: i64) -> Result<i64, &'static str> {
        if value < 0 {
            return Err("negative callback input");
        }
        self.state = self.state.checked_add(value).ok_or("callback overflow")?;
        Ok(self.state)
    }
}

#[derive(Clone, Copy)]
enum Route {
    Direct,
    Handwritten,
    Generated,
}

impl Route {
    fn label(self) -> &'static str {
        match self {
            Self::Direct => "direct_rust",
            Self::Handwritten => "handwritten_adapter",
            Self::Generated => "generated_semaprax",
        }
    }
}

#[derive(Clone, Copy)]
enum Task {
    Record,
    StatefulCallback,
}

impl Task {
    fn label(self) -> &'static str {
        match self {
            Self::Record => "generic_record",
            Self::StatefulCallback => "stateful_callback",
        }
    }
}

fn record(route: Route) -> i64 {
    match route {
        Route::Generated => JSON
            .into_iter()
            .map(|source| {
                let (record, transfer) =
                    deserialize_spxmirrorri13event_with_transfer_metrics(source).unwrap();
                assert!(transfer.string_pointers_preserved);
                assert_eq!(transfer.copied_string_bytes, Some(0));
                let encoded = serialize_spxmirrorri13event(&record).unwrap();
                assert_eq!(encoded, source);
                record.value
            })
            .sum(),
        Route::Direct => JSON
            .into_iter()
            .map(|source| {
                let event: DirectEvent = serde_json::from_str(source).unwrap();
                let encoded = serde_json::to_string(&event).unwrap();
                assert_eq!(encoded, source);
                event.value
            })
            .sum(),
        Route::Handwritten => {
            let adapter = HandwrittenRecordAdapter;
            JSON.into_iter()
                .map(|source| {
                    let event = adapter.decode(source);
                    let encoded = adapter.encode(&event);
                    assert_eq!(encoded, source);
                    event.value
                })
                .sum()
        }
    }
}

fn callback(route: Route) -> i64 {
    match route {
        Route::Direct => [1_i64, 2]
            .into_iter()
            .try_fold(10_i64, |state, value| {
                if value < 0 {
                    return Err("negative callback input");
                }
                state.checked_add(value).ok_or("callback overflow")
            })
            .unwrap(),
        Route::Handwritten => {
            let mut adapter = HandwrittenAdapter { state: 10 };
            let states = [1_i64, 2]
                .into_iter()
                .map(|value| adapter.call(value).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(states, [11, 13]);
            adapter.state
        }
        Route::Generated => {
            let domain = SpxCallbackDomain::new(2).unwrap();
            let mut proxy = SpxStatefulProxy::new(domain.clone(), 10).unwrap();
            let states = [1_i64, 2]
                .into_iter()
                .map(proxy.as_fn_mut())
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(states, [11, 13]);
            let final_state = proxy.state().unwrap();
            proxy.unregister().unwrap();
            drop(proxy);
            assert_eq!(domain.live_environments(), 0);
            final_state
        }
    }
}

fn main() {
    println!("task,route,iteration,operations,elapsed_ns,allocation_calls,allocated_bytes,adapter_buffer_copied_bytes");
    let routes = [Route::Direct, Route::Handwritten, Route::Generated];
    for task in [Task::Record, Task::StatefulCallback] {
        for iteration in 0..(WARMUP + SAMPLES) {
            for shift in 0..routes.len() {
                let route = routes[(iteration + shift) % routes.len()];
                let started = Instant::now();
                let (checksum, allocations) = measured(|| {
                    let mut checksum = 0_i64;
                    for _ in 0..OPERATIONS {
                        checksum += match task {
                            Task::Record => record(route),
                            Task::StatefulCallback => callback(route),
                        };
                    }
                    black_box(checksum)
                });
                assert_eq!(
                    checksum,
                    (OPERATIONS as i64)
                        * match task {
                            Task::Record => 3,
                            Task::StatefulCallback => 13,
                        }
                );
                let elapsed_ns = started.elapsed().as_nanos();
                if iteration >= WARMUP {
                    println!(
                        "{},{},{},{},{},{},{},0",
                        task.label(),
                        route.label(),
                        iteration - WARMUP,
                        OPERATIONS,
                        elapsed_ns,
                        allocations.calls,
                        allocations.bytes
                    );
                }
            }
        }
    }
}
