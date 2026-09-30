import hashlib,json,pathlib,sys
base=pathlib.Path('experiments/capability-monitor')
expected={
 'experiments/capability-monitor/Cargo.lock':'0feafdec67f8ce636129107fea36d247db221bca0031d96d425e7a3231ee9548',
 'experiments/capability-monitor/application.wasm':'4c8370d0145d4c6e3d6da21d413ee140118dd33e4317fa6182c2a8d2b1ee9eb9',
 'experiments/capability-monitor/monitor.wasm':'f8a7361d1bde28b6e1d387ae76ab726b321bfd98e6d8a532afd3bf10827feae4',
}
if '--check' in sys.argv:
 for path,digest in expected.items(): assert hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()==digest,path
 print(f'{len(expected)} monitor artifacts unchanged')
else:
 print(json.dumps(expected,indent=2))
