#!/usr/bin/env python3
"""Run an explicit argv command repeatedly, retaining unedited logs and timings.

No shell evaluation. Command is provided after --. Cleanup, if requested, is
restricted to one derived subdirectory under repository build/wasm-exp1.
"""
import argparse,json,os,resource,shutil,subprocess,time
from pathlib import Path
from datetime import datetime,timezone
ROOT=Path(__file__).resolve().parents[3]
p=argparse.ArgumentParser();p.add_argument('--label',required=True);p.add_argument('--count',type=int,default=5);p.add_argument('--output',type=Path,required=True);p.add_argument('--clean',type=Path);p.add_argument('command',nargs=argparse.REMAINDER);a=p.parse_args()
command=a.command[1:] if a.command[:1]==['--'] else a.command
if not command or not 1<=a.count<=20: raise SystemExit('Provide command and count1..20')
if a.clean:
 clean=a.clean.resolve();allowed=(ROOT/'build/wasm-exp1').resolve()
 if not clean.is_relative_to(allowed) or clean==allowed: raise SystemExit('Cleanup restricted to derived build/wasm-exp1 subdirectory')
a.output.mkdir(parents=True,exist_ok=True);runs=[]
for n in range(a.count):
 if a.clean:shutil.rmtree(clean,ignore_errors=True)
 start=time.perf_counter();before=resource.getrusage(resource.RUSAGE_CHILDREN)
 with (a.output/f'{a.label}-{n+1}.stdout').open('wb') as out,(a.output/f'{a.label}-{n+1}.stderr').open('wb') as err:
  r=subprocess.run(command,cwd=ROOT,stdout=out,stderr=err,env={**os.environ,'BUN_CONFIG_NO_CLEAR_TERMINAL':'1'},timeout=180)
 after=resource.getrusage(resource.RUSAGE_CHILDREN)
 runs.append({'run':n+1,'wallSeconds':time.perf_counter()-start,'childUserSeconds':after.ru_utime-before.ru_utime,'childSystemSeconds':after.ru_stime-before.ru_stime,'exitCode':r.returncode})
 if r.returncode:break
report={'label':a.label,'recordedAt':datetime.now(timezone.utc).isoformat(),'command':command,'cleanup':str(a.clean) if a.clean else None,'cacheScope':'Only the named derived build directory is removed. Download cache and shared frontend compiler remain installed.','runs':runs}
(a.output/f'{a.label}.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if any(r['exitCode'] for r in runs):raise SystemExit(1)
