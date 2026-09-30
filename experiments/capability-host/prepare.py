#!/usr/bin/env python3
"""Fail-closed binding of checked auth, routes and direct field/row policy facts."""
import json,pathlib,subprocess,sys
base=pathlib.Path(__file__).resolve().parent
projection=pathlib.Path(sys.argv[1] if len(sys.argv)>1 else base/'build/projected/program.json')
p=json.loads(projection.read_text());decl={d['name']:d for d in p['declarations']}
def require(ok, reason):
    if not ok: raise ValueError('AUTH_POLICY_UNSUPPORTED: '+reason)
def one(xs,reason):
    require(len(xs)==1,reason);return xs[0]
def ident(name):
    require(name and name[0].isalpha() and all(c.isascii() and (c.isalnum() or c=='_') for c in name),'SQL identifier')
    return '"'+name+'"'
def metadata(kind): return [d for d in p['boundaryDeclarations'] if d['kind']==kind]
application=one(metadata('application'),'one application');require(application['revocation']=='Immediate','immediate revocation only')
principal=one(metadata('principal'),'one principal');require(principal['name']==application['principal']['name'],'principal identity')
for v in principal['variants']:
    expected={'subject':'Text', 'user_id' if v['kind']=='user' else 'service_id':'Uuid'}
    require({f['name']:f['type']['name'] for f in v['fields']}==expected and all(not f['optional'] and not f['type']['nullable'] and not f['constraints'] for f in v['fields']),'closed identity principal')
