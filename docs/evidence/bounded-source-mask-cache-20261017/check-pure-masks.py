"""Small complete-value checks only. Never executes an entry/generator traversal."""
from pathlib import Path
import ast,json,re,hashlib
from memoizer import MaskMemoizer
HERE=Path(__file__).resolve().parent
text=(HERE/"originals/docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py").read_text()
node=next(n for n in ast.parse(text).body if isinstance(n,ast.FunctionDef) and n.name=="mask")
ns={"re":re};exec(compile(ast.Module(body=[node],type_ignores=[]),"<complete-original-mask-definition>","exec"),ns)
original=ns["mask"];cache=MaskMemoizer(original,8192)
corpus=["","fn run() { true }","// comment\nfn x() {}","/* outer /* inner */ end */fn x(){}",'"// string /* not comment */"\nfn x(){}',"let a='x'; let b='\\n'; let c='\\\''; 'lifetime value",'b"bytes\\\"/*" c"cstring"',"// CRLF\r\nfn x(){\"a\\r\\n\"}\r\n","/* unterminated",'"unterminated\\',"/**/ // tail",'let 文="λ/*//";\n']
for n in range(18):
    h="#"*n;corpus.extend([f'r{h}"raw // /* \" text\n"{h} suffix',f'br{h}"raw bytes\r\n"{h} //x',f'cr{h}"raw c"{h}',f'r{h}"unterminated'])
corpus += ['/* multiline\n content\n*/\n"literal\nleading whitespace\n"\n',"'a' 'x 'static &value b'\\x00'","/*/ /***** */ //\n"]
compared=0
for repeat in range(3):
    for value in corpus:
        for sample in [value,value+" ",value[:-1],("!"+value[1:]) if value else "x"]:
            assert cache(sample)==original(sample)
            assert cache.retained_bytes()<=cache.cap;compared+=1
first='fn a(){"// untouched";}';second='fn b(){"// untouched";}'
cache(first);hits=cache.hits;assert cache(first)==original(first) and cache.hits==hits+1
misses=cache.misses;assert cache(second)==original(second) and cache.misses==misses+1
for i in range(128):
    sample="fn item_"+str(i)+"(){/*body*/}";assert cache(sample)==original(sample)
large="/*"+"x"*20000+"*/\nfn large(){}";assert cache(large)==original(large)
assert cache.bypasses and cache.evictions and cache.retained_bytes()<=8192
class ForeignString(str):
    def __eq__(self,other):raise AssertionError("subclasses must bypass cache key equality")
foreign=ForeignString("fn foreign(){}")
assert cache(foreign)==original(foreign)
print(json.dumps(dict(status="PURE_MASK_VALUE_PARITY_PASS_FULL_MANIFEST_PENDING",complete_mask_comparisons=compared,corpus_cases=len(corpus),cache=cache.statistics(),entry_or_generator_executed=False,full_manifest_output_parity="PENDING actual slow frozen HEAD",artifact_hashes={f:hashlib.sha256((HERE/f).read_bytes()).hexdigest() for f in ["memoizer.py","entry.py","bindings.json","check-pure-masks.py"]}),sort_keys=True))
