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
    let records = [
        r#"{"value":1,"label":"one"}"#,
        r#"{"value":2,"label":"two"}"#,
    ]
    .into_iter()
    .map(m2::deserialize_spxmirrorri13event)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    assert_eq!(records[0].label, "one");
    assert_eq!(
        m2::serialize_spxmirrorri13event(&records[0]).unwrap(),
        r#"{"value":1,"label":"one"}"#
    );
    let domain = m2::SpxCallbackDomain::new(2).unwrap();
    let callback = m2::SpxCallback::new(domain.clone(), 7).unwrap();
    let mapped = records
        .iter()
        .map(|record| record.value)
        .map(callback.as_fn())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(mapped, [8, 9]);
    let mut stateful = m2::SpxStatefulProxy::new(domain.clone(), 10).unwrap();
    let states = records
        .iter()
        .map(|record| record.value)
        .map(stateful.as_fn_mut())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(states, [11, 13]);
    assert_eq!(stateful.state(), Ok(13));
    callback.unregister().unwrap();
    stateful.unregister().unwrap();
    drop(callback);
    drop(stateful);
    assert_eq!(domain.live_environments(), 0);
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