a=one(metadata('authentication'),'one authentication strategy')
require(a['transport']=={'bearer':'authorization_header'} and a['claims']==0,'opaque bearer only')
v=one(a['validators'],'one validator');require(v['mode']=='opaque' and v['principal']=='user','opaque user only')
settings=v['settings'];require(set(settings)=={'secret','previous_secret','audience'},'authentication settings')
require(settings['audience']['op']=='literal' and isinstance(settings['audience']['value'],str) and 0<len(settings['audience']['value'].encode('utf-16-le'))//2<=256,'bounded literal audience')
config=one(metadata('config'),'one configuration');bindings={}
for field in config['fields']:
    require(field['type']['name']=='Text' and not field['type']['nullable'] and field['secret'] and field['default'] is None and isinstance(field['binding'],str),'secret configuration')
    bindings[field['name']]=field['binding']
def binding(setting):
    e=settings[setting];require(e['op']=='load' and len(e['path'])==2 and e['path'][0]=='config','configuration binding')
    return bindings[e['path'][1]]
require(set(bindings)=={settings['secret']['path'][-1],settings['previous_secret']['path'][-1]},'only consumed secret configuration fields')
r=one(a['resolutions'],'one resolution');require(r['principal']=='user','user resolution')
entity,subject=r['authority'].split('.');authority=decl[entity]
active=r['active'];require(active['op']=='binary' and active['operator']=='Equal' and active['left']['op']=='load' and len(active['left']['path'])==1 and active['right']['op']=='literal' and active['right']['value'] is True,'active equality to true')
active=active['left']['path'][0]
require(p['effectiveTypes'][entity+'.'+active]['base']=='Bool','active Boolean')
identity=next(e['identity'] for e in p['entities'] if e['name']==entity)
require({m['source']:m['target'] for m in r['mappings']}=={subject:principal['name']+'.user.subject',identity:principal['name']+'.user.user_id'},'identity mappings')
require('Unique' in next(f for f in authority['fields'] if f['name']==subject)['storage'],'unique authority')
require(set(f['name'] for f in authority['fields'])=={identity,subject,active},'bounded authority fields')
tables={e['entity']:e for e in p['audit']['persistence/entities.json']['entities']}
for e in p['entities']:
    require(e['persistent'] and e['authorityStore']=='primary','primary authority only')
    t=tables[e['name']];ident(t['table']);ident(e['identity'])
    for f in decl[e['name']]['fields']:
        ident(f['name']);typ=p['effectiveTypes'][e['name']+'.'+f['name']]
        require(typ['base'] in ['Text','Uuid','Bool'] and not f['optional'] and f['reference'] is None,'scalar stored columns')
        require(all(c['kind'] in ['MinLength','MaxLength'] for c in typ['constraints']),'scalar constraints')
        require(all(s in ['Identity','Unique'] for s in f['storage']),'storage modifiers')
def scopes(entity,subjects):
    result=[]
    for role in subjects:
        b=one([b for b in p['policy']['bindings'] if b['entity']==entity and b['role']==role],'direct role binding')
        require(b['scope']==entity and b['principalEntity']==r['authority'].split('.')[0],'direct user scope')
        field=next(f for f in decl[entity]['fields'] if f['name']==b['field']);require(field['immutable'],'immutable scope')
        result.append(b['field'])
    require(bool(result),'nonempty policy subjects');return result
for rule in p['policy']['entities']:
    require(rule['scopeField'] is None and rule['scope']==rule['entity'],'no indirect scope')
    for f in rule['fields']:
        require(all(e in ['read','update'] for x in f['rules'] for e in x['effects']),'field read/update only')
        for x in f['rules']:scopes(rule['entity'],[x['subject']])
        require(p['effectiveTypes'][rule['entity']+'.'+f['field']]['nullable'], 'guest field redaction requires nullable protected fields')
def nodes(v):
    if isinstance(v,dict):
        yield v
        for x in v.values():yield from nodes(x)
    elif isinstance(v,list):
        for x in v:yield from nodes(x)
plans=[]
for d in p['declarations']:
    for n in nodes(d.get('body')):
        if n.get('hostEffect') not in ['storage.read','storage.update']:continue
        e=n['entity'];effect=n['hostEffect'].split('.')[1]
        record=decl[e];table=tables[e];identity_field=next(x['identity'] for x in p['entities'] if x['name']==e)
        require(n['predicate']['field']==identity_field and n['predicate']['operator']=='equal','identity equality')
        op=one([o for o in p['policy']['operations'] if o['operation']==d['name']],'checked operation')
        obligation=one([o for o in op['obligations'] if o['entity']==e and o['effect']==effect],'single obligation')
        scope=scopes(e,obligation['subjects'])
        rule=one([x for x in p['policy']['entities'] if x['entity']==e],'entity policy')
        field_scopes={}
        for f in rule['fields']:
            field_scopes[f['field']]={eff:scopes(e,[x['subject'] for x in f['rules'] if eff in x['effects']]) for eff in ['read','update']}
        read_scopes=[scopes(e,f['subjects']) for f in op['fieldReads'] if f['entity']==e]
        allowed=[]
        if effect=='update':
            require(n['patch'] is None,'fixed set update only');allowed=[f['name'] for f in n['set']]
            require(all(not next(f for f in record['fields'] if f['name']==name)['immutable'] and name!=identity_field for name in allowed),'mutable fields')
        descriptor={'entity':e,'operation':d['name'],'semanticOperationId':d['semanticId'],'freshness':'authoritative','policy':{'operation':op,'bindings':p['policy']['bindings'],'entities':p['policy']['entities']},'predicate':{'field':identity_field,'operator':'equal'}}
        plans.append({'descriptor':descriptor,'effect':effect,'table':table['table'],'identity':identity_field,'fields':[f['name'] for f in record['fields']],'allowed':allowed,'scope':scope,'readScopes':read_scopes,'fieldScopes':field_scopes})
require(len({(x['descriptor']['semanticOperationId'],x['effect']) for x in plans})==len(plans),'one plan per effect/operation')
routes=[]
for route in metadata('route'):
    call=route['run'];require(not route['public'] and not route['inline'] and not route['pathFields'] and route['method']=='Post','authenticated fixed POST routes')
    require(call and call['op']=='call' and len(call['arguments'])==1 and call['arguments'][0]['op']=='load' and call['arguments'][0]['path']==['input'],'single input route')
    target=decl[call['target']];require(len(target['parameters'])==1 and target['parameters'][0]['type']['name']==route['input']['name'] and target['result']['name']==route['output']['name'],'route types')
    routes.append({'method':'POST','path':route['path'],'operation':call['targetId'],'input':route['input']['name'],'output':route['output']['name']})
require(bool(routes),'routes required')
# All gates above run before publishing generated output.
subprocess.run(['python3',str(base/'generate.py'),str(projection),'--output',str(base/'build')],check=True,stdout=subprocess.DEVNULL)
manifest=json.loads((base/'build/manifest.json').read_text())
for route in routes:route['atomic']=one([e for e in manifest['entries'] if e['semanticId']==route['operation']],'entry')['atomic']
settings={'strategy':a['name'],'audience':settings['audience']['value'],'secretBinding':binding('secret'),'previousBinding':binding('previous_secret'),'entity':entity,'table':tables[entity]['table'],'identity':identity,'subject':subject,'active':active,'inactive':r['inactive']}
result={'auth':settings,'routes':routes,'plans':plans,'failures':{d['name']:{'status':d['httpStatus'],'code':d['code'],'message':d['message']} for d in p['declarations'] if d['kind']=='failure'}}
(base/'build/contract.json').write_text(json.dumps(result,indent=2)+'\n')
print('Generated authenticated shared Rust and checked policy contract')
