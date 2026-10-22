use super::{digest, held, read_view, table_compiled, SOURCE_CAP};
use std::path::Path;
#[rustfmt::skip] // Preserve this exact bounded historical packet loader.
pub(super) fn packet_before_overlay() -> serde_json::Value {
    let base=packet_fmt433();
    let data=include_str!("../../docs/evidence/fmt-local-441-20261018/extension.json");
    assert!(data.len()<=262144);
    assert_eq!((data.len(),digest(data.as_bytes())),(262076,"2af7299253e5cfb027186ba4fc151011a937214ff381c9267bfc11ef7d0003b6".to_owned()));
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(held(&root.join("docs/evidence/fmt-local-441-20261018/extension.json"),data.len(),&digest(data.as_bytes())).unwrap(),data);
    let ext:serde_json::Value=serde_json::from_str(data).unwrap();
    assert_eq!(ext["schema"],"semaprax.fmt433-local133-extension.v1");
    assert_eq!(ext["base_sha256"],"de6103a37cb677be139de99b4a945ddf2d588b5292a01ca058715b3d7f8734dc");
    assert_eq!(ext["compiled_upgrades"],serde_json::json!(["src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs"]));
    let mut logical=base.clone();
    let rows=logical["rows"].as_array_mut().unwrap();
    let mut seen=std::collections::BTreeSet::new();
    for row in ext["rows"].as_array().unwrap() {
        let source=row["source"].as_str().unwrap();assert!(seen.insert(source));
        if let Some(index)=rows.iter().position(|r|r["source"]==source) {rows[index]=row.clone();}else{rows.push(row.clone());}
    }
    assert_eq!(rows.len(),519);
    for (prior,current) in base["rows"].as_array().unwrap().iter().zip(rows.iter()) {
        assert_eq!(current["source"],prior["source"]);assert!(current["compiled"]==prior["compiled"] || (prior["compiled"]==false && current["compiled"]==true && ext["compiled_upgrades"].as_array().unwrap().contains(&prior["source"])));
        let views=current["views"].as_array().unwrap();let before=prior["views"].as_array().unwrap();assert_eq!(&views[..before.len()],before.as_slice());
        if current["current"]!=prior["current"] {let mut view=prior["current"].clone();view["name"]="whole-fmt433-current".into();assert!(views.contains(&view));}
        let mut restored=current.clone();restored["compiled"]=prior["compiled"].clone();restored["current"]=prior["current"].clone();restored["views"]=prior["views"].clone();
        if prior.get("fragments").is_some(){restored["fragments"]=prior["fragments"].clone();}assert_eq!(&restored,prior);
    }
    let mut restored=logical.clone();restored["rows"]=base["rows"].clone();assert_eq!(restored,base,"complete433 inverse");
    let rows=logical["rows"].as_array().unwrap();let mut edges=0;
    for row in rows {for child in row.get("fragments").and_then(|v|v.as_array()).into_iter().flatten(){assert!(child.get("fragments").is_none());assert_eq!(child["compiled"],true);assert_eq!(Some(child),rows.iter().find(|r|r["source"]==child["source"]));edges+=1;}}
    assert_eq!(edges,10);
    let local_data=include_str!("../../docs/evidence/fmt-local-441-20261018/whole/319ad85e33b0a6b384b9b4c7eeb10753cd321113bb048b27469c33c0ab7cc045.txt");
    assert_eq!((local_data.len(),digest(local_data.as_bytes())),(123158,"319ad85e33b0a6b384b9b4c7eeb10753cd321113bb048b27469c33c0ab7cc045".to_owned()));
    assert_eq!(read_view(&ext["whole_local133"]),local_data,"whole local133 physical/compiled anchor");
    let local=packet_local133();assert_eq!(local,serde_json::from_str::<serde_json::Value>(local_data).unwrap(),"entire original133 loader replay");assert_eq!(local["rows"].as_array().unwrap().len(),133);
    for prior in local["rows"].as_array().unwrap(){
        let row=rows.iter().find(|r|r["source"]==prior["source"]).unwrap();assert_eq!(row["compiled"],prior["compiled"]);
        let views=row["views"].as_array().unwrap();assert!(prior["views"].as_array().unwrap().iter().all(|v|views.contains(v)));
        if row["current"]!=prior["current"]{let mut view=prior["current"].clone();view["name"]="whole-local133-current".into();assert!(views.contains(&view));}
        let mut projected=row.clone();projected.as_object_mut().unwrap().remove("fragments");projected["current"]=prior["current"].clone();projected["views"]=prior["views"].clone();assert_eq!(&projected,prior,"complete local133 row projection");
    }
    logical
}

