#!/usr/bin/env python3
"""Checked source changes must affect all targets; unsupported plans must publish nothing."""
import hashlib,json,pathlib,subprocess,copy
root=pathlib.Path.cwd();base=root/'experiments/auth-policy';out=root/'build/auth-policy/mutation';out.mkdir(parents=True,exist_ok=True)
def run(command,name):
    result=subprocess.run(command,cwd=root,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (out/(name+'.log')).write_text(result.stdout)
    if result.returncode: raise RuntimeError(name+': '+result.stdout[-6000:])
def hashes():return {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [base/'build/application.rs',base/'build/contract.json',base/'build/application.wasm']}
original=hashes();source=(base/'fixture/app.jadpo').read_text()
variant=out/'source';variant.mkdir(exist_ok=True)
(variant/'app.jadpo').write_text(source.replace('min_length: 3','min_length: 5').replace('policy { NoteRole.owner: [read, update] }','policy { NoteRole.owner: [read, update] NoteRole.editor: [update] }'))
try:
    run(['sh',str(base/'build.sh'),'all',str(variant)],'build-variant')
    changed=hashes();assert changed!=original
    run(['bun','--no-install','--env-file=/dev/null',str(base/'seed.ts')],'seed-variant')
    run(['node',str(base/'run.mjs'),'--smoke','--variant'],'test-variant')
    for name in ['application.rs','contract.json','application.wasm']:(out/('variant-'+name)).write_bytes((base/'build'/name).read_bytes())
finally:
    run(['sh',str(base/'build.sh')],'restore')
    run(['bun','--no-install','--env-file=/dev/null',str(base/'seed.ts')],'seed-original')
    assert hashes()==original,'original generated artifacts not restored'
p=json.loads((base/'build/projected/program.json').read_text());negative=[]
for name in ['signed','bounded','indirect_scope','unknown_role','non_authoritative','public_route']:
    q=copy.deepcopy(p)
    if name=='signed': next(d for d in q['boundaryDeclarations'] if d['kind']=='authentication')['validators'][0]['mode']='signed'
    elif name=='bounded':next(d for d in q['boundaryDeclarations'] if d['kind']=='application')['revocation']='Bounded'
    elif name=='indirect_scope':q['policy']['entities'][0]['scopeField']='organization_id'
    elif name=='unknown_role':q['policy']['operations'][0]['obligations'][0]['subjects']=['Unknown.role']
    elif name=='non_authoritative':q['queries'][0]['freshness']='eventual'
    else:next(d for d in q['boundaryDeclarations'] if d['kind']=='route')['public']=True
    path=out/(name+'.json');path.write_text(json.dumps(q))
    result=subprocess.run(['python3',str(base/'prepare.py'),str(path)],text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (out/(name+'.log')).write_text(result.stdout);assert result.returncode!=0,name
    assert hashes()==original,name+' published output despite rejection';negative.append(name)
# Real checker rejects widening a restricted field's output audience.
bad=out/'bad-output';bad.mkdir(exist_ok=True);(bad/'app.jadpo').write_text(source.replace('operations { private_details { NoteRole.owner: [read] } }',''))
result=subprocess.run([str(root/'build/auth-policy/compiler/debug/jadpo-wasm-projection'),str(bad),str(out/'rejected')],text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
(out/'bad-output.log').write_text(result.stdout);assert result.returncode!=0 and 'policy.output_field_unproved' in result.stdout
(out/'results.json').write_text(json.dumps({'variant':changed,'original':original,'sourceConformancePassed':True,'negativePlans':negative,'unsafeOutputRejected':True},indent=2)+'\n')
print('Policy grant/refinement conformance passed on all three targets; six unsupported plans and unsafe output rejected; original restored')
