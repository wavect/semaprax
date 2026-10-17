"""Read-only source transport proof. Does not execute Rust or authority tools."""
from pathlib import Path
import argparse, hashlib, json, subprocess

REL = Path('docs/evidence/main34d-three-source-domains-20261017')
MAIN = '34d53510a3374537874c89a0e46be4d68ad9fe61'
sha = lambda b: hashlib.sha256(b).hexdigest()

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--repo', type=Path, required=True)
    p.add_argument('--inputs', type=Path, required=True)
    a = p.parse_args(); r = a.repo.resolve(); inputs = a.inputs.resolve(); d = r / REL
    assert r.is_relative_to('/Volumes/Semaprax') and inputs.is_relative_to('/Volumes/Semaprax')
    def read(path):
        local = r / path
        return local.read_bytes() if str(path).startswith(str(REL)) else (inputs / path).read_bytes()
    def old(path):
        return subprocess.check_output(['git', '-C', str(r), 'show', MAIN + ':' + path])
    manifest = json.loads((d/'input-manifest.json').read_bytes())
    entries = {e['path']: e for e in manifest['inputs']}
    assert len(entries) == len(manifest['inputs']) == 49
    expected_domains = [('held-status','held-status-prefix-v1',3,6), ('held-continuation','held-continuation-growth-v1',2,10), ('receiving-lookup','paid-expression-lookup-state-v1',4,10)]
    total = 0
    for domain, (name, directory, views, sources) in zip(manifest['domains'], expected_domains, strict=True):
        assert domain['name'] == name
        allowed = []
        for table, count in [('views.json', views), ('sources.json', sources)]:
            path = f'docs/evidence/{directory}/{table}'
            raw = old(path); assert (inputs/path).read_bytes() == raw
            rows = json.loads(raw); assert len(rows) == count
            allowed.append(path)
            for row in rows:
                current = old(row['path']); before = old(row['archive'])
                assert sha(current) == row['after_sha256'] and sha(before) == row['before_sha256']
                restored = current.decode('utf-8')
                for edit in row['inverse']:
                    assert restored.count(edit['after']) == 1
                    restored = restored.replace(edit['after'], edit['before'], 1)
                assert restored.encode('utf-8') == before
                allowed.extend([row['path'], row['archive']]); total += 1
        assert domain['reads'] == list(dict.fromkeys(allowed))
    assert total == 35  # nine views and all ordered 6/10/10 whole-source rows.
    for path, e in entries.items():
        current = read(e['current_archive']); prior = read(e['main_archive'])
        assert len(current) == e['current_len'] and sha(current) == e['current_sha256']
        assert len(prior) == e['main_len'] and sha(prior) == e['main_sha256']
        assert prior == old(path)
        assert max(len(current), len(prior)) <= 9_317_546
        assert e['whole_source_inverse'] == [{'at':0,'remove_len':len(current),'remove_sha256':sha(current),'insert_archive':e['main_archive']}]
    for name, count in [('main_receiver_proofs',12), ('main_live_audits',2)]:
        e = manifest[name]; raw = read(e['archive'])
        assert len(raw) == e['len'] and sha(raw) == e['sha256'] and len(json.loads(raw)) == count
    source = (r/'src/source_reflows/semantic_receivers.rs').read_text()
    # The complete new BCC owner and whole previous receiver are authenticated
    # before obtaining the unchanged original three-owner suffix below.
    bcc_checker = r/'docs/evidence/mainbcc-ordinary-source-domain-20261017/check-source.py'
    assert sha(bcc_checker.read_bytes()) == '2f86c576f4b852e739eaa79101f80e2e8d46c4391a870822e45ef579aff80a0d'
    import runpy
    source = runpy.run_path(str(bcc_checker))['previous_receiver'](r, source)
    original = (d/'original-programs/semantic_receivers.main34d.txt').read_text()
    first = 'fn held_status_views_preserve_complete_original_inputs_and_validation_loops() {'
    original = original[original.index(first):]
    adapted = source[source.index(first):]
    for name in ['held-status','held-continuation','receiving-lookup']:
        preamble = '\n    // Explicit historical MAIN34d source view; actual raw roles remain live.\n    let main_read = |path: std::path::PathBuf| main34d_read("'+name+'", &path);\n    let invert = invert_ordinary;\n    let proofs = main34d_proofs;\n    let predecessor = main34d_predecessor;'
        assert adapted.count(preamble) == 1
        adapted = adapted.replace(preamble, '', 1)
    assert adapted.replace('main_read(', 'std::fs::read_to_string(') == original
    before = (d/'original-programs/semantic_receivers.merged-before.txt').read_text()
    def between(s, left, right): return s[s.index(left):s.index(right)]
    assert between(source,'fn invert(', 'pub(super) fn predecessor(') == between(before,'fn invert(', 'pub(super) fn predecessor(')
    helper = (r/'src/source_reflows/incoming67107_semantic_receivers.rs').read_bytes()
    old_helper = (d/'original-programs/src__source_reflows__incoming67107_semantic_receivers.rs.ours492.txt').read_bytes()
    assert helper.startswith(old_helper)
    assert b'include_str!' not in helper[len(old_helper):]
    for row in json.loads((d/'source-inverse.json').read_bytes()):
        raw = (r/row['path']).read_bytes(); archive = (r/row['original_archive']).read_bytes()
        assert len(raw) == row['current_len'] and sha(raw) == row['current_sha256']
        assert len(archive) == row['original_len'] and sha(archive) == row['original_sha256']
        assert row['inverse'] == [{'at':0,'remove_len':len(raw),'remove_sha256':sha(raw),'insert_archive':row['original_archive']}]
    print(json.dumps({'status':'PASS_SOURCE_ONLY_NOT_RUST_EXECUTION','inputs':49,'original_view_rows':9,'original_whole_rows':26,'original_full_inverse_rows':35,'original_receiver_inventory':14,'original_MAIN_test_suffix_byte_exact_after_finite_acquisitions':True,'old_inverter_and_helper_prefix_unchanged':True}))

if __name__ == '__main__': main()
