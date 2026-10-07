use super::*;

#[test]
fn sg14_more_than_200_references_are_fitted_without_silent_loss() {
    let p = fake_world_project();
    write(
        &p,
        "many.ts",
        &(0..251).map(|i| format!("add({i});\n")).collect::<String>(),
    );
    let b = fake_broker(
        FakeSource::new(SI_ID, grep_answer(SI_ID)),
        &p.parent().unwrap().join("cache"),
    );
    let mut r = req("add(", 1 << 20);
    r.references = true;
    r.native_targets = Some(vec![]);
    let roomy = b.context(&p, &r).unwrap();
    assert!(roomy.external.len() >= 251);
    assert_eq!(roomy.omitted, 0);
    assert!(roomy.exhaustive);
    r.max_bytes = 4096;
    let tight = b.context(&p, &r).unwrap();
    assert!(tight.omitted > 0 && !tight.exhaustive);
    assert_eq!(tight.external.len() + tight.omitted, roomy.external.len());
}

#[test]
fn sg14_retained_actual_graft_result_reaches_broker_without_silent_cap() {
    use semaprax_harness::contract::{RequestEnvelope, ResultEnvelope};
    let fixtures = fixtures().join("sg14");
    let request = RequestEnvelope::from_json(
        &serde_json::from_slice::<Value>(
            &std::fs::read(fixtures.join("graft-request.json")).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let envelope = ResultEnvelope::parse_for(
        &request,
        &std::fs::read(fixtures.join("graft-result.json")).unwrap(),
    )
    .unwrap();
    let payload = envelope.payload.unwrap();
    let response = ExternalResponse::from_json(&json!({"status": "complete", "items": payload["items"], "coverage": payload["coverage"], "no_references": false, "upstream_version": envelope.provenance.upstream_version, "provider_id": envelope.provenance.provider_id, "diagnostics": []})).unwrap();
    assert_eq!(response.items.len(), 251);
    let dir = fixture_dir("hp-sg14-retained");
    std::fs::copy(fixtures.join("source.js"), dir.join("source.js")).unwrap();
    let id = response.provider_id.clone();
    let provider = FakeSource::new(&id, Box::new(move |_, _| response.clone()));
    let mut broker = Broker::new(None, None);
    broker.add_provider(Box::new(provider)).unwrap();
    let mut req = BrokerRequest::new("target", 1 << 20);
    req.references = true;
    req.exhaustive = true;
    req.max_items = 200;
    let result = broker.context(&dir, &req).unwrap();
    assert_eq!(result.external.len(), 251);
    assert!(result.external.iter().all(|i| i.verified));
    assert_eq!(result.omitted, 0);
    assert!(result.exhaustive);
    assert_eq!(doc(&result)["coverage"]["complete"], true);
}
