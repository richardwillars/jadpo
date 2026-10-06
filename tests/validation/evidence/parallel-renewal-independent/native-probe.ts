import { Database } from "bun:sqlite";
import { SQL } from "bun";
import { readFileSync } from "node:fs";
const source = readFileSync("/private/tmp/jadpo-rm306-recovery/jadpo/crates/core/src/runtime/delivery_scheduler.ts", "utf8");
const compiled = new Bun.Transpiler({loader:"ts"}).transformSync(source);
class Fault extends Error { constructor(public operation:string){super(operation);} }
const factory = new Function("PersistenceFault", `${compiled}\nreturn createDeliverySchedulePrimitives;`)(Fault);
const url = Bun.env.REVIEW_DATABASE_URL;
const pg = url ? new SQL(url,{prepare:false,max:1}) : null;
const db = pg ? null : new Database(":memory:");
const execute = async (_op:string, statement:string, args:unknown[] = []) => pg ? pg.unsafe(statement,args) : db!.prepare(statement).all(...args);
const transact = async (work:(native:any)=>Promise<any>) => {
  if(pg) return pg.begin(async tx=>work(factory(async(op:string,s:string,v:unknown[]=[])=>tx.unsafe(s,v),()=>{},true,(w:any)=>w())));
  db!.exec("BEGIN IMMEDIATE");
  try { const result=await work(factory(execute,()=>{},false,(w:any)=>w())); db!.exec("COMMIT"); return result; } catch(e){db!.exec("ROLLBACK");throw e;}
};
const states:any[]=[];
try {
  const binding=crypto.randomUUID(), at="2026-10-06T08:00:00.000Z";
  const first=await transact(async n=>{await n.tick_delivery_schedule(binding,900000,at);return n.claim_delivery_activation(binding,at,{executionMs:120000,leaseMs:30000});});
  await transact(n=>n.stage_delivery_activation_page(first,["original-intent"],{id:"cursor",dueAt:at}));
  const before=(await execute("read",'SELECT * FROM "__jadpo_delivery_activations_v1" WHERE binding=$1',[binding]))[0];
  const stale=await transact(n=>n.renew_delivery_activation({...first,generation:"9007199254740993",leaseUntil:"bad",executionDeadline:"9999-12-31T23:59:59.999Z"}));
  if(stale!==null || JSON.stringify(before)!==JSON.stringify((await execute("read",'SELECT * FROM "__jadpo_delivery_activations_v1" WHERE binding=$1',[binding]))[0])) throw new Error("Stale handle mutated authority");
  states.push({probe:"stale forged handle",passed:true});
  if(!pg){
    await execute("corrupt",'UPDATE "__jadpo_delivery_activations_v1" SET lease_ms=$1 WHERE binding=$2',["0x10",binding]);
    const corrupt=(await execute("read",'SELECT *,typeof(lease_ms) AS lease_storage_type FROM "__jadpo_delivery_activations_v1" WHERE binding=$1',[binding]))[0];
    let outcome:any;try{outcome=await transact(n=>n.renew_delivery_activation(first));}catch(e){outcome={fault:(e as any).operation};}
    states.push({probe:"nonnumeric durable lease_ms",stored:corrupt.lease_ms,storageType:corrupt.lease_storage_type,refused:!!outcome?.fault,result:outcome});
  } else {
    const renewed=await transact(n=>n.renew_delivery_activation({...first,leaseUntil:"bad",executionDeadline:"bad"}));
    if(!renewed || renewed.executionDeadline!==first.executionDeadline || renewed.page.intentIds[0]!=="original-intent")throw new Error("Current forged metadata affected authority");
    states.push({probe:"current handle forged metadata ignored",passed:true});
  }
  console.log(JSON.stringify({adapter:pg?"postgres":"sqlite",states},null,2));
}finally{await pg?.close();db?.close();}
