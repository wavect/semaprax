#!/usr/bin/env python3
"""Additive membership generator; never edits or executes original verifier main."""
import argparse,json
from pathlib import Path
from boundary_tools import generated_scope
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--full-dir',type=Path,required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();r=a.repo.resolve();assert r.is_relative_to('/Volumes/Semaprax');assert all(x.resolve().is_relative_to('/Volumes/Semaprax') for x in [a.full_dir,a.out]);a.out.mkdir(parents=True,exist_ok=True)
manifest,proof=generated_scope(r,a.full_dir.resolve());(a.out/'current-domains.json').write_text(json.dumps(manifest,indent=2)+'\n');(a.out/'boundary-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
