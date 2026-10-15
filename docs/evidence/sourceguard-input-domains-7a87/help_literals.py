"""Data-only decoding of exact Rust help literals and retained frozen pins."""
import re,json

def value(s,i):
 while s[i].isspace():i+=1
 if s[i]=='"':
  text,end=json.JSONDecoder().raw_decode(s[i:]);return text,i+end
 if s.startswith('concat!',i):
  i=s.index('(',i)+1;parts=[]
  while True:
   while s[i].isspace() or s[i]==',':i+=1
   if s[i]==')':return ''.join(parts),i+1
   v,i=value(s,i);parts.append(v)
 if s[i] in '[(':
  end=']' if s[i]=='[' else ')';i+=1;items=[]
  while True:
   while s[i].isspace() or s[i]==',':i+=1
   if s[i]==end:return items,i+1
   v,i=value(s,i);items.append(v)
 raise AssertionError(s[i:i+80])
def constants(code):
 return {m[1]:value(code,m.end())[0] for m in re.finditer(r'const ([A-Z_0-9]+):[^=]*=\s*',code)}
def catalog(code):
 block=code[code.index('static COMMANDS:'):code.index('fn available(')];out='SEMAPRAX — Meaning in. Verified machine code out.\n\nUsage:\n';n=0
 for m in re.finditer(r'CommandSpec\s*\{\s*id:.*?canonical:\s*("(?:\\.|[^"\\])*"),\s*aliases:.*?availability:\s*Availability::(Public|Private),\s*global:\s*(true|false),\s*usages:\s*&\[(.*?)\]\s*\}',block,re.S):
  name=json.loads(m[1]);usages=[json.loads(x) for x in re.findall(r'"(?:\\.|[^"\\])*"',m[4])]
  if m[2]=='Public' and m[3]=='true':out+=''.join((v.replace('|rust','') if name=='build' else v)+'\n' for v in usages);n+=1
 assert n==134;return out

def verify_frozen(code,current):
 code=code[code.index('fn help_keeps_frozen_package_resolve_usage_and_current_cli_snapshot'):code.index('fn fnv1a64')];c=constants(code);pins=[]
 def remove(x):
  nonlocal current
  assert current.count(x)==1,repr(x);current=current.replace(x,'',1)
 def remove_array(name):
  for x in c[name]:remove(x)
 def pin(text,length,digest):
  raw=text.encode();h=0xcbf29ce484222325
  for x in raw:h=((h^x)*0x100000001b3)&((1<<64)-1)
  assert len(raw)==length,(len(raw),length);assert h==digest,(hex(h),hex(digest));pins.append([length,hex(h)])
 assert len(c['RESTORED'])==47
 for a,b in c['RESTORED']:assert current.count(a)==1,repr(a);current=current.replace(a,b,1)
 remove(c['NEW_LINE']);remove(c['MCP_LINE']);remove_array('RECENT_READ_LINES');remove_array('RECENT_PROJECT_LINES')
 a,b=c['SESSION_PROTOCOL_FILTER_LINE'],c['LEGACY_FILTER_LINE'];assert current.count(a)==1;current=current.replace(a,b,1)
 for name in ['CACHE_LINES','ARCHIVE_LINES','DRAFT_ARCHIVE_LINES','RETENTION_LINES']:remove_array(name)
 for name in ['CXX_PACKAGE_LINE','AGENT_SKILL_LINE','ASSURANCE_POLICY_LINE','ASSURANCE_DIFF_LINE','ASSURANCE_MANIFEST_LINE','PROJECT_ASSURANCE_MANIFEST_LINE','RELEASE_VERIFY_LINE']:remove(c[name])
 for x in ['semaprax fetch --lock <lock.json> <cache-dir> <subject.json>...\n','semaprax doctor verify-release <release-dir> --trusted-root-sha256 <64-lowercase-hex>\n']:remove(x)
 for name in ['AUDIT_LINES','WORKFLOW_LINES','REGISTRY_LINES','CACHE_OPEN_LINES']:remove_array(name)
 remove(c['WEBAPP_LINE']);assert current.count(c['PROFILE_DOCTOR_LINE'])==0;assert current.count(c['NEW_PROJECT_LINE'])==0;assert current.count(c['BUILD_LINE'])==1
 current=current.replace(c['BUILD_LINE'],c['PROFILE_DOCTOR_LINE']+c['NEW_PROJECT_LINE']+c['BUILD_LINE'],1);pin(current,6628,0xf7af6dba98e3b6c5)
 assert current.count(c['PROFILE_DOCTOR_LINE'])==1 and current.count(c['LEGACY_DOCTOR_LINE'])==0;current=current.replace(c['PROFILE_DOCTOR_LINE'],c['LEGACY_DOCTOR_LINE'],1);pin(current,6611,0x9bf25887156ae19b)
 image=''.join(c['PROJECT_IMAGE_LINES']);withgit=image.replace(c['PROJECT_IMAGE_LINES'][7],c['PROJECT_IMAGE_LINES'][7]+c['GIT_PUBLISH_LINE'],1);withworkspace=withgit.replace(c['GIT_PUBLISH_LINE'],c['GIT_PUBLISH_LINE']+c['WORKSPACE_LINE'],1);assert current.count(withworkspace)==1
 remove(c['WORKSPACE_LINE']);pin(current,6556,0x40996832a0d0146f);assert current.count(withgit)==1;remove(c['GIT_PUBLISH_LINE']);pin(current,6444,0x94b0a750eec030eb);assert current.count(image)==1
 earlier=current
 for i in [1,2,11,12]:earlier=earlier.replace(c['PROJECT_IMAGE_LINES'][i],'',1)
 pin(earlier,6228,0x98c857a3cc945db2);earlier=earlier.replace(c['PROJECT_IMAGE_LINES'][10],'',1);pin(earlier,6186,0xa3b35e2f0b321ae1)
 for i in [6,7]:earlier=earlier.replace(c['PROJECT_IMAGE_LINES'][i],'',1)
 pin(earlier,6066,0xab0d98b4b1a8f9d8);earlier=earlier.replace(c['PROJECT_IMAGE_LINES'][9],'',1);pin(earlier,6029,0xd536df673cb8a1f6)
 for i in [5,8]:earlier=earlier.replace(c['PROJECT_IMAGE_LINES'][i],'',1)
 pin(earlier,5937,0xecf80ad53994259f)
 for x in c['PROJECT_IMAGE_LINES']:remove(x)
 assert current.count(c['STRINGS_LINE'])==1 and current.count(c['WEB_PROFILE'])==1;pin(current,5802,0x43481c7a053a9739);pin(current.replace(c['WEB_PROFILE'],'',1),5770,0x3f52e229c16a1830)
 return pins
