from pathlib import Path
import os,json,subprocess,difflib,hashlib,time,shlex
state=json.loads(Path('/private/tmp/jadpo-diagnostic-mutation-state.json').read_text())
root=Path(state['root']); ws=Path(state['workspace']); out=Path(state['evidence'])
build=ws/'crates/diagnostics/build.rs'; original=build.read_text(); results=[]
env=os.environ.copy(); env['CARGO_TARGET_DIR']=state['target']
mutations=[
 ('missing-families','discovery_ignores_comments_and_test_only_codes_but_includes_new_families','"CONFIG_", "POLICY_", "TEST_", ','','CONFIG_VALUE_MISSING'),
 ('nested-helper-leakage','references_are_bounded_to_real_test_bodies_and_string_literals','if self.production && !test_only(&item.attrs) {','if !self.production || !test_only(&item.attrs) {','TYPE_NESTED_HELPER'),
]
def run(name,test,expected,needle=None):
    command=['cargo','test','--offline','--manifest-path',str(ws/'Cargo.toml'),'-p','jadpo-diagnostics','--test','validation_catalogue_discovery',test,'--','--exact','--nocapture']
    log=out/(name+'.log'); start=time.monotonic()
    with log.open('w') as stream:
        stream.write('CARGO_TARGET_DIR='+shlex.quote(env['CARGO_TARGET_DIR'])+' '+shlex.join(command)+'\n');stream.flush()
        completed=subprocess.run(command,cwd=root,env=env,stdout=stream,stderr=subprocess.STDOUT,timeout=180)
    text=log.read_text()
    valid=completed.returncode==expected and ('test '+test+' ... '+('ok' if expected==0 else 'FAILED')) in text
    if expected: valid=valid and "assertion `left == right` failed" in text and needle in text and 'could not compile' not in text
    result={'name':name,'command':command,'cwd':str(root),'environment':{'CARGO_TARGET_DIR':env['CARGO_TARGET_DIR']},'exit_code':completed.returncode,'expected_exit_code':expected,'intended_outcome':valid,'seconds':round(time.monotonic()-start,3),'build_source_sha256':hashlib.sha256(build.read_bytes()).hexdigest(),'log':str(log)}
    results.append(result); (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(result),flush=True)
    if not valid: raise RuntimeError('Unexpected result: '+name)
try:
    for name,test,before,after,needle in mutations:
        assert original.count(before)==1,(name,original.count(before))
        build.write_text(original)
        run(name+'-baseline',test,0)
        mutated=original.replace(before,after)
        (out/(name+'.diff')).write_text(''.join(difflib.unified_diff(original.splitlines(keepends=True),mutated.splitlines(keepends=True),fromfile='a/jadpo/crates/diagnostics/build.rs',tofile='b/jadpo/crates/diagnostics/build.rs')))
        build.write_text(mutated)
        run(name+'-mutated',test,101,needle)
        build.write_text(original)
        run(name+'-restored',test,0)
finally:
    build.write_text(original)
(out/'summary.json').write_text(json.dumps({'targeted_mutations':2,'all_intended_outcomes':all(r['intended_outcome'] for r in results),'runs':len(results),'limits':'Two targeted diagnostic discovery regression detectors; no overall mutation score or executed-trigger coverage proof.'},indent=2)+'\n')
