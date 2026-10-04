//! Matched local RI-13 M1 batch comparison.
//!
//! Every route repeats the checked fixture's fixed Regex scan or Url parse/view
//! operation. The generated route calls the authenticated package's bounded
//! `checked-export-repeat.v1` API; it does not accept a varying input corpus.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::time::Instant;

const WARMUP: usize = 1;
const SAMPLES: usize = 5;
const OPERATIONS: usize = 4096;
const PATTERN: &str = "example";
const INPUT: &str = "https://example.invalid/path";
const INPUT_BYTES: u64 = 28;

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
    RegexScan,
    UrlParseView,
}

impl Task {
    fn label(self) -> &'static str {
        match self {
            Self::RegexScan => "regex_scan",
            Self::UrlParseView => "url_parse_view",
        }
    }
}

#[derive(Clone, Copy)]
struct BatchMetrics {
    checksum: i64,
    borrowed_input_bytes: u64,
    adapter_copy_events: u64,
    adapter_copied_bytes: u64,
    owner_live_count: u64,
    view_live_count: u64,
    string_live_count: u64,
}

struct HandwrittenRegex {
    owner: regex_direct::Regex,
}

impl HandwrittenRegex {
    fn new(pattern: &str) -> Result<Self, regex_direct::Error> {
        Ok(Self {
            owner: regex_direct::Regex::new(pattern)?,
        })
    }

    fn is_match(&self, input: &str) -> bool {
        self.owner.is_match(input)
    }
}

struct HandwrittenUrl {
    owner: url_direct::Url,
}

impl HandwrittenUrl {
    fn parse(input: &str) -> Result<Self, url_direct::ParseError> {
        Ok(Self {
            owner: url_direct::Url::parse(input)?,
        })
    }

    fn view(&self) -> &str {
        self.owner.as_str()
    }
}

fn repeated(operations: usize, mut operation: impl FnMut() -> i64) -> BatchMetrics {
    let mut checksum = 0_i64;
    for _ in 0..operations {
        checksum = checksum
            .checked_add(operation())
            .expect("fixed batch checksum");
    }
    BatchMetrics {
        checksum,
        borrowed_input_bytes: INPUT_BYTES
            .checked_mul(u64::try_from(operations).expect("bounded operation count"))
            .expect("fixed borrowed bytes"),
        adapter_copy_events: 0,
        adapter_copied_bytes: 0,
        owner_live_count: 0,
        view_live_count: 0,
        string_live_count: 0,
    }
}

fn direct(task: Task) -> BatchMetrics {
    repeated(OPERATIONS, || match task {
        Task::RegexScan => {
            let owner = regex_direct::Regex::new(PATTERN).expect("fixed Regex pattern");
            i64::from(owner.is_match(INPUT)) * 41
        }
        Task::UrlParseView => {
            let owner = url_direct::Url::parse(INPUT).expect("fixed Url input");
            i64::from(owner.as_str().len() == INPUT.len()) * 41
        }
    })
}

fn handwritten(task: Task) -> BatchMetrics {
    repeated(OPERATIONS, || match task {
        Task::RegexScan => {
            let owner = HandwrittenRegex::new(PATTERN).expect("fixed Regex pattern");
            i64::from(owner.is_match(INPUT)) * 41
        }
        Task::UrlParseView => {
            let owner = HandwrittenUrl::parse(INPUT).expect("fixed Url input");
            i64::from(owner.view().len() == INPUT.len()) * 41
        }
    })
}

fn generated(task: Task) -> BatchMetrics {
    match task {
        Task::RegexScan => {
            let metrics = ri06_regex_owner::run_batch(OPERATIONS).expect("checked Regex batch");
            BatchMetrics {
                checksum: metrics.checksum,
                borrowed_input_bytes: metrics.borrowed_input_bytes,
                adapter_copy_events: metrics.adapter_copy_events as u64,
                adapter_copied_bytes: metrics.adapter_copied_bytes,
                owner_live_count: metrics.live_owner_count as u64,
                view_live_count: 0,
                string_live_count: metrics.live_string_count,
            }
        }
        Task::UrlParseView => {
            let metrics = ri06_url_owner::run_batch(OPERATIONS).expect("checked Url batch");
            BatchMetrics {
                checksum: metrics.checksum,
                borrowed_input_bytes: metrics.borrowed_input_bytes,
                adapter_copy_events: metrics.adapter_copy_events as u64,
                adapter_copied_bytes: metrics.adapter_copied_bytes,
                owner_live_count: metrics.live_owner_count as u64,
                view_live_count: metrics.live_view_count as u64,
                string_live_count: metrics.live_string_count,
            }
        }
    }
}

fn execute(task: Task, route: Route) -> BatchMetrics {
    match route {
        Route::Direct => direct(task),
        Route::Handwritten => handwritten(task),
        Route::Generated => generated(task),
    }
}

fn main() {
    println!(
        "task,route,iteration,operations,elapsed_ns,allocation_calls,allocated_bytes,borrowed_input_bytes,adapter_copy_events,adapter_copied_bytes,owner_live_count,view_live_count,string_live_count"
    );
    let routes = [Route::Direct, Route::Handwritten, Route::Generated];
    for task in [Task::RegexScan, Task::UrlParseView] {
        for iteration in 0..(WARMUP + SAMPLES) {
            for shift in 0..routes.len() {
                let route = routes[(iteration + shift) % routes.len()];
                let started = Instant::now();
                let (metrics, allocations) = measured(|| black_box(execute(task, route)));
                assert_eq!(metrics.checksum, (OPERATIONS as i64) * 41);
                assert_eq!(
                    metrics.borrowed_input_bytes,
                    (OPERATIONS as u64) * INPUT_BYTES
                );
                assert_eq!(metrics.adapter_copy_events, 0);
                assert_eq!(metrics.adapter_copied_bytes, 0);
                assert_eq!(metrics.owner_live_count, 0);
                assert_eq!(metrics.view_live_count, 0);
                assert_eq!(metrics.string_live_count, 0);
                if iteration >= WARMUP {
                    println!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{}",
                        task.label(),
                        route.label(),
                        iteration - WARMUP,
                        OPERATIONS,
                        started.elapsed().as_nanos(),
                        allocations.calls,
                        allocations.bytes,
                        metrics.borrowed_input_bytes,
                        metrics.adapter_copy_events,
                        metrics.adapter_copied_bytes,
                        metrics.owner_live_count,
                        metrics.view_live_count,
                        metrics.string_live_count,
                    );
                }
            }
        }
    }
}