#[rustfmt::skip] // Retain the complete historical 133-row loader verbatim.
fn packet_local133() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/fmt-local-441-20261018/whole/319ad85e33b0a6b384b9b4c7eeb10753cd321113bb048b27469c33c0ab7cc045.txt");
    assert_eq!(data.len(), 123158);
    assert_eq!(digest(data.as_bytes()), "319ad85e33b0a6b384b9b4c7eeb10753cd321113bb048b27469c33c0ab7cc045");
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical=held(&root.join("docs/evidence/fmt-local-441-20261018/whole/319ad85e33b0a6b384b9b4c7eeb10753cd321113bb048b27469c33c0ab7cc045.txt"),data.len(),&digest(data.as_bytes())).unwrap();
    assert_eq!(physical,data,"complete physical and compiled current manifest");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 133);
    {
    let packet: serde_json::Value = serde_json::from_str(&read_view(&packet["whole_current124_before_fixture_repairs"])).unwrap();
    {
    let packet: serde_json::Value = serde_json::from_str(&read_view(&packet["whole_current99_before"])).unwrap();
    let before=read_view(&packet["immediate_before"]);
    let mut restored=packet.clone();
    let added=restored["rows"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(added["source"],"tests/native/string_settlement/codec_allocation/codec808_frontier.rs");
    let route=restored["rows"].as_array_mut().unwrap().iter_mut().find(|row|row["source"]=="src/source_reflows_merge/next_big_batch_inputs.rs").unwrap();
    let mut previous=route["views"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(previous["name"],"whole886-next-big");
    previous.as_object_mut().unwrap().remove("name");route["current"]=previous;
    restored.as_object_mut().unwrap().remove("immediate_before");
    assert_eq!(restored,serde_json::from_str::<serde_json::Value>(&before).unwrap(),"complete original98 source table retained");
    }
    {
    let previous: serde_json::Value = serde_json::from_str(&read_view(&packet["whole_3bf_current123_before"])).unwrap();
    let mut restored=packet.clone();
    let added=restored["rows"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(added["source"], "src/workspace_graph/tests.rs");
    let row=restored["rows"].as_array_mut().unwrap().iter_mut().find(|r|r["source"]=="src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs").unwrap();
    let mut prior=row["views"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(prior["name"], "whole3bf-before-private-lookup-import");
    prior.as_object_mut().unwrap().remove("name");row["current"]=prior;
    let anchor=restored["complete_old_table_anchors"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(anchor["sha256"], packet["whole_3bf_current123_before"]["sha256"]);
    restored.as_object_mut().unwrap().remove("whole_3bf_current123_before");
    restored["current_stage"]=previous["current_stage"].clone();
    assert_eq!(restored,previous,"complete previous123 table retained without retargeting old inputs");
    }
    let _remote = packet_remote_d88();
    }
    {
    let previous:serde_json::Value=serde_json::from_str(&read_view(&packet["whole_current124_before_fixture_repairs"])).unwrap();
    let mut restored=packet.clone();
    restored["rows"]=previous["rows"].clone();
    restored["complete_old_table_anchors"]=previous["complete_old_table_anchors"].clone();
    restored["current_stage"]=previous["current_stage"].clone();
    restored.as_object_mut().unwrap().remove("whole_current124_before_fixture_repairs");
    assert_eq!(restored,previous,"all original124 packet fields retained");
    for oldrow in previous["rows"].as_array().unwrap() {
        let row=packet["rows"].as_array().unwrap().iter().find(|r|r["source"]==oldrow["source"]).unwrap();
        assert_eq!(row["compiled"],oldrow["compiled"]);
        let oldviews=oldrow["views"].as_array().unwrap();
        assert_eq!(&row["views"].as_array().unwrap()[..oldviews.len()],oldviews.as_slice());
        assert!(row["current"]==oldrow["current"] || row["views"].as_array().unwrap().iter().any(|v|v["length"]==oldrow["current"]["length"]&&v["sha256"]==oldrow["current"]["sha256"]&&v["archive"]==oldrow["current"]["archive"]));
    }
    }
    for anchor in packet["complete_old_table_anchors"].as_array().unwrap() {
        let path=anchor["path"].as_str().unwrap();
        let raw=held(&root.join(path),anchor["length"].as_u64().unwrap()as usize,anchor["sha256"].as_str().unwrap()).unwrap();
        assert_eq!(Some(raw.as_str()),table_compiled_local133(path),"whole predecessor table physical and compiled bytes");
        assert_eq!(raw,read_view(anchor),"whole predecessor table archive");
    }
    packet
}
fn table_compiled_local133(source: &str) -> Option<&'static str> {
    match source {
        "docs/evidence/main-e177-final-source-binding-20261017/views/a1e425d6dec0ae2619b7c2f6964966a15e287aeecbdc27507166f13245a75fa6.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/a1e425d6dec0ae2619b7c2f6964966a15e287aeecbdc27507166f13245a75fa6.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/3fa32fc7c4b039abed780ccc39b34d759e329705e2674f62cc56016ab08007bd.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/3fa32fc7c4b039abed780ccc39b34d759e329705e2674f62cc56016ab08007bd.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/de9c72c5fba416e93d5fba422174cd0888c0bd6991fb2fc4367396c541bf92a7.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/de9c72c5fba416e93d5fba422174cd0888c0bd6991fb2fc4367396c541bf92a7.txt")),
        "docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json" => Some(include_str!("../../docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json")),
        "docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json" => Some(include_str!("../../docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/7ce1ea34daa31a1eda756cf7c0cd542adcfa3d351cbc3e0d12a37c51e5036986.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/7ce1ea34daa31a1eda756cf7c0cd542adcfa3d351cbc3e0d12a37c51e5036986.txt")),
        "docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json" => Some(include_str!("../../docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json")),
        "docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json" => Some(include_str!("../../docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json")),
        "docs/evidence/next-semantic-source-views-20261017/inputs.json" => Some(include_str!("../../docs/evidence/next-semantic-source-views-20261017/inputs.json")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/bc66e360cefb55655815018153316bca517702bc32791462f2dfb9ca70b90d41.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/bc66e360cefb55655815018153316bca517702bc32791462f2dfb9ca70b90d41.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/2e4b3b5441418b7e21a1424445e19bb4473d15f196ab3a897ca920417dce730f.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/2e4b3b5441418b7e21a1424445e19bb4473d15f196ab3a897ca920417dce730f.txt")),
        "docs/evidence/next-large-side-source-composition-20261017/side-inputs.json" => Some(include_str!("../../docs/evidence/next-large-side-source-composition-20261017/side-inputs.json")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/996aa3de35097ef764384db3b8fbaffa57bb500fb18368280a20dac9c3ff5f52.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/996aa3de35097ef764384db3b8fbaffa57bb500fb18368280a20dac9c3ff5f52.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/3740fb709bdfb10aeb16a6159312f12b8752d2c910ac7ea6aae727ff01847d09.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/3740fb709bdfb10aeb16a6159312f12b8752d2c910ac7ea6aae727ff01847d09.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/802a9e325c6d4e9da033593b365119441fb2c1431cb981793fe4026e29ff6734.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/802a9e325c6d4e9da033593b365119441fb2c1431cb981793fe4026e29ff6734.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/c15e26ecf4ee9f28eb21af1515c7c1d1e6aa3f1983d37b413173861d72d76a46.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/c15e26ecf4ee9f28eb21af1515c7c1d1e6aa3f1983d37b413173861d72d76a46.txt")),
        _ => None,
    }
}
fn packet_fmt433() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json"
    );
    assert_eq!(data.len(), 260977);
    assert_eq!(
        digest(data.as_bytes()),
        "de6103a37cb677be139de99b4a945ddf2d588b5292a01ca058715b3d7f8734dc"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        held(
            &root.join("docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json"),
            data.len(),
            &digest(data.as_bytes())
        )
        .unwrap(),
        data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["source_cap"].as_u64(), Some(SOURCE_CAP as u64));
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 433);
    let mut seen = std::collections::BTreeSet::new();
    let mut fragment_edges = 0;
    for row in rows {
        assert!(seen.insert(row["source"].as_str().unwrap()));
        if let Some(children) = row.get("fragments") {
            for child in children.as_array().unwrap() {
                assert!(
                    child.get("fragments").is_none(),
                    "fragment dependencies are leaves"
                );
                assert_eq!(child["compiled"], true);
                assert_eq!(
                    rows.iter()
                        .find(|candidate| candidate["source"] == child["source"])
                        .unwrap(),
                    child
                );
                fragment_edges += 1;
            }
        }
    }
    assert_eq!(fragment_edges, 10);
    // Replay every complete previous table and its original checks unchanged.
    let previous = packet_a110();
    let old_rows = previous["rows"].as_array().unwrap();
    assert!(rows.len() >= old_rows.len());
    for (before, after) in old_rows.iter().zip(rows) {
        assert_eq!(after["source"], before["source"], "original source order");
        let views = after["views"].as_array().unwrap();
        let old_views = before["views"].as_array().unwrap();
        assert!(views.len() >= old_views.len());
        assert_eq!(
            &views[..old_views.len()],
            old_views.as_slice(),
            "all original view fields and order"
        );
        if after["current"] != before["current"] {
            assert!(
                views.iter().any(|view| {
                    let mut view = view.clone();
                    view.as_object_mut().unwrap().remove("name");
                    view == before["current"]
                }),
                "complete previous current record must remain replayable"
            );
        }
        let mut restored = after.clone();
        restored["current"] = before["current"].clone();
        restored["views"] = before["views"].clone();
        restored.as_object_mut().unwrap().remove("fragments");
        assert_eq!(&restored, before, "all other original row fields");
    }
    let mut restored = packet.clone();
    restored["rows"] = previous["rows"].clone();
    assert_eq!(
        restored, previous,
        "all original packet fields and whole-table anchors"
    );
    packet
}
fn packet_a110() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/fmt-a110/packet-wV4m7PTunyjrIa8VFcfB0eaqPxmD03tBMXOGHXLXakY.json.txt"
    );
    assert_eq!(data.len(), 112949);
    assert_eq!(
        digest(data.as_bytes()),
        "c15e26ecf4ee9f28eb21af1515c7c1d1e6aa3f1983d37b413173861d72d76a46"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = held(
        &root.join(
            "docs/evidence/fmt-a110/packet-wV4m7PTunyjrIa8VFcfB0eaqPxmD03tBMXOGHXLXakY.json.txt",
        ),
        data.len(),
        &digest(data.as_bytes()),
    )
    .unwrap();
    assert_eq!(
        physical, data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 124);
    {
        let packet: serde_json::Value =
            serde_json::from_str(&read_view(&packet["whole_current99_before"])).unwrap();
        let before = read_view(&packet["immediate_before"]);
        let mut restored = packet.clone();
        let added = restored["rows"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(
            added["source"],
            "tests/native/string_settlement/codec_allocation/codec808_frontier.rs"
        );
        let route = restored["rows"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["source"] == "src/source_reflows_merge/next_big_batch_inputs.rs")
            .unwrap();
        let mut previous = route["views"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(previous["name"], "whole886-next-big");
        previous.as_object_mut().unwrap().remove("name");
        route["current"] = previous;
        restored.as_object_mut().unwrap().remove("immediate_before");
        assert_eq!(
            restored,
            serde_json::from_str::<serde_json::Value>(&before).unwrap(),
            "complete original98 source table retained"
        );
    }
    {
        let previous: serde_json::Value =
            serde_json::from_str(&read_view(&packet["whole_3bf_current123_before"])).unwrap();
        let mut restored = packet.clone();
        let added = restored["rows"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(added["source"], "src/workspace_graph/tests.rs");
        let row=restored["rows"].as_array_mut().unwrap().iter_mut().find(|r|r["source"]=="src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs").unwrap();
        let mut prior = row["views"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(prior["name"], "whole3bf-before-private-lookup-import");
        prior.as_object_mut().unwrap().remove("name");
        row["current"] = prior;
        let anchor = restored["complete_old_table_anchors"]
            .as_array_mut()
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            anchor["sha256"],
            packet["whole_3bf_current123_before"]["sha256"]
        );
        restored
            .as_object_mut()
            .unwrap()
            .remove("whole_3bf_current123_before");
        restored["current_stage"] = previous["current_stage"].clone();
        assert_eq!(
            restored, previous,
            "complete previous123 table retained without retargeting old inputs"
        );
    }
    let _remote = packet_remote_d88();
    for anchor in packet["complete_old_table_anchors"].as_array().unwrap() {
        let path = anchor["path"].as_str().unwrap();
        let raw = held(
            &root.join(path),
            anchor["length"].as_u64().unwrap() as usize,
            anchor["sha256"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            Some(raw.as_str()),
            table_compiled(path),
            "whole predecessor table physical and compiled bytes"
        );
        assert_eq!(raw, read_view(anchor), "whole predecessor table archive");
    }
    packet
}
fn packet_remote_d88() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt");
    assert_eq!(data.len(), 68224);
    assert_eq!(
        digest(data.as_bytes()),
        "f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(held(&root.join("docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt"), data.len(), &digest(data.as_bytes())).unwrap(), data, "complete physical and compiled current manifest");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    let mut restored = packet.clone();
    let row = restored["rows"].as_array_mut().unwrap().iter_mut().find(|row| row["source"] == "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors/compare.rs").unwrap();
    let mut previous = row["views"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(previous["name"], "whole-f47-msrv-parent-import");
    previous.as_object_mut().unwrap().remove("name");
    row["current"] = previous;
    assert_eq!(
        restored,
        packet_previous_remote_d88(),
        "complete predecessor table and all original controls retained"
    );
    packet
}
fn packet_previous_remote_d88() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/ci-msrv-parent-import-20261018/whole/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt");
    assert_eq!(data.len(), 84171);
    assert_eq!(
        digest(data.as_bytes()),
        "3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical=held(&root.join("docs/evidence/ci-msrv-parent-import-20261018/whole/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt"),data.len(),&digest(data.as_bytes())).unwrap();
    assert_eq!(
        physical, data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 99);
    let before = read_view(&packet["immediate_before"]);
    let mut restored = packet.clone();
    let added = restored["rows"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(
        added["source"],
        "tests/native/string_settlement/codec_allocation/codec808_frontier.rs"
    );
    let route = restored["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["source"] == "src/source_reflows_merge/next_big_batch_inputs.rs")
        .unwrap();
    let mut previous = route["views"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(previous["name"], "whole886-next-big");
    previous.as_object_mut().unwrap().remove("name");
    route["current"] = previous;
    restored.as_object_mut().unwrap().remove("immediate_before");
    assert_eq!(
        restored,
        serde_json::from_str::<serde_json::Value>(&before).unwrap(),
        "complete original98 source table retained"
    );
    for anchor in packet["complete_old_table_anchors"].as_array().unwrap() {
        let path = anchor["path"].as_str().unwrap();
        let raw = held(
            &root.join(path),
            anchor["length"].as_u64().unwrap() as usize,
            anchor["sha256"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            Some(raw.as_str()),
            table_compiled(path),
            "whole predecessor table physical and compiled bytes"
        );
        assert_eq!(raw, read_view(anchor), "whole predecessor table archive");
    }
    packet
}
