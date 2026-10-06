from pathlib import Path
import sys,json,copy
sys.path.insert(0,'/private/tmp/jadpo-review-final/tools')
import golden_checkpoint as checkpoint
root=Path('/private/tmp/jadpo-review-final')
evidence=Path('/private/tmp/jadpo-review-checkpoint-d071557')
dependencies=Path('/Users/richardwillars/Documents/Codex/LLM-coding/build/validation/jwt-dependencies')
report=json.loads((evidence/'golden-checkpoint.json').read_text())
results=[]
checkpoint.validate_checkpoint(report,evidence,dependencies,root)
results.append({'probe':'fresh final report','passed':True})
def reject(name, fn):
    try: fn()
    except ValueError as error:
        results.append({'probe':name,'refused':True,'reason':str(error)})
        return
    raise RuntimeError('Challenge incorrectly accepted: '+name)
changed=copy.deepcopy(report)
entry=next(e for e in changed['results'] if e['id']=='AUTH-001' and e['backend']=='sqlite')
entry['observations'][0]['steps'][0]['writes']=99
reject('extra ledger observation tampered despite expectation fields match',lambda:checkpoint.validate_checkpoint(changed,evidence,dependencies,root))
changed=copy.deepcopy(report)
changed['results']=[e for e in changed['results'] if e['id']!='READ-003']
reject('missing original case ID',lambda:checkpoint.validate_checkpoint(changed,evidence,dependencies,root))
for name,path in [('raw response bytes changed',evidence/'sqlite-raw.json'),('adapter source drift',root/'tools/golden_sql_observer.ts'),('generated artifact bytes changed',evidence/'application/build/target/persistence.ts')]:
    original=path.read_bytes()
    try:
        path.write_bytes(original+b'\n')
        reject(name,lambda:checkpoint.validate_checkpoint(report,evidence,dependencies,root))
    finally: path.write_bytes(original)
checkpoint.validate_checkpoint(report,evidence,dependencies,root)
print(json.dumps({'candidate':'d071557','challenges':results,'all_source_evidence_restored':True},indent=2))
