"""Build timings after HTTP runs; keep warm dependency/filesystem caches explicit."""
import pathlib,subprocess,time,json,os,hashlib
out=pathlib.Path('build/auth-policy-measure/build-times');out.mkdir(parents=True,exist_ok=True)
results=[]
def timed(name,cmd,env=None):
 start=time.perf_counter();p=subprocess.run(cmd,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 result={'name':name,'command':cmd,'seconds':time.perf_counter()-start,'exitCode':p.returncode}
 (out/(name+'.log')).write_text(p.stdout);results.append(result)
 (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
 assert p.returncode==0,p.stdout
 print(json.dumps(result),flush=True)
subprocess.run(['python3','experiments/auth-policy-measure/freeze.py'],check=True)
wasm_env={**os.environ,'RUSTFLAGS':'-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144'}
for rep in range(3):
 for target in ['native','wasm']:
  timed(f'{target}-warm-command-{rep}',['sh','experiments/auth-policy/build.sh',target])
  subprocess.run(['python3','experiments/auth-policy-measure/freeze.py'],check=True)
  cargo=['cargo','build','--offline','--locked','--release','--manifest-path','experiments/auth-policy/Cargo.toml','--target-dir',f'build/auth-policy/{"target" if target=="native" else "wasm-target"}','-p',f'jadpo-auth-policy-{target}']
  if target=='wasm':cargo+=['--target','wasm32-unknown-unknown']
  timed(f'{target}-cargo-noop-{rep}',cargo,wasm_env if target=='wasm' else None)
 timed(f'bun-check-generate-{rep}',['build/auth-policy/compiler/debug/jadpo-wasm-projection','experiments/auth-policy/fixture',str(out/f'bun-{rep}'),'--bun'])
for target in ['native','wasm']:
 clean=out/f'clean-{target}-{os.getpid()}'
 cmd=['cargo','build','--offline','--locked','--release','--manifest-path','experiments/auth-policy/Cargo.toml','--target-dir',str(clean),'-p',f'jadpo-auth-policy-{target}']
 if target=='wasm':cmd+=['--target','wasm32-unknown-unknown']
 timed(f'{target}-clean-target',cmd,wasm_env if target=='wasm' else None)
 artifact=clean/('release/jadpo-auth-policy-native' if target=='native' else 'wasm32-unknown-unknown/release/jadpo_auth_policy_wasm.wasm')
 results[-1]['sha256']=hashlib.sha256(artifact.read_bytes()).hexdigest()
 results[-1]['bytes']=artifact.stat().st_size
(out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
subprocess.run(['python3','experiments/auth-policy-measure/freeze.py'],check=True)
