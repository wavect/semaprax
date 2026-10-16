#!/usr/bin/env python3
"""Unchanged full role/source verification plus exact actual boundary predicates."""
import argparse,json
from pathlib import Path
from boundary_tools import generated_scope,negative_checks
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--full-dir',type=Path,required=True);p.add_argument('--scope-dir',type=Path,required=True);a=p.parse_args();r=a.repo.resolve();assert all(x.resolve().is_relative_to('/Volumes/Semaprax') for x in [a.repo,a.full_dir,a.scope_dir]);expected,proof=generated_scope(r,a.full_dir.resolve());actual=json.loads((a.scope_dir/'current-domains.json').read_text());assert actual==expected,'typed membership metadata differs from actual complete source registration';proof['negative_controls']=negative_checks(r,a.full_dir.resolve(),actual);proof['status']='SOURCE_ONLY_EXACT_BOUNDARIES_AND_COMPLETE_SOURCE_ROLE_CUSTODY_PASS';(a.scope_dir/'boundary-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
