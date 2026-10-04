//! Local, exploratory M3 route comparison. Prints every sample as CSV.
mod generated {
    include!("../generated.rs");
}

use semaprax::project::{with_authenticated_project, ProjectRevision};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const WARMUP: usize = 9;
const SAMPLES: usize = 90;

#[derive(Clone, Copy, Default)]
struct AllocationMetrics {
    allocation_calls: u64,
    deallocation_calls: u64,
    reallocation_calls: u64,
    allocated_bytes: u64,
    deallocated_bytes: u64,
}

impl AllocationMetrics {
    fn allocation(&mut self, bytes: usize) {
        self.allocation_calls += 1;
        self.allocated_bytes = self.allocated_bytes.saturating_add(bytes as u64);
    }

    fn deallocation(&mut self, bytes: usize) {
        self.deallocation_calls += 1;
        self.deallocated_bytes = self.deallocated_bytes.saturating_add(bytes as u64);
    }
}

thread_local! {
    static ALLOCATION_METRICS: Cell<Option<AllocationMetrics>> = const { Cell::new(None) };
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn note_allocation(layout: Layout) {
    let _ = ALLOCATION_METRICS.try_with(|slot| {
        if let Some(mut metrics) = slot.get() {
            metrics.allocation(layout.size());
            slot.set(Some(metrics));
        }
    });
}

fn note_deallocation(layout: Layout) {
    let _ = ALLOCATION_METRICS.try_with(|slot| {
        if let Some(mut metrics) = slot.get() {
            metrics.deallocation(layout.size());
            slot.set(Some(metrics));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation(layout);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation(layout);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        note_deallocation(layout);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let _ = ALLOCATION_METRICS.try_with(|slot| {
            if let Some(mut metrics) = slot.get() {
                metrics.reallocation_calls += 1;
                metrics.deallocation(layout.size());
                metrics.allocation(size);
                slot.set(Some(metrics));
            }
        });
        unsafe { System.realloc(pointer, layout, size) }
    }
}

fn measure_allocations<T>(operation: impl FnOnce() -> T) -> (T, AllocationMetrics) {
    ALLOCATION_METRICS.with(|slot| {
        assert!(slot.replace(Some(AllocationMetrics::default())).is_none());
    });
    let output = operation();
    let metrics = ALLOCATION_METRICS.with(|slot| slot.replace(None).unwrap());
    (output, metrics)
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

fn local_server(expected_requests: usize) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut header = [0u8; 2048];
            let mut used = 0;
            while !header[..used].windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let read = stream.read(&mut header[used..]).unwrap();
                assert!(read > 0 && used + read < header.len());
                used += read;
            }
            assert!(header[..used].starts_with(b"GET /value/42 HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n43")
                .unwrap();
        }
    });
    (endpoint, server)
}

async fn fetch(client: reqwest::Client, endpoint: String, request: i64) -> Result<i64, String> {
    let url = reqwest::Url::parse(&format!("{endpoint}/value/{request}"))
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    assert_eq!(response.status().as_u16(), 200);
    response
        .text()
        .await
        .map_err(|error| error.to_string())?
        .parse::<i64>()
        .map_err(|error| error.to_string())
}

fn execute(
    route: Route,
    runtime: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    endpoint: &str,
    revision: &Arc<ProjectRevision>,
) -> (i64, Option<(u128, u128)>) {
    let client = client.clone();
    let endpoint = endpoint.to_owned();
    let seed = 41_i64;
    match route {
        Route::Direct => {
            let answer = runtime.block_on(fetch(client, endpoint, seed + 1)).unwrap();
            (seed + answer, None)
        }
        Route::Handwritten => {
            assert!((0..=1000).contains(&seed));
            let answer = runtime.block_on(fetch(client, endpoint, seed + 1)).unwrap();
            let result = seed + answer;
            assert!(result >= 0);
            (result, None)
        }
        Route::Generated => {
            let prepare_start = Instant::now();
            let registered = generated::register(Arc::clone(revision), move |request| {
                fetch(client, endpoint, request)
            })
            .expect("held Project and generated module agree");
            let pending = registered.call_typed(seed, 10_000).unwrap();
            let prepare_ns = prepare_start.elapsed().as_nanos();
            let await_start = Instant::now();
            let result = runtime.block_on(pending).unwrap();
            let await_ns = await_start.elapsed().as_nanos();
            (result, Some((prepare_ns, await_ns)))
        }
    }
}

fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let revision = with_authenticated_project(&root.join("project/semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .expect("admitted held Project");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = reqwest::Client::builder()
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let probe = std::env::args().nth(1).as_deref() == Some("probe");
    if probe {
        let (endpoint, server) = local_server(33);
        println!("iteration,total_ns,prepare_ns,await_ns");
        for iteration in 0..33 {
            let start = Instant::now();
            let (result, phases) =
                execute(Route::Generated, &runtime, &client, &endpoint, &revision);
            assert_eq!(result, 84);
            if iteration >= 3 {
                let (prepare_ns, await_ns) = phases.unwrap();
                println!(
                    "{},{},{},{}",
                    iteration - 3,
                    start.elapsed().as_nanos(),
                    prepare_ns,
                    await_ns
                );
            }
        }
        server.join().unwrap();
        return;
    }
    let (endpoint, server) = local_server((WARMUP + SAMPLES) * 3);
    println!("route,iteration,elapsed_ns,body_bytes,allocation_calls,deallocation_calls,reallocation_calls,allocated_bytes,deallocated_bytes");
    for iteration in 0..(WARMUP + SAMPLES) {
        let routes = [Route::Direct, Route::Handwritten, Route::Generated];
        for shift in 0..3 {
            let route = routes[(iteration + shift) % 3];
            let start = Instant::now();
            let ((result, _), allocations) =
                measure_allocations(|| execute(route, &runtime, &client, &endpoint, &revision));
            let elapsed = start.elapsed().as_nanos();
            assert_eq!(result, 84);
            if iteration >= WARMUP {
                println!(
                    "{},{},{},2,{},{},{},{},{}",
                    route.label(),
                    iteration - WARMUP,
                    elapsed,
                    allocations.allocation_calls,
                    allocations.deallocation_calls,
                    allocations.reallocation_calls,
                    allocations.allocated_bytes,
                    allocations.deallocated_bytes,
                );
            }
        }
    }
    server.join().unwrap();
}
