from pathlib import Path
import subprocess,json,hashlib
root=Path('probes/diagnostics');root.mkdir(parents=True,exist_ok=True)
base='enum HealthStatus { healthy }\noutput PublicHealth { status: HealthStatus }\nroute GET /health/live { auth: none output: PublicHealth action: { return PublicHealth { status: HealthStatus.healthy } } }\n'
variants={
'protected':base.replace('auth: none',''),
'call':'function get_status() -> HealthStatus { return HealthStatus.healthy }\n'+base.replace('status: HealthStatus.healthy','status: get_status()'),
'extra-statement':base.replace('return PublicHealth','var ignored = HealthStatus.healthy\nreturn PublicHealth'),
'clock':base.replace('output PublicHealth { status: HealthStatus }','output PublicHealth { status: HealthStatus at: Instant }').replace('status: HealthStatus.healthy }','status: HealthStatus.healthy at: clock.now }'),
'config':'config Settings { label: Text { binding: "HEALTH_LABEL" } }\noutput PublicHealth { status: Text }\nroute GET /health/live { auth: none output: PublicHealth action: { return PublicHealth { status: config.label } } }',
'query':'input Q { label: Text }\n'+base.replace('auth: none','auth: none query: Q'),
'header':base.replace('auth: none','auth: none headers: { label: Text from "X-Health" optional }'),
'run':'action live() -> PublicHealth { return PublicHealth { status: HealthStatus.healthy } }\n'+base.replace('action: { return PublicHealth { status: HealthStatus.healthy } }','run: live()'),
'secret':'type Key = Text { min_length: 8 }\nconfig Settings { key: Key { binding: "HEALTH_SECRET" secret: true } }\noutput PublicHealth { status: Text }\nroute GET /health/live { auth: none output: PublicHealth action: { return PublicHealth { status: config.key } } }',
}
results=[]
for name,source in variants.items():
 p=root/name;p.mkdir(exist_ok=True);(p/'app.jadpo').write_text(source)
 r=subprocess.run(['cargo-target/debug/jadpo','build',str(p)],capture_output=True,text=True)
 output=r.stdout+r.stderr;(Path('evidence')/(name+'-diagnostic.log')).write_text(output)
 assert r.returncode !=0,(name,output)
 if name!='secret':
  assert 'Authored liveness route is not dependency-free' in output,(name,output)
  assert 'owner: human' in output,(name,output)
  assert 'app.jadpo:' in output,(name,output)
 else:
  assert 'secret' in output.lower(),(name,output)
 results.append({'name':name,'exitCode':r.returncode,'humanOwnedLivenessDiagnostic':name!='secret','sourceSha256':hashlib.sha256(source.encode()).hexdigest(),'secretBlockedBeforeTarget':name=='secret'})
# A repair to a constant stays public and implements the authored output.
p=root/'repaired';p.mkdir(exist_ok=True);(p/'app.jadpo').write_text(base)
r=subprocess.run(['cargo-target/debug/jadpo','build',str(p)],capture_output=True,text=True)
assert r.returncode==0,r.stderr
catalog=json.loads((p/'build/diagnostics/catalogue.json').read_text())
entries=[x for x in catalog['diagnostics'] if 'JADPO_TARGET_LIVENESS_NOT_LOCAL' in x.get('legacyAliases',[])]
assert len(entries)==1,entries
assert entries[0]['decisionOwner']=='human'
assert entries[0]['copyStatus']=='authored'
meta=json.loads(Path('probes/golden/build/app.meta.json').read_text())
route=json.loads(Path('probes/golden/build/inventory/routes.json').read_text())
health=[x for x in route['routes'] if x['route']=='GET /health/live']
assert len(health)==1
assert health[0]['auth']=='none' and health[0]['output']=='PublicHealth' and health[0]['success']['http_status']==200 and health[0]['behavior']=='inline_action'
openapi=json.loads(Path('probes/golden/build/openapi/openapi.json').read_text())
assert openapi['paths']['/health/live']['get']['responses']['200']['content']['application/json']['schema']['$ref']=='#/components/schemas/PublicHealth'
assert openapi['components']['schemas']['HealthStatus']['enum']==['healthy']
app=Path('probes/golden/build/target/app.ts').read_text()
needle='if (request.method === "GET" && url.pathname === "/health/live") return json(200, validate_PublicHealth({ status: "healthy" }, "response.body"), requestId);'
assert app.count(needle)==1
assert app.index(needle)<app.index('if (!databaseIsReady())')<app.index('await firstPartyAuthentication.authenticate(')
assert 'matchRoutePath("/health/live"' not in app
source_rev=meta.get('sourceRevision',meta.get('source_revision'))
assert 'const sourceRevision = '+json.dumps(source_rev)+';' in app
print('meta keys',list(meta))
print('source revision',source_rev)
result={'negativeCases':results,'repairBuildPass':True,'catalogueEntry':entries[0],'inventoryHealth':health[0],'openapiHealth':openapi['paths']['/health/live']['get'],'generatedEarlyRoute':needle,'sourceRevision':source_rev,'placementBeforeReadinessAndAuth':True}
Path('evidence/compiler-boundary-observations.json').write_text(json.dumps(result,indent=2)+'\n')
print('Compiler negatives passed:',len(results))
