#!/usr/bin/env python3
"""Lower checked projection into Wasm basic blocks, not Rust application source."""
import argparse, hashlib, json
from pathlib import Path

class Unsupported(Exception): pass
class Generator:
    def __init__(self, projection, entry):
        self.p = projection
        self.decls = {d['name']: d for d in projection['declarations']}
        self.entry = self.decls[entry]
        self.blocks = []
        self.registers = 0
        self.constants = bytearray()
        self.constant_index = {}
        self.reachable = {}
        self.maps = []
        self.descriptors = {}
    def reg(self):
        self.registers += 1
        return f'$v{self.registers}'
    def constant(self, value):
        data = json.dumps(value, ensure_ascii=False, separators=(',', ':')).encode()
        if data not in self.constant_index:
            offset = len(self.constants)
            self.constants.extend(data)
            if len(self.constants) > 65536: raise Unsupported('constant frame exceeds 64KiB')
            self.constant_index[data] = (offset, len(data))
        offset, length = self.constant_index[data]
        return f'(call @rt_constant (i32.const {offset}) (i32.const {length}))'
    def descriptor(self, name, nullable=False, stack=()):
        if name in stack: raise Unsupported('recursive boundary type')
        if name in self.p['effectiveTypes']:
            e = self.p['effectiveTypes'][name]
            if e['textLengthUnit'] != 'unicodeScalar': raise Unsupported('text length unit')
            if any(c['kind'] not in ('MinLength','MaxLength','Min','Max') for c in e['constraints']): raise Unsupported('constraint kind')
            result = dict(kind=e['base'], nullable=e['nullable'] or nullable, constraints=e['constraints'])
        elif name in ('Text','Uuid','Int','Bool','Float'):
            result = dict(kind=name, nullable=nullable)
        else:
            d = self.decls.get(name)
            if not d: raise Unsupported(f'unknown boundary type {name}')
            if d['kind'] == 'record':
                fields = {}
                for field in d['fields']:
                    effective = name + '.' + field['name']
                    desc = self.descriptor(effective if effective in self.p['effectiveTypes'] else field['type']['name'], field['type']['nullable'], stack+(name,))
                    fields[field['name']] = dict(desc, optional=field['optional'])
                result = dict(kind='record', nullable=nullable, fields=fields)
            elif d['kind'] == 'enum': result = dict(kind='enum', nullable=nullable, variants=d['variants'])
            else: raise Unsupported(f'boundary declaration {d["kind"]}')
        if result['kind'] not in ('Text','Uuid','Int','Bool','Float','record','enum'): raise Unsupported('primitive')
        return result
    def block(self, code, node=None):
        index = len(self.blocks) + 1
        self.blocks.append(code)
        if node and 'span' in node: self.maps.append(dict(block=index, span=node['span'], op=node.get('op')))
        return index
    def jump(self, target): return f'(global.set $pc (i32.const {target})) (br $dispatch)'
    def fault(self, semantic): return f'(return (call $finish (i32.const 3) (i32.const 0) (i32.const {semantic})))'
    def checked(self, reg, name, nullable, semantic):
        desc = self.constant(self.descriptor(name, nullable))
        return f'(if (i32.eqz (call @rt_validate (global.get {reg}) {desc})) (then {self.fault(semantic)}))'
    def failure(self, name, handlers, semantic):
        if name in handlers: return handlers[name]
        d = self.decls.get(name)
        if not d or d['kind'] != 'failure' or d['public'] or d['internal']: raise Unsupported('failure fields')
        return self.block(f'(return (call $finish (i32.const 1) {self.constant(name)} (i32.const {semantic})))')
    def body(self, statements, env, dest, success, handlers, declaration, stack):
        if not statements: raise Unsupported('missing return')
        first, rest = statements[0], statements[1:]
        sem = declaration['semanticId']
        if first['op'] == 'return' and not rest:
            validated = self.block(self.checked(dest, declaration['result']['name'], declaration['result']['nullable'], sem)+self.jump(success), first)
            return self.expr(first['value'], env, dest, validated, handlers, declaration, stack)
        if first['op'] == 'bind' and not first['mutable']:
            register = self.reg()
            tail = self.body(rest, dict(env, **{first['name']: register}), dest, success, handlers, declaration, stack)
            return self.expr(first['value'], env, register, tail, handlers, declaration, stack)
        if first['op'] == 'reject' and not rest and not first['fields']:
            return self.failure(first['failure'], handlers, sem)
        raise Unsupported('statement: '+first['op'])
    def call(self, declaration, args, caller_env, dest, success, handlers, caller, stack):
        name = declaration['name']
        if name in stack: raise Unsupported('recursive callable')
        if declaration['kind'] != 'callable' or declaration['consistency'] is not None: raise Unsupported('non-probe callable/transaction')
        if len(args) != len(declaration['parameters']): raise Unsupported('arity mismatch')
        self.reachable[name] = declaration
        env = {p['name']: self.reg() for p in declaration['parameters']}
        target = self.body(declaration['body'], env, dest, success, handlers, declaration, stack+(name,))
        # Arguments evaluate in authored left-to-right order. Distinct registers
        # preserve lexical scopes across inlined calls and suspended continuations.
        for arg, param in reversed(list(zip(args, declaration['parameters']))):
            target = self.expr(arg, caller_env, env[param['name']], target, handlers, caller, stack)
        return target
    def expr(self, e, env, dest, success, handlers, declaration, stack):
        op = e['op']; sem = declaration['semanticId']
        if op == 'attempt': return self.expr(e['value'], env, dest, success, handlers, declaration, stack)
        if op == 'load':
            path = e['path']
            if path[0] not in env: raise Unsupported('unbound load')
            code = f'(global.set {dest} (global.get {env[path[0]]}))'
            for member in path[1:]:
                code += f'(global.set {dest} (call @rt_member (global.get {dest}) {self.constant(member)}))'
                code += f'(if (i32.eqz (global.get {dest})) (then {self.fault(sem)}))'
            return self.block(code+self.jump(success), e)
        if op == 'literal':
            return self.block(f'(global.set {dest} {self.constant(e["value"])})'+self.jump(success), e)
        if op == 'call':
            callee = self.decls.get(e['target'])
            if not callee or callee['semanticId'] != e['targetId']: raise Unsupported('unresolved checked call')
            return self.call(callee,e['arguments'],env,dest,success,handlers,declaration,stack)
        if op == 'outcomeMatch':
            subject = self.reg(); inner = dict(handlers); success_target = None
            for arm in e['arms']:
                pattern = arm['pattern']
                if pattern['kind'] == 'success':
                    if success_target is not None: raise Unsupported('duplicate success arm')
                    local = dict(env)
                    if pattern.get('binding'): local[pattern['binding']] = subject
                    success_target = self.expr(arm['body'],local,dest,success,handlers,declaration,stack)
                elif pattern['kind'] == 'failure':
                    inner[pattern['failure']] = self.expr(arm['body'],env,dest,success,handlers,declaration,stack)
                else: raise Unsupported('outcome pattern')
            if success_target is None: raise Unsupported('missing success arm')
            return self.expr(e['subject'],env,subject,success_target,inner,declaration,stack)
        if op == 'query':
            if e['cardinality'] != 'required' or e['hostEffect'] != 'storage.read' or e['predicate']['operator'] != 'equal': raise Unsupported('query form')
            if declaration['freshness'] != 'Authoritative': raise Unsupported('query freshness')
            if e['missing']['fields']: raise Unsupported('missing failure fields')
            policy = next((p for p in self.p['policy']['operations'] if p['operation'] == declaration['name']), None)
            if not policy: raise Unsupported('missing checked policy operation')
            args = dict(entity=e['entity'],operation=declaration['name'],semanticOperationId=sem,freshness='authoritative',
                policy=dict(operation=policy,bindings=self.p['policy']['bindings'],entities=self.p['policy']['entities']),
                predicate=dict(field=e['predicate']['field'],operator='equal',value=None))
            key = self.reg(); args_reg = self.reg(); predicate = self.reg()
            missing = self.failure(e['missing']['failure'],handlers,sem)
            resumed = self.block(f'(global.set {dest} (global.get $host))'+
                f'(if (call @rt_is_null (global.get {dest})) (then {self.jump(missing)}))'+
                self.checked(dest,e['entity'],False,sem)+self.jump(success),e)
            suspend = self.block(f'''
                (global.set {args_reg} {self.constant(args)})
                (global.set {predicate} (call @rt_member (global.get {args_reg}) {self.constant('predicate')}))
                (drop (call @rt_set (global.get {predicate}) {self.constant('value')} (global.get {key})))
                (drop (call @rt_set (global.get {args_reg}) {self.constant('predicate')} (global.get {predicate})))
                (global.set $semantic (i32.const {sem}))
                (global.set $sequence (i32.add (global.get $sequence) (i32.const 1)))
                (global.set $pc (i32.const {resumed})) (global.set $state (i32.const 2))
                (local.set $status (call @rt_pending {self.constant(e['hostEffect'])} (global.get {args_reg}) (i32.const 1) (global.get $sequence)))
                (if (i32.ne (local.get $status) (i32.const 2)) (then (global.set $state (i32.const 3))))
                (return (local.get $status))''',e)
            return self.expr(e['predicate']['value'],env,key,suspend,handlers,declaration,stack)
        raise Unsupported('expression: '+op)
    def generate(self):
        e = self.entry
        if e['kind'] != 'callable' or len(e['parameters']) != 1 or e['consistency'] is not None: raise Unsupported('entry must have one parameter and no transaction')
        self.reachable[e['name']] = e
        input_reg, result_reg = self.reg(), self.reg()
        complete = self.block(f'(return (call $finish (i32.const 0) (global.get {result_reg}) (i32.const {e["semanticId"]})))')
        start = self.body(e['body'],{e['parameters'][0]['name']: input_reg},result_reg,complete,{},e,(e['name'],))
        param = e['parameters'][0]['type']
        descriptor = self.constant(self.descriptor(param['name'],param['nullable']))
        globals_ = '\n'.join(f'(global $v{i} (mut i32) (i32.const 0))' for i in range(1,self.registers+1))
        blocks = '\n'.join(f'(if (i32.eq (global.get $pc) (i32.const {i})) (then {code}))' for i,code in enumerate(self.blocks,1))
        wat = f'''
{globals_}
(global $pc (mut i32) (i32.const 0))
(global $state (mut i32) (i32.const 0))
(global $semantic (mut i32) (i32.const 0))
(global $sequence (mut i32) (i32.const 0))
(global $host (mut i32) (i32.const 0))
(func $finish (param $status i32) (param $value i32) (param $operation i32) (result i32)
 (global.set $state (i32.const 3)) (global.set $pc (i32.const 0))
 (call @rt_complete (local.get $status) (local.get $value) (local.get $operation)))
(func $drive (result i32) (local $status i32)
 (loop $dispatch {blocks} {self.fault(e['semanticId'])}) (unreachable))
(func $app_start (export "start") (param $operation i32) (param $ptr i32) (param $len i32) (result i32)
 (if (i32.or (global.get $state) (i32.ne (local.get $operation) (i32.const {e['semanticId']})))
  (then (return (call $finish (i32.const 3) (i32.const 0) (local.get $operation)))))
 (global.set $state (i32.const 1)) (global.set $semantic (local.get $operation))
 (global.set {input_reg} (call @rt_decode (local.get $ptr) (local.get $len)))
 (if (i32.eqz (call @rt_validate (global.get {input_reg}) {descriptor}))
  (then (return (call $finish (i32.const 4) (i32.const 0) (local.get $operation)))))
 (global.set $pc (i32.const {start})) (call $drive))
(func $app_resume (export "resume") (param $request i32) (param $operation i32) (param $ptr i32) (param $len i32) (result i32)
 (if (i32.or (i32.ne (global.get $state) (i32.const 2))
   (i32.or (i32.ne (local.get $request) (i32.const 1)) (i32.ne (local.get $operation) (global.get $sequence))))
  (then (return (call $finish (i32.const 3) (i32.const 0) (global.get $semantic)))))
 (global.set $host (call @rt_host_value (call @rt_decode (local.get $ptr) (local.get $len))))
 (if (i32.eqz (global.get $host)) (then (return (call $finish (i32.const 3) (i32.const 0) (global.get $semantic)))))
 (global.set $state (i32.const 1)) (call $drive))
'''
        return wat

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('projection'); parser.add_argument('--entry',required=True); parser.add_argument('--out',required=True)
    args = parser.parse_args()
    source = Path(args.projection).read_bytes(); projection = json.loads(source)
    if projection['schemaVersion'] != 1 or not projection['checkedRevision'].startswith('src_'): raise Unsupported('unrecognized checked projection')
    gen = Generator(projection,args.entry); wat = gen.generate()
    out = Path(args.out); out.mkdir(parents=True,exist_ok=True)
    (out/'application.wat').write_text(wat); (out/'constants.bin').write_bytes(gen.constants)
    manifest = dict(schemaVersion=1,route='direct',checkedRevision=projection['checkedRevision'],
        projectionSha256=hashlib.sha256(source).hexdigest(),applicationWatSha256=hashlib.sha256(wat.encode()).hexdigest(),
        entry=dict(name=args.entry,semanticId=gen.entry['semanticId'],input=gen.entry['parameters'][0]['type']['name']),
        reachable=[dict(name=d['name'],semanticId=d['semanticId'],span=d['span']) for d in gen.reachable.values()],
        blocks=gen.maps,registerCount=gen.registers,constantBytes=len(gen.constants),
        strategy='Direct WAT basic blocks and lexical registers; Rust generic JSON/value/validation helper contains no application control flow.',
        unsupported='Reachable recursive calls, mutations, transactions, non-required/non-authoritative reads, general expressions and failure payloads reject before output.')
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__ == '__main__':
    try: main()
    except (Unsupported, KeyError) as error: raise SystemExit('DIRECT_UNSUPPORTED: '+str(error))
