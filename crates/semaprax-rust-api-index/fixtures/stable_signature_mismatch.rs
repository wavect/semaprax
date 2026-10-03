#[path = "local_api_fixture.rs"]
mod api;

// The return type is deliberately wrong. A stable compiler must reject this
// selected signature before any adapter could call the item.
const _: fn(&api::ReExported, &str) -> u8 = api::ReExported::contains;
