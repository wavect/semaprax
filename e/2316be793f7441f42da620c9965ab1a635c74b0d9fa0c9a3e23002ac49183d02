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

// Test-only bounded-reader capture/routing. The complete original400line
// acquisition producer above remains byte-for-byte and is executed by tests.
pub(super) mod invocation_probe {
    use super::super::{held_4a6eb,SOURCE_CAP};
    use std::{cell::RefCell,path::{Path,PathBuf},rc::Rc};
    #[derive(Clone,Debug,PartialEq,Eq)]
    pub(crate) struct Pin {pub path:String,pub length:usize,pub sha:String}
    #[derive(Clone,Default,Debug)]
    pub(crate) struct Stats {pub reads:Vec<Pin>,pub calls:usize,pub stages:usize,pub bytes:usize}
    #[derive(Clone)]
    struct State {root:Option<PathBuf>,allowed:Vec<Pin>,stats:Rc<RefCell<Stats>>,late:Rc<RefCell<Option<String>>>}
    thread_local! {static ACTIVE:RefCell<Option<State>>=const{RefCell::new(None)};}
    pub(crate) struct Scope {prior:Option<State>,state:State}
    impl Scope {
        fn install(root:Option<PathBuf>,allowed:Vec<Pin>)->Self {
            let state=State {root,allowed,stats:Default::default(),late:Default::default()};let prior=ACTIVE.with(|slot|slot.replace(Some(state.clone())));Self {prior,state}
        }
        pub(crate) fn capture()->Self {Self::install(None,Vec::new())}
        pub(crate) fn route(root:PathBuf,allowed:Vec<Pin>)->Self {assert!(root.is_absolute());Self::install(Some(root),allowed)}
        pub(crate) fn stats(&self)->Stats {self.state.stats.borrow().clone()}
        pub(crate) fn reset(&self) {*self.state.stats.borrow_mut()=Stats::default();}
        pub(crate) fn late(&self,path:&str) {assert!(self.state.root.is_some()&&self.state.allowed.iter().any(|p|p.path==path));*self.state.late.borrow_mut()=Some(path.to_owned());}
    }
    impl Drop for Scope {fn drop(&mut self){ACTIVE.with(|slot|slot.replace(self.prior.take()));}}
    pub(crate) fn read(path:&Path,length:usize,sha:&str)->Option<Result<String,&'static str>> {
        if length>SOURCE_CAP {return None;}
        let root=Path::new(env!("CARGO_MANIFEST_DIR"));let relative=path.strip_prefix(root).ok()?;
        if !relative.components().all(|p|matches!(p,std::path::Component::Normal(_))) {return None;}
        let name=relative.to_str()?;
        let mapped=ACTIVE.with(|slot| {
            let slot=slot.borrow();let state=slot.as_ref()?;let mut stats=state.stats.borrow_mut();stats.bytes+=length;stats.reads.push(Pin {path:name.to_owned(),length,sha:sha.to_owned()});
            // A known path still receives the ORIGINAL requested length/hash,
            // so hostile pin requests fail in the unchanged ordinary reader.
            if !state.allowed.iter().any(|p|p.path==name) {return None;}
            state.root.as_ref().map(|root|root.join(relative))
        });
        // Drop all TLS/RefCell loans before recursion. A fixture path lies
        // outside the real root and therefore executes the full old reader.
        mapped.map(|path|held_4a6eb(&path,length,sha))
    }
    pub(crate) fn note(stage:bool) {ACTIVE.with(|slot|{if let Some(state)=slot.borrow().as_ref(){let mut stats=state.stats.borrow_mut();if stage {stats.stages+=1;}else{stats.calls+=1;}}});}
    pub(crate) fn after_body() {
        let target=ACTIVE.with(|slot| {let slot=slot.borrow();let state=slot.as_ref()?;let path=state.late.borrow_mut().take()?;Some(state.root.as_ref()?.join(path))});
        if let Some(path)=target {let mut bytes=std::fs::read(&path).unwrap();bytes[0]^=1;std::fs::write(path,bytes).unwrap();}
    }
}

