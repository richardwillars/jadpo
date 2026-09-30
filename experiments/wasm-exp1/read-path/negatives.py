from pathlib import Path
import json,subprocess,gzip
root=Path(__file__).resolve().parents[3]
old=json.loads(gzip.decompress((root/'experiments/wasm-exp1/optimization/evidence/negatives.json.gz').read_bytes()))
results=[]
for case in old:
    if case['case']=='unapproved-host-import':
        command=['bun','--no-install','--env-file=/dev/null','experiments/wasm-exp1/read-path/run-cached.ts','experiments/wasm-exp1/rust-full/build/negative/unapproved-import.wasm']
    else: command=[a.replace('/optimization/','/read-path/') for a in case['command']]
    process=subprocess.run(command,cwd=root,capture_output=True,text=True)
    diagnostic=case['diagnostic'].strip().splitlines()[0]
    assert process.returncode!=0 and diagnostic in process.stderr+process.stdout,(case['case'],process.stdout,process.stderr)
    artifact=None
    if '--output' in command:
        artifact=Path(command[command.index('--output')+1])/'generated.rs';assert not (root/artifact).exists()
    results.append({'case':case['case'],'pass':True,'command':command,'exitCode':process.returncode,'stdout':process.stdout,'stderr':process.stderr,'artifactWritten':False if artifact else None})
(root/'build/wasm-exp1/read-path/negatives.json').write_text(json.dumps(results,indent=2)+'\n')
print('Five rejection gates pass.')
