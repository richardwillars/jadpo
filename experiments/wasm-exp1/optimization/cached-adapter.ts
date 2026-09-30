/** Compiler-projection-driven experiment storage. No application recovery/control flow. */
export type SqlValue = string | number | null;
export type Row = Record<string, SqlValue>;
export interface SqlAdapter {
  rows(sql: string, parameters: readonly SqlValue[]): Row[];
  transactionSync<T>(work: () => T): T;
  isUniqueConflict(error: unknown): boolean;
}
export type TrustedPrincipal = Readonly<{entity: string; values: Readonly<Record<string, unknown>>}>;
export type Envelope = {kind: string; [key:string]: unknown};
export class UnsupportedStorage extends Error { constructor(reason:string) {super(`WASM_STORAGE_UNSUPPORTED:${reason}`);} }
const reject=(reason:string):never=>{throw new UnsupportedStorage(reason);};
const ident=(value:string)=>{if(!/^[A-Za-z_][A-Za-z0-9_]*$/.test(value))reject("identifier");return `"${value}"`;};
const canonical=(value:unknown):string=>JSON.stringify(value,(_key,v)=>v!==null&&typeof v==="object"&&!Array.isArray(v)?Object.fromEntries(Object.entries(v).sort(([a],[b])=>a.localeCompare(b))):v);
const isObject=(value:unknown):value is Record<string,unknown>=>value!==null&&typeof value==="object"&&!Array.isArray(value);
const closed=(value:unknown,keys:string[])=>{if(!isObject(value)||Object.keys(value).some(k=>!keys.includes(k)))reject("closed_request");};
const syncResult=<T>(value:T):T=>{if(value!==null&&(typeof value==="object"||typeof value==="function")&&typeof (value as {then?:unknown}).then==="function")reject("async_transaction");return value;};

