"""Pin only the new candidate; historical controls have a separate immutable guard."""
import pathlib,json,hashlib,sys
base=pathlib.Path('experiments/capability-host');dest=base/'candidate.json'
if '--check' in sys.argv:
 hashes=json.loads(dest.read_text())
 for p,h in hashes.items():assert hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==h,p
 print(f'{len(hashes)} candidate files unchanged')
else:
 assert not dest.exists(),'Candidate already pinned; investigate changes rather than silently repinning'
 paths=[]
 for part in ['core','native','wasm','guest','attack','fixture','build/projected/bun','build']:
  paths += [p for p in (base/part).rglob('*') if p.is_file()]
 paths += [base/p for p in ['Cargo.toml','Cargo.lock','build.sh','prepare.py','generate.py','authority-host.ts','wasm-driver.ts','sqlite-host.ts','bun-server.ts','seed.ts','measure.mjs','run.mjs','pin.py']]
 paths += [pathlib.Path('build/capability-host/target/release/jadpo-capability-host-native')]
 dest.write_text(json.dumps({str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))},indent=2)+'\n')
 print('Pinned candidate')
