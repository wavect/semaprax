mod m2 {
    include!("../generated/m2/module.rs");
}
mod m3 {
    include!("../generated/m3.rs");
}
use semaprax::project::{with_authenticated_project, ProjectRevision};
use std::sync::Arc;

fn main() {
    assert_eq!(ri06_regex_owner::run(), Ok(41));
    assert_eq!(ri06_url_owner::run(), Ok(41));
    let event = m2::deserialize_spxmirrorri13event(r#"{"value":1,"label":"one"}"#).unwrap();
    assert_eq!(
        m2::serialize_spxmirrorri13event(&event).unwrap(),
        r#"{"value":1,"label":"one"}"#
    );
    let domain = m2::SpxCallbackDomain::new(2).unwrap();
    let callback = m2::SpxCallback::new(domain.clone(), 7).unwrap();
    assert_eq!(callback.as_fn()(1), Ok(8));
    callback.unregister().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let revision = with_authenticated_project(
        &root.parent().unwrap().join("project/semaprax.toml"),
        |snapshot| {
            snapshot.check()?;
            Ok(snapshot.retain_revision())
        },
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let call = m3::register(Arc::<ProjectRevision>::clone(&revision), |_request| async {
        Ok::<i64, ()>(43)
    })
    .unwrap()
    .call_typed(41, 10_000)
    .unwrap();
    assert_eq!(runtime.block_on(call).unwrap(), 84);
    println!("ri13-linked-project-ok");
}
