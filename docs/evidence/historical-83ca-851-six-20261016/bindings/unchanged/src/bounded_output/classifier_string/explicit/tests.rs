//! Full original logical receivers remain the independent frontier oracle.
//! These scripted callbacks prove logical order only, not funded acquisition.
use super::*;
use crate::bounded_output::{
    set_active_floor, with_limit_usage_refusal, CappedString, ReservationRefusal,
};
use std::fmt::Write;

struct Chunks;
impl fmt::Display for Chunks {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("aaaaa")?;
        output.write_str("b")
    }
}
struct Logical {
    remaining: usize,
    floor: usize,
    refused: bool,
    first: Option<ReservationRefusal>,
}
impl Logical {
    fn reserve(&mut self, bytes: usize) -> Result<bool, ()> {
        if self.refused {
            return Ok(false);
        }
        if bytes > self.remaining {
            self.first = Some(ReservationRefusal {
                requested: bytes,
                remaining: self.remaining,
                floor: self.floor,
                stage: None,
            });
            self.refused = true;
            return Ok(false);
        }
        if bytes
            .checked_add(self.floor)
            .is_none_or(|value| value > self.remaining)
        {
            return Ok(false);
        }
        self.remaining -= bytes;
        Ok(true)
    }
}
#[test]
fn full_original_strict_and_capped_frontiers_are_preserved_without_active_access() {
    for limit in [0, 1, 5, 6, 10, 17] {
        for floor in [0, 6] {
            if floor > limit {
                continue;
            }
            for strict in [false, true] {
                let old = with_limit_usage_refusal(limit, || {
                    assert!(set_active_floor(floor));
                    if strict {
                        crate::bounded_output::budgeted_format(format_args!("{}", Chunks))
                    } else {
                        let mut output = CappedString::new();
                        write!(&mut output, "{}", Chunks).unwrap();
                        output.into_string()
                    }
                });
                let (current, production_refused, production_used, production_first) =
                    with_limit_usage_refusal(0, || {
                        let mut logical = Logical {
                            remaining: limit,
                            floor,
                            refused: false,
                            first: None,
                        };
                        let mut output =
                            ClassifierString::acquire_explicit(&mut |_| Ok::<_, ()>(())).unwrap();
                        let mut pay = |_, _: &mut Logical| Ok::<_, ()>(());
                        let mut reserve = |bytes, state: &mut Logical| state.reserve(bytes);
                        if strict {
                            output
                                .strict_explicit(
                                    format_args!("{}", Chunks),
                                    &mut logical,
                                    &mut pay,
                                    &mut reserve,
                                )
                                .unwrap();
                        } else {
                            output
                                .capped_explicit(
                                    format_args!("{}", Chunks),
                                    &mut logical,
                                    &mut pay,
                                    &mut reserve,
                                )
                                .unwrap();
                        }
                        (
                            output.into_string(),
                            logical.refused,
                            limit - logical.remaining,
                            logical.first,
                        )
                    });
                assert_eq!(
                    old, current,
                    "limit={limit}, floor={floor}, strict={strict}"
                );
                assert!(!production_refused);
                assert_eq!(production_used, 0);
                assert_eq!(production_first, None);
            }
        }
    }
}
#[test]
fn logical_payment_error_is_typed_and_precedes_backing_growth() {
    let mut output =
        ClassifierString::acquire_explicit(&mut |_| Ok::<_, &'static str>(())).unwrap();
    let mut physical_calls = 0;
    let result = output.capped_explicit(
        format_args!("{}", Chunks),
        &mut (),
        &mut |_, _| {
            physical_calls += 1;
            Ok(())
        },
        &mut |_, _| Err("sticky account refusal"),
    );
    assert!(matches!(
        result,
        Err(GrowthFailure::Payment("sticky account refusal"))
    ));
    assert_eq!(physical_calls, 0);
    assert_eq!(output.as_str(), "");
}

#[test]
fn complete_raw_receiver_keeps_all_chunks_and_refuses_work_before_growth() {
    let original = std::fmt::format(format_args!("{}", Chunks));
    let (current, refused, used, first) = with_limit_usage_refusal(0, || {
        let mut output =
            ClassifierString::acquire_explicit(&mut |_| Ok::<_, &'static str>(())).unwrap();
        let mut visits = 0;
        output
            .raw_explicit(
                format_args!("{}", Chunks),
                &mut (),
                &mut |_, _| Ok::<(), &'static str>(()),
                &mut |bytes, _| {
                    visits += 1 + bytes;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(visits, 2 + original.len());
        output.into_string()
    });
    assert_eq!(current, original);
    assert!(!refused);
    assert_eq!((used, first), (0, None));
    let mut output =
        ClassifierString::acquire_explicit(&mut |_| Ok::<_, &'static str>(())).unwrap();
    let mut physical_calls = 0;
    let mut work_calls = 0;
    let error = output.raw_explicit(
        format_args!("{}", Chunks),
        &mut (),
        &mut |_, _| {
            physical_calls += 1;
            Ok(())
        },
        &mut |_, _| {
            work_calls += 1;
            Err("first work refusal")
        },
    );
    assert!(matches!(
        error,
        Err(GrowthFailure::Payment("first work refusal"))
    ));
    assert_eq!((physical_calls, work_calls), (0, 1));
    assert_eq!(output.as_str(), "");
    assert_eq!(output.bytes.capacity(), 0);
}
