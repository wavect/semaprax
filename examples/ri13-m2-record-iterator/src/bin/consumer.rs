include!("../generated.rs");

fn main() {
    let records = [
        r#"{"value":1,"label":"one"}"#,
        r#"{"value":2,"label":"two"}"#,
    ]
    .into_iter()
    .map(deserialize_spxmirrorri13event)
    .collect::<Result<Vec<Event>, _>>()
    .expect("generated Serde mirror parses real records");
    assert_eq!(records[0].label, "one");
    assert_eq!(
        serialize_spxmirrorri13event(&records[0]).unwrap(),
        r#"{"value":1,"label":"one"}"#
    );
    let domain = SpxCallbackDomain::new(2).unwrap();
    let callback = SpxCallback::new(domain.clone(), 7).unwrap();
    let mapped = records
        .iter()
        .map(|record| record.value)
        .map(callback.as_fn())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(mapped, [8, 9]);
    let mut stateful = SpxStatefulProxy::new(domain.clone(), 10).unwrap();
    let states = records
        .iter()
        .map(|record| record.value)
        .map(stateful.as_fn_mut())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(states, [11, 13]);
    assert_eq!(stateful.state(), Ok(13));
    assert!(matches!(
        stateful.as_fn_mut()(-1),
        Err(SpxCallbackError::Contract(_))
    ));
    callback.unregister().unwrap();
    stateful.unregister().unwrap();
    drop(callback);
    drop(stateful);
    assert_eq!(domain.live_environments(), 0);
    println!("ri13-m2-record-iterator-ok");
}
