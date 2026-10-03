#[path = "local_api_fixture.rs"]
mod api;

#[allow(dead_code)]
fn check_selected_signatures(value: &api::ReExported, generated: &api::MacroGenerated) {
    let _: fn(&api::ReExported, &str) -> bool = api::ReExported::contains;
    let _: fn(&api::MacroGenerated) -> u8 = api::MacroGenerated::answer;
    let _: fn(&api::ReExported) -> u8 = <api::ReExported as api::Measures>::measure;
    let _ = (value, generated);

    #[cfg(feature = "fixture-selected")]
    {
        let _: fn(u8) -> u8 = api::cfg_selected;
    }
}
