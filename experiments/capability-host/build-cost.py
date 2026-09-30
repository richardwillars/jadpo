import pathlib,subprocess,time,json,hashlib,os
base=pathlib.Path('experiments/capability-host');out=pathlib.Path('build/capability-host/build-cost');out.mkdir(exist_ok=True);records=[]
def timed(name,command,env=None):
 start=time.perf_counter();p=subprocess.run(command,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
 (out/(name+'.log')).write_text(p.stdout);r={'name':name,'command':command,'seconds':time.perf_counter()-start,'exitCode':p.returncode};records.append(r)
 (out/'results.json').write_text(json.dumps(records,indent=2)+'\n');assert p.returncode==0,p.stdout
 print(json.dumps(r),flush=True)
for rep in range(3):
 for target in ['native','wasm']:
  timed(f'{target}-warm-{rep}',['sh',str(base/'build.sh'),target])
  subprocess.run(['python3',str(base/'pin.py'),'--check'],check=True)
for target in ['native','wasm']:
 directory=out/f'clean-{target}-{os.getpid()}'
 cmd=['cargo','build','--offline','--locked','--release','--manifest-path',str(base/'Cargo.toml'),'--target-dir',str(directory),'-p','jadpo-capability-host-'+target]
 env=None
 if target=='wasm':
  cmd+=['-p','jadpo-capability-host-guest','--target','wasm32-unknown-unknown']
  env={**os.environ,'RUSTFLAGS':'-C link-arg=--max-memory=8388608 -C link-arg=-zstack-size=262144'}
 timed(target+'-clean-target',cmd,env)
 paths=[directory/'release/jadpo-capability-host-native'] if target=='native' else [directory/f'wasm32-unknown-unknown/release/jadpo_capability_host_{p}.wasm' for p in ['wasm','guest']]
 records[-1]['artifacts']=[{'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in paths]
(out/'results.json').write_text(json.dumps(records,indent=2)+'\n')
subprocess.run(['python3',str(base/'pin.py'),'--check'],check=True)
subprocess.run(['python3',str(base/'freeze.py')],check=True)
