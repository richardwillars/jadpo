import hashlib,json,pathlib,sys
base=pathlib.Path('experiments/capability-monitor')
expected={
 'experiments/capability-monitor/Cargo.lock':'0feafdec67f8ce636129107fea36d247db221bca0031d96d425e7a3231ee9548',
 'experiments/capability-monitor/application.wasm':'359c8f3e0c9a0bcd13dff38c1a75ff80c4eb9a45741acdae7ce27e79b01cb395',
 'experiments/capability-monitor/monitor.wasm':'2a9d7712e878ab014d4f8a02c4cf35cdc4c372710e0678168b4f4dc07ac97fec',
 'experiments/capability-monitor/manifest.json':'f673710e9d7c42efb5b408ff4642a58923705cb1ac59ee18bd1f6daff7f86c8a',
}
if '--check' in sys.argv:
 for path,digest in expected.items(): assert hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()==digest,path
 print(f'{len(expected)} monitor artifacts unchanged')
else:
 print(json.dumps(expected,indent=2))
