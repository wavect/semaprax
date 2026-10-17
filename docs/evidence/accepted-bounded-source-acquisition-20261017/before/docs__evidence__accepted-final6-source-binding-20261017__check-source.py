from pathlib import Path
import hashlib,sys
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
P=R/'docs/evidence/ordinary-plan-final-ingress-20261017/verify-source.py'
raw=P.read_bytes();assert hashlib.sha256(raw).hexdigest()=='8186f62cb05b263c051f1581db8fd2bacb6722506ef94588ace11749ef6e8635'
assert raw==(D/'before/verify-source.py').read_bytes()
old="template=b.read(R/recipe['helper_template'],digest=recipe['helper_template_sha256']).decode();assert b.scrub(helper)==b.scrub(template)"
new="template=b.read(R/recipe['helper_template'],digest=recipe['helper_template_sha256']).decode();template=b.custody_source_template(template);assert b.scrub(helper)==b.scrub(template)"
assert raw.decode().count(old)==1
current=raw.decode().replace(old,new,1)
assert current.replace(new,old,1).encode()==raw
helper=R/'docs/evidence/accepted-strict-guard-generation-20261017/guard_payload.py'
helper_raw=helper.read_bytes();assert hashlib.sha256(helper_raw).hexdigest()=='f060fb61cb0116f7996f7576686f50e9ae1d2918a60a1cfb87ac7ac60257b1e4'
ns={'__file__':str(helper)};exec(compile(helper_raw,str(helper),'exec'),ns)
old_guard='guard=runpy.run_path(str(guard_path))'
new_guard='guard=_accepted_guard_extensions(R)'
assert current.count(old_guard)==1
current=current.replace(old_guard,new_guard,1)
assert current.replace(new_guard,old_guard,1).replace(new,old,1).encode()==raw
exec(compile(current,str(P),'exec'),{'__file__':str(P),'__name__':'__main__','_accepted_guard_extensions':ns['accepted_guard_extensions']})
