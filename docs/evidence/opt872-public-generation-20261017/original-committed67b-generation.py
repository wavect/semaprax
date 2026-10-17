"""Closed committed67b pin-generation adaptation; no build/capture/apply."""
from pathlib import Path
import ast,copy,hashlib,json
HERE=Path(__file__).resolve().parent
enc=lambda value:json.dumps(value,separators=(',',':'),ensure_ascii=False).encode()
sha=lambda data:hashlib.sha256(data).hexdigest()
BASELINE_BINDING_SHA256='9e538726eac40c7349fd9d6e3daddb87df77d00e2bc42e430dd9b92f7242e51c'
MAX_BUILDER_BYTES=67_108_864

def unique(pairs):
    result={}
    for key,value in pairs:
        assert key not in result,('duplicate JSON field',key)
        result[key]=value
    return result

def baseline(root):
    raw=(HERE/'baseline-binding.json').read_bytes()
    assert sha(raw)==BASELINE_BINDING_SHA256,'fixed reviewed baseline binding drift'
    binding=json.loads(raw,object_pairs_hook=unique)
    root=Path(root).resolve();path=Path(binding['path']).resolve()
    assert path==root/'docs/evidence/managed67b-semantic-history-20261016/transition-binding.json'
    raw=path.read_bytes();assert sha(raw)==binding['sha256']
    data=json.loads(raw,object_pairs_hook=unique);assert data['actual_wire_sha256']==binding['wire_sha256']
    assert data['actual_head']=='67b1b02141f19040fdd1b45bc9bea16dcff5d74e'
    capture=path.parent/'actual67b'
    assert len(binding['wire_sha256'])==10
    assert sorted(p.name for p in (capture/'wires').glob('*.json'))==sorted(binding['wire_sha256'])
    for name,digest in binding['wire_sha256'].items():
        assert Path(name).name==name and sha((capture/'wires'/name).read_bytes())==digest
    return capture,binding['wire_sha256']

def domain_digest(value,role):
    key='graph_digest' if role=='graph' else 'artifact_digest'
    domain=('semaprax.workspace-semantic-'+role+'.artifact-digest.v1\0').encode()
    raw=enc(value);field=b','+enc(key)+b':'+enc(value[key]);assert raw.count(field)==1
    payload=raw.replace(field,b'',1)
    return 'sha256:'+sha(domain+len(payload).to_bytes(8,'little')+payload)

def validate_bindings(value,role,public_graph):
    key='graph_digest' if role=='graph' else 'artifact_digest'
    assert domain_digest(value,role)==value[key],'unbound domain digest'
    budget=value['budget'] if role=='graph' else value['budget']['workspace']
    amount=budget['used_builder_bytes']
    assert type(amount) is int and 0<amount<=MAX_BUILDER_BYTES,'invalid builder bill'
    if role!='graph':
        assert value['workspace_graph_digest']==public_graph,'foreign workspace graph binding'
    if role=='review':
        for child in ('context','impact'):
            validate_bindings(value[child],child,public_graph)

def restore(value,prior,role,actual_graph,prior_graph):
    # Validate all binding relationships BEFORE resetting any allowed field.
    # Projection graph has its own independently checked graph digest/bill;
    # every analysis/review child binds the authenticated PUBLIC graph digest.
    validate_bindings(value,role,actual_graph)
    validate_bindings(prior,role,prior_graph)
    key='graph_digest' if role=='graph' else 'artifact_digest'
    value=copy.deepcopy(value)
    if role=='graph':
        value['budget']['used_builder_bytes']=prior['budget']['used_builder_bytes']
    else:
        value['workspace_graph_digest']=prior['workspace_graph_digest']
        value['budget']['workspace']['used_builder_bytes']=prior['budget']['workspace']['used_builder_bytes']
        if role=='review':
            for child in ('context','impact'):
                value[child]=restore(value[child],prior[child],child,actual_graph,prior_graph)
    value[key]=prior[key]
    assert value==prior,'nonbilling semantic difference'
    assert domain_digest(value,role)==value[key]
    return value

def document(path):
    raw=path.read_bytes();value=json.loads(raw,object_pairs_hook=unique)
    assert enc(value)==raw,'noncanonical complete wire'
    return raw,value

