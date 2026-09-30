#!/usr/bin/env python3
"""Record source/protocol/toolchain identity before route implementation."""
import hashlib, json, platform, subprocess
from datetime import datetime, timezone
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
EXP = Path(__file__).resolve().parent

def command(*args):
    p = subprocess.run(args, cwd=ROOT, text=True, capture_output=True)
    return {'command': list(args), 'exit': p.returncode, 'stdout': p.stdout.strip(), 'stderr': p.stderr.strip()}

def digest(path):
    return {'path':str(path.relative_to(ROOT)), 'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}

if __name__ == '__main__':
    output = EXP/'evidence'/'freeze.json'
    if output.exists():
        raise SystemExit('Refusing to replace frozen manifest; record an explicit protocol revision instead.')
    paths = [EXP/'fixture'/'app.jadpo', EXP/'acceptance.json', EXP/'abi.md',ROOT/'docs'/'wasm-experiment-plan.md']
    paths += [p for p in [EXP/'host-capabilities.json', EXP/'tooling'/'package.json', EXP/'tooling'/'package-lock.json'] if p.is_file()]
    paths += [ROOT/'docs'/name for name in ['semantic-model.md','type-system.md','failure-model.md','entity-query-model.md','policy-plan.md','time-testing-plan.md']]
    if any(not p.is_file() for p in paths):
        raise SystemExit('All fixture, acceptance, ABI and contract inputs must exist before freeze.')
    compiler = ROOT/'jadpo'/'target'/'debug'/'jadpo'
    if compiler.is_file(): paths.append(compiler)
    tracked = subprocess.check_output(['git','ls-files','jadpo','runtime'],cwd=ROOT,text=True).splitlines()
    tree = hashlib.sha256()
    for name in sorted(tracked):
        p=ROOT/name
        if p.is_file(): tree.update(name.encode()+b'\0'+p.read_bytes()+b'\0')
    import secrets
    order = ['rust','direct'] if secrets.randbelow(2)==0 else ['direct','rust']
    doc = {'schemaVersion':1,'frozenAt':datetime.now(timezone.utc).isoformat(),'baselineCommit':'4dc5604',
        'head':command('git','rev-parse','HEAD'),'status':command('git','status','--short'),
        'sourceTreeSha256':tree.hexdigest(),'files':[digest(p) for p in paths],
        'probeOrder':order,'coinFlip':'secrets.randbelow(2), performed after reading all frozen inputs',
        'machine':{'platform':platform.platform(),'architecture':platform.machine(),'cpu':command('sysctl','-n','machdep.cpu.brand_string'),'memoryBytes':command('sysctl','-n','hw.memsize'),'power':command('pmset','-g','custom'),'backgroundWorkloads':'Codex coordinating task and bounded experiment agents; no claim of otherwise idle machine'},
        'tools':[command(*c) for c in [('rustc','-vV'),('cargo','--version'),('bun','--version'),('node','--version'),('rustup','target','list','--installed')]],
        'host':{'cloudflareCompatibilityDate':'2026-09-30','flags':[],'plan':'not verified; subscriptions read denied','workerName':'jadpo-wasm-exp1-20260930','deployment':'not yet attempted'},
        'toolingManifest':'tooling/package.json and lockfile; pin before compiling probes',
        'measurements':{'cleanBuilds':5,'incrementalBuilds':5,'freshStarts':20,'runs':5,'concurrency':[1,8],'warmupSeconds':10,'measurementSeconds':30,'serializationBytes':[256,4096,65536]},
        'thresholds':'docs/wasm-experiment-plan.md section8 (hashed above)'}
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(doc,indent=2)+'\n')
    print(json.dumps({'manifest':str(output),'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'probeOrder':order}))
