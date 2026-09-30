#!/usr/bin/env python3
"""Reuse the frozen lowering; derive a fail-closed native storage binding."""
import json,pathlib,subprocess,sys,hashlib
base=pathlib.Path(__file__).resolve().parent
projection=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else 'experiments/wasm-exp1/compiler/build/projected/program.json').resolve()
out=base/'build';out.mkdir(exist_ok=True)
subprocess.run(['python3',str(base/'compiler/generate.py'),str(projection),'--output',str(out)],check=True,stdout=subprocess.DEVNULL)
(out/'program.json').write_bytes(projection.read_bytes())
p=json.loads(projection.read_text());decl={d['name']:d for d in p['declarations']}
entities={e['name']:e for e in p['entities']}
persistent=[e for e in p['entities'] if e['persistent']]
assert len(persistent)==1,'bounded single entity'
e=persistent[0];record=decl[e['name']];fields=[f['name'] for f in record['fields']]
def ident(s):
 assert s and all(c.isascii() and (c.isalnum() or c=='_') for c in s) and not s[0].isdigit()
 return '"'+s+'"'
def nodes(v):
 if isinstance(v,dict):
  yield v
  for x in v.values():yield from nodes(x)
 elif isinstance(v,list):
  for x in v:yield from nodes(x)
columns=[]
for f in record['fields']:
 t=p['effectiveTypes'][e['name']+'.'+f['name']]
 assert t['base'] in ['Text','Uuid'] and not f['optional'] and f['reference'] is None
 assert all(c['kind'] in ['MinLength','MaxLength'] for c in t['constraints'])
 assert all(s in ['Identity','Unique'] for s in f['storage'])
 columns.append(ident(f['name'])+' TEXT'+('' if t['nullable'] else ' NOT NULL')+(' PRIMARY KEY' if f['name']==e['identity'] else '')+(' UNIQUE' if 'Unique' in f['storage'] else ''))
plans=[]
for d in p['declarations']:
 for n in nodes(d.get('body')):
  if n.get('hostEffect') not in ['storage.read','storage.update']:continue
  effect=n['hostEffect'].split('.')[1]
  assert n['entity']==e['name'] and n['predicate']['field']==e['identity'] and n['predicate']['operator']=='equal'
  op=next(o for o in p['policy']['operations'] if o['operation']==d['name'])
  obligations=[o for o in op['obligations'] if o['entity']==e['name'] and o['effect']==effect]
  assert len(obligations)==1 and obligations[0]['origin']=='entity' and len(obligations[0]['subjects'])==1
  role=obligations[0]['subjects'][0]
  binding=[b for b in p['policy']['bindings'] if b['entity']==e['name'] and b['role']==role]
  assert len(binding)==1
  binding=binding[0]
  rule=next(r for r in p['policy']['entities'] if r['entity']==e['name'])
  assert any(r['subject']==role and effect in r['effects'] for r in rule['rules'])
  assert any(f['name']==binding['field'] and f['immutable'] for f in record['fields'])
  descriptor={'entity':e['name'],'operation':d['name'],'semanticOperationId':d['semanticId'],'freshness':'authoritative','policy':{'operation':op,'bindings':p['policy']['bindings'],'entities':p['policy']['entities']},'predicate':{'field':e['identity'],'operator':'equal'}}
  allowed=[]
  if effect=='update':
   if n['patch'] is not None:
    assert len(n['patch'])==1
    param=next(x for x in d['parameters'] if x['name']==n['patch'][0]);allowed=[f['name'] for f in decl[param['type']['name']]['fields']]
   else:allowed=[f['name'] for f in n['set']]
   assert all(f in fields and f!=e['identity'] and not next(x for x in record['fields'] if x['name']==f)['immutable'] for f in allowed)
  plans.append({'descriptor':descriptor,'effect':effect,'allowed':allowed,'scope':binding['field'],'principal':binding['principalEntity']})
assert len({(x['descriptor']['semanticOperationId'],x['effect']) for x in plans})==len(plans)
table=ident('entity_'+e['name']);manifest=json.loads((out/'manifest.json').read_text())
config={'nullable':{f['name']:p['effectiveTypes'][e['name']+'.'+f['name']]['nullable'] for f in record['fields']},'entity':e['name'],'identity':e['identity'],'fields':fields,'table':table,'create':'CREATE TABLE IF NOT EXISTS '+table+' ('+', '.join(columns)+')','plans':plans,'entries':manifest['entries'],'projectionSha256':hashlib.sha256(projection.read_bytes()).hexdigest()}
(out/'storage.json').write_text(json.dumps(config,indent=2)+'\n')
print(json.dumps({'generatedRustSha256':manifest['generatedRustSha256'],'plans':len(plans),'entries':len(config['entries'])}))
