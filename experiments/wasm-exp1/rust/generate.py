#!/usr/bin/env python3
"""Lower a bounded checked projection to actual Rust async control flow."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('projection', type=Path)
parser.add_argument('--entry', default='probe')
parser.add_argument('--output', type=Path, default=Path(__file__).parent / 'build')
args = parser.parse_args()
p = json.loads(args.projection.read_text())
if p.get('schemaVersion') != 1 or p.get('kind') != 'jadpo_checked_executable_projection':
    raise SystemExit('unsupported or unchecked projection')
if p['boundary'] != {'closedRecords': True, 'omittedNullPresent': 'distinct',
                     'textLength': 'unicodeScalar', 'trustedPrincipalOnly': True}:
    raise SystemExit('unsupported boundary contract')
decls = {d['name']: d for d in p['declarations']}
entry = decls[args.entry]
if entry['kind'] != 'callable' or len(entry['parameters']) != 1:
    raise SystemExit('probe entry must be a one-parameter checked callable')
reachable = {}
def walk(value):
    if isinstance(value, dict):
        if value.get('op') == 'call':
            target = decls[value['target']]
            if target['semanticId'] != value['targetId']:
                raise ValueError('checked call identity mismatch')
            visit(target)
        for child in value.values(): walk(child)
    elif isinstance(value, list):
        for child in value: walk(child)
def visit(d):
    if d['name'] in reachable: return
    if d['kind'] != 'callable' or d.get('consistency') is not None:
        raise ValueError('unsupported reachable callable/transaction')
    reachable[d['name']] = d
    walk(d['body'])
visit(entry)

def s(value): return json.dumps(value, ensure_ascii=True)
def lit(value):
    # Embed JSON safely as a Rust string. JSON ASCII escapes are not Rust escapes.
    encoded = json.dumps(value, separators=(',', ':'), ensure_ascii=False)
    for n in range(1, 20):
        marks = '#' * n
        if '"' + marks not in encoded:
            return f'serde_json::from_str::<Value>(r{marks}"{encoded}"{marks}).unwrap()'
    raise ValueError('unembeddable string')
def var(name): return 'v_' + ''.join(c if c.isalnum() else '_' for c in name)
def fn(d): return 'call_' + str(d['semanticId'])

def expression(e, d):
    op = e['op']; sid = d['semanticId']
    if op == 'load':
        path = e['path']
        return f'member(&{var(path[0])}, &[{",".join(s(x) for x in path[1:])}], {sid})'
    if op == 'attempt': return expression(e['value'], d)
    if op == 'call':
        target = decls[e['target']]
        arguments = ', '.join(f'({expression(a,d)})?' for a in e['arguments'])
        return f'{fn(target)}({arguments}).await'
    if op == 'query':
        if e['cardinality'] != 'required' or e['predicate']['operator'] != 'equal':
            raise ValueError('unsupported query shape')
        if e.get('hostEffect') != 'storage.read' or e.get('policy') != 'checkedOperationObligations':
            raise ValueError('query lacks checked policy/effect')
        q = next((q for q in p['queries'] if q['name'] == d['name']), None)
        if not q or q['plan'] != 'authoritative': raise ValueError('unsupported freshness')
        fail = e['missing']
        if fail['op'] != 'reject' or fail['fields'] or fail['failure'] not in d['failures']:
            raise ValueError('unsupported query failure')
        policy = next(x for x in p['policy']['operations'] if x['operation'] == d['name'])
        descriptor = {'entity': e['entity'], 'operation':d['name'], 'semanticOperationId':sid,
            'freshness':q['plan'], 'policy': {'operation':policy, 'bindings':p['policy']['bindings'],
                                           'entities':p['policy']['entities']},
            'predicate':{'field':e['predicate']['field'],'operator':'equal'}}
        value = expression(e['predicate']['value'], d)
        return ('async { let mut args = ' + lit(descriptor) + '; '
                + f'args["predicate"]["value"] = ({value})?; '
                + f'let row = host_read(args, {sid}).await?; '
                + f'if row.is_null() {{ return Err(Fault::Domain({s(fail["failure"])})); }} '
                + f'if !validate({s(e["entity"])}, &row) {{ return Err(Fault::Internal({sid})); }} '
                + 'Ok(row) }.await')
    if op == 'outcomeMatch':
        lines = [f'match {expression(e["subject"],d)} {{']
        seen_success = False
        for arm in e['arms']:
            pat = arm['pattern']; body = expression(arm['body'],d)
            if pat['kind'] == 'success':
                if seen_success: raise ValueError('duplicate success')
                seen_success = True
                lines.append(f'Ok({var(pat["binding"])}) => {body},')
            elif pat['kind'] == 'failure':
                lines.append(f'Err(Fault::Domain(name)) if name == {s(pat["failure"])} => {body},')
            else: raise ValueError('unsupported outcome pattern')
        if not seen_success: raise ValueError('missing checked success arm')
        lines.append('Err(error) => Err(error), }')
        return '\n'.join(lines)
    raise ValueError(f'unsupported reachable expression: {op}')

def callable_source(d):
    lines = [f'async fn {fn(d)}(' + ', '.join(var(x['name'])+': Value' for x in d['parameters']) + ') -> Outcome {']
    for stmt in d['body']:
        if stmt['op'] == 'bind':
            if stmt['mutable']: raise ValueError('mutable binding unsupported')
            lines.append(f'let {var(stmt["name"])} = ({expression(stmt["value"],d)})?;')
        elif stmt['op'] == 'return':
            lines.append(f'return {expression(stmt["value"],d)};')
        else: raise ValueError('unsupported reachable statement: '+stmt['op'])
    lines.append('}')
    return '\n'.join(lines)

def scalar(desc):
    base = desc['base']; nullable = desc.get('nullable',False)
    if base == 'Text': checks = ['value.is_string()']
    elif base == 'Uuid': checks = ['uuid(value)']
    elif base == 'Bool': checks = ['value.is_boolean()']
    elif base == 'Int': checks = ['value.as_i64().is_some()']
    else: raise ValueError('unsupported scalar '+base)
    for c in desc.get('constraints',[]):
        if base != 'Text' or c['kind'] not in ['MinLength','MaxLength']:
            raise ValueError('unsupported constraint '+c['kind'])
        cmp = '>=' if c['kind']=='MinLength' else '<='
        checks.append(f'value.as_str().map_or(false, |s| s.chars().count() {cmp} {c["value"]})')
    inner = '('+' && '.join(checks)+')'
    return '(value.is_null() || '+inner+')' if nullable else inner

validators = {name:scalar(desc) for name,desc in p['effectiveTypes'].items()}
for name in ['Text','Uuid','Bool','Int']:
    validators.setdefault(name,scalar({'base':name}))
for d in p['declarations']:
    if d['kind'] == 'record':
        checks = []
        fields = d['fields']; allowed = ','.join(s(f['name']) for f in fields)
        for f in fields:
            name = f['name']; typ = f['type']['name']
            # Effective field types retain inherited nullability and refinements.
            checked_name = d['name']+'.'+name
            target = checked_name if checked_name in validators else typ
            check = f'validate({s(target)}, field)'
            if f['type']['nullable']: check = '(field.is_null() || '+check+')'
            checks.append(f'object.get({s(name)}).map_or({str(f["optional"]).lower()}, |field| {check})')
        validators[d['name']] = ('value.as_object().map_or(false, |object| '
            + f'object.keys().all(|key| [{allowed}].contains(&key.as_str()))'
            + ''.join(' && '+c for c in checks)+')')
    elif d['kind']=='enum':
        validators[d['name']] = f'value.as_str().map_or(false, |v| [{",".join(s(x) for x in d["variants"])}].contains(&v))'
validation = '#[allow(unused_parens)]\nfn validate(name: &str, value: &Value) -> bool { match name {\n' + '\n'.join(
    f'{s(name)} => {check},' for name,check in sorted(validators.items())) + '\n_ => false, } }'
functions = '\n\n'.join(callable_source(d) for d in sorted(reachable.values(),key=lambda d:d['semanticId']))
parameter = entry['parameters'][0]['type']['name']
entry_source = f'''fn make_entry(operation: i32, input: Value) -> Result<Pin<Box<dyn Future<Output = Outcome>>>, Fault> {{
    if operation != {entry['semanticId']} {{ return Err(Fault::Internal(operation as u32)); }}
    if !validate({s(parameter)}, &input) {{ return Err(Fault::Invalid); }}
    Ok(Box::pin({fn(entry)}(input)))
}}'''
runtime = (Path(__file__).parent/'runtime.rs').read_text()
generated = '\n\n'.join([runtime,validation,functions,entry_source])+'\n'
args.output.mkdir(parents=True,exist_ok=True)
(args.output/'generated.rs').write_text(generated)
manifest = {'schemaVersion':1,'route':'rust','checkedRevision':p['checkedRevision'],
    'projectionSha256':hashlib.sha256(args.projection.read_bytes()).hexdigest(),
    'generatedRustSha256':hashlib.sha256(generated.encode()).hexdigest(),
    'entry':{'name':entry['name'],'semanticId':entry['semanticId'],'input':parameter},
    'reachable':[{'name':d['name'],'semanticId':d['semanticId'],'span':d['span']} for d in reachable.values()],
    'validation':'all boundary and host row validation inside Wasm; unicode scalar length',
    'hostResults':'closed success/value envelope; value null maps to authored missing failure inside Wasm; all other host envelopes internal',
    'dependencies':{'serde_json':'1.0.133'},'unsupported':'Only reachable read/load/bind/return/call/attempt/outcomeMatch subset; mutations and transactions rejected.'}
(args.output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest,indent=2))
