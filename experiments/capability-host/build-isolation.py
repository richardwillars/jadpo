"""A native-only policy rebuild must leave the old WASM lock untouched."""
import pathlib,subprocess,json,hashlib,os,shutil,tempfile
base=pathlib.Path('experiments/capability-host');out=pathlib.Path('build/capability-host/build-isolation');out.mkdir(exist_ok=True)
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def run(cmd,name):
 p=subprocess.run(cmd,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
 (out/(name+'.log')).write_text(p.stdout);assert p.returncode==0,p.stdout
before={name:sha(base/'build'/name) for name in ['authority.wasm','application.wasm','trust.json']}
source=out/'source';source.mkdir(exist_ok=True)
(source/'app.jadpo').write_text((base/'fixture/app.jadpo').read_text().replace('policy { NoteRole.owner: [read, update] }','policy { NoteRole.owner: [read, update] NoteRole.editor: [update] }'))
try:
 run(['sh',str(base/'build.sh'),'native',str(source)],'native-only')
 assert {name:sha(base/'build'/name) for name in before}==before
 lock=json.loads((base/'build/trust.json').read_text());assert sha(base/'build/contract.json')!=lock['contract']
 seed=json.loads(pathlib.Path('build/capability-host/seed.json').read_text())
 directory=pathlib.Path(tempfile.mkdtemp(dir=out));shutil.copyfile('build/capability-host/seed.sqlite',directory/'state.sqlite')
 p=subprocess.run(['bun','--no-install','--env-file=/dev/null',str(base/'bun-server.ts')],env={**os.environ,**seed['configuration'],'EXPERIMENT_TARGET':'wasm','SQLITE_PATH':str(directory/'state.sqlite'),'DATABASE_URL':'','TEST_CONTROL_TOKEN':'LOCAL_SYNTHETIC_CONTROL','PORT':'0'},text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=10)
 (out/'rejected-start.log').write_text(p.stdout)
 assert p.returncode!=0 and 'unapproved contract' in p.stdout,p.stdout
 proof={'nativeOnlyPreservedWasmAndLock':True,'mixedBundleRejectedBeforeListen':True,'oldLock':lock}
finally:
 run(['sh',str(base/'build.sh')],'restore')
 assert {name:sha(base/'build'/name) for name in before}==before
pin_path=base/'candidate.json';pin=json.loads(pin_path.read_text())
changed=[p for p,h in pin.items() if sha(p)!=h]
assert changed in ([],[str(base/'build.sh')]),changed
# Preserve the pre-fix manifest/recipe separately. Runtime artifacts and source
# used by HTTP measurements are all unchanged; only this build recipe changed.
pin[str(base/'build.sh')]=sha(base/'build.sh');pin_path.write_text(json.dumps(pin,indent=2)+'\n')
proof['onlyChangedPinnedFile']=changed[0] if changed else None;proof['measuredRuntimeArtifactsUnchanged']=True
historic=json.loads((base/'candidate-before-build-fix.json').read_text())
proof['preMeasurementManifestDiffs']=[p for p,h in historic.items() if sha(p)!=h]
assert proof['preMeasurementManifestDiffs']==[str(base/'build.sh')]
assert sha(base/'build-before-fix.sh.txt')==historic[str(base/'build.sh')]
(out/'results.json').write_text(json.dumps(proof,indent=2)+'\n')
print('Native-only rebuild preserves WASM lock; mixed bundle rejected; measured runtime artifacts restored unchanged')
