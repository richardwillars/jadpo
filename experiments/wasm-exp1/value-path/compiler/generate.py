#!/usr/bin/env python3
"""Lower a bounded checked projection to actual Rust async control flow."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('projection', type=Path)
parser.add_argument('--entry', help='Optional single checked callable; default exports all callables')
parser.add_argument('--output', type=Path, default=Path(__file__).parent / 'build')
args = parser.parse_args()
p = json.loads(args.projection.read_text())
if p.get('schemaVersion') != 1 or p.get('kind') != 'jadpo_checked_executable_projection':
    raise SystemExit('unsupported or unchecked projection')
if p['boundary'] != {'closedRecords': True, 'omittedNullPresent': 'distinct',
                     'textLength': 'unicodeScalar', 'trustedPrincipalOnly': True}:
    raise SystemExit('unsupported boundary contract')
decls = {d['name']: d for d in p['declarations']}
entries = [decls[args.entry]] if args.entry else [d for d in p['declarations'] if d['kind']=='callable']
if not entries or any(d['kind']!='callable' for d in entries):
    raise SystemExit('entry must be a checked callable')
reachable = {}
visiting = set()
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
    if d['name'] in visiting: raise ValueError('recursive call graph unsupported')
    if d['name'] in reachable: return
    if d['kind'] != 'callable' or d.get('consistency') not in (None,'Atomic'):
        raise ValueError('unsupported reachable callable/transaction')
    visiting.add(d['name'])
    reachable[d['name']] = d
    walk(d['body'])
    visiting.remove(d['name'])
for entry in entries: visit(entry)

def objects(value):
    if isinstance(value,dict):
        yield value
        for child in value.values(): yield from objects(child)
    elif isinstance(value,list):
        for child in value: yield from objects(child)
def mutates(value):
    for item in objects(value):
        if item.get('op')=='update': return True
        if item.get('op')=='call' and mutates(decls[item['target']]['body']): return True
    return False
stores={e['authorityStore'] for e in p['entities'] if e['persistent']}
if len(stores)!=1 or None in stores: raise ValueError('single authority store required')
for q in p['queries']:
    if q['plan']!='authoritative' or q['freshness']!='authoritative':
        raise ValueError('unsupported/unavailable authoritative freshness')
for rule in p['policy']['entities']:
    if rule['fields'] or rule['scopeField'] is not None: raise ValueError('indirect/field policy unsupported')
for d in reachable.values():
    if mutates(d['body']):
        tx=next((t for t in p['transactions'] if t['action']==d['name']),None)
        if not tx or tx['domain'] not in stores or tx['disposition'] not in ('entity_action','atomic'):
            raise ValueError('mutation lacks supported checked transaction plan')
        if tx['nested']!='join_declared_boundary': raise ValueError('unsupported nested transaction plan')
        if d['consistency']=='Atomic' and tx['disposition']!='atomic': raise ValueError('atomic plan mismatch')
    for item in objects(d['body']):
        if item.get('op')=='outcomeMatch' and mutates(item['subject']):
            raise ValueError('handled nested mutation/savepoint scope unsupported')

def s(value): return json.dumps(value, ensure_ascii=True)
def lit(value):
    # Embed JSON safely as a Rust string. JSON ASCII escapes are not Rust escapes.
    encoded = json.dumps(value, separators=(',', ':'), ensure_ascii=False)
    for n in range(1, 20):
        marks = '#' * n
        if '"' + marks not in encoded:
            return ('{ static CONSTANT: std::sync::OnceLock<Value> = std::sync::OnceLock::new(); '
                    + f'CONSTANT.get_or_init(|| serde_json::from_str::<Value>(r{marks}"{encoded}"{marks}).unwrap()).clone() }}')
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
    if op == 'update':
        if e.get('hostEffect')!='storage.update' or e.get('policy')!='checkedOperationObligations':
            raise ValueError('update lacks checked effect/policy')
        if e['predicate']['operator']!='equal' or e['entity']!=d['owner']:
            raise ValueError('only entity-owned equality updates supported')
        policy=next(x for x in p['policy']['operations'] if x['operation']==d['name'])
        descriptor={'entity':e['entity'],'operation':d['name'],'semanticOperationId':sid,
            'freshness':'authoritative','policy':{'operation':policy,'bindings':p['policy']['bindings'],'entities':p['policy']['entities']},
            'predicate':{'field':e['predicate']['field'],'operator':'equal'}}
        lines=['async { let mut args = '+lit(descriptor)+';',
            f'args["predicate"]["value"] = ({expression(e["predicate"]["value"],d)})?;']
        if e['patch'] is not None:
            patch=expression({'op':'load','path':e['patch']},d)
            lines.append(f'let mut changes = ({patch})?.as_object().ok_or(Fault::Internal({sid}))?.clone();')
        else: lines.append('let mut changes = serde_json::Map::new();')
        record=decls[e['entity']]
        for item in e['set']:
            field=next((f for f in record['fields'] if f['name']==item['name']),None)
            if not field or field['immutable'] or 'Identity' in field['storage']:
                raise ValueError('unsupported immutable/identity update')
            lines.append(f'changes.insert({s(item["name"])}.to_owned(), ({expression(item["value"],d)})?);')
        lines += ['args["changes"] = Value::Object(changes);',f'let mut result = host_update(args, {sid}).await?;',
            f'let object = result.as_object_mut().ok_or(Fault::Internal({sid}))?;',
            'match object.get("status").and_then(Value::as_str) {',
            f'Some("found") if object.len()==2 && object.contains_key("row") => {{ let row = &object["row"]; if !validate({s(e["entity"])},row) {{ return Err(Fault::Internal({sid})); }} Ok(object.remove("row").unwrap()) }},']
        failures={'missing':e['missing'],'empty':e['empty']}
        if len(e['conflicts'])!=1 or e['conflicts'][0]['constraint'] is not None:
            raise ValueError('only one unqualified conflict outcome supported')
        failures['conflict']=e['conflicts'][0]['failure']
        for status,failure in failures.items():
            if failure is None: continue
            if failure['op']!='reject' or failure['fields'] or failure['failure'] not in d['failures']:
                raise ValueError('unsupported authored update failure')
            lines.append(f'Some({s(status)}) if object.len()==1 => Err(Fault::Domain({s(failure["failure"])})),')
        lines += [f'_ => Err(Fault::Internal({sid})),','} }.await']
        return '\n'.join(lines)
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
entry_lines=['fn make_entry(operation: i32, input: Value) -> Result<Pin<Box<dyn Future<Output = Outcome>>>, Fault> { match operation {']
def parameter_check(reference,value):
    check=f'validate({s(reference["name"])}, &{value})'
    return f'({value}.is_null() || {check})' if reference['nullable'] else check
for entry in entries:
    parameters=entry['parameters'];entry_lines.append(f'{entry["semanticId"]} => {{')
    if len(parameters)==1:
        entry_lines.append(f'if !{parameter_check(parameters[0]["type"],"input")} {{ return Err(Fault::Invalid); }}')
        arguments='input'
    else:
        entry_lines.append(f'let Value::Array(arguments) = input else {{ return Err(Fault::Invalid); }}; if arguments.len()!={len(parameters)} {{return Err(Fault::Invalid);}}')
        for i,param in enumerate(parameters):
            entry_lines.append(f'if !{parameter_check(param["type"],f"arguments[{i}]")} {{return Err(Fault::Invalid);}}')
        entry_lines.append('let mut arguments = arguments.into_iter();')
        arguments=', '.join('arguments.next().unwrap()' for _ in parameters)
    entry_lines.append(f'Ok(Box::pin({fn(entry)}({arguments}))) }},')
entry_lines+=['_ => Err(Fault::Internal(operation as u32)), } }']
entry_source='\n'.join(entry_lines)
runtime = (Path(__file__).parent/'runtime.rs').read_text()
generated = '\n\n'.join([runtime,validation,functions,entry_source])+'\n'
args.output.mkdir(parents=True,exist_ok=True)
(args.output/'generated.rs').write_text(generated)
manifest = {'schemaVersion':1,'route':'rust-full','checkedRevision':p['checkedRevision'],
    'projectionSha256':hashlib.sha256(args.projection.read_bytes()).hexdigest(),
    'generatedRustSha256':hashlib.sha256(generated.encode()).hexdigest(),
    'entries':[{'name':d['name'],'semanticId':d['semanticId'],'parameters':d['parameters'],'span':d['span'],'atomic':mutates(d['body']),'inputShape':'direct' if len(d['parameters'])==1 else 'exactArityArray'} for d in entries],
    'reachable':[{'name':d['name'],'semanticId':d['semanticId'],'span':d['span']} for d in reachable.values()],
    'validation':'all boundary and host row validation inside Wasm; unicode scalar length',
    'hostResults':'read success/value=row|null; update success/value={status:found,row}|{status:missing|conflict|empty}; generated core maps declared outcomes',
    'dependencies':{'serde_json':'1.0.133'},'unsupported':'Handled nested mutation, cross-store, unavailable freshness, recursion and unsupported reachable expressions fail before artifact publication.'}
(args.output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(manifest,indent=2))