export function createStorage(programInput:unknown, sql:SqlAdapter, options={authoritative:true}) {
  // Owned immutable copy prevents request metadata from mutating the trusted plan.
  const program=structuredClone(programInput) as any;
  if(program?.kind!=="jadpo_checked_executable_projection"||program.schemaVersion!==1)reject("projection");
  const declarations=new Map<string,any>(program.declarations.map((d:any)=>[d.name,d]));
  const entities=new Map<string,any>(program.entities.map((e:any)=>[e.name,e]));
  const persistent=program.entities.filter((e:any)=>e.persistent);
  const stores=new Set(persistent.map((e:any)=>e.authorityStore));
  if(stores.size!==1||[...stores][0]===null)reject("cross_store");
  const store=[...stores][0];
  if(!options.authoritative)reject("authoritative_unavailable");
  for(const query of program.queries)if(query.plan!=="authoritative"||query.freshness!=="authoritative")reject("query_freshness");
  const tables=new Map<string,any>();
  const names=new Set<string>();
  for(const entity of persistent){
    const record=declarations.get(entity.name);if(record?.kind!=="record"||!record.persistent)reject("entity_record");
    const table=`entity_${entity.name}`;ident(table);if(names.has(table.toLowerCase()))reject("table_collision");names.add(table.toLowerCase());
    const fields=new Map<string,any>();const fieldNames=new Set<string>();
    for(const field of record.fields){
      ident(field.name);if(fieldNames.has(field.name.toLowerCase()))reject("field_collision");fieldNames.add(field.name.toLowerCase());const type=program.effectiveTypes[`${entity.name}.${field.name}`];
      if(!type||!["Text","Uuid"].includes(type.base)||field.optional||field.reference!==null)reject("field_type");
      if(type.constraints.some((c:any)=>!["MinLength","MaxLength"].includes(c.kind)))reject("constraint");
      if(field.storage.some((s:string)=>!["Identity","Unique"].includes(s)))reject("storage_annotation");
      fields.set(field.name,{...field,effective:type});
    }
    if(!fields.has(entity.identity))reject("identity");
    tables.set(entity.name,{...entity,table,fields});
  }
  for(const t of program.transactions){
    if(t.domain!==store||t.owners.some((o:string)=>entities.get(o)?.authorityStore!==store)||t.nested!=="join_declared_boundary")reject("transaction_domain");
  }
  const writes=new Set(program.transactions.map((t:any)=>t.action));
  let changed=true;while(changed){changed=false;for(const edge of program.callEdges){const caller=program.declarations.find((d:any)=>d.semanticId===edge.caller)?.name;const callee=program.declarations.find((d:any)=>d.semanticId===edge.callee)?.name;if(writes.has(callee)&&!writes.has(caller)){writes.add(caller);changed=true;}}}
  function inspect(value:any){if(!value||typeof value!=="object")return;if(value.op==="outcomeMatch"&&value.subject?.op==="call"&&writes.has(value.subject.target))reject("handled_nested_mutation");for(const child of Object.values(value))if(Array.isArray(child))child.forEach(inspect);else inspect(child);}
  program.declarations.forEach((d:any)=>inspect(d.body));
  for(const entity of program.policy.entities)if(entity.scope!==entity.entity||entity.scopeField!==null||entity.fields.length)reject("indirect_or_field_policy");
  for(const binding of program.policy.bindings)if(binding.scope!==binding.entity||!tables.get(binding.entity)?.fields.has(binding.field)||!entities.get(binding.principalEntity)?.identity)reject("indirect_binding");

  function validate(table:any,field:string,value:unknown):SqlValue {
    const f=table.fields.get(field);if(!f)reject("unknown_field");const t=f.effective;
    if(value===null){if(!t.nullable)reject("null_value");return null;}
    if(typeof value!=="string")reject("field_value");
    if(t.base==="Uuid"&&!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(value))reject("uuid_value");
    const length=[...value].length;
    for(const c of t.constraints)if((c.kind==="MinLength"&&length<c.value)||(c.kind==="MaxLength"&&length>c.value))reject("constraint_value");
    return value;
  }
  function validateRow(table:any,row:Row):Row {
    if(Object.keys(row).length!==table.fields.size)reject("stored_shape");
    return Object.fromEntries([...table.fields.keys()].map(field=>[field,validate(table,field,row[field])])) as Row;
  }
  function setup(){for(const table of tables.values()){
    const columns=[...table.fields.values()].map((f:any)=>`${ident(f.name)} TEXT${f.effective.nullable?"":" NOT NULL"}${f.name===table.identity?" PRIMARY KEY":""}${f.storage.includes("Unique")?" UNIQUE":""}`);
    sql.rows(`CREATE TABLE IF NOT EXISTS ${ident(table.table)} (${columns.join(", ")})`,[]);
  }}
  function descriptor(operation:string){const op=program.policy.operations.find((p:any)=>p.operation===operation);if(!op)reject("operation_policy");return {operation:structuredClone(op),bindings:structuredClone(program.policy.bindings),entities:structuredClone(program.policy.entities)};}
  // Only compiler-owned immutable facts are cached. Incoming descriptors are still checked.
  const expectedPolicy = new Map(program.policy.operations.map((op:any)=>[op.operation,canonical(descriptor(op.operation))]));
  const checkedNodes = new Map<string,any[]>();
  for(const decl of declarations.values()){
    const visit=(v:any)=>{if(!v||typeof v!=="object")return;
      if(v.hostEffect==='storage.read'||v.hostEffect==='storage.update'){
        const key=JSON.stringify([decl.name,v.hostEffect,v.entity]);
        const nodes=checkedNodes.get(key)??[];nodes.push(v);checkedNodes.set(key,nodes);
      }
      for(const child of Object.values(v))if(Array.isArray(child))child.forEach(visit);else visit(child);
    };visit(decl.body);
  }
  function scoped(args:any,principal:TrustedPrincipal|null,effect:"read"|"update") {
    closed(args,["entity","operation","semanticOperationId","freshness","policy","predicate",...(effect==="update"?["changes"]:[])]);
    const table=tables.get(args.entity),decl=declarations.get(args.operation);if(!table||decl?.kind!=="callable"||decl.semanticId!==args.semanticOperationId)reject("operation_identity");
    if(args.freshness!=="authoritative")reject("freshness");
    if(canonical(args.policy)!==expectedPolicy.get(args.operation))reject("policy_descriptor");
    closed(args.predicate,["field","operator","value"]);
    if(args.predicate.operator!=="equal"||args.predicate.field!==table.identity)reject("predicate");
    const storageNodes=checkedNodes.get(JSON.stringify([decl.name,`storage.${effect}`,args.entity]))??[];
    if(storageNodes.length!==1||storageNodes[0].predicate.field!==args.predicate.field)reject("undeclared_storage_operation");
    const node=storageNodes[0];
    if(effect==="update"){
      let allowed:string[];
      if(node.patch!==null){if(node.patch.length!==1)reject("patch_path");const parameter=decl.parameters.find((p:any)=>p.name===node.patch[0]);const patch=declarations.get(parameter?.type?.name);if(patch?.kind!=="record")reject("patch_record");allowed=patch.fields.map((f:any)=>f.name);}
      else allowed=node.set.map((f:any)=>f.name);
      if(!isObject(args.changes)||Object.keys(args.changes).some(k=>!allowed.includes(k)))reject("undeclared_update_field");
    }
    const parameters:SqlValue[]=[validate(table,args.predicate.field,args.predicate.value)];
    const clauses=[`${ident(args.predicate.field)} = ?`];
    const obligations=args.policy.operation.obligations.filter((o:any)=>o.entity===args.entity&&o.effect===effect);
    if(obligations.length===0)reject("missing_obligation");
    for(const obligation of obligations){
      if(obligation.origin!=="entity")reject("field_obligation");const alternatives:string[]=[];
      for(const role of obligation.subjects){
        const declared=program.policy.entities.find((e:any)=>e.entity===args.entity)?.rules.some((r:any)=>r.subject===role&&r.effects.includes(effect));
        if(!declared)reject("undeclared_role");
        const bindings=program.policy.bindings.filter((b:any)=>b.entity===args.entity&&b.role===role);
        if(bindings.length!==1)reject("binding_cardinality");const binding=bindings[0];
        if(principal?.entity===binding.principalEntity){const identity=entities.get(binding.principalEntity).identity;parameters.push(validate(table,binding.field,principal.values[identity]));alternatives.push(`${ident(binding.field)} = ?`);}
      }
      clauses.push(alternatives.length?`(${alternatives.join(" OR ")})`:"0 = 1");
    }
    return {table,parameters,where:clauses.join(" AND ")};
  }
  function read(args:unknown,principal:TrustedPrincipal|null):Envelope {
    const {table,parameters,where}=scoped(args,principal,"read");
    const rows=sql.rows(`SELECT ${[...table.fields.keys()].map(ident).join(", ")} FROM ${ident(table.table)} WHERE ${where} LIMIT 2`,parameters);
    if(rows.length>1)reject("cardinality");return {kind:"success",value:rows.length?validateRow(table,rows[0]):null};
  }
  let transactionActive=false;
  function update(args:any,principal:TrustedPrincipal|null):Envelope {
    if(!transactionActive)reject("update_requires_transaction");
    const {table,parameters,where}=scoped(args,principal,"update");
    if(!isObject(args.changes))reject("changes");const fields=Object.keys(args.changes);
    for(const field of fields)if(!table.fields.has(field)||table.fields.get(field).immutable||field===table.identity)reject("immutable_or_unknown_field");
    if(fields.length===0)return {kind:"success",value:{status:"empty"}};
    const values=fields.map(field=>validate(table,field,args.changes[field]));
    try{
      const rows=sql.rows(`UPDATE ${ident(table.table)} SET ${fields.map(f=>`${ident(f)} = ?`).join(", ")} WHERE ${where} RETURNING ${[...table.fields.keys()].map(ident).join(", ")}`,[...values,...parameters]);
      if(rows.length>1)reject("cardinality");return {kind:"success",value:rows.length?{status:"found",row:validateRow(table,rows[0])}:{status:"missing"}};
    }catch(error){if(sql.isUniqueConflict(error))return {kind:"success",value:{status:"conflict"}};throw error;}
  }
  class Rollback {constructor(readonly result:Envelope){}}
  function runAtomic(work:()=>Envelope):Envelope {
    if(transactionActive)reject("nested_transaction");
    try{return sql.transactionSync(()=>{transactionActive=true;try{const result=syncResult(work());if(!isObject(result)||!["success","domain","internal","invalid"].includes(result.kind))reject("terminal_envelope");if(result.kind!=="success")throw new Rollback(result);return result;}finally{transactionActive=false;}});}
    catch(error){if(error instanceof Rollback)return error.result;throw error;}
  }
  return {setup,descriptor,read,update,runAtomic,
    schema:[...tables.values()].map(t=>({entity:t.name,table:t.table,identity:t.identity,fields:[...t.fields.keys()]})),
    // Trusted test setup only, never exposed as a Wasm capability or public endpoint.
    resetFixture(rows:Record<string,Row[]>){sql.transactionSync(()=>{for(const t of tables.values()){sql.rows(`DELETE FROM ${ident(t.table)}`,[]);for(const row of rows[t.name]??[]){const valid=validateRow(t,row);const fields=[...t.fields.keys()] as string[];sql.rows(`INSERT INTO ${ident(t.table)} (${fields.map(ident).join(", ")}) VALUES (${fields.map(()=>"?").join(", ")})`,fields.map(f=>valid[f]));}}});},
    snapshot(){return Object.fromEntries([...tables.values()].map(t=>[t.name,sql.rows(`SELECT ${[...t.fields.keys()].map(ident).join(", ")} FROM ${ident(t.table)} ORDER BY ${ident(t.identity)}`,[])]));},
  };
}
