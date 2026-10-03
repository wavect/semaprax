use super::*;
use std::cell::Cell;
use std::future::poll_fn;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Wake, Waker};

const SOURCE: &str = r#"
module test.local_future;
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> i64 {
    let answer = yield seed + 1;
    answer + seed
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn canonical() -> String {
    let program = crate::parse(SOURCE, Path::new("local-future.spx")).unwrap();
    crate::format::canonical(&program)
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

#[test]
fn checked_source_future_wakes_then_resumes_exact_selected_yield() {
    let calls = Rc::new(Cell::new(0));
    let polls = Rc::new(Cell::new(0));
    let future = SourceLocalFuture::prepare(
        &canonical(),
        Path::new("local-future.spx"),
        "app.ask",
        41,
        10_000,
        {
            let calls = calls.clone();
            let polls = polls.clone();
            move |request| {
                assert_eq!(request, 42);
                calls.set(calls.get() + 1);
                poll_fn(move |cx| {
                    let current = polls.get();
                    polls.set(current + 1);
                    if current == 0 {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        Poll::Ready(Ok::<i64, ()>(request + 1))
                    }
                })
            }
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 0);
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    assert_eq!(future.as_mut().poll(&mut context), Poll::Pending);
    assert_eq!(calls.get(), 1);
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(Ok(84)));
    assert_eq!(polls.get(), 2);
}

#[test]
fn checked_source_future_refuses_noncanonical_input_and_drops_pending_host_work() {
    let source = canonical();
    let changed = format!("\n{source}");
    let refusal = SourceLocalFuture::prepare(
        &changed,
        Path::new("local-future.spx"),
        "app.ask",
        41,
        10_000,
        |_| async { Ok::<i64, ()>(0) },
    );
    assert_eq!(refusal.err().unwrap()[0].code, INVALID);

    let dropped = Rc::new(Cell::new(false));
    struct PendingDrop(Rc<Cell<bool>>);
    impl Drop for PendingDrop {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let pending = SourceLocalFuture::prepare(
        &source,
        Path::new("local-future.spx"),
        "app.ask",
        41,
        10_000,
        {
            let dropped = dropped.clone();
            move |_| {
                let marker = PendingDrop(dropped);
                async move {
                    let _held = marker;
                    std::future::pending::<Result<i64, ()>>().await
                }
            }
        },
    )
    .unwrap();
    let mut pending = Box::pin(pending);
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    assert_eq!(pending.as_mut().poll(&mut context), Poll::Pending);
    drop(pending);
    assert!(dropped.get());
}

#[test]
fn checked_source_future_settles_handler_and_poll_panics() {
    let source = canonical();
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let handler = SourceLocalFuture::prepare(
        &source,
        Path::new("local-future.spx"),
        "app.ask",
        41,
        10_000,
        |_| -> std::future::Ready<Result<i64, ()>> { panic!("handler panicked") },
    )
    .unwrap();
    assert_eq!(
        Box::pin(handler).as_mut().poll(&mut context),
        Poll::Ready(Err(SourceLocalFutureFailure::Panicked))
    );

    let poller = SourceLocalFuture::prepare(
        &source,
        Path::new("local-future.spx"),
        "app.ask",
        41,
        10_000,
        |_| async {
            panic!("host Future panicked");
            #[allow(unreachable_code)]
            Ok::<i64, ()>(0)
        },
    )
    .unwrap();
    assert_eq!(
        Box::pin(poller).as_mut().poll(&mut context),
        Poll::Ready(Err(SourceLocalFutureFailure::Panicked))
    );
}
