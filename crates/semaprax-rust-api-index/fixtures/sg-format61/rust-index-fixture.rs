pub struct Payload { pub value: u64 }

pub enum Event {
    Tuple(Payload),
    Named { inner: Payload },
}

pub fn make_event() -> Event {
    Event::Tuple(Payload { value: 7 })
}

mod internal {
    pub mod nested {
        pub fn increment(value: u64) -> u64 { value + 1 }
    }
}

mod bridge {
    pub use crate::internal::nested as public_api;
}
pub use bridge::*;

#[test]
fn exported_api_is_callable() {
    assert_eq!(public_api::increment(41), 42);
    match make_event() {
        Event::Tuple(p) => assert_eq!(p.value, 7),
        _ => panic!("wrong case"),
    }
}
