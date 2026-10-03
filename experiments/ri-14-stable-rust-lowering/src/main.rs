use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    ArithmeticOverflow,
    CallbackFailure,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Trace(Vec<&'static str>);

impl Trace {
    fn record(&mut self, event: &'static str) {
        self.0.push(event);
    }
}

/// This type has no Drop implementation.  The lowered frame consumes it at
/// the canonical plan point, rather than substituting lexical Rust cleanup.
struct Owned {
    label: &'static str,
}

impl Owned {
    fn release(self, trace: &mut Trace) {
        trace.record(self.label);
    }
}

#[derive(Clone, Copy)]
enum Slot {
    Left,
    Right,
}

/// A future lowerer must obtain this vector from verified HIR.  This fixture
/// supplies it as data solely to test that generated Rust obeys it verbatim.
const CANONICAL_CLEANUP_PLAN: [Slot; 2] = [Slot::Left, Slot::Right];

struct Frame {
    left: Option<Owned>,
    right: Option<Owned>,
}

impl Frame {
    fn new() -> Self {
        Self {
            left: Some(Owned {
                label: "cleanup.left",
            }),
            right: Some(Owned {
                label: "cleanup.right",
            }),
        }
    }

    fn cleanup(mut self, trace: &mut Trace) {
        for slot in CANONICAL_CLEANUP_PLAN {
            let owned = match slot {
                Slot::Left => self.left.take(),
                Slot::Right => self.right.take(),
            };
            if let Some(owned) = owned {
                owned.release(trace);
            }
        }
    }
}

fn generic_library_call<T, U, F>(value: T, trace: &mut Trace, mut callback: F) -> Result<U, Status>
where
    F: FnMut(T, &mut Trace) -> Result<U, Status>,
{
    // This uses a stable standard-library generic call, with the Semaprax
    // candidate callback executed inside its monomorphized closure.
    std::iter::once(value)
        .map(|item| callback(item, trace))
        .next()
        .expect("one-item iterator invokes the callback once")
}

fn lowered_add_then_callback<U, F>(
    frame: Frame,
    left: i64,
    right: i64,
    callback: F,
) -> (Result<U, Status>, Trace)
where
    F: FnMut(i64, &mut Trace) -> Result<U, Status>,
{
    let mut trace = Trace::default();
    // `checked_add` happens before the callback.  Every terminal path then
    // executes the canonical cleanup plan before returning its selected status.
    let result = match left.checked_add(right) {
        Some(sum) => generic_library_call(sum, &mut trace, callback),
        None => Err(Status::ArithmeticOverflow),
    };
    frame.cleanup(&mut trace);
    (result, trace)
}

fn success_case() {
    let (result, trace) = lowered_add_then_callback(Frame::new(), 19, 22, |sum, trace| {
        trace.record("callback.success");
        Ok(sum + 1)
    });
    assert_eq!(result, Ok(42));
    assert_eq!(
        trace,
        Trace(vec!["callback.success", "cleanup.left", "cleanup.right"])
    );
}

fn arithmetic_failure_case() {
    let (result, trace) = lowered_add_then_callback(Frame::new(), i64::MAX, 1, |_, trace| {
        trace.record("callback.must-not-run");
        Ok(0_i64)
    });
    assert_eq!(result, Err(Status::ArithmeticOverflow));
    assert_eq!(trace, Trace(vec!["cleanup.left", "cleanup.right"]));
}

fn callback_failure_case() {
    let (result, trace) = lowered_add_then_callback(Frame::new(), 1, 2, |_, trace| {
        trace.record("callback.failure");
        Err::<i64, _>(Status::CallbackFailure)
    });
    assert_eq!(result, Err(Status::CallbackFailure));
    assert_eq!(
        trace,
        Trace(vec!["callback.failure", "cleanup.left", "cleanup.right"])
    );
}

struct LexicalOwned {
    label: &'static str,
    trace: Rc<RefCell<Trace>>,
}

impl Drop for LexicalOwned {
    fn drop(&mut self) {
        self.trace.borrow_mut().record(self.label);
    }
}

fn lexical_drop_negative_control() {
    let trace = Rc::new(RefCell::new(Trace::default()));
    {
        let _left = LexicalOwned {
            label: "cleanup.left",
            trace: Rc::clone(&trace),
        };
        let _right = LexicalOwned {
            label: "cleanup.right",
            trace: Rc::clone(&trace),
        };
    }
    assert_eq!(
        *trace.borrow(),
        Trace(vec!["cleanup.right", "cleanup.left"])
    );
    assert_ne!(
        *trace.borrow(),
        Trace(vec!["cleanup.left", "cleanup.right"]),
        "lexical Drop must not stand in for the canonical cleanup plan"
    );
}

fn main() {
    success_case();
    arithmetic_failure_case();
    callback_failure_case();
    lexical_drop_negative_control();
    println!("ri-14 stable Rust lowering candidate passed");
}
