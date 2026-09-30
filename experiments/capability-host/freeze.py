import json,hashlib,pathlib
f=json.loads(pathlib.Path("experiments/capability-host/freeze.json").read_text())
for p,h in f.items(): assert hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==h,p
print(f"{len(f)} frozen files unchanged")
