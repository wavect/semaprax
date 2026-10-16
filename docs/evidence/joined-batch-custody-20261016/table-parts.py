"""Closed four-domain whole-row parts, through the unchanged bounded reader."""
import hashlib,json
from pathlib import Path
CAP=5_081_115
NAMES={"Previous16c","Maina3","Alias1ae","MainE47"}
def encoded(rows):return (json.dumps(rows,indent=2)+"\n").encode()
def table_name(domain,index):
    assert domain["name"] in NAMES
    primary="current-to-"+domain["name"]+".json"
    assert domain["table"]==primary
    return primary if index==0 else primary[:-5]+"-part-"+str(index).zfill(3)+".json"
def split(rows):
    assert rows and [r["source"] for r in rows]==sorted(set(r["source"] for r in rows))
    result=[];part=[];length=3
    for row in rows:
        # A row is never divided or transformed. Check its actual final JSON
        # encoding before any output is written; one oversized row fails closed.
        single=encoded([row]);assert len(single)<=CAP,"one complete row exceeds sealed cap"
        extra=len(single)-5+(2 if part else 0)
        if part and length+extra>CAP:result.append(part);part=[];length=3
        part.append(row);length=len(encoded(part))
    if part:result.append(part)
    assert [r for part in result for r in part]==rows
    assert all(len(encoded(part))<=CAP for part in result)
    return result
def write_parts(root,domain,rows):
    assert [r["source"] for r in rows]==domain["keys"]
    plan=split(rows);parts=[]
    # Compute all bounded payloads and descriptors before acquiring any output.
    payloads=[encoded(part) for part in plan]
    for index,(part,payload) in enumerate(zip(plan,payloads)):
        parts.append(dict(table=table_name(domain,index),first=part[0]["source"],last=part[-1]["source"],rows=len(part),length=len(payload),sha256=hashlib.sha256(payload).hexdigest()))
    for part,payload in zip(parts,payloads):(root/part["table"]).write_bytes(payload)
    domain["parts"]=parts
    return parts
def load_rows(root,domain):
    assert domain["table"]==table_name(domain,0)
    if "parts" not in domain:
        payload=(root/domain["table"]).read_bytes();assert len(payload)<=CAP
        rows=json.loads(payload)
    else:
        assert domain["parts"]
        rows=[]
        for index,part in enumerate(domain["parts"]):
            assert part["table"]==table_name(domain,index),"part index/path gap or reorder"
            payload=(root/part["table"]).read_bytes()
            assert len(payload)==part["length"]<=CAP
            assert hashlib.sha256(payload).hexdigest()==part["sha256"]
            group=json.loads(payload);assert len(group)==part["rows"] and group
            assert group[0]["source"]==part["first"] and group[-1]["source"]==part["last"]
            rows.extend(group)
    assert [r["source"] for r in rows]==domain["keys"]==sorted(set(domain["keys"])),"complete ordered unique domain"
    return rows
def specs(domain):
    parts=domain["parts"];arms=[]
    for index in range(len(parts)-1,-1,-1):
        part=parts[index];guard=' if _source >= '+json.dumps(part["first"]) if index else ''
        path='/docs/evidence/a3-e47-final-custody-20261016/'+part["table"]
        arms.append('Self::'+domain["name"]+guard+' => Spec {path: concat!(env!("CARGO_MANIFEST_DIR"), '+json.dumps(path)+'), length: '+str(part["length"])+', sha256: '+str(list(bytes.fromhex(part["sha256"])))+'},\n')
    return ''.join(arms)