#[cfg(test)]
mod invocation_acquisition_tests {
    use super::{packet_before_overlay as original_before,invocation_probe::{Pin,Scope}};
    use super::super::{overlay_v2,packet_lookup::ProjectionInvocation,digest,held_4a6eb};
    use std::{cell::Cell,path::{Path,PathBuf}};
    const SOURCE:&str="src/workspace_graph/expected_projection/borrowed_imports.rs";
    const POLICY:&str="docs/evidence/finite-source-overlay-v2-20261018/policy.json";
    const PAGE:&str="docs/evidence/finite-source-overlay-v2-20261018/page.json";
    fn original_packet()->serde_json::Value {overlay_v2::compose(original_before())}
    struct Fixture {root:PathBuf,pins:Vec<Pin>,packet:serde_json::Value,original_reads:Vec<Pin>}
    impl Fixture {
        fn new()->Self {
            static NEXT:std::sync::atomic::AtomicUsize=std::sync::atomic::AtomicUsize::new(0);
            // This is the WHOLE independently retained original producer, with
            // every loop, parser assertion, inverse and unknown field intact.
            let capture=Scope::capture();let before=original_before();let historical=capture.stats().reads;drop(capture);
            assert_eq!(before,super::super::packet_before_overlay());
            assert_eq!(historical.len(),overlay_v2::INVOCATION_PACKET_READS.len());
            for (actual,&(path,length,sha)) in historical.iter().zip(overlay_v2::INVOCATION_PACKET_READS) {assert_eq!((actual.path.as_str(),actual.length,actual.sha.as_str()),(path,length,sha));}
            let capture=Scope::capture();let packet=original_packet();let original_reads=capture.stats().reads;drop(capture);
            let capture=Scope::capture();let _=super::super::clippy_stage::rows();let stage=capture.stats().reads;drop(capture);
            let mut pins=original_reads.clone();pins.extend(stage);
            for (path,raw) in overlay_v2::invocation_controllers() {pins.push(Pin {path:path.to_owned(),length:raw.len(),sha:digest(raw.as_bytes())});}
            let root=std::env::temp_dir().join(format!("semaprax-full-acquisition-{}-{}",std::process::id(),NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));std::fs::create_dir(&root).unwrap();
            let actual=Path::new(env!("CARGO_MANIFEST_DIR"));let mut unique=std::collections::BTreeMap::new();
            for pin in &pins {if let Some(old)=unique.insert(pin.path.clone(),(pin.length,pin.sha.clone())) {assert_eq!(old,(pin.length,pin.sha.clone()));}let raw=held_4a6eb(&actual.join(&pin.path),pin.length,&pin.sha).unwrap();let target=root.join(&pin.path);std::fs::create_dir_all(target.parent().unwrap()).unwrap();std::fs::write(target,raw).unwrap();}
            Self {root,pins,packet,original_reads}
        }
        fn route(&self)->Scope {Scope::route(self.root.clone(),self.pins.clone())}
        fn flip(&self,path:&str) {assert!(self.pins.iter().any(|p|p.path==path));let target=self.root.join(path);let mut bytes=std::fs::read(&target).unwrap();bytes[0]^=1;std::fs::write(target,bytes).unwrap();}
    }
    impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.root);}}
    fn refused(body:impl FnOnce())->String {let panic=std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).unwrap_err();if let Some(s)=panic.downcast_ref::<String>() {s.clone()}else if let Some(s)=panic.downcast_ref::<&str>() {(*s).to_owned()}else{panic!("unexpected panic type")}}
    #[test]
    fn whole_original_acquisition_generates_guard_trace_and_same_owner_drift_refuses() {
        let fixture=Fixture::new();
        let raw=super::super::compiled(SOURCE).unwrap();let expected=digest(raw.as_bytes());
        let capture=Scope::capture();
        let mut old_body=ProjectionInvocation::with_loaders(||fixture.packet.clone(),super::super::clippy_stage::rows);
        assert_eq!(old_body.independent_old_value_direct(SOURCE,&expected,raw),raw);
        let body_reads=capture.stats().reads;drop(capture);drop(old_body);
        let scope=fixture.route();let loads=Cell::new(0);let mut context=ProjectionInvocation::with_loaders(||{loads.set(loads.get()+1);super::super::packet()},super::super::clippy_stage::rows);context.guard=overlay_v2::InvocationGuard::Current;
        let raw=super::super::compiled(SOURCE).unwrap();let expected=digest(raw.as_bytes());assert_eq!(context.value_direct(SOURCE,&expected,raw),raw);scope.reset();assert_eq!(context.value_direct(SOURCE,&expected,raw),raw);
        let stats=scope.stats();assert_eq!(stats.calls,2);assert_eq!(stats.stages,0);let controllers=overlay_v2::invocation_controllers();let controller_bytes:usize=controllers.iter().map(|(_,s)|s.len()).sum();let oldbytes:usize=fixture.original_reads.iter().map(|p|p.length).sum();let body_bytes:usize=body_reads.iter().map(|p|p.length).sum();assert_eq!(stats.bytes,2*(oldbytes+controller_bytes)+body_bytes);
        let one=fixture.original_reads.len()+controllers.len();assert_eq!(stats.reads.len(),2*one+body_reads.len());assert_eq!(&stats.reads[..fixture.original_reads.len()],fixture.original_reads.as_slice());assert_eq!(&stats.reads[one..one+body_reads.len()],body_reads.as_slice());let late=one+body_reads.len();assert_eq!(&stats.reads[late..late+fixture.original_reads.len()],fixture.original_reads.as_slice());
        let archive=format!("e/{expected}");for path in [SOURCE,archive.as_str(),PAGE,POLICY,controllers[0].0] {fixture.flip(path);scope.reset();let error=refused(||{context.value_direct(SOURCE,&expected,raw);});assert!(error.contains("length-or-digest"));assert_eq!(scope.stats().reads.last().unwrap().path,path);fixture.flip(path);assert_eq!(context.value_direct(SOURCE,&expected,raw),raw);}
        assert_eq!(loads.get(),1);assert_eq!(fixture.packet,super::super::packet());
    }
    #[test]
    fn two_fault_selection_matches_executed_whole_original_producer_not_a_trace_copy() {
        let fixture=Fixture::new();let scope=fixture.route();let mut context=ProjectionInvocation::with_loaders(super::super::packet,super::super::clippy_stage::rows);context.guard=overlay_v2::InvocationGuard::Current;let raw=super::super::compiled(SOURCE).unwrap();let expected=digest(raw.as_bytes());assert_eq!(context.value_direct(SOURCE,&expected,raw),raw);
        let archive=format!("e/{expected}");for (first,second) in [(fixture.original_reads[0].path.as_str(),POLICY),(POLICY,PAGE),(SOURCE,archive.as_str())] {
            fixture.flip(first);fixture.flip(second);scope.reset();let old=refused(||{original_packet();});let oldlast=scope.stats().reads.last().unwrap().clone();scope.reset();let current=refused(||{context.value_direct(SOURCE,&expected,raw);});let newlast=scope.stats().reads.last().unwrap().clone();assert_eq!(newlast,oldlast);assert_eq!(oldlast.path,first);assert!(old.contains("length-or-digest")&&current.contains("length-or-digest"));fixture.flip(first);fixture.flip(second);assert_eq!(context.value_direct(SOURCE,&expected,raw),raw);
        }
    }
    #[test]
    fn logical_endpoints_keep_one_scoped_early_and_late_guard_through_nested_readers() {
        let fixture=Fixture::new();let scope=fixture.route();let mut context=ProjectionInvocation::with_loaders(super::super::packet,super::super::clippy_stage::rows);context.guard=overlay_v2::InvocationGuard::Current;
        let row=fixture.packet["rows"].as_array().unwrap().iter().find(|r|r["source"]==SOURCE).unwrap();let prior=&row["views"][0];let path=Path::new(env!("CARGO_MANIFEST_DIR")).join(SOURCE);let first=context.file(&path,prior["length"].as_u64().unwrap()as usize,prior["sha256"].as_str().unwrap());assert!(first.is_some());scope.reset();
        assert_eq!(context.file(&path,prior["length"].as_u64().unwrap()as usize,prior["sha256"].as_str().unwrap()),first);assert_eq!(scope.stats().calls,2,"file -> private row reader shares one call state");
        let side=super::super::combined_side_packet();let row=&side["rows"][0];let prior=&row["views"][0];let path=Path::new(env!("CARGO_MANIFEST_DIR")).join(row["source"].as_str().unwrap());scope.reset();assert!(context.combined_side_file(&path,prior["length"].as_u64().unwrap()as usize,prior["sha256"].as_str().unwrap()).is_some());assert_eq!(scope.stats().calls,2,"combined -> file -> value/direct all share one call state");
        scope.reset();assert_eq!(context.file(Path::new("/foreign-unregistered-root"),0,"foreign"),None);assert_eq!(context.combined_side_file(Path::new("/foreign-unregistered-root"),0,"foreign"),None);assert_eq!((scope.stats().calls,scope.stats().bytes),(0,0));
        scope.reset();scope.late(POLICY);let raw=super::super::compiled(SOURCE).unwrap();assert!(refused(||{context.value_direct(SOURCE,&digest(raw.as_bytes()),raw);}).contains("length-or-digest"));assert_eq!(scope.stats().calls,2);assert_eq!(scope.stats().reads.last().unwrap().path,POLICY);fixture.flip(POLICY);assert_eq!(context.value_direct(SOURCE,&digest(raw.as_bytes()),raw),raw);
    }
    #[test]
    fn cached_stage_is_not_used_until_this_logical_call_requests_it() {
        let fixture=Fixture::new();let scope=fixture.route();let mut context=ProjectionInvocation::with_loaders(super::super::packet,super::super::clippy_stage::rows);context.guard=overlay_v2::InvocationGuard::Current;let known="src/ast/program_source/function_alias/presentation.rs";
        super::super::packet_lookup::test_stage_metadata(&mut context,known);assert!(scope.stats().stages>0);let path=overlay_v2::invocation_stages()[0].0;fixture.flip(path);scope.reset();let raw=super::super::compiled(SOURCE).unwrap();assert_eq!(context.value_direct(SOURCE,&digest(raw.as_bytes()),raw),raw);assert_eq!(scope.stats().stages,0);assert!(refused(||super::super::packet_lookup::test_stage_metadata(&mut context,known)).contains("length-or-digest"));assert_eq!(scope.stats().reads.last().unwrap().path,path);fixture.flip(path);
    }
    #[test]
    fn independent_old_producer_and_original_bounded_reader_are_whole() {
        let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/05ecc0163fa3593182d2044c5f2029e3c1dd0cd7143ed9904eccfd6e6b751a3d"));let current=include_str!("main_e177_historical_before_data.rs");assert_eq!(&current[..old.len()],old,"complete original independent producer remains byte prefix");
        let old=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/7ea19e20137bd5d4c203072d41eab7cc0c835f847e51b94a4b24eeac997589f4"));let current=include_str!("main_e177_final_inputs.rs");let hook="    #[cfg(test)]\n    if let Some(result)=historical_before_data::invocation_probe::read(path,length,expected) { return result; }\n";assert_eq!(current.matches(hook).count(),1);assert_eq!(current.replacen(hook,"",1),old,"complete old root including bounded reader remains exact after removing only hook");
    }
#[test]
fn combined_expected_invocation_keeps_every_prior_and_foreign_value_with_one_owned_packet() {
    let packet=super::super::combined_side_packet();let row=&packet["rows"].as_array().unwrap()[0];
    let source=row["source"].as_str().unwrap();let current=super::super::combined_side_actual(row);
    let loads=std::cell::Cell::new(0);let stages=std::cell::Cell::new(0);
    let mut invocation=ProjectionInvocation::with_loaders(||{loads.set(loads.get()+1);super::super::packet()},||{stages.set(stages.get()+1);super::super::clippy_stage::rows()});
    for prior in row["views"].as_array().unwrap() {
        let expected=prior["sha256"].as_str().unwrap();let old=super::super::read_view(prior);
        for supplied in [current.as_str(),old.as_str(),"foreign-same-invocation"] {
            assert_eq!(invocation.combined_side_expected_value(source,expected,supplied),super::super::combined_side_expected_value(source,expected,supplied));
        }
    }
    assert_eq!(loads.get(),1);assert!(stages.get()<=1);
    assert_eq!(invocation.combined_side_expected_value("foreign",&"0".repeat(64),"unchanged"),"unchanged");
}

}
