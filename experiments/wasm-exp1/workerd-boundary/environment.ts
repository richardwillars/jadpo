import {timedSqlite} from '../read-path/sql-timing.ts';
import {Database} from 'bun:sqlite';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {invokeSync as invokePrevious,configureRows as configurePrevious,poolStats as previousPoolStats} from '../typed-values/driver.ts';
import {invokeSync,configureRows,poolStats} from './driver.ts';
import {createStorage as oldStorage} from '../optimization/cached-adapter.ts';
import {createStorage as newStorage} from '../boundary-http/adapter.ts';
import {bunSqlite} from '../optimization/cached-sqlite.ts';
export const root=resolve(import.meta.dir,'../../..');
export const program=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/compiler/build/projected/program.json','utf8'));
export const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json','utf8'));
export const originalRows=spec.seeds.Item;
export async function environment(target:string,dbPath:string,diagnostics=false){
 const isBun=target==='bun';let app:any,persistence:any,module:WebAssembly.Module,storage:any;
 if(isBun){delete Bun.env.DATABASE_URL;Bun.env.SQLITE_PATH=dbPath;const base=root+'/build/wasm-exp1/baseline/generated/bun/target/';app=await import(base+'app.ts');({persistence}=await import(base+'persistence.ts'));}
 const db=new Database(dbPath,{strict:true});db.exec("PRAGMA foreign_keys = ON");
 const timing=diagnostics?timedSqlite(db):undefined;
 if(!isBun){const modulePath=target==='previous'?'typed-values/compiler/build/application.wasm':'read-path/compiler/build/application.wasm';module=new WebAssembly.Module(readFileSync(root+`/experiments/wasm-exp1/${modulePath}`));(target==='previous'?configurePrevious:configureRows)(module,JSON.parse(readFileSync(target==='previous'?root+'/experiments/wasm-exp1/typed-values/compiler/build/row-codec.json':import.meta.dir+'/../read-path/compiler/build/row-codec.json','utf8')),({previous:2,json:0,ingress:1,egress:2,both:3,typed:3,reference:6,combined:7,compact:10,'compact-reference':14,'compact-combined':15,candidate:14} as any)[target]??2);storage=newStorage(program,timing?.adapter??bunSqlite(db));storage.setup();}
 const table=isBun?'item':'entity_Item';
 const fields=Object.keys(originalRows[0]);const columns=db.query(`PRAGMA table_info("${table}")`).all() as any[];
 if(columns.length!==fields.length||columns.some(c=>!fields.includes(c.name)))throw Error('fixture table mismatch');
 const reset=(large:boolean|string=false)=>{const rows=structuredClone(originalRows);if(large!==false)rows[0].note=typeof large==='string'?large:'x'.repeat(16384);db.transaction(()=>{db.exec(`DELETE FROM "${table}"`);for(const r of rows)db.prepare(`INSERT INTO "${table}" (${fields.map(f=>`"${f}"`).join(',')}) VALUES (${fields.map(()=>'?').join(',')})`).run(...fields.map(f=>r[f]));})();return rows;};
 const snapshot=()=>db.query(`SELECT * FROM "${table}" ORDER BY id`).all();
 async function call(operation:string,input:any,principalName='owner'){
  const principal=spec.seeds.principals[principalName];
  if(!principal)throw Error('trusted fixture principal');
  if(isBun){
   let args:any[];try{
    const id=(v:any)=>app.experimentValidators.User({id:v},'input').id;
    const title=(v:any)=>app.experimentValidators.ItemTitle(v,'input.title');
    if(operation==='probe')args=[app.experimentValidators.ProbeInput(input,'input')];
    else if(operation==='Item.read'||operation==='Item.read_title')args=[id(input)];
    else {if(!Array.isArray(input))return {kind:'invalid'};
     if(operation==='Item.change'&&input.length===2)args=[id(input[0]),app.experimentValidators.ItemPatch(input[1],'input.patch')];
     else if(operation==='Item.rename'&&input.length===2)args=[id(input[0]),title(input[1])];
     else if(operation==='update_pair'&&input.length===4)args=[id(input[0]),title(input[1]),id(input[2]),title(input[3])];
     else return {kind:'invalid'};
    }
   }catch(e){if(e instanceof app.ValidationError)return {kind:'invalid'};throw e;}
   try{return {kind:'success',value:await app.experimentCallables[operation](...args,app.captureOperation(null,{kind:'user',subject:principal.id,values:{user_id:principal.id}}),persistence)};}
   catch(e){if(e instanceof app.DomainFailure)return {kind:'domain',failure:e.failureName};return {kind:'internal'};}
  }
  const decl=program.declarations.find((d:any)=>d.kind==='callable'&&d.name===operation);if(!decl)return {kind:'invalid'};
  const execute=()=>(target==='previous'?invokePrevious:invokeSync)(module!,decl.semanticId,input,(capability,args)=>{
   const trusted={entity:principal.entity,values:{id:principal.id}};
   if(capability==='storage.read')return storage.read(args,trusted);
   if(capability==='storage.update')return storage.update(args,trusted);
   throw Error('unsupported capability');
  });
  return ['Item.change','Item.rename','update_pair'].includes(operation)?storage.runAtomic(execute):execute();
 }
 return {call,reset,snapshot,db,timing,poolStats:()=>isBun?null:{...(target==='previous'?previousPoolStats:poolStats)},pragmas:{journal:db.query('PRAGMA journal_mode').get(),synchronous:db.query('PRAGMA synchronous').get(),foreignKeys:db.query('PRAGMA foreign_keys').get()},close:()=>db.close()};
}
