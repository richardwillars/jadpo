// Conformance-only boundary around unchanged generated Bun application/persistence.
import {Database} from 'bun:sqlite';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
export const root=resolve(import.meta.dir,'../..');
export const spec=JSON.parse(readFileSync(root+'/experiments/wasm-exp1/acceptance.json','utf8'));
export const originalRows=spec.seeds.Item;
const LIMIT=65536;
class BudgetFault extends Error {}
function bounded(value:any){if(Buffer.byteLength(JSON.stringify(value))>LIMIT)throw new BudgetFault();return value;}

// Preserve the generated transaction/savepoint machinery. The callback's result
// is checked BEFORE its owning transaction commits, not after env.call returns.
// Each generated storage result is also checked inside the same callback.
export function withBudgets(client:any,depth=0):any {
 return new Proxy(client,{get(target,key){const fn=target[key];if(typeof fn!=='function')return fn;
  if(key==='withPolicy'||key==='withOperationTime')return(...args:any[])=>withBudgets(fn.apply(target,args),depth);
  if(key==='transaction')return(work:any)=>fn.call(target,async(child:any)=>{
   const result=await work(withBudgets(child,depth+1));
   if(depth===0)bounded({kind:'success',value:result});
   return result;
  });
  if(String(key).startsWith('query_required_'))return async(...args:any[])=>{const result=await fn.apply(target,args);bounded({kind:'success',value:result});return result;};
  if(String(key).startsWith('update_required_'))return async(...args:any[])=>{const result=await fn.apply(target,args);bounded({kind:'success',value:result===null?{status:'missing'}:{status:'found',row:result}});return result;};
  return fn.bind(target);
 }});
}
export async function environment(_target:string,dbPath:string){
 delete Bun.env.DATABASE_URL;Bun.env.SQLITE_PATH=dbPath;
 const base=root+'/build/wasm-exp1/baseline/generated/bun/target/';
 const app=await import(base+'app.ts');const {persistence}=await import(base+'persistence.ts');const guarded=withBudgets(persistence);
 const db=new Database(dbPath,{strict:true});
 const reset=(note:boolean|string=false)=>{const rows=structuredClone(originalRows);if(note!==false)rows[0].note=typeof note==='string'?note:'x'.repeat(16384);db.transaction(()=>{db.exec('DELETE FROM item');for(const r of rows)db.prepare('INSERT INTO item(id,owner_id,title,note) VALUES(?,?,?,?)').run(r.id,r.owner_id,r.title,r.note);})();return rows;};
 const snapshot=()=>db.query('SELECT * FROM item ORDER BY id').all();
 async function call(operation:string,input:any,principalName='owner'){
  const principal=spec.seeds.principals[principalName];if(!principal)return {kind:'invalid'};
  let args:any[];
  try{
   bounded(input);const id=(v:any)=>app.experimentValidators.User({id:v},'input').id;
   const title=(v:any)=>app.experimentValidators.ItemTitle(v,'input.title');
   if(operation==='probe')args=[app.experimentValidators.ProbeInput(input,'input')];
   else if(operation==='Item.read'||operation==='Item.read_title')args=[id(input)];
   else {
    if(!Array.isArray(input))return {kind:'invalid'};
    if(operation==='Item.change'&&input.length===2)args=[id(input[0]),app.experimentValidators.ItemPatch(input[1],'input.patch')];
    else if(operation==='Item.rename'&&input.length===2)args=[id(input[0]),title(input[1])];
    else if(operation==='update_pair'&&input.length===4)args=[id(input[0]),title(input[1]),id(input[2]),title(input[3])];
    else return {kind:'invalid'};
   }
  }catch{return {kind:'invalid'};}
  try {return bounded({kind:'success',value:await app.experimentCallables[operation](...args,app.captureOperation(null,{kind:'user',subject:principal.id,values:{user_id:principal.id}}),guarded)});}
  catch(e){if(e instanceof app.DomainFailure)return {kind:'domain',failure:e.failureName};return {kind:'internal'};}
 }
 return {db,reset,snapshot,call,close:()=>db.close()};
}