def validate_whole_capture(capture,old):
    names=sorted(p.name for p in (old/'wires').glob('*.json'));assert len(names)==10
    assert sorted(p.name for p in (capture/'wires').glob('*.json'))==names
    _,actual_public=document(capture/'wires/public-graph.json')
    _,prior_public=document(old/'wires/public-graph.json')
    actual_graph=actual_public['graph_digest'];prior_graph=prior_public['graph_digest']
    validate_bindings(actual_public,'graph',actual_graph)
    validate_bindings(prior_public,'graph',prior_graph)
    rows=[]
    for name in names:
        before,b=document(old/'wires'/name);actual,a=document(capture/'wires'/name)
        role='graph' if name in ('public-graph.json','projection-wire.json') else 'review' if name.endswith('review.json') else 'impact' if name.endswith('impact.json') else 'context'
        assert enc(restore(a,b,role,actual_graph,prior_graph))==before
        rows.append({'name':name,'prior_sha256':sha(before),'actual_sha256':sha(actual),'complete_prior_restored':True})
    return rows

# Recheck the complete descriptor-controlled inputs immediately before each
# new helper/baseline import. The record still needs a parent-reviewed external
# digest at invocation; its own hash map is not an independent trust anchor.
PREFLIGHT="""
record_now=(HERE/'skeleton-record.json').read_bytes()
assert hashlib.sha256(record_now).hexdigest()==EXPECTED_WORKFLOW_RECORD_SHA256,'workflow record drift before committed admission'
assert RECORD==json.loads(record_now),'cached workflow record differs'
for tool,expected in RECORD['trusted_tool_sha256'].items():
 assert hashlib.sha256((HERE/tool).read_bytes()).hexdigest()==expected,'trusted tool drift before committed baseline admission'
for source,expected in RECORD['old_source_sha256'].items():
 raw=(ROOT/source).read_bytes()
 assert hashlib.sha256(raw).hexdigest()==expected and raw==(HERE/'frozen-source-before'/source).read_bytes(),'source drift before committed baseline admission'
"""
def adapt_committed67b(tools,proof,root):
    capture,hashes=baseline(root);tools=dict(tools);proof=copy.deepcopy(proof)
    name='strict-828f-capture-only.py';old=tools[name];s=old
    assignment=next(n for n in ast.parse(s).body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='ANALYSIS' for t in n.targets))
    pairs=ast.literal_eval(assignment.value)
    changes=[]
    for label,oldhash in pairs:
        assert s.count(oldhash)==1
        new=hashes[label+'.json'];s=s.replace(oldhash,new,1);changes.append((oldhash,new))
    for oldhash,newhash in [('6b5cc543f824f65ea9deaeb9d713176c8c3ba2d77261fe32ed866ebb69c1f6e6',hashes['public-graph.json']),('74dd9cc25b0f9f331aa831d6fac0e44f1d51488d6c3a548f3c60b6a86ff8eb04',hashes['projection-wire.json'])]:
        assert s.count(oldhash)==1;s=s.replace(oldhash,newhash,1);changes.append((oldhash,newhash))
    for a,b in [('1_209_652','1_114_864'),('1209652','1114864')]:
        assert s.count(a)==1;s=s.replace(a,b,1);changes.append((a,b))
    inverted=s
    for a,b in reversed(changes):assert inverted.count(b)==1;inverted=inverted.replace(b,a,1)
    assert inverted==old;ast.parse(s);tools[name]=s
    proof[name]['committed67b_literals']={'before_sha256':sha(old.encode()),'after_sha256':sha(s.encode()),'exact_inverse':changes,'all_original_guards_byteexact_after_literal_inverse':True}
    name='archive-actual-and-build-patch.py';old=tools[name]
    a="OLD=Path('/Volumes/Semaprax/provenance/actual35cc-pin-capture-20261015')"
    b='OLD=Path(json.loads((Path(__file__).resolve().parent/"skeleton-record.json").read_text())["old_actual_capture"])'
    archive_anchor="for origin,name in [(OLD,'previous-actual-35cc-capture'),(NEW,'actual-current-capture')]:"
    archive_extra="HERE=Path(__file__).resolve().parent\nrecord_raw=(HERE/'skeleton-record.json').read_bytes()\nassert len(sys.argv)==7 and hashlib.sha256(record_raw).hexdigest()==sys.argv[4],'parent-reviewed workflow record differs'\nEXPECTED_WORKFLOW_RECORD_SHA256=sys.argv[4]\nRECORD=json.loads(record_raw)\nassert ROOT.resolve()==Path(RECORD['repository']).resolve()\n"+PREFLIGHT+"from committed67b_generation import baseline,validate_whole_capture\nold_capture,old_hashes=baseline(ROOT)\nassert old_capture==OLD and old_hashes==RECORD['old_actual_all10_wire_sha256']\nfor held in [ROOT.resolve(),OLD.resolve(),NEW]:\n assert not OUT.is_relative_to(held) and not held.is_relative_to(OUT),'archive output overlaps ROOT or a capture'\nmeta=json.loads((OUT/'trusted-inputs.json').read_text())\nassert meta['build_receipt_sha256']==sys.argv[5] and meta['producer_receipt_sha256']==sys.argv[6]\nassert meta['capture_head']==RECORD['frozen_source_head'] and Path(meta['round']).resolve()==Path(RECORD['required_round']).resolve()\nstrict_command=['python3',str(HERE/'strict-828f-capture-only.py'),str(NEW),'--repo',str(ROOT),'--expected-capture-head',RECORD['frozen_source_head'],'--round',str(Path(RECORD['required_round'])),'--expected-build-receipt-sha256',sys.argv[5],'--expected-producer-receipt-sha256',sys.argv[6]]\nstrict=subprocess.run(strict_command,capture_output=True)\nassert strict.returncode==0,'strict five-build/capture admission refused before archive'\nassert json.loads(strict.stdout)==json.loads((OUT/'readonly-plan.json').read_text()),'archive plan differs from strict replay'\nvalidate_whole_capture(NEW,OLD)\n"
    archive_edits=[(a,b),(archive_anchor,archive_extra+archive_anchor),('previous-actual-35cc-capture','previous-actual-committed67b-capture')]
    s=old
    for a,b in archive_edits:
        assert s.count(a)==1,(name,a);s=s.replace(a,b,1)
    inverted=s
    for a,b in reversed(archive_edits):
        assert inverted.count(b)==1;inverted=inverted.replace(b,a,1)
    assert inverted==old;ast.parse(s);tools[name]=s
    proof[name]['committed67b_input']={'before_sha256':sha(old.encode()),'after_sha256':sha(s.encode()),'exact_inverse':archive_edits,'all_original_guards_byteexact_after_inverse':True}
    proof[name]['after_sha256']=sha(s.encode())
    name='prepare-after-actual-capture.py';old=tools[name]
    anchor="plan=json.loads(result.stdout);assert plan['capture_head']==HEAD and len(plan['edits'])==11"
    extra=PREFLIGHT+"\nfrom committed67b_generation import baseline,validate_whole_capture\nold_capture,old_hashes=baseline(ROOT)\nassert old_capture==Path(RECORD['old_actual_capture']) and old_hashes==RECORD['old_actual_all10_wire_sha256']\n(out/'committed67b-whole-wire-review.json').write_text(json.dumps(validate_whole_capture(capture,old_capture),indent=2)+'\\n')"
    parse="a=p.parse_args();capture=a.capture.resolve();round=a.round.resolve();out=a.out.resolve()"
    descriptor="\nrecord_raw=(HERE/'skeleton-record.json').read_bytes()\nassert len(a.expected_workflow_record_sha256)==64 and all(c in '0123456789abcdef' for c in a.expected_workflow_record_sha256)\nassert hashlib.sha256(record_raw).hexdigest()==a.expected_workflow_record_sha256,'parent-reviewed workflow record differs'\nassert RECORD==json.loads(record_raw),'workflow record changed during argument parsing'\nEXPECTED_WORKFLOW_RECORD_SHA256=a.expected_workflow_record_sha256"
    invoke="subprocess.run(['python3',str(HERE/'archive-actual-and-build-patch.py'),str(out),str(capture),'39130d9237121b61e40b47f95c072c09a456554e'],check=True)"
    command_anchor="command=['python3',str(HERE/'strict-828f-capture-only.py')"
    prepare_edits=[(command_anchor,PREFLIGHT+command_anchor),("p.add_argument('--capture',required=True,type=Path)","p.add_argument('--capture',required=True,type=Path)\np.add_argument('--expected-workflow-record-sha256',required=True)"),(parse,parse+descriptor),(anchor,anchor+extra),(invoke,invoke.replace("],check=True)",",a.expected_workflow_record_sha256,a.expected_build_receipt_sha256,a.expected_producer_receipt_sha256],check=True)"))]
    s=old
    for a,b in prepare_edits:
        assert s.count(a)==1,(name,a);s=s.replace(a,b,1)
    inverted=s
    for a,b in reversed(prepare_edits):
        assert inverted.count(b)==1;inverted=inverted.replace(b,a,1)
    assert inverted==old;ast.parse(s);tools[name]=s
    proof[name]['additional_whole_wire_guard']={'before_sha256':sha(old.encode()),'after_sha256':sha(s.encode()),'exact_inverse':prepare_edits,'all_original_guards_byteexact_after_inverse':True}
    proof[name]['after_sha256']=sha(s.encode())
    proof['strict-828f-capture-only.py']['after_sha256']=sha(tools['strict-828f-capture-only.py'].encode())
    return tools,proof,{'old_actual_capture':str(capture),'old_actual_all10_wire_sha256':hashes}
