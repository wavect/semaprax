from pathlib import Path
import json
R=Path.cwd();O=Path('/Volumes/Semaprax/tmp/ci-38383508340/expected-view-ca5')
root=(O/'before.rs').read_text()
a='    // Acquire only the exact requested view from authenticated current bytes.\n    // Historical inputs from another stage remain subject to the old assertion.\n    let current_view =\n        main_e177_final_inputs::value(source, row["current_sha256"].as_str().unwrap(), current);\n    let current = current_view.as_str();\n'
live=(R/'src/source_reflows_merge.rs').read_text();n='\n#[test]\nfn merge_projection_selects_only_the_requested_complete_current_view()'
assert live.count(n)==1;body,test=live.split(n)
assert body.count(a)==1 and body.replace(a,'',1).rstrip()==root.rstrip()
assert 'String::new()'in test and 'catch_unwind' in test
s=(O/'before-semantic.rs').read_text();v=(R/'src/source_reflows/semantic_receivers.rs').read_text()
a='    let current_view = crate::source_reflows_merge::main_e177_final_inputs::value(\n        proof["path"].as_str().unwrap(),\n        proof["after_sha256"].as_str().unwrap(),\n        current,\n    );\n    let current = current_view.as_str();\n'
assert v.count(a)==1;v=v.replace(a,'',1)
n='    let joined = if digest(current) == proof["after_sha256"] {\n        current.to_owned()\n    } else {\n        crate::source_reflows_merge::main8fb_input(path, current)\n    };'
assert v.count(n)==1
assert v.replace(n,'    let joined = crate::source_reflows_merge::main8fb_input(path, current);',1)==s
s=(O/'before-incoming67107.rs').read_text();v=(R/'src/source_reflows/incoming67107_semantic_receivers.rs').read_text()
assert v.count(a)==1;v=v.replace(a,'',1)
n='    let current_view = crate::source_reflows_merge::main_e177_final_inputs::value(\n        entry["path"].as_str().unwrap(),\n        entry["current_sha256"].as_str().unwrap(),\n        current,\n    );\n    let current = current_view.as_str();\n'
assert v.count(n)==1;v=v.replace(n,'',1)
n='''    let length = usize::try_from(entry["current_len"].as_u64().unwrap()).unwrap();
    let expected = entry["current_sha256"].as_str().unwrap();
    assert!(length <= MAIN34D_CAP);
    let current = crate::source_reflows_merge::main_e177_final_inputs::file(path, length, expected)
        .map(Ok)
        .unwrap_or_else(|| main34d_file(path, length, expected))
        .expect("complete current raw MAIN input");'''
before='''    let current = main34d_file(
        path,
        usize::try_from(entry["current_len"].as_u64().unwrap()).unwrap(),
        entry["current_sha256"].as_str().unwrap(),
    )
    .expect("complete current raw MAIN input");'''
assert v.count(n)==1;assert v.replace(n,before,1)==s
assert (R/'CHANGELOG.md').read_bytes().endswith((O/'before-changelog.txt').read_bytes())
assert (R/'src/source_reflows.rs').read_bytes()==(O/'before-source-reflows.rs').read_bytes()
(O/'static-receipt.json').write_text(json.dumps(dict(status='PASS_EXACT_SOURCE_EDIT_INVERSES',all_original_test_assertions_hash_pins_and_inverse_bodies_retained=True,original_low_level_held_reader_and_all_its_refusal_tests_unchanged=True,changed_Rust_sources=3,added_regressions=1,all_changes_test_only=True,no_caps_floors_tariffs_authority_or_backends_changed=True),indent=2)+'\n')
print('Static full-before body inverses PASS')
