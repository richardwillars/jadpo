"""Pin the conforming artifacts and historical controls before measurement."""
import pathlib,hashlib,json,subprocess,sys
base=pathlib.Path('experiments/auth-policy-measure');dest=base/'freeze.json'
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
if dest.exists():
 hashes=json.loads(dest.read_text())
 for p,h in hashes.items():assert sha(p)==h,p
 print(f'{len(hashes)} frozen files unchanged')
else:
 hashes=json.loads(pathlib.Path('experiments/auth-policy/evidence/freeze.json').read_text())
 paths=subprocess.check_output(['git','ls-files','experiments/auth-policy','docs/auth-policy-conformance-results.md'],text=True).splitlines()
 paths += [str(p) for p in pathlib.Path('experiments/auth-policy/build').rglob('*') if p.is_file()]
 paths += ['build/auth-policy/target/release/jadpo-auth-policy-native']
 for p in paths:hashes[p]=sha(p)
 dest.write_text(json.dumps(hashes,indent=2)+'\n')
 print(f'Pinned {len(hashes)} files')
